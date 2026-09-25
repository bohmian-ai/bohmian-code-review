//! Erlang extraction — a faithful Rust port of `languages/erlang.ts` plus the
//! Erlang branch of `TreeSitterExtractor.extractCall` and the gen_server
//! target resolution (src/extraction/tree-sitter.ts).
//!
//! Erlang is form-based; every symbol-bearing top-level form is dispatched
//! through the visitNode hook: a function's name lives on its CLAUSE (one
//! fun_decl per clause — consecutive same-name forms merge into one node),
//! `-spec`/`-callback`/`-type` bodies parse as `call` nodes and are consumed
//! without minting bogus call refs, records carry their fields as direct
//! children, `-define` macros become `constant` nodes whose replacement's
//! calls attribute to the macro, `-include`/`-include_lib`/`-import` become
//! imports, and `-behaviour` links the module to the behaviour contract. The
//! `-module(m)` attribute wraps the file in a namespace so every function's
//! qualifiedName is `m::f`, matching the `mod::f` reference shape remote
//! calls emit. `.app`/`.app.src` resource files are handled by the
//! application-tuple hook (entry-module `references` + dependency `imports`).
//!
//! The frozen oracle is the vendored `src/extraction/wasm/
//! tree-sitter-erlang.wasm` (ABI 15), built from WhatsApp/tree-sitter-erlang
//! v0.19.0 = 836aa2b (the ELP grammar, Apache-2.0); the vendored C under
//! `crates/codegraph-extract/grammars/erlang` is that commit's grammar
//! regenerated with tree-sitter-cli 0.25.10 (checked-in parser.c is ABI 14)
//! plus the checked-in hand-written scanner.c. Behavior parity is asserted
//! per fixture against the oracle goldens. Positions are emitted in UTF-16
//! code units.

use crate::buffers::{
    Arena, BoolFlags, EdgeRow, EmitOut, FLAG_IS_EXPORTED, NONE, NONE_STR, NodeRow, RefRow, StrRef,
    Tables, build_meta, edge_kind_index, node_kind_index,
};
use crate::docstring::preceding_docstring;
use crate::ids;
use crate::textutil as util;
use std::collections::{HashMap, HashSet};
use tree_sitter::{Node, Parser};

/// Compiler-predefined Erlang macros — no `-define` exists to link a use to.
const ERLANG_PREDEFINED_MACROS: [&str; 10] = [
    "MODULE",
    "MODULE_STRING",
    "FILE",
    "LINE",
    "MACHINE",
    "FUNCTION_NAME",
    "FUNCTION_ARITY",
    "OTP_RELEASE",
    "FEATURE_AVAILABLE",
    "FEATURE_ENABLED",
];

/// Erlang calls that take their real callee as (Module, Function, Args)
/// ARGUMENTS — the spawn/apply family.
const ERLANG_MFA_CALLS: [&str; 22] = [
    "spawn",
    "spawn_link",
    "spawn_monitor",
    "spawn_opt",
    "apply",
    "erlang:spawn",
    "erlang:spawn_link",
    "erlang:spawn_monitor",
    "erlang:spawn_opt",
    "erlang:apply",
    "proc_lib:spawn",
    "proc_lib:spawn_link",
    "proc_lib:spawn_opt",
    "proc_lib:start",
    "proc_lib:start_link",
    "timer:apply_after",
    "timer:apply_interval",
    "rpc:call",
    "rpc:cast",
    "rpc:async_call",
    "erpc:call",
    "erpc:cast",
];

/// One scope-stack entry so nested symbols receive stable qualified names and containment edges.
#[derive(Clone)]
struct Scope {
    /// Emitted node row that owns the scope.
    row: u32,
    /// Closed upstream kind tag of the owning symbol.
    kind: &'static str,
    /// Source spelling retained for symbol names, qualified names, or reference matching.
    name: String,
}

/// Optional node attributes collected before a node row is emitted atomically.
#[derive(Default, Clone)]
struct NodeExtra {
    /// Source signature text preserved on the emitted symbol.
    signature: Option<String>,
    /// Whether the declaration is exported, encoded in the node flag bit-pair.
    is_exported: Option<bool>,
    /// Leading documentation text attached to the emitted symbol.
    docstring: Option<String>,
}

/// Owns per-file Erlang traversal state, lexical scopes, and ordered fact buffers.
pub struct Walker<'t> {
    /// Borrowed source text used for node slices and byte-based coordinates.
    src: &'t str,
    /// Repository-relative path incorporated into file facts, node identifiers, and references.
    file_path: &'t str,
    /// Byte offsets of line starts, used to translate tree-sitter points into emitted coordinates.
    line_starts: Vec<usize>,
    /// String arena that owns text referenced by the emitted fixed-width rows.
    arena: Arena,
    /// Flat node, edge, and reference tables accumulated for the current file.
    tables: Tables,
    /// Lexical containment stack used to form qualified names and containment edges.
    stack: Vec<Scope>,
    /// Exported function names for the current file ('all' for
    /// `-compile(export_all)`).
    exports_memo: ExportsMemo,
    /// The previous fun_decl's name and node row for clause merging.
    last_fn_name: String,
    /// The previous fun_decl's emitted node row (0 = none).
    last_fn_row: u32,
    /// gen_server target macros defined as `?MODULE`.
    self_macros: HashSet<String>,
    /// gen_server target macros defined as a bare atom.
    atom_macros: HashMap<String, String>,
}

