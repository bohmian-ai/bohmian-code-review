//! CFQuery extraction — a faithful Rust port of the CFQuery slice of
//! `TreeSitterExtractor` (src/extraction/tree-sitter.ts) driven by
//! `languages/cfquery.ts`.
//!
//! `<cfquery>` SQL bodies hold `#hash#` expressions that tree-sitter-cfml's
//! `cfquery` grammar parses structurally (`call_expression` /
//! `member_expression`, the same shape as cfscript's). The cfquery extractor
//! maps ONLY `call_expression` — the surrounding SQL keywords and identifiers
//! are not symbols CodeGraph models — so extraction yields call references
//! and nothing else: no functions, classes, imports, or variables.
//!
//! The frozen oracle is the vendored `src/extraction/wasm/
//! tree-sitter-cfquery.wasm` (ABI 15), built from cfmleditor/tree-sitter-cfml
//! commit a224ff1 (MIT); the vendored C under
//! `crates/codegraph-extract/grammars/cfquery` is that commit's checked-in
//! parser.c + scanner.c (the upstream wasm build was verified bit-for-bit
//! reproducible from the same commit). Behavior parity is asserted per
//! fixture against the oracle goldens. Positions are emitted in UTF-16 code
//! units.

use crate::buffers::{
    Arena, BoolFlags, EmitOut, FLAG_IS_EXPORTED, NONE_STR, NodeRow, RefRow, Tables, build_meta,
    edge_kind_index, node_kind_index,
};
use crate::ids;
use crate::textutil as util;
use regex::Regex;
use std::sync::OnceLock;
use tree_sitter::{Node, Parser};

/// `/^\(\s*\*?\s*([A-Za-z_][\w.]*)\s*\)$/` — the parenthesized-type
/// conversion gate (tree-sitter.ts).
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn paren_conversion_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\(\s*\*?\s*([A-Za-z_][\w.]*)\s*\)$").unwrap())
}

/// LITERAL_RECEIVER_TYPES (tree-sitter.ts) — a literal receiver never calls a
/// project symbol; membership is what the TS code tests.
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
    /// Emitted node row that owns the scope.
    row: u32,
}

/// Owns per-file CFQuery traversal state, lexical scopes, and ordered fact buffers.
pub struct Walker<'t> {
    /// Borrowed source text used for node slices and byte-based coordinates.
    src: &'t str,
    /// Byte offsets of line starts, used to translate tree-sitter points into emitted coordinates.
    line_starts: Vec<usize>,
    /// String arena that owns text referenced by the emitted fixed-width rows.
    arena: Arena,
    /// Flat node, edge, and reference tables accumulated for the current file.
    tables: Tables,
    /// Lexical containment stack used to form qualified names and containment edges.
    stack: Vec<Scope>,
}

