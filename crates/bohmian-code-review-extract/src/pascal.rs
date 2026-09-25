//! Pascal/Delphi extraction — a faithful Rust port of the Pascal slice of
//! `TreeSitterExtractor` (src/extraction/tree-sitter.ts) driven by
//! `languages/pascal.ts`, plus the DFM/FMX form extractor
//! (`dfm-extractor.ts`) for `.dfm`/`.fmx` paths.
//!
//! Pascal's AST routes through the `visitPascalNode` hook: units/programs/
//! libraries become `module` nodes; `declType` wrappers become
//! class/interface/enum/type-alias nodes (with `extends`/`implements` from
//! typeref children); `declUses` clauses become import nodes + refs;
//! `declConst`/`declVar`/`declProp`/`declField` become constant/variable/
//! property/field nodes; `defProc` implementation bodies re-attach their
//! calls to the interface-declared method (via an indexed lookup) or mint a
//! free-routine function node; `exprCall` and statement-level `exprDot`
//! (paren-less calls) become call refs. Visibility rides the enclosing
//! `declSection` (public/published/private/protected). Pascal is in
//! VALUE_REF_LANGS (constant targets only) and carries the PASCAL_SPEC
//! function-as-value capture (`OnClick := Handler`).
//!
//! The frozen oracle is the vendored `src/extraction/wasm/
//! tree-sitter-pascal.wasm` (ABI 15), built from Isopod/tree-sitter-pascal
//! v0.10.2 = 042119ec (MIT); the vendored C under
//! `crates/codegraph-extract/grammars/pascal` is that commit's grammar
//! regenerated with tree-sitter-cli 0.25.10 (checked-in parser.c is ABI 14).
//! Behavior parity is asserted per fixture against the oracle goldens.
//! Positions are emitted in UTF-16 code units.

use crate::buffers::{
    Arena, BoolFlags, EdgeRow, EmitOut, FLAG_IS_EXPORTED, FLAG_IS_STATIC, FUNCTION_REF_CODE, NONE,
    NONE_STR, NodeRow, RefRow, StrRef, Tables, build_meta, edge_kind_index, node_kind_index,
};
use crate::ids;
use crate::textutil as util;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use tree_sitter::{Node, Parser};

/// `/^[A-Za-z_]\w*$/` with JS's ASCII `\w` — the return-type gate and the
/// capitalized-chain gate.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn ascii_ident_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z_][0-9A-Za-z_]*$").unwrap())
}

/// `/^[TI][A-Z]/` — the Delphi type-naming convention gate for chained
/// static-factory calls.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn ti_prefix_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[TI][A-Z]").unwrap())
}

/// `/^\w+$/` with JS's ASCII `\w` — the unary outer-method gate.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn word_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\w+$").unwrap())
}

/// `/^[A-Z_]|_/` is not the gate; the value-ref target test is
/// `/[A-Z_]/` (any uppercase or underscore) plus length >= 3.
fn is_distinctive_value_name(name: &str) -> bool {
    name.len() >= 3 && name.chars().any(|c| c.is_ascii_uppercase() || c == '_')
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
#[derive(Default)]
struct NodeExtra {
    /// Source signature text preserved on the emitted symbol.
    signature: Option<String>,
    /// Whether the declaration is exported, encoded in the node flag bit-pair.
    is_exported: Option<bool>,
    /// Visibility wire value ('public' | 'private' | 'protected' | 'internal').
    visibility: Option<&'static str>,
    /// Whether the symbol is static, encoded in the node flag bit-pair.
    is_static: Option<bool>,
    /// Declared return type retained for the node payload and type references.
    return_type: Option<String>,
}

/// One buffered function-as-value candidate (function-ref.ts).
struct FnRefCand {
    /// Emitted row of the enclosing scope.
    from: u32,
    /// Candidate function name.
    name: String,
    /// One-based line.
    line: u32,
    /// Byte column of the candidate node.
    column_byte: usize,
    /// Row of the candidate node.
    row: usize,
}

/// One value-reference reader scope (value-reference-edges.md).
struct ValueRefScope<'t> {
    /// Emitted row of the reader symbol.
    id: u32,
    /// The AST node whose subtree holds the reads (the routine signature).
    node: Node<'t>,
    /// The reader symbol's name.
    name: String,
}

/// Owns per-file Pascal traversal state, lexical scopes, and ordered fact buffers.
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
    /// Emitted node ids by row, used for value-ref scope/id checks.
    node_ids: Vec<String>,
    /// Lowercase routine-name → emitted row, built lazily for defProc lookup.
    method_index: Option<HashMap<String, u32>>,
    /// File-scope constant targets (name → row) for value-reference edges.
    fs_values: HashMap<String, u32>,
    /// File-scope target-name counts (conditional-def detection).
    fs_value_counts: HashMap<String, u32>,
    /// Reader scopes whose subtrees are scanned for value reads.
    value_scopes: Vec<ValueRefScope<'t>>,
    /// Buffered function-as-value candidates.
    fn_ref_cands: Vec<FnRefCand>,
    /// Same-file function/method names (the fn-ref gate).
    defined_fn_names: HashSet<String>,
    /// Imported binding names (the fn-ref gate, simple + qualified-last-segment).
    imported_names: HashSet<String>,
    /// Whether value references are enabled (CODEGRAPH_VALUE_REFS=0 disables).
    value_refs_enabled: bool,
}