/// The module exports memo: the set of exported function names, or `All`
/// when `-compile(export_all)` is present.
#[derive(Default, Clone)]
enum ExportsMemo {
    /// No exports recorded yet for this file.
    #[default]
    Unset,
    /// `-compile(export_all)` — every function is exported.
    All,
    /// The explicit `-export([...])` function names.
    Set(HashSet<String>),
}

impl ExportsMemo {
    /// Whether the memo contains the given function name.
    fn contains(&self, name: &str) -> bool {
        match self {
            ExportsMemo::Unset => false,
            ExportsMemo::All => true,
            ExportsMemo::Set(set) => set.contains(name),
        }
    }
}

/// Parses one Erlang source file and returns its flat facts, diagnostics, and measured kernel duration.
///
/// # Errors
///
/// Returns an error when the native grammar is unavailable, tree-sitter rejects grammar installation, parsing returns no tree, or the tree contains errors and must defer to canonical wasm recovery.
///
/// # Panics
///
/// Panics if a hard-coded node-kind spelling is absent from the frozen `NODE_KINDS` registry.
pub fn extract(file_path: &str, source: &str) -> Result<EmitOut, String> {
    let grammar = crate::langs::grammar_for("erlang").ok_or("no erlang grammar")?;
    let t0 = std::time::Instant::now();
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .map_err(|e| format!("set_language(erlang) failed: {e}"))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "parser returned null tree".to_string())?;
    if tree.root_node().has_error() {
        return Err(crate::error::CANONICAL_DEFER_MESSAGE.to_string());
    }

    let mut w = Walker {
        src: source,
        file_path,
        line_starts: util::line_starts(source),
        arena: Arena::default(),
        tables: Tables::default(),
        stack: Vec::new(),
        exports_memo: ExportsMemo::Unset,
        last_fn_name: String::new(),
        last_fn_row: 0,
        self_macros: HashSet::new(),
        atom_macros: HashMap::new(),
    };

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
    w.stack.push(Scope {
        row: 0,
        kind: "file",
        name: base_name.to_string(),
    });

    // extractFilePackage: `-module(m)` wraps the file in an implicit namespace.
    let package_scope = w
        .extract_file_package(tree.root_node())
        .map(|(row, name)| Scope {
            row,
            kind: "namespace",
            name,
        });
    let had_package = package_scope.is_some();
    if let Some(scope) = package_scope {
        w.stack.push(scope);
    }

    w.visit_node(tree.root_node());

    if had_package {
        w.stack.pop();
    }
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
    fn push_ref_at(&mut self, from_row: u32, name: &str, kind_code: u8, node: Node) {
        let name_ref = self.arena.put(name);
        self.tables.push_ref(&RefRow {
            from_idx: from_row,
            kind: kind_code,
            line: self.line_of(node),
            column: self.col_of(node),
            reference_name: name_ref,
            candidates: NONE_STR,
            from_id_str: NONE_STR,
        });
    }

    /// Encodes ref at the given line/column into the current flat table.
    fn push_ref(&mut self, from_row: u32, name: &str, kind_code: u8, line: u32, column: u32) {
        let name_ref = self.arena.put(name);
        self.tables.push_ref(&RefRow {
            from_idx: from_row,
            kind: kind_code,
            line,
            column,
            reference_name: name_ref,
            candidates: NONE_STR,
            from_id_str: NONE_STR,
        });
    }

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
        extra: NodeExtra,
    ) -> Option<u32> {
        if name.is_empty() {
            return None;
        }
        let start_line = self.line_of(node);
        let id = ids::node_id(self.file_path, kind, name, start_line);

        let qualified = {
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
        };

        let mut flags = BoolFlags::default();
        if let Some(v) = extra.is_exported {
            flags.set(FLAG_IS_EXPORTED, v);
        }
        let name_ref = self.arena.put(name);
        let qn_ref = self.arena.put(&qualified);
        let id_ref = self.arena.put(&id);
        let sig_ref = opt_str(&mut self.arena, extra.signature.as_deref());
        let doc_ref = opt_str(&mut self.arena, extra.docstring.as_deref());
        let row = self.tables.push_node(&NodeRow {
            kind: node_kind_index(kind).unwrap(),
            visibility: 0,
            flags,
            start_line,
            end_line: node.end_position().row as u32 + 1,
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

    /// atomText (erlang.ts): strip quoted-atom quotes (`'EXIT'` → `EXIT`).
    fn atom_text(&self, node: Node<'t>) -> String {
        let text = self.text(node);
        if text.starts_with('\'') && text.ends_with('\'') && text.len() >= 2 {
            text[1..text.len() - 1].to_string()
        } else {
            text.to_string()
        }
    }

    /// collapseWs (erlang.ts).
    fn collapse_ws(&self, text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// The `-module(m)` namespace (packageTypes/extractPackage in erlang.ts).
    fn extract_file_package(&mut self, root: Node<'t>) -> Option<(u32, String)> {
        let mut cursor = root.walk();
        let module_attr = root
            .named_children(&mut cursor)
            .find(|c| c.kind() == "module_attribute")?;
        let name = module_attr.child_by_field_name("name")?;
        let text = self.atom_text(name);
        if text.is_empty() {
            return None;
        }
        let row = self.create_node("namespace", &text, module_attr, NodeExtra::default())?;
        Some((row, text))
    }

    /// moduleExports (erlang.ts): exported function names from
    /// `-export([...])` / `-compile(export_all)`, memoized per file.
    fn module_exports(&mut self, node: Node<'t>) -> ExportsMemo {
        if !matches!(self.exports_memo, ExportsMemo::Unset) {
            return self.exports_memo.clone();
        }
        let mut root = node;
        while let Some(parent) = root.parent() {
            root = parent;
        }
        let mut result: ExportsMemo = ExportsMemo::Set(HashSet::new());
        let mut cursor = root.walk();
        for form in root.named_children(&mut cursor) {
            if form.kind() == "compile_options_attribute" && self.text(form).contains("export_all")
            {
                result = ExportsMemo::All;
                break;
            }
            if form.kind() == "export_attribute" {
                let mut fa_cursor = form.walk();
                for fa in form.named_children(&mut fa_cursor) {
                    if fa.kind() != "fa" {
                        continue;
                    }
                    if let Some(fun) = fa.child_by_field_name("fun")
                        && let ExportsMemo::Set(set) = &mut result
                    {
                        set.insert(self.atom_text(fun));
                    }
                }
            }
        }
        self.exports_memo = result.clone();
        result
    }

    /// precedingSpec (erlang.ts): the `-spec` directly above a function
    /// (comments may sit between), if it names it.
    fn preceding_spec(&self, node: Node<'t>, name: &str) -> Option<Node<'t>> {
        let mut prev = node.prev_named_sibling();
        while let Some(p) = prev {
            if p.kind() == "comment" {
                prev = p.prev_named_sibling();
            } else {
                break;
            }
        }
        if let Some(prev) = prev
            && prev.kind() == "spec"
        {
            let spec_fun = prev.child_by_field_name("fun");
            if let Some(spec_fun) = spec_fun
                && self.atom_text(spec_fun) == name
            {
                return Some(prev);
            }
        }
        None
    }

    /// clauseHeader (erlang.ts): `name(Args) when Guard` up to the `->`.
    fn clause_header(&self, clause: Node<'t>) -> Option<String> {
        let body = clause.child_by_field_name("body");
        let end = body
            .map(|b| b.start_byte())
            .unwrap_or_else(|| clause.end_byte());
        let start = clause.start_byte();
        let header = &self.src[start..end];
        let collapsed = self.collapse_ws(header);
        if collapsed.is_empty() {
            None
        } else {
            Some(collapsed)
        }
    }

    // --- visitNode hook handlers ----------------------------------------------------

    /// handleFunDecl (erlang.ts): merge same-name clause forms into one
    /// function node and attribute each clause's calls to it.
    fn handle_fun_decl(&mut self, node: Node<'t>) -> bool {
        let mut cursor = node.walk();
        let clauses: Vec<Node<'t>> = node
            .named_children(&mut cursor)
            .filter(|c| c.kind() == "function_clause")
            .collect();
        let Some(first) = clauses.first().copied() else {
            return true; // macro-templated clause — no static name
        };
        let Some(name_node) = first.child_by_field_name("name") else {
            return true;
        };
        let name = self.atom_text(name_node);
        if name.is_empty() {
            return true;
        }

        // Continuation clause: extend the existing node's span and attribute
        // this clause's calls to it.
        if name == self.last_fn_name && self.last_fn_row != 0 {
            // Find the last emitted node row and extend its end line.
            let end_line = node.end_position().row as u32 + 1;
            self.patch_end_line(self.last_fn_row, end_line);
            self.stack.push(Scope {
                row: self.last_fn_row,
                kind: "function",
                name: name.clone(),
            });
            for clause in &clauses {
                self.visit_body(*clause);
            }
            self.stack.pop();
            return true;
        }

        let spec = self.preceding_spec(node, &name);
        let exports = self.module_exports(node);
        let docstring = match spec {
            Some(spec_node) => preceding_docstring(spec_node, self.src),
            None => preceding_docstring(node, self.src),
        };
        let signature = match spec {
            Some(spec_node) => {
                let text = self.collapse_ws(self.text(spec_node));
                if text.chars().count() > 300 {
                    Some(text.chars().take(300).collect())
                } else {
                    Some(text)
                }
            }
            None => self.clause_header(first),
        };
        let row = self.create_node(
            "function",
            &name,
            node,
            NodeExtra {
                docstring,
                signature,
                is_exported: Some(exports.contains(&name)),
            },
        );
        let Some(row) = row else { return true };
        self.stack.push(Scope {
            row,
            kind: "function",
            name: name.clone(),
        });
        for clause in &clauses {
            self.visit_body(*clause);
        }
        self.stack.pop();
        self.last_fn_name = name;
        self.last_fn_row = row;
        true
    }

    /// handleRecordDecl (erlang.ts): record_decl → struct with field children.
    fn handle_record_decl(&mut self, node: Node<'t>) -> bool {
        let Some(name_node) = node.child_by_field_name("name") else {
            return true;
        };
        let name = self.atom_text(name_node);
        let docstring = preceding_docstring(node, self.src);
        let signature_text = self.collapse_ws(self.text(node));
        let signature = if signature_text.chars().count() > 300 {
            Some(signature_text.chars().take(300).collect())
        } else {
            Some(signature_text)
        };
        let row = self.create_node(
            "struct",
            &name,
            node,
            NodeExtra {
                docstring,
                signature,
                ..NodeExtra::default()
            },
        );
        let Some(row) = row else { return true };
        self.stack.push(Scope {
            row,
            kind: "struct",
            name,
        });
        let mut cursor = node.walk();
        for field in node.named_children(&mut cursor) {
            if field.kind() != "record_field" {
                continue;
            }
            if let Some(field_name) = field.child_by_field_name("name") {
                let field_name = self.atom_text(field_name);
                self.create_node("field", &field_name, field, NodeExtra::default());
            }
        }
        self.stack.pop();
        true
    }

    /// handleTypeAlias (erlang.ts): `-type`/`-opaque` → type_alias node.
    fn handle_type_alias(&mut self, node: Node<'t>) -> bool {
        let type_name = node.child_by_field_name("name");
        let name_node = type_name.and_then(|t| t.child_by_field_name("name"));
        if let Some(name_node) = name_node {
            let name = self.atom_text(name_node);
            let signature_text = self.collapse_ws(self.text(node));
            let signature = if signature_text.chars().count() > 200 {
                Some(signature_text.chars().take(200).collect())
            } else {
                Some(signature_text)
            };
            self.create_node(
                "type_alias",
                &name,
                node,
                NodeExtra {
                    signature,
                    ..NodeExtra::default()
                },
            );
        }
        true
    }

    /// handlePpDefine (erlang.ts): `-define` → constant node whose
    /// replacement's calls attribute to the macro.
    fn handle_pp_define(&mut self, node: Node<'t>) -> bool {
        let lhs = node.child_by_field_name("lhs");
        let name_node = lhs.and_then(|l| l.child_by_field_name("name"));
        let Some(name_node) = name_node else {
            return true;
        };
        let name = self.text(name_node).to_string();
        let signature_text = self.collapse_ws(self.text(node));
        let signature = if signature_text.chars().count() > 200 {
            Some(signature_text.chars().take(200).collect())
        } else {
            Some(signature_text)
        };
        let row = self.create_node(
            "constant",
            &name,
            node,
            NodeExtra {
                signature,
                ..NodeExtra::default()
            },
        );
        let replacement = node.child_by_field_name("replacement");
        if let Some(replacement) = replacement
            && let Some(row) = row
        {
            self.stack.push(Scope {
                row,
                kind: "constant",
                name: name.clone(),
            });
            self.visit_body(replacement);
            self.stack.pop();
        }
        true
    }

    /// # Panics
    /// handleBehaviour (erlang.ts): `-behaviour(x)` → implements ref from the
    /// enclosing namespace (the module).
    fn handle_behaviour(&mut self, node: Node<'t>) -> bool {
        let name_node = node.child_by_field_name("name");
        let parent_row = self.top_row();
        if let Some(name_node) = name_node
            && parent_row != 0
        {
            let name = self.atom_text(name_node);
            self.push_ref_at(
                parent_row,
                &name,
                edge_kind_index("implements").unwrap(),
                node,
            );
        }
        true
    }

    /// # Panics
    /// handleAppResourceTuple (erlang.ts): `{application, Name, Props}.` at
    /// the top of an `.app`/`.app.src` file — entry-module references and
    /// dependency imports.
    fn handle_app_resource_tuple(&mut self, node: Node<'t>) -> bool {
        let parent_row = self.top_row();
        let props = node.named_child(2);
        let Some(props) = props else { return true };
        if props.kind() != "list" {
            return true;
        }
        let mut cursor = props.walk();
        for prop in props.named_children(&mut cursor) {
            if prop.kind() != "tuple" || prop.named_child_count() < 2 {
                continue;
            }
            let key = prop.named_child(0);
            let value = prop.named_child(1);
            let Some(key) = key else { continue };
            let Some(value) = value else { continue };
            if key.kind() != "atom" {
                continue;
            }
            let key_name = self.atom_text(key);
            if key_name == "mod" && value.kind() == "tuple" {
                if let Some(mod_node) = value.named_child(0)
                    && mod_node.kind() == "atom"
                {
                    let name = self.atom_text(mod_node);
                    if !name.is_empty() {
                        self.push_ref_at(
                            parent_row,
                            &name,
                            edge_kind_index("references").unwrap(),
                            mod_node,
                        );
                    }
                }
            } else if (key_name == "applications" || key_name == "included_applications")
                && value.kind() == "list"
            {
                let mut app_cursor = value.walk();
                for app in value.named_children(&mut app_cursor) {
                    if app.kind() == "atom" {
                        let name = self.atom_text(app);
                        if !name.is_empty() {
                            self.push_ref_at(
                                parent_row,
                                &name,
                                edge_kind_index("imports").unwrap(),
                                app,
                            );
                        }
                    }
                }
            }
        }
        true
    }

    // --- extractImport -------------------------------------------------------------

    /// # Panics
    /// extractImport (erlang.ts hook): `-import(mod, ...)` →
    /// `import_attribute`; `-include`/`-include_lib` → the header path.
    fn extract_import(&mut self, node: Node<'t>) {
        if node.kind() == "import_attribute" {
            let Some(mod_node) = node.child_by_field_name("module") else {
                return;
            };
            let module_name = self.atom_text(mod_node);
            if module_name.is_empty() {
                return;
            }
            let signature_text = self.collapse_ws(self.text(node));
            let signature = if signature_text.chars().count() > 200 {
                Some(signature_text.chars().take(200).collect())
            } else {
                Some(signature_text)
            };
            self.create_node(
                "import",
                &module_name,
                node,
                NodeExtra {
                    signature,
                    ..NodeExtra::default()
                },
            );
            let from = self.top_row();
            self.push_ref_at(
                from,
                &module_name,
                edge_kind_index("imports").unwrap(),
                node,
            );
            return;
        }
        // pp_include / pp_include_lib
        let Some(file) = node.child_by_field_name("file") else {
            return;
        };
        let mut header_path = self.text(file).to_string();
        if header_path.starts_with('"') {
            header_path.remove(0);
        }
        if header_path.ends_with('"') {
            header_path.pop();
        }
        if header_path.is_empty() {
            return;
        }
        self.create_node(
            "import",
            &header_path,
            node,
            NodeExtra {
                signature: Some(self.text(node).trim().to_string()),
                ..NodeExtra::default()
            },
        );
        let from = self.top_row();
        self.push_ref_at(
            from,
            &header_path,
            edge_kind_index("imports").unwrap(),
            node,
        );
    }

    // --- extractCall (the erlang branch) -------------------------------------------

    /// # Panics
    /// extractCall (tree-sitter.ts erlang branch): local/remote calls, fun
    /// references, macro use sites, and record references.
    fn extract_call(&mut self, node: Node<'t>) {
        let from = self.top_row();
        let line = node.start_position().row as u32 + 1;
        let column = self.col_of(node);

        if node.kind() == "call" {
            let mut callee = node.child_by_field_name("expr");
            let mut module_node: Option<Node<'t>> = None;
            if let Some(parent) = node.parent()
                && parent.kind() == "remote"
            {
                module_node = parent.child_by_field_name("module");
            } else if let Some(c) = callee
                && c.kind() == "remote"
            {
                module_node = c.child_by_field_name("module");
                callee = c.child_by_field_name("fun");
            }
            if let Some(callee) = callee
                && callee.kind() == "atom"
            {
                let fn_bare = self.atom_text(callee);
                let mut callee_name = fn_bare.clone();
                let module_expr = module_node.and_then(|m| m.child_by_field_name("module"));
                if let Some(module_expr) = module_expr {
                    if module_expr.kind() == "atom" {
                        callee_name = format!("{}::{callee_name}", self.atom_text(module_expr));
                    } else if let Some(module_expr) = Some(module_expr) {
                        // Non-atom module qualifier: `?MODULE:f(X)` keeps the bare
                        // name; anything else stays silent.
                        let macro_name = if module_expr.kind() == "macro_call_expr" {
                            module_expr.child_by_field_name("name")
                        } else {
                            None
                        };
                        match macro_name {
                            Some(name) if self.text(name) == "MODULE" => {}
                            _ => return,
                        }
                    }
                }
                self.push_ref(
                    from,
                    &callee_name,
                    edge_kind_index("calls").unwrap(),
                    line,
                    column,
                );

                // gen_server dispatch.
                if let Some(module_expr) = module_expr
                    && module_expr.kind() == "atom"
                    && self.atom_text(module_expr) == "gen_server"
                    && matches!(fn_bare.as_str(), "call" | "cast" | "send_request")
                {
                    let args_node = node.child_by_field_name("args");
                    let target = args_node.and_then(|a| a.named_child(0));
                    if let Some(target) = target
                        && let Some(target_module) = self.resolve_gen_server_target(target)
                    {
                        let handler = if fn_bare == "cast" {
                            "handle_cast"
                        } else {
                            "handle_call"
                        };
                        self.push_ref(
                            from,
                            &format!("{target_module}::{handler}"),
                            edge_kind_index("calls").unwrap(),
                            line,
                            column,
                        );
                    }
                }

                // MFA-in-argument dispatch.
                let family_key = match module_expr {
                    Some(module_expr) if module_expr.kind() == "atom" => {
                        format!("{}:{fn_bare}", self.atom_text(module_expr))
                    }
                    _ => fn_bare.clone(),
                };
                if ERLANG_MFA_CALLS.contains(&family_key.as_str()) {
                    let args_node = node.child_by_field_name("args");
                    if let Some(args_node) = args_node {
                        let mut cursor = args_node.walk();
                        let arg_exprs: Vec<Node<'t>> =
                            args_node.named_children(&mut cursor).collect();
                        for i in 0..arg_exprs.len().saturating_sub(1) {
                            let m = arg_exprs[i];
                            let f = arg_exprs[i + 1];
                            if f.kind() != "atom" {
                                continue;
                            }
                            let is_local_module = m.kind() == "macro_call_expr"
                                && m.child_by_field_name("name")
                                    .map(|n| self.text(n) == "MODULE")
                                    .unwrap_or(false);
                            if m.kind() != "atom" && !is_local_module {
                                continue;
                            }
                            let ref_name = if is_local_module {
                                self.atom_text(f)
                            } else {
                                format!("{}::{}", self.atom_text(m), self.atom_text(f))
                            };
                            let f_line = f.start_position().row as u32 + 1;
                            let f_column = util::col16(
                                self.src,
                                &self.line_starts,
                                f.start_position().row,
                                f.start_byte(),
                            );
                            self.push_ref(
                                from,
                                &ref_name,
                                edge_kind_index("calls").unwrap(),
                                f_line,
                                f_column,
                            );
                            break;
                        }
                    }
                }
            }
            return;
        }

        if matches!(node.kind(), "internal_fun" | "external_fun") {
            let fun_node = node.child_by_field_name("fun");
            let Some(fun_node) = fun_node else { return };
            if fun_node.kind() != "atom" {
                return; // dynamic parts
            }
            let mut ref_name = self.atom_text(fun_node);
            if node.kind() == "external_fun" {
                let module_wrapper = node.child_by_field_name("module");
                let module_atom = module_wrapper.and_then(|m| m.child_by_field_name("name"));
                let Some(module_atom) = module_atom else {
                    return;
                };
                if module_atom.kind() != "atom" {
                    return;
                }
                ref_name = format!("{}::{ref_name}", self.atom_text(module_atom));
            }
            self.push_ref(
                from,
                &ref_name,
                edge_kind_index("references").unwrap(),
                line,
                column,
            );
            return;
        }

        if node.kind() == "macro_call_expr" {
            let macro_name = node.child_by_field_name("name");
            let Some(macro_name) = macro_name else { return };
            let name = self.text(macro_name).to_string();
            if ERLANG_PREDEFINED_MACROS.contains(&name.as_str()) {
                return;
            }
            let kind_code = if node.child_by_field_name("args").is_some() {
                edge_kind_index("calls").unwrap()
            } else {
                edge_kind_index("references").unwrap()
            };
            self.push_ref(from, &name, kind_code, line, column);
            return;
        }

        // record_expr / record_update_expr / record_index_expr / record_field_expr
        let record_name = node.child_by_field_name("name");
        let record_atom = record_name.and_then(|r| {
            if r.kind() == "record_name" {
                r.child_by_field_name("name")
            } else {
                None
            }
        });
        if let Some(record_atom) = record_atom
            && record_atom.kind() == "atom"
        {
            let name = self.atom_text(record_atom);
            self.push_ref(
                from,
                &name,
                edge_kind_index("references").unwrap(),
                line,
                column,
            );
        }
    }

    /// resolveErlangGenServerTarget (tree-sitter.ts): a static atom, ?MODULE,
    /// or a macro defined as ?MODULE / a bare atom.
    fn resolve_gen_server_target(&mut self, target: Node<'t>) -> Option<String> {
        // The TS ownModule: basename with a trailing `.erl` removed (a `.hrl`
        // header keeps its full name — the `?MODULE` target for a header is
        // the header's own name, matching the upstream `.replace(/\.erl$/, '')`).
        let basename = self.file_path.rsplit(['/', '\\']).next().unwrap_or("");
        let own_module = basename
            .strip_suffix(".erl")
            .unwrap_or(basename)
            .to_string();
        if target.kind() == "atom" {
            let name = self.atom_text(target);
            return if name.is_empty() { None } else { Some(name) };
        }
        if target.kind() != "macro_call_expr" {
            return None;
        }
        let name_node = target.child_by_field_name("name")?;
        let macro_name = self.text(name_node).to_string();
        if macro_name == "MODULE" {
            return if own_module.is_empty() {
                None
            } else {
                Some(own_module)
            };
        }
        if self.self_macros.is_empty() && self.atom_macros.is_empty() {
            // Scan the file's -define forms once.
            let mut root = target;
            while let Some(parent) = root.parent() {
                root = parent;
            }
            let mut cursor = root.walk();
            for form in root.named_children(&mut cursor) {
                if form.kind() != "pp_define" {
                    continue;
                }
                let lhs = form.child_by_field_name("lhs");
                let def_name = lhs.and_then(|l| l.child_by_field_name("name"));
                let replacement = form.child_by_field_name("replacement");
                let (Some(def_name), Some(replacement)) = (def_name, replacement) else {
                    continue;
                };
                if replacement.kind() == "macro_call_expr"
                    && replacement
                        .child_by_field_name("name")
                        .map(|n| self.text(n) == "MODULE")
                        .unwrap_or(false)
                {
                    self.self_macros.insert(self.text(def_name).to_string());
                } else if replacement.kind() == "atom" {
                    let text = self.text(replacement);
                    let stripped =
                        if text.starts_with('\'') && text.ends_with('\'') && text.len() >= 2 {
                            text[1..text.len() - 1].to_string()
                        } else {
                            text.to_string()
                        };
                    self.atom_macros
                        .insert(self.text(def_name).to_string(), stripped);
                }
            }
        }
        if self.self_macros.contains(&macro_name) {
            return if own_module.is_empty() {
                None
            } else {
                Some(own_module)
            };
        }
        self.atom_macros.get(&macro_name).cloned()
    }

    /// # Panics
    /// Patches an emitted node row's end line (clause-merge span extension).
    fn patch_end_line(&mut self, row_index: u32, end_line: u32) {
        let index = row_index as usize;
        let row_start = index * crate::buffers::NODE_ROW_SIZE;
        let bytes = &mut self.tables.nodes[row_start..row_start + crate::buffers::NODE_ROW_SIZE];
        // Extend only when the clause is later than the current end line.
        let current = u32::from_le_bytes(
            bytes[crate::buffers::node::END_LINE..crate::buffers::node::END_LINE + 4]
                .try_into()
                .unwrap(),
        );
        if end_line > current {
            bytes[crate::buffers::node::END_LINE..crate::buffers::node::END_LINE + 4]
                .copy_from_slice(&end_line.to_le_bytes());
        }
    }

    // --- the dispatcher (visitNode) ------------------------------------------------

    /// Traverses node syntax and dispatches each recognized construct exactly once.
    fn visit_node(&mut self, node: Node<'t>) {
        let kind = node.kind();
        // erlang.ts visitNode hook.
        if self.visit_erlang_node(node) {
            return;
        }
        // Generic dispatch for import/call node kinds the hook leaves alone.
        if matches!(kind, "import_attribute" | "pp_include" | "pp_include_lib") {
            self.extract_import(node);
        } else if matches!(
            kind,
            "call"
                | "internal_fun"
                | "external_fun"
                | "record_expr"
                | "record_update_expr"
                | "record_index_expr"
                | "record_field_expr"
                | "macro_call_expr"
        ) {
            self.extract_call(node);
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_node(child);
        }
    }

    /// # Panics
    /// visitNode (languages/erlang.ts): the hook dispatches the form-level
    /// handlers; returns true when the subtree was consumed.
    fn visit_erlang_node(&mut self, node: Node<'t>) -> bool {
        match node.kind() {
            "fun_decl" => self.handle_fun_decl(node),
            "record_decl" => self.handle_record_decl(node),
            "type_alias" | "opaque" => self.handle_type_alias(node),
            "pp_define" => self.handle_pp_define(node),
            "behaviour_attribute" => self.handle_behaviour(node),
            // -spec / -callback: their type expressions parse as `call` nodes;
            // consume the subtree so the walker doesn't mint bogus call refs.
            "spec" | "callback" => true,
            // `{application, Name, Props}.` at the top of an .app/.app.src.
            "tuple" => {
                let parent = node.parent();
                let first = node.named_child(0);
                if let Some(parent) = parent
                    && parent.kind() == "source_file"
                    && is_app_resource_path(self.file_path)
                    && first.map(|f| f.kind()) == Some("atom")
                    && self.atom_text(first.unwrap()) == "application"
                {
                    self.handle_app_resource_tuple(node)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    // --- body walk -----------------------------------------------------------------

    /// visitFunctionBody (tree-sitter.ts): the body walker visits calls and
    /// nested fun_decl forms but never re-dispatches the form-level hook.
    fn visit_body(&mut self, node: Node<'t>) {
        let kind = node.kind();
        if matches!(
            kind,
            "call"
                | "internal_fun"
                | "external_fun"
                | "record_expr"
                | "record_update_expr"
                | "record_index_expr"
                | "record_field_expr"
                | "macro_call_expr"
        ) {
            self.extract_call(node);
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_body(child);
        }
    }
}

/// `/\.app(?:\.src)?$/i` — the OTP application resource file suffix test.
trait AppFileSuffix {
    /// Tests the path against the frozen `.app`/`.app.src` suffix pattern.
    fn app_file(&self) -> bool;
}

// The regex test above is implemented via a small helper to keep the hook
// faithful to the JS `\.app(?:\.src)?$` test.
impl AppFileSuffix for &str {
    /// Tests the path against the frozen `.app`/`.app.src` suffix pattern.
    fn app_file(&self) -> bool {
        let lower = self.to_lowercase();
        lower.ends_with(".app") || lower.ends_with(".app.src")
    }
}

// Re-export as a plain function used by visit_erlang_node.
/// Whether the path is an OTP application resource file (`.app`/`.app.src`).
fn is_app_resource_path(file_path: &str) -> bool {
    file_path.app_file()
}

/// Converts the arena absent sentinel into `None` and preserves present strings without normalization.
fn opt_str(arena: &mut Arena, s: Option<&str>) -> StrRef {
    match s {
        Some(s) => arena.put(s),
        None => NONE_STR,
    }
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    /// Extracts the fixture source and returns the decoded facts.
    ///
    /// # Panics
    ///
    /// Panics if the grammar is unavailable, parsing fails, or the walker emits
    /// malformed ABI tables.
    fn extract_source(path: &str, source: &str) -> crate::result::ExtractionResult {
        let emit = extract(path, source).expect("erlang extraction succeeds");
        crate::decode::decode_buffers(&emit, path, bohmian_code_review_core::Language::Erlang)
            .expect("erlang decode succeeds")
    }

    #[test]
    /// # Panics
    /// Functions merge clauses, are namespace-qualified, and export flags apply.
    fn functions_merge_clauses_and_qualify() {
        let src = "-module(foo).\n-export([bar/1]).\n\nbar(X) -> X + 1.\nbar(X, Y) -> X + Y.\n";
        let result = extract_source("foo.erl", src);
        let functions: Vec<(&str, Option<bool>)> = result
            .nodes
            .iter()
            .filter(|n| n.kind == bohmian_code_review_core::NodeKind::Function)
            .map(|n| (n.name.as_str(), n.is_exported))
            .collect();
        // Both clauses merge into one node named bar, exported.
        assert_eq!(functions, vec![("bar", Some(true))]);
        let bar = result
            .nodes
            .iter()
            .find(|n| n.name == "bar")
            .expect("bar node");
        assert_eq!(bar.qualified_name, "foo::bar");
    }

    #[test]
    /// Remote calls emit `mod::fn` references.
    fn remote_calls_emit_qualified_names() {
        let src = "-module(foo).\nf() -> gen_server:call(?SERVER, ping).\n";
        let result = extract_source("foo.erl", src);
        let calls: Vec<&str> = result
            .references
            .iter()
            .filter(|r| {
                r.reference_kind
                    == bohmian_code_review_core::ReferenceKind::Edge(bohmian_code_review_core::EdgeKind::Calls)
            })
            .map(|r| r.reference_name.as_str())
            .collect();
        assert!(calls.contains(&"gen_server::call"));
    }

    #[test]
    /// Records become structs with field members.
    fn records_become_structs_with_fields() {
        let src = "-module(foo).\n-record(user, {id, name}).\n";
        let result = extract_source("foo.erl", src);
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Struct && n.name == "user")
        );
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Field && n.name == "id")
        );
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Field && n.name == "name")
        );
    }

    #[test]
    /// Includes become imports; macros become constants.
    fn includes_and_macros_extract() {
        let src = "-module(foo).\n-include(\"bar.hrl\").\n-define(TIMEOUT, 5000).\n";
        let result = extract_source("foo.erl", src);
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Import && n.name == "bar.hrl")
        );
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Constant && n.name == "TIMEOUT")
        );
    }

    #[test]
    /// An OTP app resource file links its entry module and dependencies.
    fn app_resource_files_link_modules_and_deps() {
        let src =
            "{application, myapp, [{mod, {myapp_app, []}}, {applications, [kernel, stdlib]}]}.\n";
        let result = extract_source("myapp.app.src", src);
        let refs: Vec<(String, &str)> = result
            .references
            .iter()
            .map(|r| {
                (
                    r.reference_name.clone(),
                    match &r.reference_kind {
                        bohmian_code_review_core::ReferenceKind::Edge(kind) => kind.wire_name(),
                        bohmian_code_review_core::ReferenceKind::FunctionRef => "function_ref",
                    },
                )
            })
            .collect();
        assert!(refs.contains(&("myapp_app".to_string(), "references")));
        assert!(refs.contains(&("kernel".to_string(), "imports")));
        assert!(refs.contains(&("stdlib".to_string(), "imports")));
    }
}
