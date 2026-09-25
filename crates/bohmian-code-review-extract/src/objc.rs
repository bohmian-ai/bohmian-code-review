//! Objective-C extraction — a faithful Rust port of the ObjC slice of
//! `TreeSitterExtractor` (src/extraction/tree-sitter.ts) plus
//! `languages/objc.ts`.
//!
//! The frozen oracle is the wasm path: tree-sitter-wasms 0.1.13's
//! `tree-sitter-objc.wasm`, built from amaanq/tree-sitter-objc 2.1.0
//! (commit dea2b2d6, MIT), whose parser.c is vendored under
//! `crates/codegraph-extract/grammars/objc` and compiled natively — table
//! identity by construction (ABI 14, no external scanner). Behavior parity
//! is asserted per fixture against the oracle goldens; a drift here is a
//! port defect, never a reason to touch the oracle.
//!
//! Ported quirks preserved on purpose: `@implementation` reuses the class
//! node from `@interface` (never mints a second class); selectors build
//! `foo:` / `foo:bar:` names from the method keywords; `@selector` values
//! keep their colon text verbatim; `@property` names nest through
//! struct_declaration → struct_declarator; class-message receivers
//! (`[NSString …]`) emit an extra `references` ref for the capitalized
//! receiver; method return types are captured from the `method_type` (skipping
//! nullability/ARC qualifiers), and `void`/`id`/`instancetype` yield none;
//! `declaration` variables only mint for a direct `identifier` declarator
//! (the generic fallback shape). ObjC is NOT in TYPE_ANNOTATION_LANGUAGES
//! (no parameter/return type refs) and NOT in VALUE_REF_LANGS. It IS in
//! FN_REF_SPECS with the C-family spec plus the `selector_expression`
//! special form. Positions are emitted in UTF-16 code units.

use crate::buffers::{
    Arena, BoolFlags, EdgeRow, EmitOut, FLAG_IS_EXPORTED, FLAG_IS_STATIC, FUNCTION_REF_CODE, NONE,
    NONE_STR, NodeRow, RefRow, StrRef, Tables, build_meta, edge_kind_index, node_kind_index,
};
use crate::docstring::preceding_docstring;
use crate::ids;
use crate::textutil as util;
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;
use tree_sitter::{Node, Parser};

/// fn-ref capture depth cap (function-ref.ts normalizeValue recursion bound).
const MAX_FN_REF_DEPTH: u32 = 4;

/// `/^[A-Za-z_]\w*$/` with JS's ASCII `\w` — the extractObjcReturnType gate
/// and the class-message receiver test.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn ascii_ident_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z_][0-9A-Za-z_]*$").unwrap())
}

/// `/^[A-Z][A-Za-z0-9_]*$/` — the capitalized class-message receiver test.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn capitalized_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Z][A-Za-z0-9_]*$").unwrap())
}

/// NAME_STOPLIST (function-ref.ts) — names that are never function values.
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

/// LITERAL_RECEIVER_TYPES (tree-sitter.ts) — literal receivers never call
/// project symbols; membership is what the TS code tests.
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

/// OBJC_TYPE_QUALIFIERS (languages/objc.ts) — nullability/ARC qualifiers that
/// sit where a return type's first type identifier does; never the type.
fn is_objc_type_qualifier(name: &str) -> bool {
    matches!(
        name,
        "nonnull"
            | "nullable"
            | "null_unspecified"
            | "null_resettable"
            | "_Nonnull"
            | "_Nullable"
            | "_Null_unspecified"
            | "__nonnull"
            | "__nullable"
            | "const"
            | "volatile"
            | "strong"
            | "weak"
            | "copy"
            | "assign"
            | "retain"
            | "oneway"
            | "__strong"
            | "__weak"
            | "__unsafe_unretained"
            | "__autoreleasing"
            | "__kindof"
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
    /// Declared return type retained for the node payload and type references.
    return_type: Option<String>,
}

/// Records a deferred function-reference candidate with its origin and source coordinate.
struct Cand {
    /// Source node row from which the unresolved reference originates.
    from: u32,
    /// Source spelling retained for symbol names, qualified names, or reference matching.
    name: String,
    /// One-based source line attached to the emitted fact.
    line: u32,
    /// Byte column retained until it is converted to the upstream UTF-16 coordinate.
    column_byte: usize,
    /// Byte row of the candidate node, retained for column conversion.
    row: usize,
    /// Which capture position produced this candidate (gate policy keys on it).
    mode: Mode,
}

/// fn-ref capture position (function-ref.ts CaptureMode).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Selects the Args branch of the enclosing closed parser state.
    Args,
    /// Selects the Rhs branch of the enclosing closed parser state.
    Rhs,
    /// Selects the Value branch of the enclosing closed parser state.
    Value,
    /// Selects the List branch of the enclosing closed parser state.
    List,
    /// Selects the VarInit branch of the enclosing closed parser state.
    Varinit,
}

