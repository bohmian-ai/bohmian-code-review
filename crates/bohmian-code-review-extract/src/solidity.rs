//! Solidity extraction — a faithful Rust port of the Solidity slice of
//! `TreeSitterExtractor` (src/extraction/tree-sitter.ts) plus
//! `languages/solidity.ts`.
//!
//! The frozen oracle is the wasm path: tree-sitter-wasms 0.1.13's
//! `tree-sitter-solidity.wasm`, built from
//! JoranHonig/tree-sitter-solidity b239a95f (npm 1.2.0, MIT), whose
//! parser.c is vendored under `crates/codegraph-extract/grammars/solidity`
//! and compiled natively — table identity by construction (ABI 14, no
//! external scanner). Behavior parity is asserted per fixture against the
//! oracle goldens.
//!
//! Ported quirks preserved on purpose: contracts/libraries/interfaces emit
//! `extends` refs for EVERY inheritance ancestor via the visitNode hook (the
//! resolver reclassifies class→interface edges as `implements` later);
//! struct/enum members are DIRECT children (no body field), so the hook
//! walks them itself; `enum_value` is the bare identifier and takes the
//! node's own text; events/errors become `field`-kind nodes carrying their
//! declaration signature; constructor/fallback/receive synthesize their
//! names (`constructor`/`fallback`/`receive`); modifiers and base-constructor
//! invocations in the header emit `calls` refs via the decorator scan;
//! `emit`/`revert`/`modifier_invocation` are call-shaped nodes; signatures
//! reconstruct `(params) visibility mutability return` from direct children;
//! `external` maps to public visibility but keeps its raw text in the
//! signature; constants declared with an explicit type parse as
//! `state_variable_declaration` and become fields inside a contract;
//! `typedef` is NOT part of this grammar (it parses as an ERROR and defers).
//! Solidity is NOT in TYPE_ANNOTATION_LANGUAGES, VALUE_REF_LANGS, or
//! FN_REF_SPECS. Positions are emitted in UTF-16 code units.

use crate::buffers::{
    Arena, BoolFlags, EdgeRow, EmitOut, FLAG_IS_EXPORTED, FLAG_IS_STATIC, NONE, NONE_STR, NodeRow,
    RefRow, StrRef, Tables, build_meta, edge_kind_index, node_kind_index,
};
use crate::docstring::preceding_docstring;
use crate::ids;
use crate::textutil as util;
use std::collections::HashSet;
use tree_sitter::{Node, Parser};

/// LITERAL_RECEIVER_TYPES (tree-sitter.ts) — literal receivers never call project symbols.
fn is_literal_receiver(kind: &str) -> bool {
    matches!(
        kind,
        "string"
            | "string_literal"
            | "interpreted_string_literal"
            | "raw_string_literal"
            | "template_string"
            | "concatenated_string"
            | "formatted_string"
            | "f_string"
            | "line_string_literal"
            | "string_content"
            | "heredoc_body"
            | "number"
            | "number_literal"
            | "integer"
            | "integer_literal"
            | "float"
            | "float_literal"
            | "int_literal"
            | "decimal_integer_literal"
            | "real_literal"
            | "char_literal"
            | "character_literal"
            | "rune_literal"
            | "regex"
            | "regex_literal"
            | "true"
            | "false"
            | "boolean_literal"
            | "bool_literal"
            | "none"
            | "null"
            | "nil"
            | "null_literal"
            | "undefined"
            | "list"
            | "list_literal"
            | "array"
            | "array_literal"
            | "array_creation_expression"
            | "dictionary"
            | "dict_literal"
            | "object"
            | "tuple"
            | "set"
    )
}

/// One scope-stack entry so nested symbols receive stable qualified names and containment edges.
struct Scope {
    /// Emitted node row that owns the scope or deferred candidate.
    row: u32,
    /// Borrowed source spelling associated with this scope or syntax classification.
    kind: &'static str,
    /// Source spelling retained for symbol names, qualified names, or reference matching.
    name: String,
}

/// Optional node attributes collected before a node row is emitted atomically.
#[derive(Default)]
struct Extra {
    /// Normalized leading documentation attached to the emitted symbol.
    docstring: Option<String>,
    /// Source signature text preserved on the emitted symbol.
    signature: Option<String>,
    /// Language visibility mapped to the closed wire-level visibility value.
    visibility: Option<u8>,
    /// Whether the declaration is exported, encoded in the node flag bit-pair.
    is_exported: Option<bool>,
    /// Whether the declaration is static, encoded in the node flag bit-pair.
    is_static: Option<bool>,
}