/// Parses one CFQuery SQL source file and returns its flat facts, diagnostics, and measured kernel duration.
///
/// # Errors
///
/// Returns an error when the native grammar is unavailable, tree-sitter rejects grammar installation, parsing returns no tree, or the tree contains errors and must defer to canonical wasm recovery.
///
/// # Panics
///
/// Panics if a hard-coded node-kind spelling is absent from the frozen `NODE_KINDS` registry.
pub fn extract(file_path: &str, source: &str) -> Result<EmitOut, String> {
    let grammar = crate::langs::grammar_for("cfquery").ok_or("no cfquery grammar")?;
    let t0 = std::time::Instant::now();
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .map_err(|e| format!("set_language(cfquery) failed: {e}"))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "parser returned null tree".to_string())?;
    if tree.root_node().has_error() {
        return Err(crate::error::CANONICAL_DEFER_MESSAGE.to_string());
    }

    let mut w = Walker {
        src: source,
        line_starts: util::line_starts(source),
        arena: Arena::default(),
        tables: Tables::default(),
        stack: Vec::new(),
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
    w.stack.push(Scope { row: 0 });

    w.visit(tree.root_node());
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

    /// Walks the whole tree, dispatching every `call_expression` to call extraction.
    fn visit(&mut self, node: Node<'t>) {
        if node.kind() == "call_expression" {
            self.extract_call(node);
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit(child);
        }
    }

    /// # Panics
    /// extractCall (tree-sitter.ts) — the generic callee resolution for
    /// cfquery: a member_expression function yields `receiver.method`, a bare
    /// identifier yields its text, literal receivers stay silent.
    fn extract_call(&mut self, node: Node<'t>) {
        let from = self.top_row();
        let func = node
            .child_by_field_name("function")
            .or_else(|| node.named_child(0));
        let Some(func) = func else { return };
        let callee_name;

        if matches!(
            func.kind(),
            "member_expression"
                | "attribute"
                | "selector_expression"
                | "navigation_expression"
                | "field_expression"
        ) {
            let property = func
                .child_by_field_name("property")
                .or_else(|| func.child_by_field_name("field"))
                .or_else(|| func.named_child(1));
            let Some(property) = property else { return };
            let method_name = self.text(property).to_string();
            let receiver = func
                .child_by_field_name("object")
                .or_else(|| func.child_by_field_name("operand"))
                .or_else(|| func.child_by_field_name("argument"))
                .or_else(|| func.named_child(0));
            if let Some(receiver) = receiver {
                if is_literal_receiver(receiver.kind()) {
                    return;
                }
                if matches!(
                    receiver.kind(),
                    "identifier" | "simple_identifier" | "field_identifier"
                ) {
                    let receiver_name = self.text(receiver).to_string();
                    if !matches!(receiver_name.as_str(), "self" | "this" | "cls" | "super") {
                        callee_name = format!("{receiver_name}.{method_name}");
                    } else {
                        callee_name = method_name;
                    }
                } else {
                    callee_name = method_name;
                }
            } else {
                callee_name = method_name;
            }
        } else {
            callee_name = self.text(func).to_string();
        }

        // Parenthesized type conversions — `(T)(x)` — normalize to the inner name.
        let callee_name = paren_conversion_re()
            .captures(&callee_name)
            .and_then(|captures| captures.get(1))
            .map(|inner| inner.as_str().to_string())
            .unwrap_or(callee_name);

        if !callee_name.is_empty() {
            self.push_ref_at(from, &callee_name, edge_kind_index("calls").unwrap(), node);
        }
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
        let emit = extract(path, source).expect("cfquery extraction succeeds");
        crate::decode::decode_buffers(&emit, path, bohmian_code_review_core::Language::Cfquery)
            .expect("cfquery decode succeeds")
    }

    #[test]
    /// `#hash#` expressions parse as calls; plain SQL emits nothing.
    fn hash_expressions_emit_calls_only() {
        let src = "SELECT * FROM users WHERE id = #getCurrentUser().getId()#";
        let result = extract_source("s.sql", src);
        let calls: Vec<&str> = result
            .references
            .iter()
            .filter(|r| {
                r.reference_kind
                    == bohmian_code_review_core::ReferenceKind::Edge(bohmian_code_review_core::EdgeKind::Calls)
            })
            .map(|r| r.reference_name.as_str())
            .collect();
        assert!(calls.contains(&"getCurrentUser"));
        assert!(calls.contains(&"getId"));
        // No symbol nodes beyond the file node.
        assert_eq!(
            result
                .nodes
                .iter()
                .filter(|n| n.kind != bohmian_code_review_core::NodeKind::File)
                .count(),
            0
        );
    }

    #[test]
    /// A member call on a plain SQL-side identifier still resolves its callee.
    fn member_calls_emit_qualified_names() {
        let src = "SELECT #formatter.render(row)# FROM t";
        let result = extract_source("s.sql", src);
        let calls: Vec<&str> = result
            .references
            .iter()
            .filter(|r| {
                r.reference_kind
                    == bohmian_code_review_core::ReferenceKind::Edge(bohmian_code_review_core::EdgeKind::Calls)
            })
            .map(|r| r.reference_name.as_str())
            .collect();
        assert!(calls.contains(&"formatter.render"));
    }
}
