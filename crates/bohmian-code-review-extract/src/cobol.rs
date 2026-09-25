//! COBOL extraction — a faithful Rust port of `languages/cobol.ts` (the
//! visitNode-driven COBOL path of src/extraction/tree-sitter.ts).
//!
//! COBOL's AST is flat and column-oriented, so extraction runs entirely
//! through the visitNode hook: a program (PROGRAM-ID) becomes a `module`
//! node; PROCEDURE DIVISION sections and paragraphs become `function` nodes
//! with extents reconstructed from header positions; PERFORM (incl. THRU
//! ranges), GO TO, CALL 'literal', and EXEC CICS LINK/XCTL PROGRAM('X')
//! become `calls`; EXEC SQL INCLUDE becomes an `imports` ref; COPY
//! statements become import nodes + `imports` refs; DATA DIVISION entries
//! become `variable` (01/77), `field` (nested levels), or `constant`
//! (88-level) nodes; MOVE/ADD/SUBTRACT/COMPUTE write-sites become
//! `references`; and standalone `.cpy` copybooks parse via the grammar's
//! `copybook_fragment` entry point.
//!
//! The grammar is fixed-format (code area columns 8-72). `preParse` detects
//! a free-format file (a division header or level number opening before
//! column 8) and left-prefixes every line with seven spaces (with a `CGWIDE`
//! sentinel in the first line's sequence area that relaxes the column-72
//! margin); it also
//! terminates single-line `EXEC SQL INCLUDE ... END-EXEC` forms with a
//! period. The walker parses and emits positions from the SAME preprocessed
//! bytes the oracle parses — emitted ranges carry the oracle's exact
//! processed-source coordinates, including the free-format drift: each line is
//! left-prefixed with seven spaces, shifting observable columns by +7
//! (line numbers are preserved) exactly as the frozen oracle emits them — the
//! parity contract, never translated back to original bytes.
//!
//! The frozen oracle is the vendored `src/extraction/wasm/
//! tree-sitter-cobol.wasm` (ABI 15), built from yutaro-sakamoto/
//! tree-sitter-cobol commit e99dbdc3 with docs/grammars/tree-sitter-cobol.patch
//! applied and the parser regenerated with tree-sitter-cli 0.25.10 (the
//! checked-in parser.c is ABI 14); the vendored C under
//! `crates/codegraph-extract/grammars/cobol` is that generated parser.c plus
//! the patched scanner.c. Behavior parity is asserted per fixture against the
//! oracle goldens. Positions are emitted in UTF-16 code units.

use crate::buffers::{
    Arena, BoolFlags, EdgeRow, EmitOut, FLAG_IS_EXPORTED, NONE, NONE_STR, NodeRow, RefRow, StrRef,
    Tables, build_meta, edge_kind_index, node_kind_index,
};
use crate::ids;
use crate::textutil as util;
use regex::Regex;
use std::sync::OnceLock;
use tree_sitter::{Node, Parser};

/// EXEC CICS LINK/XCTL — program target, quoted literal or data name.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn exec_cics_program_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)\b(?:LINK|XCTL)\b[\s\S]*?\bPROGRAM\s*\(\s*(?:['"]([A-Za-z0-9$#@-]+)['"]|([A-Za-z0-9-]+))\s*\)"#)
            .unwrap()
    })
}

/// EXEC CICS RETURN/START — next transaction, quoted literal or data name.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn exec_cics_transid_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)\b(?:RETURN|START)\b[\s\S]*?\bTRANSID\s*\(\s*(?:['"]([A-Za-z0-9$#@]{1,4})['"]|([A-Za-z0-9-]+))\s*\)"#)
            .unwrap()
    })
}

/// EXEC SQL INCLUDE <member> — the SQL flavor of COPY.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn exec_sql_include_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bSQL\b[\s\S]*?\bINCLUDE\s+([A-Za-z0-9$#@-]+)").unwrap())
}

/// The VALUE literal in a data item's signature ("PIC X(04) VALUE 'CB00'").
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn value_literal_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)\bVALUE\s+['"]([A-Za-z0-9$#@-]+)['"]"#).unwrap())
}

