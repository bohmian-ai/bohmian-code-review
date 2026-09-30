//! Lua + Luau extraction — a faithful Rust port of the lua/luau paths of
//! `TreeSitterExtractor` (src/extraction/tree-sitter.ts) plus
//! languages/lua.ts and languages/luau.ts (36 lines extending lua).
//!
//! One walker, two dialects (ccpp precedent): the differences are exactly
//! four — luau's typeAliasTypes=['type_definition'], the `export `-slice
//! isExported hook, the return-type signature suffix, and the grammar handle.
//! The authoritative quirk list is docs/design/lua-luau-kernel-port-checklist.md.
//! Load-bearing oddities preserved on purpose: the require/visitNode-hook
//! ASYMMETRIES (top-level requires — including inside top-level if/for/while
//! blocks — mint import nodes, while the identical statement in a function
//! body emits `calls "require"`; a top-level `local x = foo()` initializer
//! emits NO calls ref while a top-level global `x = foo()` does), the BFS
//! string-win inside require args (`require(script:WaitForChild("Kid"))` →
//! import "Kid"; `require("a".."b")` → import "a"), raw-text callees verbatim
//! (colon forms `M:render` with `self` never stripped, brackets `t2[k2]`,
//! newline-glued chains byte-verbatim, the `(handler)` paren-conversion),
//! receiver-QN methods (`M.sub.deep::chained`, stack-QN nested globals like
//! `render::leakedGlobal`), variable nodes positioned at the IDENTIFIER with
//! positional value pairing, LuaDoc `---` keeping a leading `- ` and
//! `--!strict` joining docstring chains, the lua↔luau isExported wire
//! divergence (lua functions: flag ABSENT; luau functions: present-false;
//! methods: absent in both; variables: present-false in both), and duplicate
//! same-(kind,name,line) ids emitted twice. Positions in UTF-16 code units.
//! Files with parse errors extract from tree-sitter's recovered tree.

use crate::buffers::{
    Arena, BoolFlags, EdgeRow, EmitOut, FLAG_IS_EXPORTED, FUNCTION_REF_CODE, NONE, NONE_STR,
    NodeRow, RefRow, StrRef, Tables, build_meta, edge_kind_index, node_kind_index,
};
use crate::docstring::preceding_docstring;
use crate::ids;
use crate::textutil as util;
use std::collections::{HashSet, VecDeque};
use tree_sitter::{Node, Parser};

/// NAME_STOPLIST (function-ref.ts).
fn is_stoplisted(name: &str) -> bool {
    matches!(
        name,
        "this"
            | "self"
            | "super"
            | "null"
            | "nil"
            | "true"
            | "false"
            | "undefined"
            | "new"
            | "NULL"
            | "nullptr"
            | "None"
    )
}

/// Tracks one lexical owner so nested symbols receive stable qualified names and containment edges.
struct Scope {
    /// Emitted node row that owns the scope or deferred candidate.
    row: u32,
    /// Borrowed source spelling associated with this scope or syntax classification.
    kind: &'static str,
    /// Source spelling retained for symbol names, qualified names, or reference matching.
    name: String,
}

/// Records a deferred reference candidate with its origin and source coordinate.
struct Cand {
    /// Source node row from which the unresolved reference originates.
    from: u32,
    /// Source spelling retained for symbol names, qualified names, or reference matching.
    name: String,
    /// One-based source line attached to the emitted fact.
    line: u32,
    /// Byte column retained until it is converted to the upstream UTF-16 coordinate.
    column_byte: usize,
    /// Emitted node row that owns the scope or deferred candidate.
    row: usize,
}

#[derive(Default)]
/// Collects optional symbol attributes before a node row is emitted atomically.
struct Extra<'a> {
    /// Normalized leading documentation attached to the emitted symbol.
    docstring: Option<String>,
    /// Source signature text preserved on the emitted symbol.
    signature: Option<String>,
    /// Some(_) sets the present bit (luau functions/type_aliases, variables
    /// in both dialects); None leaves the pair absent (lua functions,
    /// methods, imports).
    is_exported: Option<bool>,
    /// Language-provided qualified name used instead of lexical scope concatenation.
    qualified_name_override: Option<String>,
    /// Preserves the source lifetime on metadata that otherwise owns its extracted strings.
    _marker: std::marker::PhantomData<&'a ()>,
}