/// Owns per-file Solidity traversal state, lexical scopes, and ordered fact buffers.
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
    /// Map from emitted node identifiers to table rows, used to avoid duplicate facts.
    node_ids: Vec<String>,
    /// Function names declared in the file (kept for symmetry with the shared emitters; solidity has no fn-ref spec).
    defined_fn_names: HashSet<String>,
}

/// Parses one Solidity source file and returns its flat facts, diagnostics, and measured kernel duration.
///
/// # Errors
///
/// Returns an error when the native grammar is unavailable, tree-sitter rejects grammar installation, parsing returns no tree, or the tree contains errors and must defer to canonical wasm recovery.
///
/// # Panics
///
/// Panics if a hard-coded node-kind spelling is absent from the frozen `NODE_KINDS` registry.
pub fn extract(file_path: &str, source: &str) -> Result<EmitOut, String> {
    let grammar = crate::langs::grammar_for("solidity").ok_or("no solidity grammar")?;
    let t0 = std::time::Instant::now();
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .map_err(|e| format!("set_language(solidity) failed: {e}"))?;
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
        node_ids: Vec::new(),
        defined_fn_names: HashSet::new(),
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
    w.node_ids.push(ids::file_node_id(file_path));
    w.stack.push(Scope {
        row: 0,
        kind: "file",
        name: base_name.to_string(),
    });

    w.visit_node(tree.root_node());
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
    /// Detects whether the active scope stack is owned by a class-like declaration.
    fn inside_class_like(&self) -> bool {
        self.stack
            .last()
            .map(|s| {
                matches!(
                    s.kind,
                    "class" | "struct" | "interface" | "trait" | "enum" | "module"
                )
            })
            .unwrap_or(false)
    }

    /// Encodes ref into the current flat table while preserving insertion order and row indexes.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded edge-kind spelling is absent from the frozen `EDGE_KINDS` registry.
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

    /// Encodes ref at into the current flat table while preserving insertion order and row indexes.
    fn push_ref_at(&mut self, from_row: u32, name: &str, kind_code: u8, node: Node) {
        self.push_ref(
            from_row,
            name,
            kind_code,
            self.line_of(node),
            self.col_of(node),
        );
    }

    // --- createNode ------------------------------------------------------------

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

        // endLine body extension (resolveBody): solidity functions/methods
        // extend through their function_body when it sits beyond the node.
        let mut end_line = node.end_position().row as u32 + 1;
        if matches!(kind, "function" | "method")
            && let Some(body) = node.child_by_field_name("body")
        {
            let be = body.end_position().row as u32 + 1;
            if be > end_line {
                end_line = be;
            }
        }

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
        if let Some(v) = extra.is_static {
            flags.set(FLAG_IS_STATIC, v);
        }
        let name_ref = self.arena.put(name);
        let qn_ref = self.arena.put(&qualified);
        let id_ref = self.arena.put(&id);
        let doc_ref = opt_str(&mut self.arena, extra.docstring.as_deref());
        let sig_ref = opt_str(&mut self.arena, extra.signature.as_deref());
        let row = self.tables.push_node(&NodeRow {
            kind: node_kind_index(kind).unwrap(),
            visibility: extra.visibility.unwrap_or(0),
            flags,
            start_line,
            end_line,
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

        if kind == "function" || kind == "method" {
            self.defined_fn_names.insert(name.to_string());
        }
        Some(row)
    }

    // --- name / signature / modifier helpers ------------------------------------

    /// extractName / extractNameRaw for the solidity config: the resolveName
    /// hook (constructor/fallback/receive), the `name` field, then the
    /// identifier fallback.
    fn extract_name(&self, node: Node) -> String {
        // resolveName: constructor/fallback/receive synthesize their names.
        if let Some(name) = self.resolve_name(node) {
            return name;
        }
        if let Some(name_node) = node.child_by_field_name("name") {
            return self.text(name_node).to_string();
        }
        for i in 0..node.named_child_count() {
            if let Some(c) = node.named_child(i)
                && matches!(
                    c.kind(),
                    "identifier" | "type_identifier" | "simple_identifier" | "constant"
                )
            {
                return self.text(c).to_string();
            }
        }
        "<anonymous>".to_string()
    }

    /// resolveName (languages/solidity.ts): `constructor` / the
    /// fallback-receive keyword.
    fn resolve_name(&self, node: Node) -> Option<String> {
        match node.kind() {
            "constructor_definition" => Some("constructor".to_string()),
            "fallback_receive_definition" => Some(self.fallback_receive_name(node)),
            _ => None,
        }
    }

    /// fallbackReceiveName (languages/solidity.ts): the first child (named or
    /// unnamed) whose text is `fallback` or `receive`.
    fn fallback_receive_name(&self, node: Node) -> String {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                let t = self.text(child);
                if t == "fallback" || t == "receive" {
                    return t.to_string();
                }
            }
        }
        "fallback".to_string()
    }

    /// getSignature (languages/solidity.ts): reconstruct
    /// `(params) visibility mutability return` from the direct children.
    fn signature_of(&self, node: Node<'t>) -> Option<String> {
        let mut params: Vec<String> = Vec::new();
        let mut return_type: Option<Node<'t>> = None;
        let mut visibility: Option<Node<'t>> = None;
        let mut mutability: Option<Node<'t>> = None;
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            let field_name = node.field_name_for_named_child(i as u32);
            if child.kind() == "parameter" && field_name != Some("return_type") {
                params.push(self.text(child).to_string());
            } else if child.kind() == "return_type_definition" || field_name == Some("return_type")
            {
                return_type = Some(child);
            } else if child.kind() == "visibility" {
                visibility = Some(child);
            } else if child.kind() == "state_mutability" {
                mutability = Some(child);
            }
        }

        let mut parts: Vec<String> = Vec::new();
        parts.push(format!("({})", params.join(", ")));
        if let Some(visibility) = visibility {
            parts.push(self.text(visibility).to_string());
        }
        if let Some(mutability) = mutability {
            parts.push(self.text(mutability).to_string());
        }
        if let Some(return_type) = return_type {
            parts.push(self.text(return_type).to_string());
        }
        Some(parts.join(" "))
    }

    /// getVisibility (languages/solidity.ts): public/external → public,
    /// private → private, internal → internal.
    fn visibility_of(&self, node: Node) -> Option<u8> {
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            if child.kind() != "visibility" {
                continue;
            }
            let t = self.text(child).trim();
            match t {
                "public" | "external" => return Some(1),
                "private" => return Some(2),
                "internal" => return Some(4),
                _ => return None,
            }
        }
        None
    }

    /// isConst (languages/solidity.ts): `constant_variable_declaration` is a
    /// constant by definition.
    fn is_const(&self, node: Node) -> bool {
        node.kind() == "constant_variable_declaration"
    }

    /// getInheritanceAncestors (languages/solidity.ts): the last identifier
    /// under each `inheritance_specifier`'s `ancestor` field.
    fn inheritance_ancestors(&self, node: Node<'t>) -> Vec<String> {
        let mut ancestors: Vec<String> = Vec::new();
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            if child.kind() != "inheritance_specifier" {
                continue;
            }
            let Some(ancestor) = child.child_by_field_name("ancestor") else {
                continue;
            };
            let ids: Vec<Node> = (0..ancestor.named_child_count())
                .filter_map(|j| ancestor.named_child(j))
                .filter(|c| c.kind() == "identifier")
                .collect();
            if let Some(last) = ids.last() {
                ancestors.push(self.text(*last).to_string());
            }
        }
        ancestors
    }

    // --- the dispatcher (visitNode, Solidity-relevant branches) -------------------

    /// Traverses node syntax and dispatches each recognized construct exactly once.
    fn visit_node(&mut self, node: Node<'t>) {
        let kind = node.kind();
        let mut skip_children = false;

        // Language-specific custom visitor first.
        if self.visit_solidity_node(node) {
            return;
        }

        if matches!(
            kind,
            "function_definition"
                | "modifier_definition"
                | "constructor_definition"
                | "fallback_receive_definition"
        ) {
            if self.inside_class_like() {
                self.extract_method(node);
            } else {
                self.extract_function(node);
            }
            skip_children = true;
        } else if kind == "user_defined_type_definition" {
            skip_children = self.extract_type_alias(node);
        } else if matches!(kind, "state_variable_declaration" | "struct_member")
            && self.inside_class_like()
        {
            self.extract_field(node);
            skip_children = true;
        } else if matches!(
            kind,
            "state_variable_declaration" | "constant_variable_declaration"
        ) && !self.inside_class_like()
        {
            self.extract_variable(node);
            skip_children = true;
        } else if kind == "import_directive" {
            self.extract_import(node);
        } else if matches!(
            kind,
            "call_expression" | "emit_statement" | "revert_statement" | "modifier_invocation"
        ) {
            self.extract_call(node);
        }

        if !skip_children {
            for i in 0..node.named_child_count() {
                if let Some(c) = node.named_child(i) {
                    self.visit_node(c);
                }
            }
        }
    }

    /// visitNode (languages/solidity.ts): the hook fully handles
    /// contract/library/interface, struct/enum declarations, enum values,
    /// event definitions, and error declarations. Returns true when handled.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn visit_solidity_node(&mut self, node: Node<'t>) -> bool {
        let t = node.kind();

        // Solidity inheritance: emit `extends` for every ancestor, then walk
        // the body under the new node. Returns true to skip generic dispatch.
        if matches!(
            t,
            "contract_declaration" | "library_declaration" | "interface_declaration"
        ) {
            let ancestors = self.inheritance_ancestors(node);
            let Some(name_node) = node.child_by_field_name("name") else {
                return false;
            };
            let name = self.text(name_node).to_string();
            let kind: &'static str = if t == "interface_declaration" {
                "interface"
            } else {
                "class"
            };
            let Some(created) = self.create_node(
                kind,
                &name,
                node,
                Extra {
                    docstring: preceding_docstring(node, self.src),
                    ..Extra::default()
                },
            ) else {
                return true;
            };
            for ancestor in ancestors {
                self.push_ref_at(
                    created,
                    &ancestor,
                    edge_kind_index("extends").unwrap(),
                    node,
                );
            }
            self.stack.push(Scope {
                row: created,
                kind,
                name,
            });
            if let Some(body) = node.child_by_field_name("body") {
                for i in 0..body.named_child_count() {
                    if let Some(child) = body.named_child(i) {
                        self.visit_node(child);
                    }
                }
            }
            self.stack.pop();
            return true;
        }

        // struct_declaration / enum_declaration: no body field — members are
        // direct children. Emit the node and walk the direct children.
        if matches!(t, "struct_declaration" | "enum_declaration") {
            let Some(name_node) = node.child_by_field_name("name") else {
                return true;
            };
            let name = self.text(name_node).to_string();
            let kind: &'static str = if t == "struct_declaration" {
                "struct"
            } else {
                "enum"
            };
            let Some(created) = self.create_node(kind, &name, node, Extra::default()) else {
                return true;
            };
            self.stack.push(Scope {
                row: created,
                kind,
                name,
            });
            for i in 0..node.named_child_count() {
                let Some(child) = node.named_child(i) else {
                    continue;
                };
                if child == name_node {
                    continue;
                }
                self.visit_node(child);
            }
            self.stack.pop();
            return true;
        }

        // enum_value is the bare identifier of an enum case.
        if t == "enum_value" {
            let name = self.text(node).to_string();
            self.create_node("enum_member", &name, node, Extra::default());
            return true;
        }

        // event SomeEvent(...) → field-shaped node carrying the event name.
        if t == "event_definition" {
            let Some(name_node) = node.child_by_field_name("name") else {
                return true;
            };
            let name = self.text(name_node).to_string();
            let sig = util::slice_utf16(self.text(node).trim(), 200).0;
            self.create_node(
                "field",
                &name,
                node,
                Extra {
                    signature: Some(sig),
                    ..Extra::default()
                },
            );
            return true;
        }

        // error MyError(...) → field-shaped node carrying the error name.
        if t == "error_declaration" {
            let Some(name_node) = node.child_by_field_name("name") else {
                return true;
            };
            let name = self.text(name_node).to_string();
            let sig = util::slice_utf16(self.text(node).trim(), 200).0;
            self.create_node(
                "field",
                &name,
                node,
                Extra {
                    signature: Some(sig),
                    ..Extra::default()
                },
            );
            return true;
        }

        false
    }

    // --- visitFunctionBody ---------------------------------------------------------

    /// Traverses function body syntax and dispatches each recognized construct exactly once.
    fn visit_function_body(&mut self, body: Node<'t>) {
        self.visit_for_calls_and_structure(body);
    }

    /// Traverses for calls and structure syntax and dispatches each recognized construct exactly once.
    fn visit_for_calls_and_structure(&mut self, node: Node<'t>) {
        let kind = node.kind();

        if matches!(
            kind,
            "call_expression" | "emit_statement" | "revert_statement" | "modifier_invocation"
        ) {
            self.extract_call(node);
        } else if kind == "new_expression" {
            self.extract_instantiation(node);
        }

        // Nested NAMED functions become their own nodes.
        if matches!(kind, "function_definition" | "modifier_definition") {
            let name = self.extract_name(node);
            if name != "<anonymous>" {
                self.extract_function(node);
                return;
            }
        }

        // Nested contracts use the GENERIC class extractor here (the body
        // walker does not consult the language visitNode hook).
        if matches!(kind, "contract_declaration" | "library_declaration") {
            self.extract_class(node);
            return;
        }
        if kind == "interface_declaration" {
            self.extract_interface(node);
            return;
        }
        if kind == "struct_declaration" {
            self.extract_struct(node);
            return;
        }
        if kind == "enum_declaration" {
            self.extract_enum(node);
            return;
        }

        for i in 0..node.named_child_count() {
            if let Some(c) = node.named_child(i) {
                self.visit_for_calls_and_structure(c);
            }
        }
    }

    // --- extractors -----------------------------------------------------------------

    /// Recognizes function syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_function(&mut self, node: Node<'t>) {
        let name = self.extract_name(node);
        if name == "<anonymous>" {
            if let Some(body) = node.child_by_field_name("body") {
                self.visit_function_body(body);
            }
            return;
        }
        let extra = Extra {
            docstring: preceding_docstring(node, self.src),
            signature: self.signature_of(node),
            visibility: self.visibility_of(node),
            ..Extra::default()
        };
        let Some(row) = self.create_node("function", &name, node, extra) else {
            return;
        };
        self.extract_decorators_for(node, row);
        self.stack.push(Scope {
            row,
            kind: "function",
            name,
        });
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_function_body(body);
        }
        self.stack.pop();
    }

    /// Recognizes method syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_method(&mut self, node: Node<'t>) {
        let name = self.extract_name(node);
        let extra = Extra {
            docstring: preceding_docstring(node, self.src),
            signature: self.signature_of(node),
            visibility: self.visibility_of(node),
            ..Extra::default()
        };
        let Some(row) = self.create_node("method", &name, node, extra) else {
            return;
        };
        self.extract_decorators_for(node, row);
        self.stack.push(Scope {
            row,
            kind: "method",
            name,
        });
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_function_body(body);
        }
        self.stack.pop();
    }

    /// Recognizes class syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_class(&mut self, node: Node<'t>) {
        let name = self.extract_name(node);
        let extra = Extra {
            docstring: preceding_docstring(node, self.src),
            ..Extra::default()
        };
        let Some(row) = self.create_node("class", &name, node, extra) else {
            return;
        };
        self.extract_inheritance(node, row);
        self.extract_decorators_for(node, row);
        self.stack.push(Scope {
            row,
            kind: "class",
            name,
        });
        let body = node.child_by_field_name("body").unwrap_or(node);
        for i in 0..body.named_child_count() {
            if let Some(c) = body.named_child(i) {
                self.visit_node(c);
            }
        }
        self.stack.pop();
    }

    /// Recognizes interface syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_interface(&mut self, node: Node<'t>) {
        let name = self.extract_name(node);
        let extra = Extra {
            docstring: preceding_docstring(node, self.src),
            ..Extra::default()
        };
        let Some(row) = self.create_node("interface", &name, node, extra) else {
            return;
        };
        self.extract_inheritance(node, row);
        self.stack.push(Scope {
            row,
            kind: "interface",
            name,
        });
        let body = node.child_by_field_name("body").unwrap_or(node);
        for i in 0..body.named_child_count() {
            if let Some(c) = body.named_child(i) {
                self.visit_node(c);
            }
        }
        self.stack.pop();
    }

    /// Recognizes struct syntax (generic extractStruct: body field required).
    fn extract_struct(&mut self, node: Node<'t>) {
        let Some(body) = node.child_by_field_name("body") else {
            return;
        };
        let name = self.extract_name(node);
        let extra = Extra {
            docstring: preceding_docstring(node, self.src),
            ..Extra::default()
        };
        let Some(row) = self.create_node("struct", &name, node, extra) else {
            return;
        };
        self.extract_inheritance(node, row);
        self.stack.push(Scope {
            row,
            kind: "struct",
            name,
        });
        for i in 0..body.named_child_count() {
            if let Some(c) = body.named_child(i) {
                self.visit_node(c);
            }
        }
        self.stack.pop();
    }

    /// Recognizes enum syntax (generic extractEnum: body field required).
    fn extract_enum(&mut self, node: Node<'t>) {
        let Some(body) = node.child_by_field_name("body") else {
            return;
        };
        let name = self.extract_name(node);
        let extra = Extra {
            docstring: preceding_docstring(node, self.src),
            ..Extra::default()
        };
        let Some(row) = self.create_node("enum", &name, node, extra) else {
            return;
        };
        self.extract_inheritance(node, row);
        self.stack.push(Scope {
            row,
            kind: "enum",
            name,
        });
        for i in 0..body.named_child_count() {
            let Some(child) = body.named_child(i) else {
                continue;
            };
            if child.kind() == "enum_value" {
                let member_name = self.text(child).to_string();
                self.create_node("enum_member", &member_name, child, Extra::default());
            } else {
                self.visit_node(child);
            }
        }
        self.stack.pop();
    }

    /// extractTypeAlias (user_defined_type_definition): a plain type_alias
    /// node. Returns skipChildren=false (children also recursed).
    fn extract_type_alias(&mut self, node: Node<'t>) -> bool {
        let name = self.extract_name(node);
        if name == "<anonymous>" {
            return false;
        }
        let docstring = preceding_docstring(node, self.src);
        self.create_node(
            "type_alias",
            &name,
            node,
            Extra {
                docstring,
                ..Extra::default()
            },
        );
        false
    }

    /// extractField for solidity: state variables and struct members inside a
    /// class-like scope become `field` nodes (isStatic present-false).
    fn extract_field(&mut self, node: Node<'t>) {
        let docstring = preceding_docstring(node, self.src);
        let visibility = self.visibility_of(node);
        let name_node = node.child_by_field_name("name").or_else(|| {
            (0..node.named_child_count())
                .filter_map(|i| node.named_child(i))
                .find(|c| c.kind() == "identifier")
        });
        let Some(name_node) = name_node else { return };
        let name = self.text(name_node).to_string();
        self.create_node(
            "field",
            &name,
            node,
            Extra {
                docstring,
                visibility,
                is_static: Some(false),
                ..Extra::default()
            },
        );
    }

    /// extractVariable — the solidity generic fallback: direct identifier /
    /// variable_declarator children; constant_variable_declaration → constant.
    fn extract_variable(&mut self, node: Node<'t>) {
        let is_const = self.is_const(node);
        let kind: &'static str = if is_const { "constant" } else { "variable" };
        let docstring = preceding_docstring(node, self.src);
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            if child.kind() == "identifier" || child.kind() == "variable_declarator" {
                let name = if child.kind() == "identifier" {
                    self.text(child).to_string()
                } else {
                    self.extract_name(child)
                };
                if !name.is_empty() && name != "<anonymous>" {
                    self.create_node(
                        kind,
                        &name,
                        child,
                        Extra {
                            docstring: docstring.clone(),
                            ..Extra::default()
                        },
                    );
                }
            }
        }
    }

    /// extractImport via the solidity hook: the SOURCE path is the moduleName.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn extract_import(&mut self, node: Node<'t>) {
        let import_text = self.text(node).trim().to_string();
        let Some(source_field) = node.child_by_field_name("source") else {
            return;
        };
        // source is a `string` node — take its own text and strip quotes.
        let raw = self.text(source_field);
        let mut module_name = raw.to_string();
        module_name = module_name
            .trim_start_matches(['"', '\''])
            .trim_end_matches(['"', '\''])
            .trim()
            .to_string();
        if module_name.is_empty() {
            return;
        }
        self.create_node(
            "import",
            &module_name,
            node,
            Extra {
                signature: Some(import_text),
                ..Extra::default()
            },
        );
        self.push_ref_at(
            self.top_row(),
            &module_name,
            edge_kind_index("imports").unwrap(),
            node,
        );
    }

    /// extractCall — solidity call/emit/revert/modifier-invocation forms ride
    /// the generic member branch and the raw-text else.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded edge-kind spelling is absent from the frozen `EDGE_KINDS` registry.
    fn extract_call(&mut self, node: Node<'t>) {
        if self.stack.is_empty() {
            return;
        }
        let caller = self.top_row();
        let func = node
            .child_by_field_name("function")
            .or_else(|| node.child_by_field_name("name"))
            .or_else(|| node.child_by_field_name("error"))
            .or_else(|| node.named_child(0));
        let Some(func) = func else { return };
        let mut callee_name = String::new();

        if matches!(
            func.kind(),
            "member_expression"
                | "attribute"
                | "selector_expression"
                | "navigation_expression"
                | "field_expression"
        ) {
            let mut property = func
                .child_by_field_name("property")
                .or_else(|| func.child_by_field_name("field"));
            if property.is_none() {
                let child1 = func.named_child(1);
                if let Some(c) = child1
                    && c.kind() == "navigation_suffix"
                {
                    property = (0..c.named_child_count())
                        .filter_map(|i| c.named_child(i))
                        .find(|g| g.kind() == "simple_identifier")
                        .or(Some(c));
                } else {
                    property = child1;
                }
            }
            if let Some(property) = property {
                let method_name = self.text(property).to_string();
                let receiver = func
                    .child_by_field_name("object")
                    .or_else(|| func.child_by_field_name("operand"))
                    .or_else(|| func.child_by_field_name("argument"))
                    .or_else(|| func.named_child(0));
                if let Some(r) = receiver
                    && is_literal_receiver(r.kind())
                {
                    return;
                }
                if let Some(r) = receiver
                    && matches!(
                        r.kind(),
                        "identifier" | "simple_identifier" | "field_identifier"
                    )
                {
                    let receiver_name = self.text(r);
                    if receiver_name != "self"
                        && receiver_name != "this"
                        && receiver_name != "cls"
                        && receiver_name != "super"
                    {
                        callee_name = format!("{receiver_name}.{method_name}");
                    } else {
                        callee_name = method_name;
                    }
                } else {
                    callee_name = method_name;
                }
            }
        } else {
            callee_name = self.text(func).to_string();
        }

        if !callee_name.is_empty() {
            self.push_ref(
                caller,
                &callee_name,
                edge_kind_index("calls").unwrap(),
                self.line_of(node),
                self.col_of(node),
            );
        }
    }

    /// extractInstantiation: `new Foo()` emits an `instantiates` ref.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn extract_instantiation(&mut self, node: Node<'t>) {
        if self.stack.is_empty() {
            return;
        }
        let from = self.top_row();
        let ctor = node
            .child_by_field_name("constructor")
            .or_else(|| node.child_by_field_name("type"))
            .or_else(|| node.child_by_field_name("name"))
            .or_else(|| node.named_child(0));
        let Some(ctor) = ctor else { return };
        let mut class_name = self.text(ctor).to_string();
        let lt = class_name.find('<');
        if let Some(idx) = lt
            && idx > 0
        {
            class_name = class_name[..idx].to_string();
        }
        let last_dot = class_name.rfind('.').max(class_name.rfind("::"));
        if let Some(idx) = last_dot {
            class_name = class_name[idx + 1..]
                .trim_start_matches([':', '.'])
                .to_string();
        }
        class_name = class_name.trim().to_string();
        if !class_name.is_empty() {
            self.push_ref(
                from,
                &class_name,
                edge_kind_index("instantiates").unwrap(),
                self.line_of(node),
                self.col_of(node),
            );
        }
    }

    /// extractDecoratorsFor — the shared scan; Solidity's `modifier_invocation`
    /// in the function header emits a `calls` ref (modifier and base-constructor
    /// hops), other decorator kinds do not exist in this grammar.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded edge-kind spelling is absent from the frozen `EDGE_KINDS` registry.
    fn extract_decorators_for(&mut self, decl: Node<'t>, decorated_row: u32) {
        for i in 0..decl.named_child_count() {
            let Some(child) = decl.named_child(i) else {
                continue;
            };
            self.consider_decorator(child, decorated_row);
        }
    }

    /// consider(): the modifier_invocation branch of extractDecoratorsFor.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn consider_decorator(&mut self, n: Node<'t>, decorated_row: u32) {
        if n.kind() == "modifier_invocation" {
            let target = n.named_child(0);
            let name = target
                .filter(|t| t.kind() == "identifier")
                .map(|t| self.text(t).to_string());
            if let Some(name) = name {
                self.push_ref_at(decorated_row, &name, edge_kind_index("calls").unwrap(), n);
            }
            return;
        }
        let _ = decorated_row;
        // No other decorator node kinds exist in the Solidity grammar.
    }

    /// extractInheritance — the generic clause walk; Solidity has no
    /// extends_clause/base_class_clause shapes, so this is a no-op (the hook
    /// emits extends refs itself).
    fn extract_inheritance(&mut self, _node: Node<'t>, _class_row: u32) {}
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
        let emit = extract(path, source).expect("solidity extraction succeeds");
        crate::decode::decode_buffers(&emit, path, bohmian_code_review_core::Language::Solidity)
            .expect("solidity decode succeeds")
    }

    #[test]
    /// Contracts, interfaces, and libraries classify into class/interface nodes.
    fn contract_like_containers_classify() {
        let src = "contract A {}\ninterface I {}\nlibrary L {}\n";
        let result = extract_source("s.sol", src);
        let kinds: Vec<(&str, &str)> = result
            .nodes
            .iter()
            .map(|n| (n.kind.wire_name(), n.name.as_str()))
            .collect();
        assert!(kinds.contains(&("class", "A")));
        assert!(kinds.contains(&("interface", "I")));
        assert!(kinds.contains(&("class", "L")));
    }

    #[test]
    /// Contract inheritance emits one extends ref per ancestor.
    fn inheritance_emits_extends_refs() {
        let src = "contract Child is Base, IERC20 {}\n";
        let result = extract_source("s.sol", src);
        let extends: Vec<&str> = result
            .references
            .iter()
            .filter(|r| {
                r.reference_kind
                    == bohmian_code_review_core::ReferenceKind::Edge(
                        bohmian_code_review_core::EdgeKind::Extends,
                    )
            })
            .map(|r| r.reference_name.as_str())
            .collect();
        assert_eq!(extends, vec!["Base", "IERC20"]);
    }

    #[test]
    /// Constructor/fallback/receive synthesize their names and signatures.
    fn nameless_definitions_synthesize_names() {
        let src = "contract C {\n  constructor(address a) {}\n  fallback() external payable {}\n  receive() external payable {}\n}\n";
        let result = extract_source("s.sol", src);
        let methods: Vec<(String, Option<String>)> = result
            .nodes
            .iter()
            .filter(|n| n.kind == bohmian_code_review_core::NodeKind::Method)
            .map(|n| (n.name.clone(), n.signature.clone()))
            .collect();
        assert_eq!(
            methods,
            vec![
                ("constructor".to_string(), Some("(address a)".to_string())),
                (
                    "fallback".to_string(),
                    Some("() external payable".to_string())
                ),
                (
                    "receive".to_string(),
                    Some("() external payable".to_string())
                ),
            ]
        );
    }

    #[test]
    /// emit/revert/modifier_invocation are call-shaped and resolve by name.
    fn emit_and_revert_emit_calls() {
        let src = "contract C {\n  event E(address a);\n  error Bad();\n  modifier onlyOwner() { _; }\n  function f() public onlyOwner { emit E(msg.sender); revert Bad(); }\n}\n";
        let result = extract_source("s.sol", src);
        let calls: Vec<&str> = result
            .references
            .iter()
            .filter(|r| {
                r.reference_kind
                    == bohmian_code_review_core::ReferenceKind::Edge(
                        bohmian_code_review_core::EdgeKind::Calls,
                    )
            })
            .map(|r| r.reference_name.as_str())
            .collect();
        assert!(calls.contains(&"onlyOwner"));
        assert!(calls.contains(&"E"));
        assert!(calls.contains(&"Bad"));
    }

    #[test]
    /// Struct and enum members become field/enum_member nodes under the container.
    fn struct_and_enum_members() {
        let src = "struct Point { uint x; uint y; }\nenum Color { RED, GREEN }\n";
        let result = extract_source("s.sol", src);
        let nodes: Vec<(String, String)> = result
            .nodes
            .iter()
            .filter(|n| n.kind != bohmian_code_review_core::NodeKind::File)
            .map(|n| (n.kind.wire_name().to_string(), n.name.clone()))
            .collect();
        assert!(nodes.contains(&("struct".into(), "Point".into())));
        assert!(nodes.contains(&("field".into(), "x".into())));
        assert!(nodes.contains(&("field".into(), "y".into())));
        assert!(nodes.contains(&("enum".into(), "Color".into())));
        assert!(nodes.contains(&("enum_member".into(), "RED".into())));
        assert!(nodes.contains(&("enum_member".into(), "GREEN".into())));
    }
}