/// Special registers and CICS/SQL communication areas.
fn is_special_register(name: &str) -> bool {
    matches!(
        name.to_uppercase().as_str(),
        "RETURN-CODE" | "SQLCODE" | "SQLSTATE" | "TALLY" | "WHEN-COMPILED" | "LENGTH" | "ADDRESS"
    ) || {
        let upper = name.to_uppercase();
        upper.starts_with("EIB") || upper.starts_with("DFH")
    }
}

/// One scope-stack entry (the ctx.nodeStack rows).
type ScopeRow = u32;

/// Optional node attributes collected before a node row is emitted atomically.
#[derive(Default)]
struct NodeExtra {
    /// Source signature text preserved on the emitted symbol.
    signature: Option<String>,
    /// Explicit end line override (group/paragraph extent reconstruction).
    end_line: Option<u32>,
}

/// Owns per-file COBOL traversal state, lexical scopes, and ordered fact buffers.
pub struct Walker<'t> {
    /// Borrowed preprocessed source text used for node slices and byte-based coordinates.
    src: &'t str,
    /// Repository-relative path incorporated into file facts, node identifiers, and references.
    file_path: &'t str,
    /// Byte offsets of line starts, used to translate tree-sitter points into emitted coordinates.
    line_starts: Vec<usize>,
    /// String arena that owns text referenced by the emitted fixed-width rows.
    arena: Arena,
    /// Flat node, edge, and reference tables accumulated for the current file.
    tables: Tables,
    /// Scope stack of emitted node rows (the ctx.nodeStack).
    stack: Vec<ScopeRow>,
    /// Same-file data items (upper name → signature) for CICS target
    /// dereferencing (the DATA DIVISION precedes the PROCEDURE DIVISION).
    data_items: Vec<(String, String)>,
    /// Emitted node names by row for qualified-name building.
    row_names: Vec<String>,
}

/// Parses one COBOL source file and returns its flat facts, diagnostics, and measured kernel duration.
///
/// # Errors
///
/// Returns an error when the native grammar is unavailable, tree-sitter rejects grammar installation, parsing returns no tree, or the tree contains errors and must defer to canonical wasm recovery.
///
/// # Panics
///
/// Panics if a hard-coded node-kind spelling is absent from the frozen `NODE_KINDS` registry.
pub fn extract(file_path: &str, source: &str) -> Result<EmitOut, String> {
    let grammar = crate::langs::grammar_for("cobol").ok_or("no cobol grammar")?;
    // preParse (cobol.ts): free-format detection on the ORIGINAL source;
    // free-format files are left-prefixed with seven spaces per line (with a
    // CGWIDE sentinel on line 1), then single-line EXEC SQL INCLUDE forms are
    // terminated.
    // The prepared source is parsed directly; emitted ranges carry the
    // processed-source coordinates exactly as the frozen oracle emits them
    // (no original-source remapping — that layer does not exist upstream).
    let prepared = pre_parse(source);
    extract_prepared(file_path, &prepared, &grammar)
}