/// Owns per-file lua traversal state, lexical scopes, and ordered fact buffers.
pub struct Walker<'t> {
    /// Borrowed source text used for node slices and byte-based coordinates.
    src: &'t str,
    /// Repository-relative path incorporated into file facts, node identifiers, and references.
    file_path: &'t str,
    /// Whether Lua extraction applies the Luau-specific grammar and declaration branches.
    is_luau: bool,
    /// Byte offsets of line starts, used to translate tree-sitter points into emitted coordinates.
    line_starts: Vec<usize>,
    /// String arena that owns text referenced by the emitted fixed-width rows.
    arena: Arena,
    /// Flat node, edge, and reference tables accumulated for the current file.
    tables: Tables,
    /// Lexical containment stack used to form qualified names and containment edges.
    stack: Vec<Scope>,
    /// Map from emitted node identifiers to table rows, used to avoid duplicate facts.
    node_ids: Vec<String>,
    /// Function names declared in the file, retained to validate delayed call candidates.
    defined_fn_names: HashSet<String>,
    /// Names introduced by imports, retained when qualifying unresolved references.
    imported_names: HashSet<String>,
    /// Deferred function-reference candidates resolved after all declarations are known.
    fn_ref_cands: Vec<Cand>,
}

/// Parses one lua source file and returns its flat facts, diagnostics, and measured kernel duration.
///
/// # Errors
///
/// Returns an error when the native grammar is unavailable, tree-sitter rejects grammar installation, or parsing returns no tree.
///
/// # Panics
///
/// Panics if a hard-coded node-kind spelling is absent from the frozen `NODE_KINDS` registry.
pub fn extract(file_path: &str, source: &str, language: &str) -> Result<EmitOut, String> {
    let grammar = crate::langs::grammar_for(language).ok_or("no lua/luau grammar")?;
    let t0 = std::time::Instant::now();
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .map_err(|e| format!("set_language({language}) failed: {e}"))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "parser returned null tree".to_string())?;
    let mut w = Walker {
        src: source,
        file_path,
        is_luau: language == "luau",
        line_starts: util::line_starts(source),
        arena: Arena::default(),
        tables: Tables::default(),
        stack: Vec::new(),
        node_ids: Vec::new(),
        defined_fn_names: HashSet::new(),
        imported_names: HashSet::new(),
        fn_ref_cands: Vec::new(),
    };

    // File node (tree-sitter.ts:508-521).
    let line_count = source.bytes().filter(|b| *b == b'\n').count() as u32 + 1;
    let base_name = file_path.rsplit(['/', '\\']).next().unwrap_or(file_path);
    let mut flags = BoolFlags::default();
    flags.set(FLAG_IS_EXPORTED, false);
    let file_id = w.arena.put(&ids::file_node_id(file_path));
    let name_ref = w.arena.put(base_name);
    let qn_ref = w.arena.put(file_path);
    w.tables.push_node(&NodeRow {
        kind: node_kind_index("file").unwrap(),
        visibility: 0,
        flags,
        start_line: 1,
        end_line: line_count,
        start_column: 0,
        end_column: 0,
        name: name_ref,
        qualified_name: qn_ref,
        id: file_id,
        docstring: NONE_STR,
        signature: NONE_STR,
        decorators: NONE_STR,
        type_parameters: NONE_STR,
        return_type: NONE_STR,
        extra_json: NONE_STR,
    });
    w.node_ids.push(ids::file_node_id(file_path));
    w.stack.push(Scope {
        row: 0,
        kind: "file",
        name: base_name.to_string(),
    });

    // No packageTypes → no namespace node. Value-refs are language-gated off.
    w.visit(tree.root_node());
    w.flush_fn_ref_candidates();
    w.stack.pop();

    let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let meta = build_meta(&w.tables, w.arena.len(), NONE_STR, duration_ms);
    Ok(EmitOut {
        meta,
        nodes: w.tables.nodes,
        edges: w.tables.edges,
        refs: w.tables.refs,
        arena: w.arena.into_vec(),
    })
}