/// Owns per-file Objective-C traversal state, lexical scopes, and ordered fact buffers.
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
    /// Ordered (class name, row) pairs for the `@implementation` class reuse hook.
    class_rows: Vec<(String, u32)>,
    /// Function names declared in the file, retained to validate delayed call candidates.
    defined_fn_names: HashSet<String>,
    /// Names introduced by imports, retained when qualifying unresolved references.
    imported_names: HashSet<String>,
    /// Deferred function-reference candidates resolved after all declarations are known.
    fn_ref_cands: Vec<Cand>,
}

/// Parses one Objective-C source file and returns its flat facts, diagnostics, and measured kernel duration.
///
/// # Errors
///
/// Returns an error when the native grammar is unavailable, tree-sitter rejects grammar installation, parsing returns no tree, or the tree contains errors and must defer to canonical wasm recovery.
///
/// # Panics
///
/// Panics if a hard-coded node-kind spelling is absent from the frozen `NODE_KINDS` registry.
pub fn extract(file_path: &str, source: &str) -> Result<EmitOut, String> {
    let grammar = crate::langs::grammar_for("objc").ok_or("no objc grammar")?;
    let t0 = std::time::Instant::now();
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .map_err(|e| format!("set_language(objc) failed: {e}"))?;
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
        class_rows: Vec::new(),
        defined_fn_names: HashSet::new(),
        imported_names: HashSet::new(),
        fn_ref_cands: Vec::new(),
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
        if kind_code == edge_kind_index("imports").unwrap() {
            // Feed the fn-ref flush gate the way flushFnRefCandidates derives
            // importedNames from `imports` refs.
            if util::simple_name().is_match(name) {
                self.imported_names.insert(name.to_string());
            } else if let Some(c) = util::qualified_import().captures(name) {
                self.imported_names.insert(c[1].to_string());
            }
        }
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

    /// Adds an unresolved reference at the top of the scope stack.
    fn push_top_ref(&mut self, name: &str, kind_code: u8, node: Node) {
        self.push_ref_at(self.top_row(), name, kind_code, node);
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

        // endLine body extension (resolveBody): objc functions/methods extend
        // through their compound_statement when it sits beyond the node.
        let mut end_line = node.end_position().row as u32 + 1;
        if matches!(kind, "function" | "method")
            && let Some(body) = self.resolve_body(node)
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
        let ret_ref = opt_str(&mut self.arena, extra.return_type.as_deref());
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
            return_type: ret_ref,
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
        if kind == "class" {
            self.class_rows.push((name.to_string(), row));
        }
        Some(row)
    }

    // --- fn refs (#756): cFamilySpec + selector_expression special -------------

    /// maybeCaptureFnRefs + captureFnRefCandidates for the objc cFamily
    /// dispatch: argument_list(args), assignment_expression(rhs:right),
    /// init_declarator(varinit:value), initializer_list(list),
    /// initializer_pair(value:value).
    fn maybe_capture_fn_refs(&mut self, node: Node<'t>) {
        let mode = match node.kind() {
            "argument_list" => Mode::Args,
            "assignment_expression" => Mode::Rhs,
            "init_declarator" => Mode::Varinit,
            "initializer_list" => Mode::List,
            "initializer_pair" => Mode::Value,
            _ => return,
        };
        if self.stack.is_empty() {
            return;
        }
        let from = self.top_row();

        let mut values: Vec<Node<'t>> = Vec::new();
        match mode {
            Mode::Args | Mode::List => {
                for i in 0..node.named_child_count() {
                    if let Some(c) = node.named_child(i) {
                        values.push(c);
                    }
                }
            }
            Mode::Rhs => {
                if let Some(rhs) = node.child_by_field_name("right") {
                    // Param-storage skip: `o->cb = cb` (LHS last name == RHS).
                    let lhs_text = node
                        .child_by_field_name("left")
                        .map(|l| self.text(l))
                        .unwrap_or("");
                    let lhs_last = util::lhs_last_name()
                        .captures(lhs_text)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str());
                    if !(lhs_last.is_some() && lhs_last == Some(self.text(rhs).trim())) {
                        values.push(rhs);
                    }
                }
            }
            Mode::Value => {
                let v = node.child_by_field_name("value").or_else(|| {
                    if node.named_child_count() > 0 {
                        node.named_child(node.named_child_count() - 1)
                    } else {
                        None
                    }
                });
                if let Some(v) = v {
                    values.push(v);
                }
            }
            Mode::Varinit => {
                if let Some(v) = node.child_by_field_name("value") {
                    values.push(v);
                }
            }
        }

        for v in values {
            self.normalize_fn_ref_value(v, from, mode, 0);
        }
    }

    /// normalizeValue for the objc cFamily spec: bare identifiers, the
    /// pointer_expression unwrap (`&fn`), and the selector_expression special.
    fn normalize_fn_ref_value(&mut self, v: Node<'t>, from: u32, mode: Mode, depth: u32) {
        if depth > MAX_FN_REF_DEPTH {
            return;
        }
        match v.kind() {
            "identifier" => {
                let name = self.text(v);
                if name.is_empty() || is_stoplisted(name) {
                    return;
                }
                self.push_fn_ref_cand(from, name.to_string(), mode, v);
            }
            "pointer_expression" => {
                // `&x` is a function value; `*x` is a data read.
                if v.child(0).map(|c| c.kind() != "&").unwrap_or(true) {
                    return;
                }
                let Some(inner) = v.child_by_field_name("argument") else {
                    return;
                };
                self.normalize_fn_ref_value(inner, from, mode, depth + 1);
            }
            // selector_expression special (normalizeSpecial): `@selector(foo:)`
            // — the inner text verbatim (ObjC method nodes keep their colon).
            "selector_expression" => {
                let Some(inner) = v.named_child(0) else {
                    return;
                };
                if matches!(inner.kind(), "identifier" | "simple_identifier") {
                    self.push_fn_ref_cand(from, self.text(inner).to_string(), mode, inner);
                } else if let Some(last) = last_simple_identifier(v) {
                    self.push_fn_ref_cand(from, self.text(last).to_string(), mode, last);
                } else {
                    let text = self.text(inner).trim();
                    if !text.is_empty() {
                        self.push_fn_ref_cand(from, text.to_string(), mode, inner);
                    }
                }
            }
            _ => {}
        }
    }

    /// Encodes fn ref cand into the current flat table while preserving insertion order and row indexes.
    fn push_fn_ref_cand(&mut self, from: u32, name: String, mode: Mode, node: Node) {
        let p = node.start_position();
        self.fn_ref_cands.push(Cand {
            from,
            name,
            mode,
            line: p.row as u32 + 1,
            column_byte: node.start_byte(),
            row: p.row,
        });
    }

    /// scanFnRefSubtree: capture-only walk of subtrees the main walkers skip.
    fn scan_fn_ref_subtree(&mut self, node: Node<'t>, depth: u32) {
        if depth > 12 {
            return;
        }
        if depth > 0 && node.kind() == "function_definition" {
            return;
        }
        self.maybe_capture_fn_refs(node);
        for i in 0..node.named_child_count() {
            if let Some(c) = node.named_child(i) {
                self.scan_fn_ref_subtree(c, depth + 1);
            }
        }
    }

    /// flushFnRefCandidates with the cFamily gate policy: value/list positions
    /// at FILE scope skip the same-file/import gate (C-family has no symbol
    /// imports); everything else must match a same-file function or an import.
    fn flush_fn_ref_candidates(&mut self) {
        let cands = std::mem::take(&mut self.fn_ref_cands);
        if cands.is_empty() || util::is_generated_file(self.file_path) {
            return;
        }
        let mut seen: HashSet<(String, String)> = HashSet::new();
        for c in cands {
            let at_file_scope = self.node_ids[c.from as usize].starts_with("file:");
            if !c.name.starts_with("this.") && !c.name.contains("::") {
                let skip_gate = matches!(c.mode, Mode::Value | Mode::List) && at_file_scope;
                if !skip_gate
                    && !self.defined_fn_names.contains(&c.name)
                    && !self.imported_names.contains(&c.name)
                {
                    continue;
                }
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

    // --- name / signature helpers ------------------------------------------------

    /// extractName / extractNameRaw for the objc config: the resolveName hook
    /// (selectors), the `declarator` name field, then the identifier fallback.
    fn extract_name(&self, node: Node) -> String {
        // resolveName: extractObjcMethodName.
        if matches!(node.kind(), "method_definition" | "method_declaration")
            && let Some(name) = self.objc_method_name(node)
        {
            return name;
        }
        if let Some(name_node) = node.child_by_field_name("declarator") {
            // Unwrap pointer/reference declarators, then function_declarator /
            // declarator wrappers to the inner identifier (extractNameRaw).
            let mut resolved = name_node;
            while matches!(
                resolved.kind(),
                "pointer_declarator" | "reference_declarator"
            ) {
                let Some(inner) = resolved
                    .child_by_field_name("declarator")
                    .or_else(|| resolved.named_child(0))
                else {
                    break;
                };
                resolved = inner;
            }
            if matches!(resolved.kind(), "function_declarator" | "declarator") {
                let inner = resolved
                    .child_by_field_name("declarator")
                    .or_else(|| resolved.named_child(0));
                return inner
                    .map(|n| self.text(n).to_string())
                    .unwrap_or_else(|| self.text(resolved).to_string());
            }
            return self.text(resolved).to_string();
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

    /// extractObjcMethodName (languages/objc.ts): `greet`, `doThing:`, or
    /// `doThing:with:` from the method's identifier keywords.
    fn objc_method_name(&self, node: Node) -> Option<String> {
        let identifiers: Vec<Node> = (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .filter(|c| c.kind() == "identifier")
            .collect();
        if identifiers.is_empty() {
            return None;
        }
        let has_parameters = (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .any(|c| c.kind() == "method_parameter");
        let first = identifiers[0];
        if !has_parameters {
            return Some(self.text(first).to_string());
        }
        Some(
            identifiers
                .iter()
                .map(|id| format!("{}:", self.text(*id)))
                .collect::<Vec<_>>()
                .join(""),
        )
    }

    /// extractObjcReturnType (languages/objc.ts): the first non-qualifier type
    /// identifier under `method_type`; `void`/`id`/`instancetype` yield none.
    fn objc_return_type(&self, node: Node) -> Option<String> {
        if !matches!(node.kind(), "method_definition" | "method_declaration") {
            return None;
        }
        let method_type = (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .find(|c| c.kind() == "method_type")?;
        let mut ids: Vec<String> = Vec::new();
        self.collect_type_identifiers(method_type, &mut ids);
        let name = ids.into_iter().find(|n| !is_objc_type_qualifier(n))?;
        if !ascii_ident_re().is_match(&name)
            || matches!(name.as_str(), "void" | "id" | "instancetype")
        {
            return None;
        }
        Some(name)
    }

    /// Collects the type identifiers under a node, in document order.
    fn collect_type_identifiers(&self, node: Node<'t>, out: &mut Vec<String>) {
        if node.kind() == "type_identifier" {
            out.push(self.text(node).trim().to_string());
        }
        for i in 0..node.named_child_count() {
            if let Some(c) = node.named_child(i) {
                self.collect_type_identifiers(c, out);
            }
        }
    }

    /// isStatic (languages/objc.ts): the declaration text starts with `+`.
    fn is_static(&self, node: Node) -> bool {
        self.text(node).trim_start().starts_with('+')
    }

    /// resolveBody (languages/objc.ts): the `body` field, else the first
    /// `compound_statement` child.
    fn resolve_body(&self, node: Node<'t>) -> Option<Node<'t>> {
        if let Some(from_field) = node.child_by_field_name("body") {
            return Some(from_field);
        }
        (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .find(|c| c.kind() == "compound_statement")
    }

    /// resolveTypeAliasKind (languages/objc.ts): a `typedef enum { … }` /
    /// `typedef struct { … }` with a body resolves to enum/struct.
    fn resolve_type_alias_kind(&self, node: Node<'t>) -> Option<&'static str> {
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            if child.kind() == "enum_specifier" && child.child_by_field_name("body").is_some() {
                return Some("enum");
            }
            if child.kind() == "struct_specifier" && child.child_by_field_name("body").is_some() {
                return Some("struct");
            }
        }
        None
    }

    /// extractObjcPropertyName (languages/objc.ts): the identifier nested under
    /// property_declaration → struct_declaration → struct_declarator.
    fn objc_property_name(&self, node: Node<'t>) -> Option<String> {
        if node.kind() != "property_declaration" {
            return None;
        }
        let struct_decl = (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .find(|c| c.kind() == "struct_declaration")?;
        let struct_declarator = (0..struct_decl.named_child_count())
            .filter_map(|i| struct_decl.named_child(i))
            .find(|c| c.kind() == "struct_declarator")?;
        let mut current: Option<Node<'t>> = Some(struct_declarator);
        while let Some(cur) = current {
            let inner = cur.child_by_field_name("declarator").or_else(|| {
                (0..cur.named_child_count())
                    .filter_map(|i| cur.named_child(i))
                    .find(|c| matches!(c.kind(), "identifier" | "pointer_declarator"))
            });
            let Some(inner) = inner else { break };
            if inner.kind() == "identifier" {
                return Some(self.text(inner).to_string());
            }
            current = Some(inner);
        }
        None
    }

    // --- the dispatcher (visitNode, ObjC-relevant branches) -----------------------

    /// Traverses node syntax and dispatches each recognized construct exactly once.
    fn visit_node(&mut self, node: Node<'t>) {
        let kind = node.kind();
        let mut skip_children = false;

        // Language-specific custom visitor: class_implementation.
        if self.visit_class_implementation(node) {
            return;
        }

        self.maybe_capture_fn_refs(node);

        if kind == "function_definition" {
            if self.inside_class_like() {
                self.extract_method(node);
            } else {
                self.extract_function(node);
            }
            skip_children = true;
        } else if kind == "class_interface" {
            self.extract_class(node);
            skip_children = true;
        } else if kind == "method_definition" {
            if self.inside_class_like() {
                self.extract_method(node);
            } else {
                self.extract_function(node);
            }
            skip_children = true;
        } else if kind == "protocol_declaration" {
            self.extract_interface(node);
            skip_children = true;
        } else if kind == "struct_specifier" {
            self.extract_struct(node);
            skip_children = true;
        } else if kind == "enum_specifier" {
            self.extract_enum(node);
            skip_children = true;
        } else if kind == "type_definition" {
            skip_children = self.extract_type_alias(node);
        } else if kind == "property_declaration" && self.inside_class_like() {
            self.extract_property(node);
            self.scan_fn_ref_subtree(node, 0);
            skip_children = true;
        } else if kind == "declaration" && !self.inside_class_like() {
            self.extract_variable(node);
            self.scan_fn_ref_subtree(node, 0);
            skip_children = true;
        } else if kind == "preproc_include" {
            self.extract_import(node);
        } else if matches!(kind, "call_expression" | "message_expression") {
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

    /// visitNode (languages/objc.ts): `@implementation Foo` reuses the class
    /// node from `@interface` and walks its method bodies under that scope.
    /// Returns true when the node was fully handled.
    fn visit_class_implementation(&mut self, node: Node<'t>) -> bool {
        if node.kind() != "class_implementation" {
            return false;
        }
        let class_name = (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .find(|c| c.kind() == "identifier");
        let Some(class_name_node) = class_name else {
            return true;
        };
        let class_name = self.text(class_name_node).to_string();

        // Reuse the existing class node if present; otherwise create one.
        let class_row = self.find_class_row(&class_name);
        let class_row = match class_row {
            Some(row) => row,
            None => {
                let Some(row) = self.create_node("class", &class_name, node, Extra::default())
                else {
                    return true;
                };
                row
            }
        };

        self.stack.push(Scope {
            row: class_row,
            kind: "class",
            name: class_name,
        });
        for i in 0..node.named_child_count() {
            if let Some(child) = node.named_child(i)
                && child.kind() == "implementation_definition"
            {
                for j in 0..child.named_child_count() {
                    if let Some(impl_child) = child.named_child(j) {
                        self.visit_node(impl_child);
                    }
                }
            }
        }
        self.stack.pop();
        true
    }

    /// Finds the emitted row of a same-file class node by name (first match,
    /// mirroring `ctx.nodes.find`).
    fn find_class_row(&self, name: &str) -> Option<u32> {
        self.class_rows
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, row)| *row)
    }

    // --- visitFunctionBody ---------------------------------------------------------

    /// Traverses function body syntax and dispatches each recognized construct exactly once.
    fn visit_function_body(&mut self, body: Node<'t>) {
        self.visit_for_calls_and_structure(body);
    }

    /// Traverses for calls and structure syntax and dispatches each recognized construct exactly once.
    fn visit_for_calls_and_structure(&mut self, node: Node<'t>) {
        let kind = node.kind();
        self.maybe_capture_fn_refs(node);

        if matches!(kind, "call_expression" | "message_expression") {
            self.extract_call(node);
        }

        if kind == "function_definition" {
            let name = self.extract_name(node);
            if name != "<anonymous>" {
                self.extract_function(node);
                return;
            }
        }
        if kind == "class_interface" {
            self.extract_class(node);
            return;
        }
        if kind == "protocol_declaration" {
            self.extract_interface(node);
            return;
        }
        if kind == "struct_specifier" {
            self.extract_struct(node);
            return;
        }
        if kind == "enum_specifier" {
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
            if let Some(body) = self.resolve_body(node) {
                self.visit_function_body(body);
            }
            return;
        }
        let extra = Extra {
            docstring: preceding_docstring(node, self.src),
            signature: None,
            is_static: Some(self.is_static(node)),
            return_type: self.objc_return_type(node),
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
        if let Some(body) = self.resolve_body(node) {
            self.visit_function_body(body);
        }
        self.stack.pop();
    }

    /// Recognizes method syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_method(&mut self, node: Node<'t>) {
        let name = self.extract_name(node);
        let extra = Extra {
            docstring: preceding_docstring(node, self.src),
            signature: None,
            is_static: Some(self.is_static(node)),
            return_type: self.objc_return_type(node),
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
        if let Some(body) = self.resolve_body(node) {
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
        let body = self.resolve_body(node).unwrap_or(node);
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
        let Some(row) = self.create_node("protocol", &name, node, extra) else {
            return;
        };
        self.extract_inheritance(node, row);
        self.stack.push(Scope {
            row,
            kind: "interface",
            name,
        });
        let body = self.resolve_body(node).unwrap_or(node);
        for i in 0..body.named_child_count() {
            if let Some(c) = body.named_child(i) {
                self.visit_node(c);
            }
        }
        self.stack.pop();
    }

    /// Recognizes struct syntax and emits its symbol, containment, and reference facts in source order.
    fn extract_struct(&mut self, node: Node<'t>) {
        // Body gate — a bodiless struct_specifier is a type reference.
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

    /// Recognizes enum syntax and emits its symbol, containment, and reference facts in source order.
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
            if child.kind() == "enumerator" {
                self.extract_enum_members(child);
            } else {
                self.visit_node(child);
            }
        }
        self.stack.pop();
    }

    /// extractEnumMembers: the `name` field of an enumerator.
    fn extract_enum_members(&mut self, node: Node<'t>) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.text(name_node).to_string();
            self.create_node("enum_member", &name, node, Extra::default());
        }
    }

    /// extractTypeAlias (:2890): a `type_definition` whose inner specifier has
    /// a body becomes a struct/enum node; otherwise a `type_alias`. Returns
    /// skipChildren=false (children are also recursed).
    fn extract_type_alias(&mut self, node: Node<'t>) -> bool {
        let name = self.extract_name(node);
        if name == "<anonymous>" {
            return false;
        }
        let docstring = preceding_docstring(node, self.src);

        if let Some(resolved) = self.resolve_type_alias_kind(node) {
            if resolved == "struct" {
                let Some(struct_row) = self.create_node(
                    "struct",
                    &name,
                    node,
                    Extra {
                        docstring,
                        ..Extra::default()
                    },
                ) else {
                    return true;
                };
                self.stack.push(Scope {
                    row: struct_row,
                    kind: "struct",
                    name,
                });
                let type_child = (0..node.named_child_count())
                    .filter_map(|i| node.named_child(i))
                    .find(|c| c.kind() == "struct_specifier");
                if let Some(type_child) = type_child {
                    self.extract_inheritance(type_child, struct_row);
                    let body = type_child.child_by_field_name("body").unwrap_or(type_child);
                    for i in 0..body.named_child_count() {
                        if let Some(c) = body.named_child(i) {
                            self.visit_node(c);
                        }
                    }
                }
                self.stack.pop();
                return true;
            }
            if resolved == "enum" {
                let Some(enum_row) = self.create_node(
                    "enum",
                    &name,
                    node,
                    Extra {
                        docstring,
                        ..Extra::default()
                    },
                ) else {
                    return true;
                };
                self.stack.push(Scope {
                    row: enum_row,
                    kind: "enum",
                    name,
                });
                let inner = (0..node.named_child_count())
                    .filter_map(|i| node.named_child(i))
                    .find(|c| c.kind() == "enum_specifier");
                if let Some(inner) = inner {
                    self.extract_inheritance(inner, enum_row);
                    let body = inner.child_by_field_name("body");
                    if let Some(body) = body {
                        for i in 0..body.named_child_count() {
                            let Some(child) = body.named_child(i) else {
                                continue;
                            };
                            if child.kind() == "enumerator" {
                                self.extract_enum_members(child);
                            } else {
                                self.visit_node(child);
                            }
                        }
                    }
                }
                self.stack.pop();
                return true;
            }
        }

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

    /// extractProperty for objc: `@property` names resolve through
    /// extractPropertyName; the type text and isStatic ride the node.
    fn extract_property(&mut self, node: Node<'t>) -> Option<u32> {
        let docstring = preceding_docstring(node, self.src);
        let is_static = self.is_static(node);
        let name = self.objc_property_name(node)?;
        // The generic extractProperty type scan: first named child that is not
        // modifier/modifiers/identifier/accessor_list/accessors/equals_value_clause.
        let type_node = (0..node.named_child_count()).find_map(|i| {
            let c = node.named_child(i)?;
            if matches!(
                c.kind(),
                "modifier"
                    | "modifiers"
                    | "identifier"
                    | "accessor_list"
                    | "accessors"
                    | "equals_value_clause"
            ) {
                None
            } else {
                Some(c)
            }
        });
        let type_text = type_node.map(|t| {
            let raw = self.text(t);
            raw.strip_prefix(": ")
                .unwrap_or(raw)
                .trim_start()
                .to_string()
        });
        let signature = match type_text {
            Some(t) => format!("{t} {name}"),
            None => name.clone(),
        };
        self.create_node(
            "property",
            &name,
            node,
            Extra {
                docstring,
                signature: Some(signature),
                is_static: Some(is_static),
                ..Extra::default()
            },
        )
    }

    /// extractVariable — the objc generic fallback: a `declaration`'s direct
    /// identifier/variable_declarator children.
    fn extract_variable(&mut self, node: Node<'t>) {
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
                        "variable",
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

    /// extractImport via the objc hook: preproc_include with a system library
    /// (stripped of `<>`) or a string literal (its string_content).
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn extract_import(&mut self, node: Node<'t>) {
        let import_text = self.text(node).trim().to_string();
        let system_lib = (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .find(|c| c.kind() == "system_lib_string");
        if let Some(system_lib) = system_lib {
            let module_name = self
                .text(system_lib)
                .trim()
                .trim_start_matches('<')
                .trim_end_matches('>')
                .to_string();
            self.create_node(
                "import",
                &module_name,
                node,
                Extra {
                    signature: Some(import_text),
                    ..Extra::default()
                },
            );
            self.push_top_ref(&module_name, edge_kind_index("imports").unwrap(), node);
            return;
        }
        let string_literal = (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .find(|c| c.kind() == "string_literal");
        if let Some(string_literal) = string_literal {
            let string_content = (0..string_literal.named_child_count())
                .filter_map(|i| string_literal.named_child(i))
                .find(|c| c.kind() == "string_content");
            if let Some(string_content) = string_content {
                let module_name = self.text(string_content).to_string();
                self.create_node(
                    "import",
                    &module_name,
                    node,
                    Extra {
                        signature: Some(import_text),
                        ..Extra::default()
                    },
                );
                self.push_top_ref(&module_name, edge_kind_index("imports").unwrap(), node);
            }
        }
    }

    /// extractCall — objc rides the message_expression branch and the generic
    /// member branch; the full matrix is in the module docs.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded edge-kind spelling is absent from the frozen `EDGE_KINDS` registry.
    fn extract_call(&mut self, node: Node<'t>) {
        if self.stack.is_empty() {
            return;
        }
        let caller = self.top_row();
        let line = self.line_of(node);
        let column = self.col_of(node);
        let mut callee_name = String::new();

        if node.kind() == "message_expression" {
            // Multi-keyword selector reconstruction: `[obj a:1 b:2 c:3]` has
            // three `method`-field identifiers; join with `:` to match the
            // method node names.
            let mut method_keywords: Vec<Node<'t>> = Vec::new();
            for i in 0..node.named_child_count() {
                if node.field_name_for_named_child(i as u32) == Some("method")
                    && let Some(kw) = node.named_child(i)
                {
                    method_keywords.push(kw);
                }
            }
            if !method_keywords.is_empty() {
                let mut has_colon = false;
                for i in 0..node.child_count() {
                    if let Some(c) = node.child(i)
                        && c.kind() == ":"
                    {
                        has_colon = true;
                        break;
                    }
                }
                let method_name = if has_colon {
                    method_keywords
                        .iter()
                        .map(|k| format!("{}:", self.text(*k)))
                        .collect::<Vec<_>>()
                        .join("")
                } else {
                    self.text(method_keywords[0]).to_string()
                };
                let receiver_field = node.child_by_field_name("receiver");
                if let Some(receiver) = receiver_field
                    && receiver.kind() != "message_expression"
                {
                    let receiver_name = self.text(receiver);
                    if receiver_name != "self" && receiver_name != "super" {
                        callee_name = format!("{receiver_name}.{method_name}");
                        // A CLASS-message receiver (`[SDImageCache alloc]`) is a
                        // capitalized class name; the class itself is a real
                        // dependency, so emit a references ref to it.
                        if capitalized_re().is_match(receiver_name) {
                            self.push_ref_at(
                                caller,
                                receiver_name,
                                edge_kind_index("references").unwrap(),
                                receiver,
                            );
                        }
                    } else {
                        callee_name = method_name;
                    }
                } else if let Some(receiver) = receiver_field
                    && receiver.kind() == "message_expression"
                    && util::word().is_match(&method_name)
                {
                    // Chained class-factory message send `[[Foo create] doIt]`.
                    let inner_recv = receiver.child_by_field_name("receiver");
                    let inner_name = inner_recv.map(|r| self.text(r).to_string());
                    if let (Some(inner_recv), Some(inner_name)) = (inner_recv, inner_name)
                        && inner_recv.kind() == "identifier"
                        && inner_name
                            .chars()
                            .next()
                            .map(|c| c.is_uppercase())
                            .unwrap_or(false)
                    {
                        let mut inner_kw: Vec<Node<'t>> = Vec::new();
                        for i in 0..receiver.named_child_count() {
                            if receiver.field_name_for_named_child(i as u32) == Some("method")
                                && let Some(kw) = receiver.named_child(i)
                            {
                                inner_kw.push(kw);
                            }
                        }
                        let mut inner_colon = false;
                        for i in 0..receiver.child_count() {
                            if let Some(c) = receiver.child(i)
                                && c.kind() == ":"
                            {
                                inner_colon = true;
                                break;
                            }
                        }
                        let inner_selector = if inner_colon {
                            inner_kw
                                .iter()
                                .map(|k| format!("{}:", self.text(*k)))
                                .collect::<Vec<_>>()
                                .join("")
                        } else {
                            inner_kw
                                .first()
                                .map(|k| self.text(*k).to_string())
                                .unwrap_or_default()
                        };
                        callee_name = if !inner_selector.is_empty() {
                            format!("{inner_name}.{inner_selector}().{method_name}")
                        } else {
                            method_name
                        };
                    } else {
                        callee_name = method_name;
                    }
                } else {
                    callee_name = method_name;
                }
            }
        } else {
            // Generic call_expression path.
            let func = node
                .child_by_field_name("function")
                .or_else(|| node.named_child(0));
            let Some(func) = func else { return };
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
        }

        if !callee_name.is_empty() {
            self.push_ref(
                caller,
                &callee_name,
                edge_kind_index("calls").unwrap(),
                line,
                column,
            );
        }
    }

    // --- extractDecoratorsFor / extractInheritance ---------------------------------

    /// extractDecoratorsFor — generic decorator scan. ObjC grammars carry no
    /// decorator node kinds, so this is a no-op in practice, kept for parity
    /// with the shared dispatch.
    fn extract_decorators_for(&mut self, _decl: Node<'t>, _decorated_row: u32) {}

    /// extractInheritance — the class_interface branch (superclass → extends,
    /// parameterized protocol list → implements); generic clauses are absent
    /// from the ObjC grammar, so nothing else fires.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded edge-kind spelling is absent from the frozen `EDGE_KINDS` registry.
    fn extract_inheritance(&mut self, node: Node<'t>, class_row: u32) {
        if node.kind() == "class_interface" {
            if let Some(superclass) = node.child_by_field_name("superclass") {
                let name = self.text(superclass).to_string();
                self.push_ref_at(
                    class_row,
                    &name,
                    edge_kind_index("extends").unwrap(),
                    superclass,
                );
            }
            for j in 0..node.named_child_count() {
                let Some(arg_list) = node.named_child(j) else {
                    continue;
                };
                if arg_list.kind() != "parameterized_arguments" {
                    continue;
                }
                for k in 0..arg_list.named_child_count() {
                    let Some(type_name) = arg_list.named_child(k) else {
                        continue;
                    };
                    let type_id = (0..type_name.named_child_count())
                        .filter_map(|i| type_name.named_child(i))
                        .find(|c| matches!(c.kind(), "type_identifier" | "identifier"));
                    let Some(type_id) = type_id else { continue };
                    let protocol_name = self.text(type_id).to_string();
                    self.push_ref_at(
                        class_row,
                        &protocol_name,
                        edge_kind_index("implements").unwrap(),
                        type_id,
                    );
                }
            }
            return;
        }

        // Generic clause loop — the ObjC grammar has none of these shapes.
        for i in 0..node.named_child_count() {
            let _ = node.named_child(i);
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

/// Rightmost descendant-or-self `simple_identifier` in document order
/// (function-ref.ts lastNamedOfType) — the Swift dotted selector fallback.
fn last_simple_identifier<'t>(node: Node<'t>) -> Option<Node<'t>> {
    let mut found: Option<Node<'t>> = None;
    for i in 0..node.named_child_count() {
        let Some(child) = node.named_child(i) else {
            continue;
        };
        if child.kind() == "simple_identifier" {
            found = Some(child);
        }
        if let Some(deeper) = last_simple_identifier(child) {
            found = Some(deeper);
        }
    }
    found
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
        let emit = extract(path, source).expect("objc extraction succeeds");
        crate::decode::decode_buffers(&emit, path, bohmian_code_review_core::Language::Objc)
            .expect("objc decode succeeds")
    }

    #[test]
    /// Builds ObjC selectors: unary, single-argument, and multi-keyword forms.
    /// (`@interface` method_declaration nodes mint nothing — only the
    /// `@implementation` method_definition bodies do, matching the oracle.)
    fn selectors_build_colon_suffixed_names() {
        let src = "@interface A\n@end\n@implementation A\n- (void)reset {}\n- (void)doThing:(id)x with:(id)y {}\n@end\n";
        let result = extract_source("s.m", src);
        let names: Vec<&str> = result
            .nodes
            .iter()
            .filter(|n| n.kind == bohmian_code_review_core::NodeKind::Method)
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(names, vec!["reset", "doThing:with:"]);
    }

    #[test]
    /// The class-message receiver emits both the call and a references ref.
    fn class_message_receiver_emits_reference() {
        let src = "@interface B\n@end\n@implementation B\n+ (void)go { [NSString stringWithFormat:@\"x\"]; }\n@end\n";
        let result = extract_source("s.m", src);
        let calls: Vec<&str> = result
            .references
            .iter()
            .filter(|r| {
                r.reference_kind
                    == bohmian_code_review_core::ReferenceKind::Edge(bohmian_code_review_core::EdgeKind::Calls)
            })
            .map(|r| r.reference_name.as_str())
            .collect();
        assert!(calls.contains(&"NSString.stringWithFormat:"));
        let refs: Vec<&str> = result
            .references
            .iter()
            .filter(|r| {
                r.reference_kind
                    == bohmian_code_review_core::ReferenceKind::Edge(bohmian_code_review_core::EdgeKind::References)
            })
            .map(|r| r.reference_name.as_str())
            .collect();
        assert!(refs.contains(&"NSString"));
    }

    #[test]
    /// `@implementation` reuses the interface class node instead of duplicating it.
    fn implementation_reuses_class_node() {
        let src = "@interface Foo\n@end\n@implementation Foo\n@end\n";
        let result = extract_source("s.m", src);
        let classes: Vec<&str> = result
            .nodes
            .iter()
            .filter(|n| n.kind == bohmian_code_review_core::NodeKind::Class)
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(classes, vec!["Foo"]);
    }

    #[test]
    /// Return types skip nullability qualifiers and void/id/instancetype.
    fn return_type_skips_qualifiers() {
        let src = "@interface C\n@end\n@implementation C\n- (nonnull instancetype)make {}\n- (nullable NSString *)name {}\n- (void)noop {}\n@end\n";
        let result = extract_source("s.m", src);
        let methods: Vec<(String, Option<String>)> = result
            .nodes
            .iter()
            .filter(|n| n.kind == bohmian_code_review_core::NodeKind::Method)
            .map(|n| (n.name.clone(), n.return_type.clone()))
            .collect();
        assert_eq!(
            methods,
            vec![
                ("make".to_string(), None),
                ("name".to_string(), Some("NSString".to_string())),
                ("noop".to_string(), None),
            ]
        );
    }
}