/// Parses the preprocessed COBOL bytes and runs the walker.
///
/// # Errors
///
/// Returns an error when tree-sitter rejects grammar installation, parsing returns no tree, or the tree contains errors and must defer to canonical wasm recovery.
///
/// # Panics
///
/// Panics if a hard-coded node-kind spelling is absent from the frozen `NODE_KINDS` registry.
pub(crate) fn extract_prepared(
    file_path: &str,
    source: &str,
    grammar: &tree_sitter::Language,
) -> Result<EmitOut, String> {
    let t0 = std::time::Instant::now();
    let mut parser = Parser::new();
    parser
        .set_language(grammar)
        .map_err(|e| format!("set_language(cobol) failed: {e}"))?;
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
        data_items: Vec::new(),
        row_names: Vec::new(),
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
    w.row_names.push(base_name.to_string());
    w.stack.push(0);

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
    /// line(node): 1-based start row.
    fn line_of(&self, node: Node) -> u32 {
        node.start_position().row as u32 + 1
    }
    /// endLineOf(node): 1-based end row.
    fn end_line_of(&self, node: Node) -> u32 {
        node.end_position().row as u32 + 1
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
    /// The current scope row (ctx.nodeStack top).
    fn current_scope(&self) -> Option<u32> {
        self.stack.last().copied()
    }

    /// collapse (cobol.ts): collapse whitespace runs, cap at 120 chars.
    fn collapse(&self, text: &str, cap: usize) -> String {
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if flat.chars().count() > cap {
            let mut out: String = flat.chars().take(cap - 1).collect();
            out.push('…');
            out
        } else {
            flat
        }
    }

    /// # Panics
    /// addRef (cobol.ts): push an unresolved reference from the scope row.
    fn add_ref(&mut self, from_row: Option<u32>, name: &str, kind: &str, at: Node<'t>) {
        let Some(from_row) = from_row else { return };
        if name.is_empty() {
            return;
        }
        let kind_code = edge_kind_index(kind).unwrap();
        let name_ref = self.arena.put(name);
        self.tables.push_ref(&RefRow {
            from_idx: from_row,
            kind: kind_code,
            line: self.line_of(at),
            column: self.col_of(at),
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
            for row in &self.stack {
                if *row == 0 {
                    continue; // the file node is never part of a qualified name
                }
                if let Some(name) = self.row_name(*row) {
                    parts.push(name);
                }
            }
            let mut qn = parts.join("::");
            if !qn.is_empty() {
                qn.push_str("::");
            }
            qn.push_str(name);
            qn
        };

        // cobol.ts createNode sets no isExported flag.
        let flags = BoolFlags::default();
        let name_ref = self.arena.put(name);
        let qn_ref = self.arena.put(&qualified);
        let id_ref = self.arena.put(&id);
        let sig_ref = opt_str(&mut self.arena, extra.signature.as_deref());
        let end_line = extra.end_line.unwrap_or_else(|| self.end_line_of(node));
        let row = self.tables.push_node(&NodeRow {
            kind: node_kind_index(kind).unwrap(),
            visibility: 0,
            flags,
            start_line,
            end_line,
            start_column: self.col_of(node),
            end_column: self.end_col_of(node),
            name: name_ref,
            qualified_name: qn_ref,
            id: id_ref,
            docstring: NONE_STR,
            signature: sig_ref,
            decorators: NONE_STR,
            type_parameters: NONE_STR,
            return_type: NONE_STR,
            extra_json: NONE_STR,
        });

        let parent_row = self.current_scope().unwrap_or(0);
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

        if matches!(kind, "variable" | "field" | "constant") {
            self.data_items
                .push((name.to_uppercase(), extra.signature.unwrap_or_default()));
        }
        self.row_names.push(name.to_string());
        Some(row)
    }

    /// Reads an emitted node row's name for qualified-name building.
    fn row_name(&self, row: u32) -> Option<&str> {
        self.row_names.get(row as usize).map(|s| s.as_str())
    }

    /// pushScope (cobol.ts).
    fn push_scope(&mut self, row: u32) {
        self.stack.push(row);
    }

    /// popScope (cobol.ts).
    fn pop_scope(&mut self) {
        self.stack.pop();
    }

    /// handleCopy (cobol.ts): COPY <book> — import node + imports ref.
    fn handle_copy(&mut self, node: Node<'t>) {
        let Some(book) = node.child_by_field_name("book") else {
            return;
        };
        let name = self.text(book).trim().to_string();
        let name = name.trim_matches(|c| c == '\'' || c == '"').to_string();
        if name.is_empty() {
            return;
        }
        self.create_node(
            "import",
            &name,
            node,
            NodeExtra {
                signature: Some(self.collapse(self.text(node), 120)),
                ..NodeExtra::default()
            },
        );
        self.add_ref(self.current_scope(), &name, "imports", node);
    }

    /// derefSameFileValue (cobol.ts): resolve a CICS target data item against
    /// the same file's already-extracted data items.
    fn deref_same_file_value(&self, name: &str) -> Option<String> {
        let upper = name.to_uppercase();
        for (item_name, signature) in &self.data_items {
            if *item_name == upper
                && let Some(captures) = value_literal_re().captures(signature)
                && let Some(value) = captures.get(1)
            {
                return Some(value.as_str().to_string());
            }
        }
        None
    }

    /// handleExec (cobol.ts): mine EXEC ... END-EXEC blocks for CICS/SQL
    /// statically-resolvable shapes.
    fn handle_exec(&mut self, node: Node<'t>, from_row: Option<u32>) {
        let text = self.text(node);
        if let Some(captures) = exec_cics_program_re().captures(text) {
            let program = captures.get(1).map(|m| m.as_str().to_string()).or_else(|| {
                captures
                    .get(2)
                    .and_then(|m| self.deref_same_file_value(m.as_str()))
            });
            if let Some(program) = program {
                self.add_ref(from_row, &program, "calls", node);
            }
        }
        if let Some(captures) = exec_cics_transid_re().captures(text) {
            let tx = captures.get(1).map(|m| m.as_str().to_string()).or_else(|| {
                captures
                    .get(2)
                    .and_then(|m| self.deref_same_file_value(m.as_str()))
            });
            if let Some(tx) = tx {
                let upper = tx.to_uppercase();
                let valid = upper.len() <= 4
                    && upper
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '$' | '#' | '@'));
                if valid {
                    self.add_ref(from_row, &format!("cics-transid:{upper}"), "calls", node);
                }
            }
        }
        if let Some(captures) = exec_sql_include_re().captures(text)
            && let Some(include) = captures.get(1)
        {
            let member = include.as_str().to_string();
            self.create_node(
                "import",
                &member,
                node,
                NodeExtra {
                    signature: Some(self.collapse(text, 120)),
                    ..NodeExtra::default()
                },
            );
            let from = from_row.or_else(|| self.current_scope());
            self.add_ref(from, &member, "imports", node);
        }
    }

    /// targetBaseName (cobol.ts): the base data name of an assignment target.
    fn target_base_name(&self, target: Node<'t>) -> Option<Node<'t>> {
        if target.kind() == "WORD" {
            return Some(target);
        }
        let mut cursor = target.walk();
        for child in target.named_children(&mut cursor) {
            if let Some(found) = self.target_base_name(child) {
                return Some(found);
            }
        }
        None
    }

    /// emitWriteRefs (cobol.ts): references for every data item a statement
    /// writes.
    fn emit_write_refs(&mut self, statement: Node<'t>, fields: &[&str], from_row: Option<u32>) {
        for field in fields {
            let mut cursor = statement.walk();
            for target in statement.children_by_field_name(field, &mut cursor) {
                let Some(word) = self.target_base_name(target) else {
                    continue;
                };
                let name = self.text(word).trim().to_string();
                if !name.is_empty() && !is_special_register(&name) {
                    self.add_ref(from_row, &name, "references", word);
                }
            }
        }
    }

    /// collectRefs (cobol.ts): collect call/import/write references from a
    /// statement subtree, attributed to the enclosing scope.
    fn collect_refs(&mut self, node: Node<'t>, from_row: Option<u32>) {
        match node.kind() {
            "move_statement" => {
                self.emit_write_refs(node, &["dst"], from_row);
                return;
            }
            "add_statement" => {
                self.emit_write_refs(node, &["to", "giving"], from_row);
                return;
            }
            "compute_statement" => {
                self.emit_write_refs(node, &["left"], from_row);
                return;
            }
            "subtract_statement" => {
                let has_giving = {
                    let mut cursor = node.walk();
                    node.children_by_field_name("giving", &mut cursor).count() > 0
                };
                self.emit_write_refs(
                    node,
                    if has_giving { &["giving"] } else { &["from"] },
                    from_row,
                );
                return;
            }
            "perform_statement_call_proc" => {
                if let Some(procedure) = node.child_by_field_name("procedure") {
                    let mut cursor = procedure.walk();
                    for label in procedure.named_children(&mut cursor) {
                        if label.kind() != "label" {
                            continue;
                        }
                        let name = self.text(label).trim().to_string();
                        self.add_ref(from_row, &name, "calls", label);
                    }
                }
                return;
            }
            "call_statement" => {
                if let Some(x) = node.child_by_field_name("x")
                    && x.kind() == "string"
                {
                    let name = self
                        .text(x)
                        .trim()
                        .trim_matches(|c| c == '\'' || c == '"')
                        .to_string();
                    self.add_ref(from_row, &name, "calls", x);
                }
                return;
            }
            "goto_statement" => {
                if let Some(to) = node.child_by_field_name("to") {
                    let name = self.text(to).trim().to_string();
                    self.add_ref(from_row, &name, "calls", to);
                }
                return;
            }
            "exec_statement" => {
                self.handle_exec(node, from_row);
                return;
            }
            "copy_statement" => {
                self.handle_copy(node);
                return;
            }
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.collect_refs(child, from_row);
        }
    }

    /// headerName (cobol.ts): "PARA-NAME." → "PARA-NAME".
    fn header_name(&self, header: Node<'t>) -> String {
        let text = self.text(header).trim();
        let text = text.strip_suffix('.').unwrap_or(text).trim();
        text.split_whitespace().next().unwrap_or(text).to_string()
    }

    /// walkProcedureChildren (cobol.ts): reconstruct section/paragraph
    /// extents from header positions and attribute sibling statements.
    fn walk_procedure_children(&mut self, children: &[Node<'t>], division_end_line: u32) {
        let region_end = |i: usize, sections_only: bool| -> u32 {
            for j in i + 1..children.len() {
                let c = children[j];
                if c.kind() == "section_header"
                    || (!sections_only && c.kind() == "paragraph_header")
                {
                    // regionEnd (cobol.ts): `Math.max(nextHeader.row, line(cur))`
                    // — the region ends on the 1-based line BEFORE the next header.
                    return (c.start_position().row as u32)
                        .max(children[i].start_position().row as u32 + 1);
                }
            }
            division_end_line
        };

        let mut current_fn: Option<u32> = self.current_scope();
        let mut section_pushed = false;

        for (i, child) in children.iter().enumerate() {
            if child.kind() == "section_header" {
                if section_pushed {
                    self.pop_scope();
                    section_pushed = false;
                }
                let created = self.create_node(
                    "function",
                    &self.header_name(*child),
                    *child,
                    NodeExtra {
                        end_line: Some(region_end(i, true)),
                        signature: Some("SECTION".to_string()),
                    },
                );
                if let Some(created) = created {
                    self.push_scope(created);
                    section_pushed = true;
                    current_fn = Some(created);
                }
            } else if child.kind() == "paragraph_header" {
                let created = self.create_node(
                    "function",
                    &self.header_name(*child),
                    *child,
                    NodeExtra {
                        end_line: Some(region_end(i, false)),
                        ..NodeExtra::default()
                    },
                );
                if let Some(created) = created {
                    current_fn = Some(created);
                }
            } else {
                self.collect_refs(*child, current_fn);
            }
        }
        if section_pushed {
            self.pop_scope();
        }
    }

    /// programName (cobol.ts): the identification_division > program_name.
    fn program_name(&self, program_node: Node<'t>) -> Option<String> {
        let id_div = {
            let mut cursor = program_node.walk();
            program_node
                .named_children(&mut cursor)
                .find(|c| c.kind() == "identification_division")
        }?;
        let name_node = {
            let mut cursor = id_div.walk();
            id_div
                .named_children(&mut cursor)
                .find(|c| c.kind() == "program_name")
        }?;
        let name = self.text(name_node).trim().to_string();
        let name = name.trim_matches(|c| c == '\'' || c == '"').to_string();
        let name = name.strip_suffix('.').unwrap_or(&name).to_string();
        if name.is_empty() { None } else { Some(name) }
    }

    /// # Panics
    /// walkDataEntries (cobol.ts): run of DATA DIVISION entries with level
    /// nesting.
    fn walk_data_entries(&mut self, entries: &[Node<'t>]) {
        /// One DATA DIVISION entry being walked: its node, level number,
        /// and entry name.
        struct Item<'t> {
            /// The `data_description` syntax node.
            node: Node<'t>,
            /// Parsed level number (01/05/77/88...; 1 when unparseable).
            level: u32,
            /// The entry name, or `None` for unnamed (FILLER) entries.
            name: Option<String>,
        }
        let items: Vec<Option<Item<'t>>> = entries
            .iter()
            .map(|e| {
                if e.kind() != "data_description" {
                    return None;
                }
                let level_node = {
                    let mut cursor = e.walk();
                    e.named_children(&mut cursor)
                        .find(|c| c.kind() == "level_number")
                };
                let name_node = {
                    let mut cursor = e.walk();
                    e.named_children(&mut cursor)
                        .find(|c| c.kind() == "entry_name")
                };
                let level = level_node
                    .and_then(|l| self.text(l).trim().parse::<u32>().ok())
                    .unwrap_or(1);
                let name = name_node.map(|n| self.text(n).trim().to_string());
                Some(Item {
                    node: *e,
                    level,
                    name,
                })
            })
            .collect();

        let is_top_level = |level: u32| level == 1 || level == 66 || level == 77;

        let group_end = |i: usize, items: &[Option<Item<'t>>], walker: &Walker<'t>| -> u32 {
            let self_node = items[i].as_ref().unwrap().node;
            let mut end = walker.end_line_of(self_node);
            for j in i + 1..items.len() {
                let Some(item) = &items[j] else { continue };
                if item.level != 88
                    && (item.level <= items[i].as_ref().unwrap().level || is_top_level(item.level))
                {
                    break;
                }
                end = end.max(walker.end_line_of(item.node));
            }
            end
        };

        let mut open: Vec<u32> = Vec::new();
        let mut pushed = 0usize;
        let close_to =
            |level: u32, open: &mut Vec<u32>, walker: &mut Walker<'t>, pushed: &mut usize| {
                #[cfg(test)]
                if std::env::var("COBOL_DBG").is_ok() {
                    eprintln!(
                        "DBG close_to level={} open_before={:?} pushed_before={}",
                        level, open, pushed
                    );
                }
                while !open.is_empty() && open[open.len() - 1] >= level {
                    open.pop();
                    walker.pop_scope();
                    *pushed -= 1;
                }
            };

        for (i, entry) in entries.iter().enumerate() {
            if entry.kind() == "copy_statement" {
                self.handle_copy(*entry);
                continue;
            }
            if entry.kind() == "exec_statement" {
                self.handle_exec(*entry, self.current_scope());
                continue;
            }
            let Some(item) = &items[i] else { continue };
            let is_condition = item.level == 88;
            #[cfg(test)]
            if std::env::var("COBOL_DBG").is_ok() {
                eprintln!(
                    "DBG before name={:?} level={} open={:?} pushed={} stack_len={}",
                    item.name,
                    item.level,
                    open,
                    pushed,
                    self.stack.len()
                );
            }
            if !is_condition {
                close_to(
                    if is_top_level(item.level) {
                        0
                    } else {
                        item.level
                    },
                    &mut open,
                    self,
                    &mut pushed,
                );
            }
            let Some(name) = &item.name else { continue };
            if name.to_uppercase() == "FILLER" {
                continue;
            }
            let kind = if is_condition {
                "constant"
            } else if open.is_empty() {
                "variable"
            } else {
                "field"
            };
            let end_line = if is_condition {
                self.end_line_of(item.node)
            } else {
                group_end(i, &items, self)
            };
            let created = self.create_node(
                kind,
                name,
                item.node,
                NodeExtra {
                    end_line: Some(end_line),
                    signature: Some(self.collapse(self.text(item.node), 120)),
                },
            );
            if let Some(created) = created
                && !is_condition
            {
                self.push_scope(created);
                open.push(item.level);
                pushed += 1;
            }
        }
        while pushed > 0 {
            self.pop_scope();
            pushed -= 1;
        }
    }

    /// visitNode (cobol.ts hook): the flat column-oriented dispatch.
    fn visit_cobol_node(&mut self, node: Node<'t>) -> bool {
        match node.kind() {
            "program_definition" => {
                let name = self.program_name(node);
                let module_row = name
                    .as_deref()
                    .and_then(|n| self.create_node("module", n, node, NodeExtra::default()));
                if let Some(module_row) = module_row {
                    self.push_scope(module_row);
                }
                let mut cursor = node.walk();
                let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
                for child in children {
                    self.visit_node(child);
                }
                if module_row.is_some() {
                    self.pop_scope();
                }
                true
            }
            "procedure_division" => {
                let mut cursor = node.walk();
                let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
                self.walk_procedure_children(&children, self.end_line_of(node));
                true
            }
            "working_storage_section" | "record_description_list" => {
                let mut cursor = node.walk();
                let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
                self.walk_data_entries(&children);
                true
            }
            "copybook_fragment" => {
                let mut cursor = node.walk();
                let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
                if children
                    .iter()
                    .any(|c| c.kind() == "record_description_list")
                {
                    for child in children {
                        self.visit_node(child);
                    }
                } else {
                    self.walk_procedure_children(&children, self.end_line_of(node));
                }
                true
            }
            "copy_statement" => {
                self.handle_copy(node);
                true
            }
            "exec_statement" => {
                self.handle_exec(node, self.current_scope());
                true
            }
            _ => false,
        }
    }

    /// The dispatcher: the cobol visitNode hook runs first; the default
    /// dispatch descends.
    fn visit_node(&mut self, node: Node<'t>) {
        if self.visit_cobol_node(node) {
            return;
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_node(child);
        }
    }
}

/// # Panics
/// terminateSqlIncludes (cobol.ts): terminate the single-line EXEC SQL
/// INCLUDE ... END-EXEC form with a period. Split/join on '\n' preserves the
/// source's line structure exactly (no trailing newline is added).
fn terminate_sql_includes(source: &str) -> String {
    let line_re = Regex::new(
        r"(?i)^([ \t]*(?:[0-9]{6})?[ \t]+EXEC\s+SQL\s+INCLUDE\s+[A-Za-z0-9$#@-]+\s+END-EXEC)([ \t]|$)",
    )
    .unwrap();
    let end_exec_dot = Regex::new(r"(?i)END-EXEC\s*\.").unwrap();
    let lines: Vec<&str> = source.split('\n').collect();
    let mut out_lines: Vec<String> = Vec::with_capacity(lines.len());
    for line in lines {
        if !line.to_uppercase().contains("END-EXEC") || end_exec_dot.is_match(line) {
            out_lines.push(line.to_string());
            continue;
        }
        if let Some(captures) = line_re.captures(line) {
            let head = captures.get(1).unwrap().as_str();
            let tail = &line[head.len()..];
            if tail.is_empty() {
                out_lines.push(format!("{head}."));
            } else {
                out_lines.push(format!("{head}.{}", &tail[1..]));
            }
        } else {
            out_lines.push(line.to_string());
        }
    }
    out_lines.join("\n")
}

/// # Panics
/// Detects a free-format file (a decisive marker line opening before column 8).
fn free_format(source: &str) -> bool {
    let marker = Regex::new(
        r"(?i)^([ \t]*)(IDENTIFICATION\s+DIVISION|ID\s+DIVISION|PROGRAM-ID\b|\d{2}[ \t]+[A-Za-z])",
    )
    .unwrap();
    for l in source.split('\n') {
        if let Some(captures) = marker.captures(l) {
            return captures.get(1).unwrap().as_str().len() < 7;
        }
    }
    false
}

/// preParse (cobol.ts): free-format files are indented (CGWIDE sentinel on
/// line 1), then every file's single-line EXEC SQL INCLUDE forms terminate.
fn pre_parse(source: &str) -> String {
    if !free_format(source) {
        return terminate_sql_includes(source);
    }
    let indented: Vec<String> = source
        .split('\n')
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                format!("CGWIDE {l}")
            } else if l.is_empty() {
                l.to_string()
            } else {
                format!("       {l}")
            }
        })
        .collect();
    terminate_sql_includes(&indented.join("\n"))
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
        let emit = extract(path, source).expect("cobol extraction succeeds");
        crate::decode::decode_buffers(&emit, path, bohmian_code_review_core::Language::Cobol)
            .expect("cobol decode succeeds")
    }

    #[test]
    /// # Panics
    /// Programs, paragraphs, calls, and copybooks extract.
    fn program_paragraphs_and_calls_extract() {
        let src = "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. HELLO.\n       PROCEDURE DIVISION.\n       MAIN-PARA.\n           PERFORM SUB-PARA\n           STOP RUN.\n       SUB-PARA.\n           CALL 'LOGGER'\n           EXIT.\n";
        let result = extract_source("hello.cbl", src);
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Module && n.name == "HELLO")
        );
        let main = result
            .nodes
            .iter()
            .find(|n| n.name == "MAIN-PARA")
            .expect("MAIN-PARA node");
        assert!(result.references.iter().any(|r| {
            r.from_node_id == main.id
                && r.reference_name == "SUB-PARA"
                && r.reference_kind
                    == bohmian_code_review_core::ReferenceKind::Edge(bohmian_code_review_core::EdgeKind::Calls)
        }));
        assert!(result.references.iter().any(|r| {
            r.reference_name == "LOGGER"
                && r.reference_kind
                    == bohmian_code_review_core::ReferenceKind::Edge(bohmian_code_review_core::EdgeKind::Calls)
        }));
    }

    #[test]
    /// Free-format files parse with the 7-column indent preprocessing.
    fn free_format_parses() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. FREE.\nPROCEDURE DIVISION.\nMAIN.\n    DISPLAY 'hi'.\n";
        let result = extract_source("free.cbl", src);
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Module && n.name == "FREE")
        );
    }

    #[test]
    /// Data division entries become variables/fields/constants.
    fn data_division_entries_extract() {
        let src = "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. D.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 WS-REC.\n          05 WS-NAME PIC X(20).\n          88 WS-EMPTY VALUE 'NONE'.\n       PROCEDURE DIVISION.\n           MOVE WS-NAME TO WS-TMP.\n";
        let result = extract_source("d.cbl", src);
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Variable && n.name == "WS-REC")
        );
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Field && n.name == "WS-NAME")
        );
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Constant && n.name == "WS-EMPTY")
        );
    }

    #[test]
    /// # Panics
    /// The free-format preprocessing emits the frozen oracle's processed-source
    /// coordinates: every line is left-prefixed with seven spaces and the
    /// parser runs on the padded bytes, so a construct at original column 0
    /// is emitted at column 7 (observable columns shift by +7) — the
    /// observable drift is preserved, never translated back to original
    /// bytes.
    fn free_format_emits_oracle_processed_coordinates() {
        let source = "IDENTIFICATION DIVISION.\nPROGRAM-ID. FREE.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-ITEM.\n   05 WS-CODE PIC X(10).\nPROCEDURE DIVISION.\nMAIN.\n    DISPLAY \"hello\"\n    STOP RUN.\n";
        let result = extract_source("free.cbl", source);
        let main = result
            .nodes
            .iter()
            .find(|n| n.name == "MAIN")
            .expect("MAIN paragraph node");
        // The free-format detector left-prefixes every line with seven
        // spaces; the oracle parses the padded source and emits MAIN at
        // processed column 7 (original column 0 + 7) — the frozen oracle's
        // observable coordinate, never remapped back to column 0.
        assert_eq!(main.start_column, 7);
        assert_eq!(main.start_line, 8);
        // Line numbers are preserved by the padding.
        let item = result
            .nodes
            .iter()
            .find(|n| n.name == "WS-ITEM")
            .expect("WS-ITEM node");
        assert_eq!(item.start_column, 7);
        assert_eq!(item.start_line, 5);
    }

    #[test]
    /// # Panics
    /// The free-format preprocessing boundary: a fixed-format file is NOT
    /// padded, so its emitted columns are the original source columns.
    fn fixed_format_emits_unshifted_coordinates() {
        let source = "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. X.\n       PROCEDURE DIVISION.\n       MAIN.\n           DISPLAY 'hi'.\n";
        let result = extract_source("fixed.cbl", source);
        let main = result
            .nodes
            .iter()
            .find(|n| n.name == "MAIN")
            .expect("MAIN paragraph node");
        // Fixed-format source already carries seven leading spaces; no
        // left-prefix padding is applied, so the emitted column is the
        // original column 7.
        assert_eq!(main.start_column, 7);
        assert_eq!(main.start_line, 4);
    }
}