impl<'t> Walker<'t> {
    /// Returns the UTF-8 source slice covered by a syntax node.
    fn text(&self, node: Node) -> &'t str {
        &self.src[node.byte_range()]
    }
    /// Converts a tree-sitter node start row into the one-based emitted line.
    fn line_of(&self, node: Node) -> u32 {
        node.start_position().row as u32 + 1
    }
    /// Converts the node byte column into the UTF-16 column required by the upstream contract.
    fn col_of(&self, node: Node) -> u32 {
        util::col16(
            self.src,
            &self.line_starts,
            node.start_position().row,
            node.start_byte(),
        )
    }
    /// Converts the node byte column into the UTF-16 column required by the upstream contract.
    fn end_col_of(&self, node: Node) -> u32 {
        util::col16(
            self.src,
            &self.line_starts,
            node.end_position().row,
            node.end_byte(),
        )
    }
    /// Returns the row of the outermost active lexical scope for containment decisions.
    fn top_row(&self) -> u32 {
        self.stack.last().map(|s| s.row).unwrap_or(0)
    }

    /// Encodes ref at into the current flat table while preserving insertion order and row indexes.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded edge-kind spelling is absent from the frozen `EDGE_KINDS` registry.
    fn push_ref_at(&mut self, from_row: u32, name: &str, kind: &str, node: Node) {
        let name_ref = self.arena.put(name);
        self.tables.push_ref(&RefRow {
            from_idx: from_row,
            kind: edge_kind_index(kind).unwrap(),
            line: self.line_of(node),
            column: self.col_of(node),
            reference_name: name_ref,
            candidates: NONE_STR,
            from_id_str: NONE_STR,
        });
        // flushFnRefCandidates' importedNames gate (tree-sitter.ts:661-675):
        // dotted lua module paths contribute their LAST segment; simple names
        // (Roblox leaves) pass whole.
        if kind == "imports" {
            if util::simple_name().is_match(name) {
                self.imported_names.insert(name.to_string());
            } else if let Some(c) = util::qualified_import().captures(name) {
                self.imported_names.insert(c[1].to_string());
            }
        }
    }

    // --- createNode (tree-sitter.ts:1308) ---------------------------------

    /// Builds and records node with its stable identifier, source range, and containment edge.
    ///
    /// # Panics
    ///
    /// Panics if the supplied node kind or the hard-coded containment edge is absent from the frozen ABI registries.
    fn create_node(
        &mut self,
        kind: &'static str,
        name: &str,
        node: Node<'t>,
        extra: Extra,
    ) -> Option<u32> {
        if name.is_empty() {
            return None;
        }
        let start_line = self.line_of(node);
        let id = ids::node_id(self.file_path, kind, name, start_line);

        // buildQualifiedName (1447-1460) — non-file stack NAMES joined `::`;
        // the receiver override (extractMethod:1790-1792) replaces it whole.
        let qualified = match &extra.qualified_name_override {
            Some(qn) => qn.clone(),
            None => {
                let mut parts: Vec<&str> = Vec::new();
                for s in &self.stack {
                    if s.kind != "file" {
                        parts.push(&s.name);
                    }
                }
                let mut qn = parts.join("::");
                if !qn.is_empty() {
                    qn.push_str("::");
                }
                qn.push_str(name);
                qn
            }
        };

        let name_ref = self.arena.put(name);
        let qn_ref = self.arena.put(&qualified);
        let id_ref = self.arena.put(&id);
        let doc_ref = opt_str(&mut self.arena, extra.docstring.as_deref());
        let sig_ref = opt_str(&mut self.arena, extra.signature.as_deref());
        let mut flags = BoolFlags::default();
        if let Some(v) = extra.is_exported {
            flags.set(FLAG_IS_EXPORTED, v);
        }
        let row = self.tables.push_node(&NodeRow {
            kind: node_kind_index(kind).unwrap(),
            visibility: 0,
            flags,
            start_line,
            end_line: node.end_position().row as u32 + 1, // no resolveBody
            start_column: self.col_of(node),
            end_column: self.end_col_of(node),
            name: name_ref,
            qualified_name: qn_ref,
            id: id_ref,
            docstring: doc_ref,
            signature: sig_ref,
            decorators: NONE_STR,
            type_parameters: NONE_STR,
            return_type: NONE_STR,
            extra_json: NONE_STR,
        });
        self.node_ids.push(id);
        if kind == "function" || kind == "method" {
            self.defined_fn_names.insert(name.to_string());
        }

        let parent_row = self.top_row();
        self.tables.push_edge(&EdgeRow {
            source_idx: parent_row,
            target_idx: row,
            kind: edge_kind_index("contains").unwrap(),
            provenance: 0,
            line: NONE,
            column: NONE,
            metadata_json: NONE_STR,
            source_id_str: NONE_STR,
            target_id_str: NONE_STR,
        });

        Some(row)
    }

    // --- lua.ts helper transcriptions -------------------------------------

    /// findDescendant (lua.ts:9-17) — breadth-first over namedChildren.
    fn find_descendant(&self, node: Node<'t>, kind: &str) -> Option<Node<'t>> {
        let mut queue: VecDeque<Node<'t>> = VecDeque::new();
        let mut cursor = node.walk();
        for c in node.named_children(&mut cursor) {
            queue.push_back(c);
        }
        while let Some(n) = queue.pop_front() {
            if n.kind() == kind {
                return Some(n);
            }
            let mut cur = n.walk();
            for c in n.named_children(&mut cur) {
                queue.push_back(c);
            }
        }
        None
    }

    /// requireModule (lua.ts:28-60).
    fn require_module(&self, call: Node<'t>) -> Option<String> {
        let name = call.child_by_field_name("name")?;
        if name.kind() != "identifier" || self.text(name) != "require" {
            return None;
        }
        let args = call.child_by_field_name("arguments")?;

        // String win: first string_content descendant, BFS order.
        if let Some(content) = self.find_descendant(args, "string_content") {
            let t = self.text(content).trim();
            return if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            };
        }
        // Fallback: a string node with no content child — strip [[ ]] / quotes.
        if let Some(s) = self.find_descendant(args, "string") {
            let mut t = self.text(s).trim();
            t = t.strip_prefix("[[").unwrap_or(t);
            t = t.strip_suffix("]]").unwrap_or(t);
            t = t.strip_prefix(['"', '\'']).unwrap_or(t);
            t = t.strip_suffix(['"', '\'']).unwrap_or(t);
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        // Roblox instance path: trailing field/method segment.
        let idx = self
            .find_descendant(args, "dot_index_expression")
            .or_else(|| self.find_descendant(args, "method_index_expression"));
        if let Some(idx) = idx
            && let Some(field) = idx
                .child_by_field_name("field")
                .or_else(|| idx.child_by_field_name("method"))
        {
            let t = self.text(field).trim();
            return if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            };
        }
        None
    }

    /// The hook's `emit` (lua.ts:108-126): import node at the CALL node +
    /// imports ref from the stack top.
    fn emit_require(&mut self, call: Node<'t>, module: &str) {
        let (sig, _) = util::slice_utf16(self.text(call).trim(), 100);
        let imp = self.create_node(
            "import",
            module,
            call,
            Extra {
                signature: Some(sig),
                ..Default::default()
            },
        );
        if imp.is_some() && !self.stack.is_empty() {
            let parent_row = self.top_row();
            self.push_ref_at(parent_row, module, "imports", call);
        }
    }

    /// getReceiverType (lua.ts:92-99).
    fn receiver_type(&self, node: Node<'t>) -> Option<&'t str> {
        let name = node.child_by_field_name("name")?;
        if name.kind() == "dot_index_expression" || name.kind() == "method_index_expression" {
            return name.child_by_field_name("table").map(|t| self.text(t));
        }
        None
    }

    /// extractName (tree-sitter.ts:98-192) — the lua-reachable branches.
    fn extract_name(&self, node: Node<'t>) -> String {
        if let Some(name_node) = node.child_by_field_name("name") {
            // Lua: dot/method index → the trailing field/method segment.
            if name_node.kind() == "dot_index_expression"
                && let Some(f) = name_node.child_by_field_name("field")
            {
                return self.text(f).to_string();
            }
            if name_node.kind() == "method_index_expression"
                && let Some(m) = name_node.child_by_field_name("method")
            {
                return self.text(m).to_string();
            }
            return self.text(name_node).to_string();
        }
        // Fallback: first identifier-ish named child.
        let mut cursor = node.walk();
        for c in node.named_children(&mut cursor) {
            if matches!(
                c.kind(),
                "identifier" | "type_identifier" | "simple_identifier" | "constant"
            ) {
                return self.text(c).to_string();
            }
        }
        "<anonymous>".to_string()
    }

    /// getSignature — lua (lua.ts:83-86) / luau (luau.ts:26-35).
    fn signature_of(&self, node: Node<'t>) -> Option<String> {
        let params = node.child_by_field_name("parameters")?;
        let mut sig = self.text(params).to_string();
        if self.is_luau {
            // Return type = the named child AFTER `parameters` (found by
            // startIndex match), unless it's the block.
            let mut cursor = node.walk();
            let kids: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
            if let Some(idx) = kids
                .iter()
                .position(|k| k.start_byte() == params.start_byte())
                && let Some(ret) = kids.get(idx + 1)
                && ret.kind() != "block"
            {
                sig.push_str(": ");
                sig.push_str(self.text(*ret));
            }
        }
        Some(sig)
    }

    /// isExported (luau.ts:23) — the raw 7-unit slice is an ASCII prefix test.
    fn is_exported_of(&self, node: Node<'t>) -> Option<bool> {
        if self.is_luau {
            Some(self.text(node).starts_with("export "))
        } else {
            None
        }
    }

    // --- the main walk (visitNode, tree-sitter.ts:936-1303) ---------------

    /// Dispatches one syntax node to the matching declaration, call, import, or reference handler.
    fn visit(&mut self, node: Node<'t>) {
        let kind = node.kind();

        // The visitNode hook (lua.ts:105-151) runs FIRST.
        if kind == "function_call" {
            if let Some(module) = self.require_module(node) {
                self.emit_require(node, &module);
                // Consumed → scanFnRefSubtree (tree-sitter.ts:951).
                self.scan_fn_ref_subtree(node, 0);
                return;
            }
            // falls through — extractCall claims it below
        } else if kind == "variable_declaration" {
            // `local x = require(...)` — dig requires out of the initializer
            // the variable branch will skip. Always falls through.
            let mut cursor = node.walk();
            let assign = node
                .named_children(&mut cursor)
                .find(|c| c.kind() == "assignment_statement");
            if let Some(assign) = assign {
                let mut ac = assign.walk();
                let expr_list = assign
                    .named_children(&mut ac)
                    .find(|c| c.kind() == "expression_list");
                if let Some(expr_list) = expr_list {
                    let mut ec = expr_list.walk();
                    let vals: Vec<Node<'t>> = expr_list.named_children(&mut ec).collect();
                    for val in vals {
                        if val.kind() == "function_call"
                            && let Some(module) = self.require_module(val)
                        {
                            self.emit_require(val, &module);
                        }
                    }
                }
            }
        }

        // maybeCaptureFnRefs (tree-sitter.ts:990).
        self.maybe_capture_fn_refs(node);

        // The dispatch ladder — lua/luau rows only.
        if kind == "function_declaration" {
            // isInsideClassLikeNode is always false (no class-like kinds).
            self.extract_function(node);
            return; // skipChildren — the body walk handles children
        }
        if self.is_luau && kind == "type_definition" {
            let skip = self.extract_type_alias(node);
            if skip {
                return;
            }
            // plain path returns false → children re-visited (the
            // typeof(require(...)) alias+import pair rides this).
        } else if kind == "variable_declaration" {
            self.extract_variable(node);
            // Initializer subtrees are never walked — candidates only.
            self.scan_fn_ref_subtree(node, 0);
            return; // skipChildren
        } else if kind == "function_call" {
            self.extract_call(node);
            // no skipChildren — nested/inner calls each get their own ref
        }

        let mut cursor = node.walk();
        let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
        for child in children {
            self.visit(child);
        }
    }

    // --- extractFunction / extractMethod (1517 / 1737) --------------------

    /// Recognizes function syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_function(&mut self, node: Node<'t>) {
        // :1522 receiver short-circuit IS the method routing.
        if let Some(receiver) = self.receiver_type(node) {
            let receiver = receiver.to_string();
            self.extract_method(node, receiver);
            return;
        }
        let name = self.extract_name(node);
        if name == "<anonymous>" {
            // Unreachable for function_declaration (grammar requires a name)
            // but preserved: body walked with nothing pushed.
            if let Some(body) = node.child_by_field_name("body") {
                self.visit_body(body);
            }
            return;
        }
        let docstring = preceding_docstring(node, self.src);
        let signature = self.signature_of(node);
        let is_exported = self.is_exported_of(node); // lua None / luau Some(false)
        let fn_row = self.create_node(
            "function",
            &name,
            node,
            Extra {
                docstring,
                signature,
                is_exported,
                ..Default::default()
            },
        );
        let Some(row) = fn_row else { return };
        // extractTypeAnnotations / extractDecoratorsFor: structurally zero
        // output for lua/luau (gates + no decorator kinds in scan positions).
        self.stack.push(Scope {
            row,
            kind: "function",
            name,
        });
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_body(body);
        }
        self.stack.pop();
    }

    /// Recognizes method syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_method(&mut self, node: Node<'t>, receiver: String) {
        let name = self.extract_name(node);
        let docstring = preceding_docstring(node, self.src);
        let signature = self.signature_of(node);
        // extractMethod passes NO isExported — absent for BOTH dialects.
        // QN override (:1790-1792): `receiver::name` verbatim (namespacePrefix
        // is empty outside C++).
        let qn = format!("{receiver}::{name}");
        let method_row = self.create_node(
            "method",
            &name,
            node,
            Extra {
                docstring,
                signature,
                qualified_name_override: Some(qn),
                ..Default::default()
            },
        );
        let Some(row) = method_row else { return };
        // Owner-contains (:1799-1813) never fires: lua mints no
        // struct/class/enum/trait nodes for a receiver name to match.
        self.stack.push(Scope {
            row,
            kind: "method",
            name,
        });
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_body(body);
        }
        self.stack.pop();
    }

    // --- extractVariable — the lua/luau branch (2538-2549, 2789-2805) -----

    /// Recognizes variable syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_variable(&mut self, node: Node<'t>) {
        // isConst absent → kind is ALWAYS `variable`; docstring from the
        // DECLARATION node; isExported = hook ?? false → false for BOTH
        // dialects (luau's slice sees `local …`).
        let docstring = preceding_docstring(node, self.src);
        let is_exported = self.is_exported_of(node).unwrap_or(false);

        let mut cursor = node.walk();
        let assign = node
            .named_children(&mut cursor)
            .find(|c| c.kind() == "assignment_statement")
            .unwrap_or(node);
        let mut ac = assign.walk();
        let var_list = assign
            .named_children(&mut ac)
            .find(|c| c.kind() == "variable_list");
        let mut ec = assign.walk();
        let expr_list = assign
            .named_children(&mut ec)
            .find(|c| c.kind() == "expression_list");
        let values: Vec<Node<'t>> = match expr_list {
            Some(el) => {
                let mut c = el.walk();
                el.named_children(&mut c).collect()
            }
            None => Vec::new(),
        };
        let names: Vec<Node<'t>> = match var_list {
            Some(vl) => {
                let mut c = vl.walk();
                vl.named_children(&mut c)
                    .filter(|n| n.kind() == "identifier")
                    .collect()
            }
            None => Vec::new(),
        };
        for (i, name_node) in names.iter().enumerate() {
            let name = self.text(*name_node);
            if name.is_empty() {
                continue;
            }
            // Positional value pairing; a missing value → NO signature key.
            let signature = values.get(i).map(|v| util::init_signature(self.text(*v)));
            let name = name.to_string();
            self.create_node(
                "variable",
                &name,
                *name_node, // positioned at the IDENTIFIER
                Extra {
                    docstring: docstring.clone(),
                    signature,
                    is_exported: Some(is_exported),
                    ..Default::default()
                },
            );
        }
    }

    // --- extractTypeAlias (2890; plain path 2967-2991) — luau only --------

    /// Returns skipChildren (always false on the plain path).
    fn extract_type_alias(&mut self, node: Node<'t>) -> bool {
        let name = self.extract_name(node); // generic_type name → verbatim text
        if name == "<anonymous>" {
            return false;
        }
        let docstring = preceding_docstring(node, self.src);
        let is_exported = self.is_exported_of(node); // Some(true) for `export type`
        self.create_node(
            "type_alias",
            &name,
            node,
            Extra {
                docstring,
                is_exported,
                ..Default::default()
            },
        );
        // TYPE_ANNOTATION_LANGUAGES excludes luau → no alias-value refs.
        false // children re-visited by the ladder
    }

    // --- extractCall (3684; generic tail 4313, 4518-4532, 4572-4580) ------

    /// Recognizes call syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_call(&mut self, node: Node<'t>) {
        if self.stack.is_empty() {
            return;
        }
        let caller_row = self.top_row();
        // The `function` field is NULL in this grammar → namedChild(0) (the
        // `name:` child). Member branch never fires (dot/method_index aren't
        // in its type list) → raw source text, then the paren-conversion.
        let func = node
            .child_by_field_name("function")
            .or_else(|| node.named_child(0));
        let Some(func) = func else { return };
        let mut callee: &str = self.text(func);
        if let Some(caps) = util::paren_conversion().captures(callee)
            && let Some(inner) = caps.get(1)
        {
            callee = &callee[inner.range()];
        }
        if callee.is_empty() {
            return;
        }
        let callee = callee.to_string();
        self.push_ref_at(caller_row, &callee, "calls", node);
    }

    // --- visitFunctionBody (5129-5286) — the hook-free body walk ----------

    /// Traverses body syntax and dispatches each recognized construct exactly once.
    fn visit_body(&mut self, node: Node<'t>) {
        // maybeCaptureFnRefs (5137) fires in the body walker too.
        self.maybe_capture_fn_refs(node);

        let kind = node.kind();
        if kind == "function_call" {
            // The hook NEVER runs here — a body-level require emits
            // `calls "require"` (the neovim lazy-loading idiom).
            self.extract_call(node);
            // falls through to recursion — chains emit every link
        } else if kind == "function_declaration" {
            // Nested NAMED functions (5245-5250): extractFunction walks the
            // nested body itself, so return. extractName is never
            // `<anonymous>` for function_declaration.
            self.extract_function(node);
            return;
        }
        // variable_declaration / type_definition have NO branch here → plain
        // recursion: body-local initializers ARE walked (calls emit), no
        // variable/type_alias nodes minted.

        let mut cursor = node.walk();
        let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
        for child in children {
            self.visit_body(child);
        }
    }

    // --- function-as-value capture (#756) — LUA_SPEC ----------------------

    /// Collects function-valued identifier candidates for deferred, file-wide disambiguation.
    fn maybe_capture_fn_refs(&mut self, node: Node<'t>) {
        // LUA_SPEC dispatch: arguments → args; assignment_statement → rhs
        // (no field — last named child; param-storage skip via namedChild(0));
        // field → value (field 'value', last-named-child fallback).
        let mode: &str = match node.kind() {
            "arguments" => "args",
            "assignment_statement" => "rhs",
            "field" => "value",
            _ => return,
        };
        if self.stack.is_empty() {
            return;
        }
        let from = self.top_row();

        let mut values: Vec<Node<'t>> = Vec::new();
        match mode {
            "args" => {
                let mut cursor = node.walk();
                for c in node.named_children(&mut cursor) {
                    values.push(c);
                }
            }
            "rhs" => {
                // No `field` in the rule → RHS = LAST named child (the
                // expression_list). Param-storage skip: lhs =
                // left/lhs/target field ?? namedChild(0) when ≥2 children;
                // its trailing identifier vs the whole RHS text.
                let count = node.named_child_count();
                let rhs = if count > 0 {
                    node.named_child(count - 1)
                } else {
                    None
                };
                if let Some(rhs) = rhs {
                    let lhs = node
                        .child_by_field_name("left")
                        .or_else(|| node.child_by_field_name("lhs"))
                        .or_else(|| node.child_by_field_name("target"))
                        .or_else(|| {
                            if count >= 2 {
                                node.named_child(0)
                            } else {
                                None
                            }
                        });
                    let lhs_text = lhs.map(|l| self.text(l)).unwrap_or("");
                    let lhs_last = util::lhs_last_name()
                        .captures(lhs_text)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str());
                    if !(lhs_last.is_some() && lhs_last == Some(self.text(rhs).trim())) {
                        values.push(rhs);
                    }
                }
            }
            _ => {
                // value — the `value` field (keyed AND positional table
                // fields carry it), falling back to the last named child.
                let v = node.child_by_field_name("value").or_else(|| {
                    let count = node.named_child_count();
                    if count > 0 {
                        node.named_child(count - 1)
                    } else {
                        None
                    }
                });
                if let Some(v) = v {
                    values.push(v);
                }
            }
        }

        for v in values {
            self.normalize_fn_ref_value(v, from, 0);
        }
    }

    /// normalizeValue with LUA_SPEC's one transparent layer (expression_list
    /// fans out to named children).
    fn normalize_fn_ref_value(&mut self, v: Node<'t>, from: u32, depth: u32) {
        if depth > 4 {
            return;
        }
        match v.kind() {
            "identifier" => {
                let name = self.text(v).to_string();
                if name.is_empty() || is_stoplisted(&name) {
                    return;
                }
                let p = v.start_position();
                self.fn_ref_cands.push(Cand {
                    from,
                    name,
                    line: p.row as u32 + 1,
                    column_byte: v.start_byte(),
                    row: p.row,
                });
            }
            "expression_list" => {
                let mut cursor = v.walk();
                let kids: Vec<Node<'t>> = v.named_children(&mut cursor).collect();
                for c in kids {
                    self.normalize_fn_ref_value(c, from, depth + 1);
                }
            }
            _ => {}
        }
    }

    /// Scans fn ref subtree syntax for deferred references without emitting unrelated declarations.
    fn scan_fn_ref_subtree(&mut self, node: Node<'t>, depth: u32) {
        if depth > 12 {
            return;
        }
        // Halt at nested function definitions (their bodies are walked — and
        // attributed — by extractFunction). function_definition (anonymous)
        // is deliberately NOT in the halt list — the scan descends into
        // anonymous initializer bodies, attributing candidates to the file.
        if depth > 0
            && matches!(
                node.kind(),
                "function_declaration"
                    | "arrow_function"
                    | "function_expression"
                    | "lambda_literal"
                    | "lambda_expression"
            )
        {
            return;
        }
        self.maybe_capture_fn_refs(node);
        let mut cursor = node.walk();
        let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
        for c in children {
            self.scan_fn_ref_subtree(c, depth + 1);
        }
    }

    /// Resolves buffered fn ref candidates after file declarations are known, then emits only unambiguous references.
    fn flush_fn_ref_candidates(&mut self) {
        let cands = std::mem::take(&mut self.fn_ref_cands);
        if cands.is_empty() || util::is_generated_file(self.file_path) {
            return;
        }
        let mut seen: HashSet<(String, String)> = HashSet::new();
        for c in cands {
            // Gate: same-file function/method names ∪ imported names (lua
            // candidates are always bare identifiers — no `this.`/`::`).
            if !c.name.starts_with("this.")
                && !c.name.contains("::")
                && !self.defined_fn_names.contains(&c.name)
                && !self.imported_names.contains(&c.name)
            {
                continue;
            }
            if !seen.insert((self.node_ids[c.from as usize].clone(), c.name.clone())) {
                continue;
            }
            let column = util::col16(self.src, &self.line_starts, c.row, c.column_byte);
            let name_ref = self.arena.put(&c.name);
            self.tables.push_ref(&RefRow {
                from_idx: c.from,
                kind: FUNCTION_REF_CODE,
                line: c.line,
                column,
                reference_name: name_ref,
                candidates: NONE_STR,
                from_id_str: NONE_STR,
            });
        }
    }
}

/// Converts the arena absent sentinel into `None` and preserves present strings without normalization.
fn opt_str(arena: &mut Arena, s: Option<&str>) -> StrRef {
    match s {
        Some(s) => arena.put(s),
        None => NONE_STR,
    }
}