/// Parses one Pascal source file and returns its flat facts, diagnostics, and measured kernel duration.
///
/// # Errors
///
/// Returns an error when the native grammar is unavailable, tree-sitter rejects grammar installation, parsing returns no tree, or the tree contains errors and must defer to canonical wasm recovery.
///
/// # Panics
///
/// Panics if a hard-coded node-kind spelling is absent from the frozen `NODE_KINDS` registry.
pub fn extract(file_path: &str, source: &str) -> Result<EmitOut, String> {
    // extractFromSource: `.dfm`/`.fmx` files route to the DfmExtractor, not
    // the tree-sitter walker.
    let lower = file_path.to_lowercase();
    if lower.ends_with(".dfm") || lower.ends_with(".fmx") {
        return crate::pascal::dfm::extract_dfm(file_path, source);
    }

    let grammar = crate::langs::grammar_for("pascal").ok_or("no pascal grammar")?;
    let t0 = std::time::Instant::now();
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .map_err(|e| format!("set_language(pascal) failed: {e}"))?;
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
        method_index: None,
        fs_values: HashMap::new(),
        fs_value_counts: HashMap::new(),
        value_scopes: Vec::new(),
        fn_ref_cands: Vec::new(),
        defined_fn_names: HashSet::new(),
        imported_names: HashSet::new(),
        value_refs_enabled: std::env::var("CODEGRAPH_VALUE_REFS").as_deref() != Ok("0"),
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

    w.flush_fn_ref_candidates();
    w.flush_value_refs(tree.root_node());

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
    /// The emitted id of the top scope, or empty.
    fn top_id(&self) -> String {
        self.stack
            .last()
            .map(|s| {
                self.node_ids
                    .get(s.row as usize)
                    .cloned()
                    .unwrap_or_default()
            })
            .unwrap_or_default()
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
        if let Some(v) = extra.is_static {
            flags.set(FLAG_IS_STATIC, v);
        }
        let name_ref = self.arena.put(name);
        let qn_ref = self.arena.put(&qualified);
        let id_ref = self.arena.put(&id);
        let sig_ref = opt_str(&mut self.arena, extra.signature.as_deref());
        let ret_ref = opt_str(&mut self.arena, extra.return_type.as_deref());
        let visibility = visibility_byte(extra.visibility);
        let row = self.tables.push_node(&NodeRow {
            kind: node_kind_index(kind).unwrap(),
            visibility,
            flags,
            start_line,
            end_line: node.end_position().row as u32 + 1,
            start_column: self.col_of(node),
            end_column: self.end_col_of(node),
            name: name_ref,
            qualified_name: qn_ref,
            id: id_ref,
            docstring: NONE_STR,
            signature: sig_ref,
            decorators: NONE_STR,
            type_parameters: NONE_STR,
            return_type: ret_ref,
            extra_json: NONE_STR,
        });
        self.node_ids.push(id.clone());

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

        // captureValueRefScope (tree-sitter.ts): Pascal targets `constant`
        // only, at file/class/module/struct/enum scope, with distinctive
        // names; functions/methods/constants/variables become reader scopes.
        self.capture_value_ref_scope(kind, name, row, node);
        if matches!(kind, "function" | "method" | "constant" | "variable") {
            self.defined_fn_names.insert(name.to_string());
        }
        Some(row)
    }

    /// captureValueRefScope (tree-sitter.ts): Pascal targets `constant` only;
    /// reader scopes are functions/methods/constants/variables.
    fn capture_value_ref_scope(&mut self, kind: &str, name: &str, row: u32, node: Node<'t>) {
        let target_ok = kind == "constant";
        if target_ok && is_distinctive_value_name(name) {
            let parent_id = self.top_id();
            if !parent_id.is_empty()
                && (parent_id.starts_with("file:")
                    || parent_id.starts_with("class:")
                    || parent_id.starts_with("module:")
                    || parent_id.starts_with("struct:")
                    || parent_id.starts_with("enum:"))
            {
                self.fs_values.insert(name.to_string(), row);
                *self.fs_value_counts.entry(name.to_string()).or_insert(0) += 1;
            }
        }
        if matches!(kind, "function" | "method" | "constant" | "variable") {
            self.value_scopes.push(ValueRefScope {
                id: row,
                node,
                name: name.to_string(),
            });
        }
    }

    // --- visitNode / visitPascalNode ----------------------------------------------

    /// Traverses node syntax and dispatches each recognized construct exactly once.
    fn visit_node(&mut self, node: Node<'t>) {
        if self.visit_pascal_node(node) {
            return;
        }
        let kind = node.kind();
        if kind == "declProc" {
            // functionTypes/methodTypes: inside a class-like node it is a
            // method, otherwise a function.
            if self.inside_class_like() {
                self.extract_method(node);
            } else {
                self.extract_function(node);
            }
            return;
        }
        if kind == "declClass" {
            self.extract_class(node);
            return;
        }
        if kind == "declIntf" {
            self.extract_interface(node);
            return;
        }
        if kind == "declEnum" {
            self.extract_enum(node);
            return;
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_node(child);
        }
    }

    /// Whether the top of the scope stack is a class-like symbol.
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

    /// visitPascalNode (tree-sitter.ts): the Pascal-specific AST handling.
    fn visit_pascal_node(&mut self, node: Node<'t>) -> bool {
        match node.kind() {
            "unit" | "program" | "library" => {
                let module_name_node = {
                    let mut cursor = node.walk();
                    node.named_children(&mut cursor)
                        .find(|c| c.kind() == "moduleName")
                };
                let name = module_name_node
                    .map(|n| self.text(n).to_string())
                    .unwrap_or_default();
                let module_name = if name.is_empty() {
                    let base = self
                        .file_path
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(self.file_path);
                    let dot = base.rfind('.');
                    match dot {
                        Some(dot) => base[..dot].to_string(),
                        None => base.to_string(),
                    }
                } else {
                    name
                };
                self.create_node("module", &module_name, node, NodeExtra::default());
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    self.visit_node(child);
                }
                true
            }
            "declType" => {
                self.extract_pascal_decl_type(node);
                true
            }
            "declUses" => {
                self.extract_pascal_uses(node);
                true
            }
            "declConsts" => {
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    if child.kind() == "declConst" {
                        self.extract_pascal_const(child);
                    }
                }
                true
            }
            "declConst" => {
                self.extract_pascal_const(node);
                true
            }
            "declTypes" => {
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    self.visit_node(child);
                }
                true
            }
            "declVars" => {
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    if child.kind() == "declVar"
                        && let Some(name_node) = child.child_by_field_name("name")
                    {
                        let name = self.text(name_node).to_string();
                        self.create_node("variable", &name, child, NodeExtra::default());
                    }
                }
                true
            }
            "defProc" => {
                self.extract_pascal_def_proc(node);
                true
            }
            "declProp" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.text(name_node).to_string();
                    let visibility = self.node_visibility(node);
                    self.create_node(
                        "property",
                        &name,
                        node,
                        NodeExtra {
                            visibility,
                            ..NodeExtra::default()
                        },
                    );
                }
                true
            }
            "declField" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.text(name_node).to_string();
                    let visibility = self.node_visibility(node);
                    self.create_node(
                        "field",
                        &name,
                        node,
                        NodeExtra {
                            visibility,
                            ..NodeExtra::default()
                        },
                    );
                }
                true
            }
            "declSection" => {
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    self.visit_node(child);
                }
                true
            }
            "exprCall" => {
                self.extract_pascal_call(node);
                true
            }
            "interface" | "implementation" => {
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    self.visit_node(child);
                }
                true
            }
            "block" => {
                self.visit_pascal_block(node);
                true
            }
            _ => false,
        }
    }

    /// extractPascalDeclType (tree-sitter.ts): declType → class/interface/
    /// enum/type_alias by the inner declaration.
    fn extract_pascal_decl_type(&mut self, node: Node<'t>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = self.text(name_node).to_string();
        let mut cursor = node.walk();
        let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
        let decl_class = children.iter().find(|c| c.kind() == "declClass").copied();
        let decl_intf = children.iter().find(|c| c.kind() == "declIntf").copied();
        let type_child = children.iter().find(|c| c.kind() == "type").copied();

        if let Some(decl_class) = decl_class {
            let row = self.create_node("class", &name, node, NodeExtra::default());
            if let Some(row) = row {
                self.extract_pascal_inheritance(decl_class, row);
                self.stack.push(Scope {
                    row,
                    kind: "class",
                    name: name.clone(),
                });
                let mut class_cursor = decl_class.walk();
                for child in decl_class.named_children(&mut class_cursor) {
                    self.visit_node(child);
                }
                self.stack.pop();
            }
        } else if let Some(decl_intf) = decl_intf {
            let row = self.create_node("interface", &name, node, NodeExtra::default());
            if let Some(row) = row {
                self.stack.push(Scope {
                    row,
                    kind: "interface",
                    name: name.clone(),
                });
                let mut intf_cursor = decl_intf.walk();
                for child in decl_intf.named_children(&mut intf_cursor) {
                    self.visit_node(child);
                }
                self.stack.pop();
            }
        } else if let Some(type_child) = type_child {
            let decl_enum = {
                let mut type_cursor = type_child.walk();
                type_child
                    .named_children(&mut type_cursor)
                    .find(|c| c.kind() == "declEnum")
            };
            if let Some(decl_enum) = decl_enum {
                let row = self.create_node("enum", &name, node, NodeExtra::default());
                if let Some(row) = row {
                    self.stack.push(Scope {
                        row,
                        kind: "enum",
                        name: name.clone(),
                    });
                    let mut enum_cursor = decl_enum.walk();
                    for child in decl_enum.named_children(&mut enum_cursor) {
                        if child.kind() == "declEnumValue"
                            && let Some(member_name) = child.child_by_field_name("name")
                        {
                            let member_name = self.text(member_name).to_string();
                            self.create_node(
                                "enum_member",
                                &member_name,
                                child,
                                NodeExtra::default(),
                            );
                        }
                    }
                    self.stack.pop();
                }
            } else {
                self.create_node("type_alias", &name, node, NodeExtra::default());
            }
        } else {
            self.create_node("type_alias", &name, node, NodeExtra::default());
        }
    }

    /// # Panics
    /// extractPascalUses (tree-sitter.ts): each moduleName in the uses clause
    /// becomes an import node + imports ref.
    fn extract_pascal_uses(&mut self, node: Node<'t>) {
        let import_text = self.text(node).trim().to_string();
        let mut cursor = node.walk();
        let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
        for child in children {
            if child.kind() == "moduleName" {
                let unit_name = self.text(child).to_string();
                self.create_node(
                    "import",
                    &unit_name,
                    child,
                    NodeExtra {
                        signature: Some(import_text.clone()),
                        ..NodeExtra::default()
                    },
                );
                let parent_row = self.top_row();
                self.push_ref_at(
                    parent_row,
                    &unit_name,
                    edge_kind_index("imports").unwrap(),
                    child,
                );
            }
        }
    }

    /// extractPascalConst (tree-sitter.ts): a constant declaration with an
    /// optional default-value signature.
    fn extract_pascal_const(&mut self, node: Node<'t>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = self.text(name_node).to_string();
        let default_value = {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|c| c.kind() == "defaultValue")
        };
        let sig = default_value.map(|d| self.text(d).to_string());
        self.create_node(
            "constant",
            &name,
            node,
            NodeExtra {
                signature: sig,
                ..NodeExtra::default()
            },
        );
    }

    /// # Panics
    /// extractPascalInheritance (tree-sitter.ts): typeref children of a
    /// declClass — first is `extends`, the rest `implements`.
    fn extract_pascal_inheritance(&mut self, decl_class: Node<'t>, class_row: u32) {
        let mut cursor = decl_class.walk();
        let typerefs: Vec<Node<'t>> = decl_class
            .named_children(&mut cursor)
            .filter(|c| c.kind() == "typeref")
            .collect();
        for (i, reference) in typerefs.iter().enumerate() {
            let name = self.text(*reference).to_string();
            let kind_code = if i == 0 {
                edge_kind_index("extends").unwrap()
            } else {
                edge_kind_index("implements").unwrap()
            };
            self.push_ref_at(class_row, &name, kind_code, *reference);
        }
    }

    /// extractPascalDefProc (tree-sitter.ts): implementation bodies re-attach
    /// their calls to the interface-declared routine via the indexed lookup;
    /// a free routine with no declaration mints a function node.
    fn extract_pascal_def_proc(&mut self, node: Node<'t>) {
        let decl_proc = {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|c| c.kind() == "declProc")
        };
        let Some(decl_proc) = decl_proc else { return };
        let Some(name_node) = decl_proc.child_by_field_name("name") else {
            return;
        };
        let full_name = self.text(name_node).trim().to_string();
        let short_name = match full_name.rfind('.') {
            Some(dot) => full_name[dot + 1..].to_string(),
            None => full_name.clone(),
        };
        let full_name_key = full_name.to_lowercase();
        let short_name_key = short_name.to_lowercase();

        if self.method_index.is_none() {
            let mut index = HashMap::new();
            for (row, _id) in self.node_ids.iter().enumerate() {
                let kind = node_kind_of_row(&self.tables.nodes, row);
                let name = row_name_of(&self.tables.nodes, &self.arena, row);
                if matches!(kind, "method" | "function") {
                    let name_key = name.to_lowercase();
                    index.entry(name_key).or_insert(row as u32);
                    if kind == "method" {
                        let qualified = row_qn_of(&self.tables.nodes, &self.arena, row);
                        let qualified_parts: Vec<&str> = qualified.split("::").collect();
                        if qualified_parts.len() >= 2 {
                            for i in 0..qualified_parts.len() - 1 {
                                let scoped = qualified_parts[i..].join(".").to_lowercase();
                                index.insert(scoped, row as u32);
                            }
                        }
                    }
                }
            }
            self.method_index = Some(index);
        }

        let mut parent_id = self
            .method_index
            .as_ref()
            .and_then(|index| index.get(&full_name_key).copied())
            .or_else(|| {
                self.method_index
                    .as_ref()
                    .and_then(|index| index.get(&short_name_key).copied())
            });

        if parent_id.is_none() && !full_name.contains('.') {
            let signature = self.node_signature(decl_proc);
            let visibility = self.node_visibility(decl_proc);
            let row = self.create_node(
                "function",
                &full_name,
                decl_proc,
                NodeExtra {
                    signature,
                    visibility,
                    ..NodeExtra::default()
                },
            );
            if let Some(row) = row {
                parent_id = Some(row);
                if let Some(index) = &mut self.method_index {
                    index.insert(full_name_key.clone(), row);
                    index.entry(short_name_key).or_insert(row);
                }
            }
        }

        if parent_id.is_none() {
            parent_id = Some(self.top_row());
        }
        let Some(parent_id) = parent_id else { return };

        let block = {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|c| c.kind() == "block")
        };
        if let Some(block) = block {
            self.stack.push(Scope {
                row: parent_id,
                kind: "function",
                name: short_name,
            });
            self.visit_pascal_block(block);
            self.stack.pop();
        }
    }

    /// # Panics
    /// extractPascalCall (tree-sitter.ts): `exprCall` — identifier or
    /// exprDot callee, chained static-factory re-encode, args walked.
    fn extract_pascal_call(&mut self, node: Node<'t>) {
        let from = self.top_row();
        let Some(first_child) = node.named_child(0) else {
            return;
        };
        let callee_name = if first_child.kind() == "exprDot" {
            let inner_call = {
                let mut cursor = first_child.walk();
                first_child
                    .named_children(&mut cursor)
                    .find(|c| c.kind() == "exprCall")
            };
            let outer_id = {
                let mut cursor = first_child.walk();
                first_child
                    .named_children(&mut cursor)
                    .filter(|c| c.kind() == "identifier")
                    .last()
            };
            let method = outer_id
                .map(|o| self.text(o).to_string())
                .unwrap_or_default();
            if let Some(inner_call) = inner_call
                && !method.is_empty()
                && word_re().is_match(&method)
            {
                let inner_first = inner_call.named_child(0);
                let mut inner_callee = String::new();
                if let Some(inner_first) = inner_first {
                    if inner_first.kind() == "exprDot" {
                        let parts: Vec<String> = {
                            let mut cursor = inner_first.walk();
                            inner_first
                                .named_children(&mut cursor)
                                .filter(|c| c.kind() == "identifier")
                                .map(|c| self.text(c).to_string())
                                .collect()
                        };
                        inner_callee = parts.join(".");
                    } else if inner_first.kind() == "identifier" {
                        inner_callee = self.text(inner_first).to_string();
                    }
                }
                if !inner_callee.is_empty() && ti_prefix_re().is_match(&inner_callee) {
                    format!("{inner_callee}().{method}")
                } else {
                    method
                }
            } else {
                let identifiers: Vec<String> = {
                    let mut cursor = first_child.walk();
                    first_child
                        .named_children(&mut cursor)
                        .filter(|c| c.kind() == "identifier")
                        .map(|c| self.text(c).to_string())
                        .collect()
                };
                identifiers.join(".")
            }
        } else if first_child.kind() == "identifier" {
            self.text(first_child).to_string()
        } else {
            String::new()
        };

        if !callee_name.is_empty() {
            self.push_ref_at(from, &callee_name, edge_kind_index("calls").unwrap(), node);
        }

        // Also visit arguments for nested calls.
        let args = {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|c| c.kind() == "exprArgs")
        };
        if let Some(args) = args {
            self.visit_pascal_block(args);
        }
    }

    /// # Panics
    /// extractPascalParenlessCall (tree-sitter.ts): a STATEMENT-level bare
    /// exprDot is a paren-less call.
    fn extract_pascal_parenless_call(&mut self, node: Node<'t>) {
        let from = self.top_row();
        let receiver = node.named_child(0);
        let outer_id = {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .filter(|c| c.kind() == "identifier")
                .last()
        };
        let method = outer_id
            .map(|o| self.text(o).to_string())
            .unwrap_or_default();
        if method.is_empty() {
            return;
        }

        let callee_name;
        if (receiver.map(|r| r.kind()) == Some("exprDot")
            || receiver.map(|r| r.kind()) == Some("exprCall"))
            && word_re().is_match(&method)
        {
            let inner_callee_node = if receiver.unwrap().kind() == "exprCall" {
                receiver.unwrap().named_child(0)
            } else {
                receiver
            };
            let inner_callee = match inner_callee_node {
                Some(inner) if inner.kind() == "identifier" => self.text(inner).to_string(),
                Some(inner) => {
                    let mut cursor = inner.walk();
                    inner
                        .named_children(&mut cursor)
                        .filter(|c| c.kind() == "identifier")
                        .map(|c| self.text(c).to_string())
                        .collect::<Vec<_>>()
                        .join(".")
                }
                None => String::new(),
            };
            let receiver_node = receiver.unwrap();
            if !inner_callee.is_empty() && ti_prefix_re().is_match(&inner_callee) {
                callee_name = format!("{inner_callee}().{method}");
                if receiver_node.kind() == "exprCall" {
                    self.extract_pascal_call(receiver_node);
                } else {
                    self.extract_pascal_parenless_call(receiver_node);
                }
            } else {
                callee_name = method;
            }
        } else {
            let identifiers: Vec<String> = {
                let mut cursor = node.walk();
                node.named_children(&mut cursor)
                    .filter(|c| c.kind() == "identifier")
                    .map(|c| self.text(c).to_string())
                    .collect()
            };
            callee_name = identifiers.join(".");
        }

        if !callee_name.is_empty() {
            self.push_ref_at(from, &callee_name, edge_kind_index("calls").unwrap(), node);
        }
    }

    /// visitPascalBlock (tree-sitter.ts): walk a block/statement tree for
    /// exprCall/exprDot calls, firing fn-ref capture on each child.
    fn visit_pascal_block(&mut self, node: Node<'t>) {
        let mut cursor = node.walk();
        let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
        for child in children {
            self.maybe_capture_fn_refs(child);
            if child.kind() == "exprCall" {
                self.extract_pascal_call(child);
                let args = {
                    let mut c = child.walk();
                    child
                        .named_children(&mut c)
                        .find(|x| x.kind() == "exprArgs")
                };
                if let Some(args) = args {
                    self.maybe_capture_fn_refs(args);
                }
            } else if child.kind() == "exprDot" {
                if node.kind() == "statement" {
                    self.extract_pascal_parenless_call(child);
                } else {
                    let mut c = child.walk();
                    for grandchild in child.named_children(&mut c) {
                        if grandchild.kind() == "exprCall" {
                            self.extract_pascal_call(grandchild);
                        }
                    }
                }
            } else {
                self.visit_pascal_block(child);
            }
        }
    }

    // --- extractor-config helpers --------------------------------------------------

    /// getSignature (pascal.ts): `(args)` + `: returntype` when present.
    fn node_signature(&self, node: Node<'t>) -> Option<String> {
        let args = node.child_by_field_name("args");
        let return_type = {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|c| c.kind() == "typeref")
        };
        if args.is_none() && return_type.is_none() {
            return None;
        }
        let mut sig = String::new();
        if let Some(args) = args {
            sig.push_str(self.text(args));
        }
        if let Some(return_type) = return_type {
            sig.push_str(": ");
            sig.push_str(self.text(return_type));
        }
        if sig.is_empty() { None } else { Some(sig) }
    }

    /// getVisibility (pascal.ts): the enclosing declSection's keyword.
    fn node_visibility(&self, node: Node<'t>) -> Option<&'static str> {
        let mut current = node.parent();
        while let Some(parent) = current {
            if parent.kind() == "declSection" {
                let mut cursor = parent.walk();
                for child in parent.named_children(&mut cursor) {
                    match child.kind() {
                        "kPublic" | "kPublished" => return Some("public"),
                        "kPrivate" => return Some("private"),
                        "kProtected" => return Some("protected"),
                        _ => {}
                    }
                }
            }
            current = parent.parent();
        }
        None
    }

    /// getReturnType (pascal.ts): the bare class name of the typeref child.
    fn pascal_return_type(&self, node: Node<'t>) -> Option<String> {
        let typeref = {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|c| c.kind() == "typeref")
        };
        let typeref = typeref?;
        let id = {
            let mut cursor = typeref.walk();
            typeref
                .named_children(&mut cursor)
                .find(|c| c.kind() == "identifier")
        }
        .unwrap_or(typeref);
        let name = self.text(id).trim().to_string();
        if ascii_ident_re().is_match(&name) {
            Some(name)
        } else {
            None
        }
    }

    /// isStatic (pascal.ts): a `kClass` token child.
    fn node_is_static(&self, node: Node<'t>) -> bool {
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .any(|c| c.kind() == "kClass")
    }

    /// extractFunction (tree-sitter.ts): a top-level declProc.
    fn extract_function(&mut self, node: Node<'t>) {
        let Some(name) = self.node_name(node) else {
            return;
        };
        if name == "<anonymous>" {
            return;
        }
        let signature = self.node_signature(node);
        let visibility = self.node_visibility(node);
        let is_static = Some(self.node_is_static(node));
        let return_type = self.pascal_return_type(node);
        let row = self.create_node(
            "function",
            &name,
            node,
            NodeExtra {
                signature,
                visibility,
                is_static,
                return_type,
                ..NodeExtra::default()
            },
        );
        let Some(row) = row else { return };
        self.stack.push(Scope {
            row,
            kind: "function",
            name,
        });
        // Pascal routine bodies are `block` children of the defProc, not the
        // declProc; a bare declProc (interface section) has no body.
        self.stack.pop();
    }

    /// extractMethod (tree-sitter.ts): a declProc inside a class-like node.
    fn extract_method(&mut self, node: Node<'t>) {
        let Some(name) = self.node_name(node) else {
            return;
        };
        if name == "<anonymous>" {
            return;
        }
        let signature = self.node_signature(node);
        let visibility = self.node_visibility(node);
        let is_static = Some(self.node_is_static(node));
        let return_type = self.pascal_return_type(node);
        let row = self.create_node(
            "method",
            &name,
            node,
            NodeExtra {
                signature,
                visibility,
                is_static,
                return_type,
                ..NodeExtra::default()
            },
        );
        let Some(row) = row else { return };
        self.stack.push(Scope {
            row,
            kind: "method",
            name,
        });
        self.stack.pop();
    }

    /// extractClass (tree-sitter.ts): a bare declClass (always wrapped in
    /// declType in practice, but the generic dispatch is faithful).
    fn extract_class(&mut self, node: Node<'t>) {
        let Some(name) = self.node_name(node) else {
            return;
        };
        let row = self.create_node("class", &name, node, NodeExtra::default());
        let Some(row) = row else { return };
        self.extract_pascal_inheritance(node, row);
        self.stack.push(Scope {
            row,
            kind: "class",
            name,
        });
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_node(child);
        }
        self.stack.pop();
    }

    /// extractInterface (tree-sitter.ts): a bare declIntf.
    fn extract_interface(&mut self, node: Node<'t>) {
        let Some(name) = self.node_name(node) else {
            return;
        };
        let row = self.create_node("interface", &name, node, NodeExtra::default());
        let Some(row) = row else { return };
        self.stack.push(Scope {
            row,
            kind: "interface",
            name,
        });
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_node(child);
        }
        self.stack.pop();
    }

    /// extractEnum (tree-sitter.ts): a bare declEnum (always wrapped in
    /// declType in practice).
    fn extract_enum(&mut self, node: Node<'t>) {
        let Some(name) = self.node_name(node) else {
            return;
        };
        let row = self.create_node("enum", &name, node, NodeExtra::default());
        let Some(row) = row else { return };
        self.stack.push(Scope {
            row,
            kind: "enum",
            name,
        });
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "declEnumValue" {
                if let Some(member_name) = child.child_by_field_name("name") {
                    let member_name = self.text(member_name).to_string();
                    self.create_node("enum_member", &member_name, child, NodeExtra::default());
                }
            } else {
                self.visit_node(child);
            }
        }
        self.stack.pop();
    }

    /// extractName: the `name` field, or `<anonymous>` when absent.
    fn node_name(&self, node: Node<'t>) -> Option<String> {
        node.child_by_field_name("name")
            .map(|n| self.text(n).to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| Some("<anonymous>".to_string()))
    }

    // --- function-as-value capture (PASCAL_SPEC) -----------------------------------

    /// maybeCaptureFnRefs (tree-sitter.ts + function-ref.ts PASCAL_SPEC):
    /// `exprArgs` and `assignment` (OnClick := Handler) containers.
    fn maybe_capture_fn_refs(&mut self, node: Node<'t>) {
        if self.stack.is_empty() {
            return;
        }
        let from = self.top_row();
        let mut values: Vec<Node<'t>> = Vec::new();
        match node.kind() {
            "exprArgs" => {
                let mut cursor = node.walk();
                values.extend(node.named_children(&mut cursor));
            }
            "assignment" => {
                if let Some(rhs) = node.child_by_field_name("rhs") {
                    values.push(rhs);
                }
            }
            _ => return,
        }
        for v in values {
            self.normalize_fn_ref_value(v, from, 0);
        }
    }

    /// normalizeValue with PASCAL_SPEC: identifiers and `@Handler` unwrap.
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
                let pos = v.start_position();
                self.fn_ref_cands.push(FnRefCand {
                    from,
                    name,
                    line: pos.row as u32 + 1,
                    column_byte: v.start_byte(),
                    row: pos.row,
                });
            }
            "exprUnary" => {
                if let Some(operand) = v.child_by_field_name("operand") {
                    self.normalize_fn_ref_value(operand, from, depth + 1);
                }
            }
            _ => {}
        }
    }

    /// scanFnRefSubtree (tree-sitter.ts): capture-only scan of subtrees the
    /// main walker won't traverse.
    #[allow(dead_code)]
    fn scan_fn_ref_subtree(&mut self, node: Node<'t>, depth: u32) {
        if depth > 12 {
            return;
        }
        if depth > 0 && node.kind() == "declProc" {
            return;
        }
        self.maybe_capture_fn_refs(node);
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.scan_fn_ref_subtree(child, depth + 1);
        }
    }

    /// flushFnRefCandidates (tree-sitter.ts): gate + dedup + emit.
    fn flush_fn_ref_candidates(&mut self) {
        let cands = std::mem::take(&mut self.fn_ref_cands);
        if cands.is_empty() || util::is_generated_file(self.file_path) {
            return;
        }
        // Gate: name ∈ same-file functions/methods ∪ imported names
        // (imported names include the last dotted segment of dotted imports).
        let mut seen: HashSet<(u32, String)> = HashSet::new();
        for c in cands {
            if !self.defined_fn_names.contains(&c.name) && !self.imported_names.contains(&c.name) {
                continue;
            }
            if !seen.insert((c.from, c.name.clone())) {
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

    // --- value-reference edges -----------------------------------------------------

    /// # Panics
    /// flushValueRefs (tree-sitter.ts): same-file `references` edges from
    /// reader scopes to file-scope constants they read.
    fn flush_value_refs(&mut self, root: Node<'t>) {
        let scopes = std::mem::take(&mut self.value_scopes);
        let mut targets = std::mem::take(&mut self.fs_values);
        let counts = std::mem::take(&mut self.fs_value_counts);
        if !self.value_refs_enabled || targets.is_empty() || scopes.is_empty() {
            return;
        }
        if util::is_generated_file(self.file_path) {
            return;
        }

        // Prune shadowed targets: count declarators per name across the tree
        // (declConst/declVar name fields); a name with more declarators than
        // file-scope nodes has a local shadow.
        let mut decl_counts: HashMap<String, u32> = HashMap::new();
        let mut stack: Vec<Node<'t>> = vec![root];
        let mut visited = 0u32;
        while let Some(n) = stack.pop() {
            visited += 1;
            if visited > 20_000 {
                break;
            }
            if matches!(n.kind(), "declConst" | "declVar")
                && let Some(name_node) = n.child_by_field_name("name")
            {
                let name = self.text(name_node).to_string();
                if targets.contains_key(&name) {
                    *decl_counts.entry(name).or_insert(0) += 1;
                }
            }
            let mut cursor = n.walk();
            for child in n.named_children(&mut cursor) {
                stack.push(child);
            }
        }
        for (name, count) in &decl_counts {
            if *count > *counts.get(name).unwrap_or(&1) {
                targets.remove(name);
            }
        }
        if targets.is_empty() {
            return;
        }

        for scope in scopes {
            let mut seen: HashSet<u32> = HashSet::new();
            let mut walk_stack: Vec<Node<'t>> = vec![scope.node];
            // Pascal attaches the routine BODY as a next named sibling of the
            // signature node that is the reader scope.
            if let Some(sib) = scope.node.next_named_sibling()
                && matches!(sib.kind(), "function_body" | "block")
            {
                walk_stack.push(sib);
            }
            let mut visited = 0u32;
            while let Some(n) = walk_stack.pop() {
                visited += 1;
                if visited > 20_000 {
                    break;
                }
                if matches!(
                    n.kind(),
                    "identifier" | "constant" | "name" | "simple_identifier"
                ) {
                    let ref_name = self.text(n).to_string();
                    if let Some(target_row) = targets.get(&ref_name).copied()
                        && target_row != scope.id
                        && ref_name != scope.name
                        && seen.insert(target_row)
                    {
                        let meta = serde_json::json!({ "valueRef": true });
                        let meta_ref = self.arena.put(&meta.to_string());
                        self.tables.push_edge(&EdgeRow {
                            source_idx: scope.id,
                            target_idx: target_row,
                            kind: edge_kind_index("references").unwrap(),
                            provenance: 0,
                            line: NONE,
                            column: NONE,
                            metadata_json: meta_ref,
                            source_id_str: NONE_STR,
                            target_id_str: NONE_STR,
                        });
                    }
                }
                let mut cursor = n.walk();
                for child in n.named_children(&mut cursor) {
                    walk_stack.push(child);
                }
            }
        }
    }
}

/// Reads a node row's kind index (used by the defProc method index).
fn node_kind_of_row(nodes: &[u8], row: usize) -> &'static str {
    let start = row * crate::buffers::NODE_ROW_SIZE;
    let kind_index = nodes[start + crate::buffers::node::KIND] as usize;
    crate::buffers::NODE_KINDS
        .get(kind_index)
        .copied()
        .unwrap_or("")
}

/// # Panics
/// Reads a node row's name string from the arena.
fn row_name_of(nodes: &[u8], arena: &Arena, row: usize) -> String {
    let start = row * crate::buffers::NODE_ROW_SIZE;
    let off = u32::from_le_bytes(
        nodes[start + crate::buffers::node::NAME..start + crate::buffers::node::NAME + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let len = u32::from_le_bytes(
        nodes[start + crate::buffers::node::NAME + 4..start + crate::buffers::node::NAME + 8]
            .try_into()
            .unwrap(),
    ) as usize;
    if off == crate::buffers::NONE as usize {
        return String::new();
    }
    arena.get((off as u32, len as u32)).to_string()
}

/// # Panics
/// Reads a node row's qualified-name string from the arena.
fn row_qn_of(nodes: &[u8], arena: &Arena, row: usize) -> String {
    let start = row * crate::buffers::NODE_ROW_SIZE;
    let slot = crate::buffers::node::QUALIFIED_NAME;
    let off =
        u32::from_le_bytes(nodes[start + slot..start + slot + 4].try_into().unwrap()) as usize;
    let len = u32::from_le_bytes(
        nodes[start + slot + 4..start + slot + 8]
            .try_into()
            .unwrap(),
    ) as usize;
    if off == crate::buffers::NONE as usize {
        return String::new();
    }
    arena.get((off as u32, len as u32)).to_string()
}

/// Maps a visibility spelling to the closed wire byte.
fn visibility_byte(visibility: Option<&'static str>) -> u8 {
    match visibility {
        Some("public") => 1,
        Some("private") => 2,
        Some("protected") => 3,
        Some("internal") => 4,
        _ => 0,
    }
}

/// Converts the arena absent sentinel into `None` and preserves present strings without normalization.
fn opt_str(arena: &mut Arena, s: Option<&str>) -> StrRef {
    match s {
        Some(s) => arena.put(s),
        None => NONE_STR,
    }
}

/// The DFM/FMX form-file extractor (dfm-extractor.ts).
pub mod dfm {
    use super::*;

    /// # Panics
    /// Extracts a Delphi DFM/FMX form file into components, containment
    /// edges, and event-handler references (regex-based; no tree-sitter
    /// grammar exists for the format).
    ///
    /// # Errors
    ///
    /// Returns an error only when the walker cannot emit buffers (never for
    /// malformed DFM — parse failures are recorded as diagnostics).
    pub fn extract_dfm(file_path: &str, source: &str) -> Result<EmitOut, String> {
        let t0 = std::time::Instant::now();
        let mut arena = Arena::default();
        let mut tables = Tables::default();
        let mut errors_json: Option<String> = None;

        let base_name = file_path.rsplit(['/', '\\']).next().unwrap_or(file_path);
        let lines: Vec<&str> = source.split('\n').collect();
        let end_line = lines.len() as u32;
        let last_len = lines.last().map(|l| l.len() as u32).unwrap_or(0);

        // The DFM file node uses generateNodeId(filePath, 'file', filePath, 1),
        // NOT the `file:` special case; no bool flags (the TS DfmExtractor
        // sets no isExported on its nodes).
        let file_id = ids::node_id(file_path, "file", file_path, 1);
        let flags = BoolFlags::default();
        let id_ref = arena.put(&file_id);
        let name_ref = arena.put(base_name);
        let qn_ref = arena.put(file_path);
        let file_row = tables.push_node(&NodeRow {
            kind: node_kind_index("file").unwrap(),
            visibility: 0,
            flags,
            start_line: 1,
            end_line,
            start_column: 0,
            end_column: last_len,
            name: name_ref,
            qualified_name: qn_ref,
            id: id_ref,
            docstring: NONE_STR,
            signature: NONE_STR,
            decorators: NONE_STR,
            type_parameters: NONE_STR,
            return_type: NONE_STR,
            extra_json: NONE_STR,
        });

        // Parse object/end blocks.
        let mut stack: Vec<u32> = vec![file_row];
        let object_pattern =
            Regex::new(r"^\s*(object|inherited|inline)\s+(\w+)\s*:\s*(\w+)").unwrap();
        let event_pattern = Regex::new(r"^\s*(On\w+)\s*=\s*(\w+)\s*$").unwrap();
        let end_pattern = Regex::new(r"^\s*end\s*$").unwrap();
        let multi_line_start = Regex::new(r"=\s*\(\s*$").unwrap();
        let multi_line_item_start = Regex::new(r"=\s*<\s*$").unwrap();
        let mut in_multi_line = false;
        let mut multi_line_end_char = ')';

        let parse_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for (i, line) in lines.iter().enumerate() {
                let line_num = i as u32 + 1;
                if in_multi_line {
                    if line.trim_end().ends_with(multi_line_end_char) {
                        in_multi_line = false;
                    }
                    continue;
                }
                if multi_line_start.is_match(line) {
                    in_multi_line = true;
                    multi_line_end_char = ')';
                    continue;
                }
                if multi_line_item_start.is_match(line) {
                    in_multi_line = true;
                    multi_line_end_char = '>';
                    continue;
                }

                if let Some(captures) = object_pattern.captures(line) {
                    let name = captures.get(2).unwrap().as_str().to_string();
                    let type_name = captures.get(3).unwrap().as_str().to_string();
                    let component_id = ids::node_id(file_path, "component", &name, line_num);
                    let qualified = format!("{file_path}#{name}");
                    let flags = BoolFlags::default();
                    let id_ref = arena.put(&component_id);
                    let name_ref = arena.put(&name);
                    let qn_ref = arena.put(&qualified);
                    let sig_ref = arena.put(&type_name);
                    let row = tables.push_node(&NodeRow {
                        kind: node_kind_index("component").unwrap(),
                        visibility: 0,
                        flags,
                        start_line: line_num,
                        end_line: line_num,
                        start_column: 0,
                        end_column: line.len() as u32,
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
                    let parent = *stack.last().unwrap();
                    tables.push_edge(&EdgeRow {
                        source_idx: parent,
                        target_idx: row,
                        kind: edge_kind_index("contains").unwrap(),
                        provenance: 0,
                        line: NONE,
                        column: NONE,
                        metadata_json: NONE_STR,
                        source_id_str: NONE_STR,
                        target_id_str: NONE_STR,
                    });
                    stack.push(row);
                    continue;
                }

                if let Some(captures) = event_pattern.captures(line) {
                    let method_name = captures.get(2).unwrap().as_str().to_string();
                    let from = *stack.last().unwrap();
                    let name_ref = arena.put(&method_name);
                    tables.push_ref(&RefRow {
                        from_idx: from,
                        kind: edge_kind_index("references").unwrap(),
                        line: line_num,
                        column: 0,
                        reference_name: name_ref,
                        candidates: NONE_STR,
                        from_id_str: NONE_STR,
                    });
                    continue;
                }

                if end_pattern.is_match(line) && stack.len() > 1 {
                    stack.pop();
                }
            }
        }));

        if parse_result.is_err() {
            errors_json = Some(
                serde_json::json!([{
                    "message": "DFM extraction error: parse failure",
                    "severity": "error",
                    "code": "parse_error"
                }])
                .to_string(),
            );
        }

        let errors_ref = match errors_json {
            Some(json) => arena.put(&json),
            None => NONE_STR,
        };
        let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let meta = build_meta(&tables, arena.len(), errors_ref, duration_ms);
        Ok(EmitOut {
            meta,
            nodes: tables.nodes,
            edges: tables.edges,
            refs: tables.refs,
            arena: arena.into_vec(),
        })
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
        let emit = extract(path, source).expect("pascal extraction succeeds");
        crate::decode::decode_buffers(&emit, path, bohmian_code_review_core::Language::Pascal)
            .expect("pascal decode succeeds")
    }

    #[test]
    /// # Panics
    /// Units, classes, methods, uses-imports, and calls extract.
    fn unit_class_methods_and_calls_extract() {
        let src = "unit MyUnit;\n\ninterface\n\nuses\n  SysUtils;\n\ntype\n  TFoo = class(TObject)\n  private\n    FValue: Integer;\n  public\n    procedure DoIt;\n    class function Create: TFoo;\n  end;\n\nimplementation\n\nprocedure TFoo.DoIt;\nbegin\n  WriteLn('hi');\nend;\n\nend.\n";
        let result = extract_source("MyUnit.pas", src);
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Module && n.name == "MyUnit")
        );
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Class && n.name == "TFoo")
        );
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Method && n.name == "DoIt")
        );
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Import && n.name == "SysUtils")
        );
        // The implementation body's WriteLn call attributes to the method.
        let doit = result
            .nodes
            .iter()
            .find(|n| n.name == "DoIt")
            .expect("DoIt node");
        assert!(result.references.iter().any(|r| {
            r.from_node_id == doit.id
                && r.reference_name == "WriteLn"
                && r.reference_kind
                    == bohmian_code_review_core::ReferenceKind::Edge(bohmian_code_review_core::EdgeKind::Calls)
        }));
    }

    #[test]
    /// Free routines with implementation-only bodies get function nodes.
    fn free_routines_get_function_nodes() {
        let src = "unit U;\n\ninterface\n\nimplementation\n\nprocedure Helper;\nbegin\n  DoThing;\nend;\n\nend.\n";
        let result = extract_source("U.pas", src);
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Function && n.name == "Helper")
        );
    }

    #[test]
    /// # Panics
    /// DFM files extract components, containment, and event references.
    fn dfm_files_extract_components_and_events() {
        let src = "object Form1: TForm\n  object Panel1: TPanel\n    object Button1: TButton\n      OnClick = Button1Click\n    end\n  end\nend\n";
        let result = extract_source("Form1.dfm", src);
        let components: Vec<&str> = result
            .nodes
            .iter()
            .filter(|n| n.kind == bohmian_code_review_core::NodeKind::Component)
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(components, vec!["Form1", "Panel1", "Button1"]);
        assert!(result.references.iter().any(|r| {
            r.reference_name == "Button1Click"
                && r.reference_kind
                    == bohmian_code_review_core::ReferenceKind::Edge(bohmian_code_review_core::EdgeKind::References)
        }));
        // Containment: Panel1 contains Button1.
        let panel = result
            .nodes
            .iter()
            .find(|n| n.name == "Panel1")
            .expect("panel");
        let button = result
            .nodes
            .iter()
            .find(|n| n.name == "Button1")
            .expect("button");
        assert!(result.edges.iter().any(|e| {
            e.source == panel.id
                && e.target == button.id
                && e.kind == bohmian_code_review_core::EdgeKind::Contains
        }));
    }
}
