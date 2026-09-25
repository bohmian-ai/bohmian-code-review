//! Terraform extraction — a faithful Rust port of `languages/terraform.ts`
//! (the visitNode-driven Terraform path of src/extraction/tree-sitter.ts).
//!
//! The frozen oracle is the vendored `src/extraction/wasm/
//! tree-sitter-terraform.wasm`, byte-identical to
//! @tree-sitter-grammars/tree-sitter-hcl 1.2.0's package artifact and built
//! from tree-sitter-grammars/tree-sitter-hcl v1.2.0 = fad99186 (Apache-2.0);
//! its parser.c + scanner.c are vendored under
//! `crates/codegraph-extract/grammars/hcl` and compiled natively — table
//! identity by construction (ABI 15). Behavior parity is asserted per
//! fixture against the oracle goldens.
//!
//! The HCL grammar is intentionally generic: ALL Terraform top-level
//! constructs share the `block` node type, distinguished by the first
//! `identifier` child (the block type) and `string_lit`/`identifier` labels.
//! Ported quirks preserved on purpose: `locals` blocks emit one `constant`
//! per attribute; `terraform {}` blocks emit nothing; `moved`/`import`/
//! `removed`/`assert` blocks anchor their references to the file node with
//! `:`-scoped module refs suppressed; resources/data/modules/variables/
//! outputs/providers become class/module/variable/namespace nodes carrying
//! `var.`/`local.`/`module.`/`data.`/`output.`/`provider.` qualified names;
//! `provider = aws.east` selection references use the `provider.` prefix;
//! module meta-arguments (`source`/`version`/`count`/…) are skipped while
//! input attributes emit `module.M:var.<in>` refs and local `./`/`../`
//! sources emit `module.M:file` imports refs; `.tfvars` files turn top-level
//! attributes into `var.<name>` references from the file node. Built-in
//! heads (`each`, `count`, `self`, `path`, `terraform`) and keywords
//! (`null`/`true`/`false`) never become references. Positions are emitted in
//! UTF-16 code units.

use crate::buffers::{
    Arena, BoolFlags, EdgeRow, EmitOut, FLAG_IS_EXPORTED, NONE, NONE_STR, NodeRow, RefRow, StrRef,
    Tables, build_meta, edge_kind_index, node_kind_index,
};
use crate::ids;
use crate::textutil as util;
use std::collections::VecDeque;
use tree_sitter::{Node, Parser};

/// BUILTIN_HEADS (terraform.ts) — reference roots that never resolve to project nodes.
fn is_builtin_head(name: &str) -> bool {
    matches!(name, "each" | "count" | "self" | "path" | "terraform")
}

/// BUILTIN_KEYWORDS (terraform.ts) — bare strings never treated as references.
fn is_builtin_keyword(name: &str) -> bool {
    matches!(name, "null" | "true" | "false")
}

/// MODULE_META_ARGS (terraform.ts) — module-block attributes that configure the call itself.
fn is_module_meta_arg(name: &str) -> bool {
    matches!(
        name,
        "source" | "version" | "count" | "for_each" | "providers" | "depends_on"
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
    /// Scope-qualified symbol name used for stable lookup and display.
    qualified_name: Option<String>,
    /// Source signature text preserved on the emitted symbol.
    signature: Option<String>,
    /// Whether the declaration is exported, encoded in the node flag bit-pair.
    is_exported: Option<bool>,
}

/// A parsed Terraform block header: the block "type" and its label values.
struct BlockHeader {
    /// The first identifier child (the block type keyword).
    block_type: String,
    /// Label values following the type (string_lit / identifier children).
    labels: Vec<String>,
}

/// The declared symbol a Terraform block maps to.
struct BlockDecl {
    /// Closed node-kind wire spelling for the declared symbol.
    kind: &'static str,
    /// Simple name carried by the emitted node.
    name: String,
    /// Scope-qualified symbol name used for stable lookup and display.
    qualified_name: String,
    /// Source signature text preserved on the emitted symbol.
    signature: String,
}

/// Options controlling reference emission inside a block body.
struct EmitRefsOptions {
    /// Drop `:`-scoped module refs (moved/import blocks name resources INSIDE a module instance).
    suppress_scoped: bool,
    /// Direct attributes of the body to skip (meta-arguments handled explicitly elsewhere).
    skip_top_attrs: Vec<String>,
}

impl Default for EmitRefsOptions {
    /// Emits refs without suppression or attribute skipping.
    fn default() -> Self {
        EmitRefsOptions {
            suppress_scoped: false,
            skip_top_attrs: Vec::new(),
        }
    }
}

/// Owns per-file Terraform traversal state, lexical scopes, and ordered fact buffers.
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
}

/// Parses one Terraform/HCL source file and returns its flat facts, diagnostics, and measured kernel duration.
///
/// # Errors
///
/// Returns an error when the native grammar is unavailable, tree-sitter rejects grammar installation, parsing returns no tree, or the tree contains errors and must defer to canonical wasm recovery.
///
/// # Panics
///
/// Panics if a hard-coded node-kind spelling is absent from the frozen `NODE_KINDS` registry.
pub fn extract(file_path: &str, source: &str) -> Result<EmitOut, String> {
    let grammar = crate::langs::grammar_for("terraform").ok_or("no terraform grammar")?;
    let t0 = std::time::Instant::now();
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .map_err(|e| format!("set_language(terraform) failed: {e}"))?;
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
    /// Returns the row of the file node (the first scope-stack entry).
    fn file_row(&self) -> u32 {
        self.stack.first().map(|s| s.row).unwrap_or(0)
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

        let qualified = extra.qualified_name.unwrap_or_else(|| {
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
        });

        let mut flags = BoolFlags::default();
        if let Some(v) = extra.is_exported {
            flags.set(FLAG_IS_EXPORTED, v);
        }
        let name_ref = self.arena.put(name);
        let qn_ref = self.arena.put(&qualified);
        let id_ref = self.arena.put(&id);
        let sig_ref = opt_str(&mut self.arena, extra.signature.as_deref());
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
            docstring: NONE_STR,
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
        Some(row)
    }

    // --- HCL shape helpers -------------------------------------------------------

    /// readBlockHeader (terraform.ts): the first identifier is the block type;
    /// the following string_lit/identifier children are labels.
    fn read_block_header(&self, block: Node<'t>) -> Option<BlockHeader> {
        let named: Vec<Node<'t>> = (0..block.named_child_count())
            .filter_map(|i| block.named_child(i))
            .collect();
        let first = *named.first()?;
        if first.kind() != "identifier" {
            return None;
        }
        let block_type = self.text(first).to_string();
        let mut labels: Vec<String> = Vec::new();
        for child in named.iter().skip(1) {
            let c = *child;
            if c.kind() == "string_lit" {
                labels.push(self.string_lit_value(c));
            } else if c.kind() == "identifier" {
                labels.push(self.text(c).to_string());
            } else {
                break;
            }
        }
        Some(BlockHeader { block_type, labels })
    }

    /// getBlockBody (terraform.ts): the `body` child of a block.
    fn get_block_body(&self, block: Node<'t>) -> Option<Node<'t>> {
        (0..block.named_child_count())
            .filter_map(|i| block.named_child(i))
            .find(|c| c.kind() == "body")
    }

    /// stringLitValue (terraform.ts): the template_literal text, or "" for an
    /// empty string literal.
    fn string_lit_value(&self, node: Node<'t>) -> String {
        let literal = (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .find(|c| c.kind() == "template_literal");
        match literal {
            Some(literal) => self.text(literal).to_string(),
            None => String::new(),
        }
    }

    /// findStringLit (terraform.ts): the first string_lit anywhere under a node.
    fn find_string_lit(&self, expr: Node<'t>) -> Option<Node<'t>> {
        let mut queue: VecDeque<Node<'t>> = VecDeque::new();
        queue.push_back(expr);
        while let Some(n) = queue.pop_front() {
            if n.kind() == "string_lit" {
                return Some(n);
            }
            for i in 0..n.named_child_count() {
                if let Some(c) = n.named_child(i) {
                    queue.push_back(c);
                }
            }
        }
        None
    }

    /// readStringAttr (terraform.ts): the value of a direct string attribute
    /// of a body (`alias = "east"`), or None.
    fn read_string_attr(&self, body: Node<'t>, name: &str) -> Option<String> {
        for i in 0..body.named_child_count() {
            let Some(attr) = body.named_child(i) else {
                continue;
            };
            if attr.kind() != "attribute" {
                continue;
            }
            let id_node = (0..attr.named_child_count())
                .filter_map(|j| attr.named_child(j))
                .find(|c| c.kind() == "identifier");
            let Some(id_node) = id_node else { continue };
            if self.text(id_node) != name {
                continue;
            }
            let expr = (0..attr.named_child_count())
                .filter_map(|j| attr.named_child(j))
                .find(|c| c.kind() == "expression");
            let lit = expr.and_then(|e| self.find_string_lit(e));
            return lit.map(|l| self.string_lit_value(l));
        }
        None
    }

    // --- describeBlock / emitLocals ----------------------------------------------

    /// describeBlock (terraform.ts): the declared symbol for a known block type.
    fn describe_block(block_type: &str, labels: &[String]) -> Option<BlockDecl> {
        let first = labels.first().cloned();
        let second = labels.get(1).cloned();
        match block_type {
            "resource" => {
                let (first, second) = (first?, second?);
                Some(BlockDecl {
                    kind: "class",
                    name: format!("{first}.{second}"),
                    qualified_name: format!("{first}.{second}"),
                    signature: format!("resource \"{first}\" \"{second}\""),
                })
            }
            "data" => {
                let (first, second) = (first?, second?);
                Some(BlockDecl {
                    kind: "class",
                    name: format!("{first}.{second}"),
                    qualified_name: format!("data.{first}.{second}"),
                    signature: format!("data \"{first}\" \"{second}\""),
                })
            }
            "module" => {
                let first = first?;
                Some(BlockDecl {
                    kind: "module",
                    name: first.clone(),
                    qualified_name: format!("module.{first}"),
                    signature: format!("module \"{first}\""),
                })
            }
            "variable" => {
                let first = first?;
                Some(BlockDecl {
                    kind: "variable",
                    name: first.clone(),
                    qualified_name: format!("var.{first}"),
                    signature: format!("variable \"{first}\""),
                })
            }
            "output" => {
                let first = first?;
                Some(BlockDecl {
                    kind: "variable",
                    name: first.clone(),
                    qualified_name: format!("output.{first}"),
                    signature: format!("output \"{first}\""),
                })
            }
            "provider" => {
                let first = first?;
                Some(BlockDecl {
                    kind: "namespace",
                    name: first.clone(),
                    qualified_name: format!("provider.{first}"),
                    signature: format!("provider \"{first}\""),
                })
            }
            _ => None,
        }
    }

    /// emitLocals (terraform.ts): every attribute of a `locals` block becomes
    /// its own `constant` whose references are collected under its scope.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn emit_locals(&mut self, body: Option<Node<'t>>) {
        let Some(body) = body else { return };
        for i in 0..body.named_child_count() {
            let Some(attr) = body.named_child(i) else {
                continue;
            };
            if attr.kind() != "attribute" {
                continue;
            }
            let id_node = (0..attr.named_child_count())
                .filter_map(|j| attr.named_child(j))
                .find(|c| c.kind() == "identifier");
            let Some(id_node) = id_node else { continue };
            let name = self.text(id_node).to_string();
            let Some(created) = self.create_node(
                "constant",
                &name,
                attr,
                Extra {
                    qualified_name: Some(format!("local.{name}")),
                    signature: Some(format!("local.{name}")),
                    ..Extra::default()
                },
            ) else {
                continue;
            };
            let expr = (0..attr.named_child_count())
                .filter_map(|j| attr.named_child(j))
                .find(|c| c.kind() == "expression");
            if let Some(expr) = expr {
                self.stack.push(Scope {
                    row: created,
                    kind: "constant",
                    name: name.clone(),
                });
                let refs_kind = edge_kind_index("references").unwrap();
                self.collect_references(expr, |walker, qname, line, column| {
                    walker.push_ref(created, qname, refs_kind, line, column);
                });
                self.stack.pop();
            }
        }
    }

    // --- reference collection ------------------------------------------------------

    /// collectReferences (terraform.ts): BFS an expression subtree and emit a
    /// reference for every dotted name whose head is a Terraform reference root.
    fn collect_references<F>(&mut self, expr: Node<'t>, mut on_ref: F)
    where
        F: FnMut(&mut Self, &str, u32, u32),
    {
        let mut queue: VecDeque<Node<'t>> = VecDeque::new();
        queue.push_back(expr);
        while let Some(n) = queue.pop_front() {
            if n.kind() == "variable_expr" {
                self.emit_ref_from_variable_expr(n, &mut on_ref);
            }
            for i in 0..n.named_child_count() {
                if let Some(c) = n.named_child(i) {
                    queue.push_back(c);
                }
            }
        }
    }

    /// emitRefFromVariableExpr (terraform.ts): the head identifier plus the
    /// get_attr/index chain of siblings on the parent expression.
    fn emit_ref_from_variable_expr<F>(&mut self, var_expr: Node<'t>, on_ref: &mut F)
    where
        F: FnMut(&mut Self, &str, u32, u32),
    {
        let id = (0..var_expr.named_child_count())
            .filter_map(|i| var_expr.named_child(i))
            .find(|c| c.kind() == "identifier");
        let Some(id) = id else { return };
        let head = self.text(id).to_string();
        if is_builtin_head(&head) || is_builtin_keyword(&head) {
            return;
        }

        let mut attrs: Vec<String> = Vec::new();
        let mut cursor = var_expr.next_named_sibling();
        while let Some(c) = cursor {
            if c.kind() == "get_attr" {
                let attr_id = (0..c.named_child_count())
                    .filter_map(|j| c.named_child(j))
                    .find(|x| x.kind() == "identifier");
                let Some(attr_id) = attr_id else { break };
                attrs.push(self.text(attr_id).to_string());
                cursor = c.next_named_sibling();
            } else if matches!(
                c.kind(),
                "index" | "new_index" | "legacy_index" | "splat" | "attr_splat" | "full_splat"
            ) {
                cursor = c.next_named_sibling();
            } else {
                break;
            }
        }

        let line = var_expr.start_position().row as u32 + 1;
        let column = var_expr.start_position().column as u32;
        for qname in qualify_reference(&head, &attrs) {
            on_ref(self, &qname, line, column);
        }
    }

    /// emitReferencesInBody (terraform.ts): BFS the body's children, emitting
    /// references from every expression (without descending into them).
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn emit_references_in_body(&mut self, body: Node<'t>, from_row: u32, opts: EmitRefsOptions) {
        let mut queue: VecDeque<Node<'t>> = VecDeque::new();
        for i in 0..body.named_child_count() {
            let Some(c) = body.named_child(i) else {
                continue;
            };
            if c.kind() == "attribute"
                && let Some(id) = (0..c.named_child_count())
                    .filter_map(|j| c.named_child(j))
                    .find(|x| x.kind() == "identifier")
                && opts.skip_top_attrs.iter().any(|a| self.text(id) == a)
            {
                continue;
            }
            queue.push_back(c);
        }
        let refs_kind = edge_kind_index("references").unwrap();
        while let Some(n) = queue.pop_front() {
            if n.kind() == "expression" {
                self.collect_references(n, |walker, qname, line, column| {
                    if opts.suppress_scoped && qname.contains(':') {
                        return;
                    }
                    walker.push_ref(from_row, qname, refs_kind, line, column);
                });
                continue;
            }
            for i in 0..n.named_child_count() {
                if let Some(c) = n.named_child(i) {
                    queue.push_back(c);
                }
            }
        }
    }

    // --- provider selection / module wiring ------------------------------------------

    /// emitProviderSelectionRef (terraform.ts): `provider = aws.east` selects a
    /// provider CONFIGURATION — reference `provider.aws.east`.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn emit_provider_selection_ref(&mut self, body: Node<'t>, from_row: u32) {
        for i in 0..body.named_child_count() {
            let Some(attr) = body.named_child(i) else {
                continue;
            };
            if attr.kind() != "attribute" {
                continue;
            }
            let id_node = (0..attr.named_child_count())
                .filter_map(|j| attr.named_child(j))
                .find(|c| c.kind() == "identifier");
            let Some(id_node) = id_node else { continue };
            if self.text(id_node) != "provider" {
                continue;
            }
            let expr = (0..attr.named_child_count())
                .filter_map(|j| attr.named_child(j))
                .find(|c| c.kind() == "expression");
            let Some(expr) = expr else { return };
            if let Some(sel) = self.provider_selection_from_expr(expr) {
                self.push_ref_at(
                    from_row,
                    &format!("provider.{sel}"),
                    edge_kind_index("references").unwrap(),
                    attr,
                );
            }
            return;
        }
    }

    /// emitModuleProvidersRefs (terraform.ts): `providers = { aws = aws.east }`
    /// references each map VALUE (the provider configuration).
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn emit_module_providers_refs(&mut self, body: Node<'t>, from_row: u32) {
        for i in 0..body.named_child_count() {
            let Some(attr) = body.named_child(i) else {
                continue;
            };
            if attr.kind() != "attribute" {
                continue;
            }
            let id_node = (0..attr.named_child_count())
                .filter_map(|j| attr.named_child(j))
                .find(|c| c.kind() == "identifier");
            let Some(id_node) = id_node else { continue };
            if self.text(id_node) != "providers" {
                continue;
            }
            let mut queue: VecDeque<Node<'t>> = VecDeque::new();
            queue.push_back(attr);
            while let Some(n) = queue.pop_front() {
                if n.kind() == "object_elem" {
                    let val = n.child_by_field_name("val");
                    if let Some(val) = val
                        && let Some(sel) = self.provider_selection_from_expr(val)
                    {
                        self.push_ref_at(
                            from_row,
                            &format!("provider.{sel}"),
                            edge_kind_index("references").unwrap(),
                            n,
                        );
                    }
                    continue;
                }
                for j in 0..n.named_child_count() {
                    if let Some(c) = n.named_child(j) {
                        queue.push_back(c);
                    }
                }
            }
            return;
        }
    }

    /// providerSelectionFromExpr (terraform.ts): a provider-configuration
    /// address (`aws`, `aws.east`, `google-beta`) or None when dynamic.
    fn provider_selection_from_expr(&self, expr: Node<'t>) -> Option<String> {
        let mut queue: VecDeque<Node<'t>> = VecDeque::new();
        queue.push_back(expr);
        while let Some(n) = queue.pop_front() {
            if n.kind() == "variable_expr" {
                let id = (0..n.named_child_count())
                    .filter_map(|i| n.named_child(i))
                    .find(|c| c.kind() == "identifier");
                let id = id?;
                let head = self.text(id).to_string();
                let next = n.next_named_sibling();
                if let Some(next) = next
                    && next.kind() == "get_attr"
                {
                    let attr_id = (0..next.named_child_count())
                        .filter_map(|j| next.named_child(j))
                        .find(|c| c.kind() == "identifier");
                    let attr_id = attr_id?;
                    if next.next_named_sibling().is_some() {
                        return None;
                    }
                    return Some(format!("{head}.{}", self.text(attr_id)));
                }
                return if next.is_some() { None } else { Some(head) };
            }
            if matches!(n.kind(), "function_call" | "conditional" | "for_expr") {
                return None;
            }
            for i in 0..n.named_child_count() {
                if let Some(c) = n.named_child(i) {
                    queue.push_back(c);
                }
            }
        }
        None
    }

    /// emitModuleWiring (terraform.ts): `:`-scoped refs that bridge a module
    /// call to the child module's own nodes.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn emit_module_wiring(
        &mut self,
        module_name: &str,
        block: Node<'t>,
        body: Node<'t>,
        from_row: u32,
    ) {
        let imports_kind = edge_kind_index("imports").unwrap();
        let refs_kind = edge_kind_index("references").unwrap();
        for i in 0..body.named_child_count() {
            let Some(attr) = body.named_child(i) else {
                continue;
            };
            if attr.kind() != "attribute" {
                continue;
            }
            let id_node = (0..attr.named_child_count())
                .filter_map(|j| attr.named_child(j))
                .find(|c| c.kind() == "identifier");
            let Some(id_node) = id_node else { continue };
            let attr_name = self.text(id_node).to_string();
            if attr_name == "source" {
                let expr = (0..attr.named_child_count())
                    .filter_map(|j| attr.named_child(j))
                    .find(|c| c.kind() == "expression");
                let source = expr
                    .and_then(|e| self.find_string_lit(e))
                    .map(|l| self.string_lit_value(l))
                    .unwrap_or_default();
                if source.starts_with("./") || source.starts_with("../") {
                    self.push_ref_at(
                        from_row,
                        &format!("module.{module_name}:file"),
                        imports_kind,
                        block,
                    );
                }
                continue;
            }
            if is_module_meta_arg(&attr_name) {
                continue;
            }
            self.push_ref_at(
                from_row,
                &format!("module.{module_name}:var.{attr_name}"),
                refs_kind,
                attr,
            );
        }
    }

    // --- the dispatcher (visitNode) ------------------------------------------------

    /// Traverses node syntax and dispatches each recognized construct exactly once.
    fn visit_node(&mut self, node: Node<'t>) {
        // The language visitNode hook drives all Terraform extraction.
        if self.visit_terraform_node(node) {
            return;
        }
        for i in 0..node.named_child_count() {
            if let Some(c) = node.named_child(i) {
                self.visit_node(c);
            }
        }
    }

    /// visitNode (languages/terraform.ts): block-driven extraction. Returns
    /// true when the node was fully handled.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn visit_terraform_node(&mut self, node: Node<'t>) -> bool {
        if node.kind() != "block" {
            // .tfvars files carry no blocks — top-level `name = value`
            // assignments SET root module variables.
            if node.kind() == "attribute"
                && self.file_path.ends_with(".tfvars")
                && node.parent().map(|p| p.kind()) == Some("body")
                && node.parent().and_then(|p| p.parent()).map(|p| p.kind()) == Some("config_file")
            {
                let id_node = (0..node.named_child_count())
                    .filter_map(|i| node.named_child(i))
                    .find(|c| c.kind() == "identifier");
                let file_row = self.file_row();
                if let Some(id_node) = id_node {
                    self.push_ref_at(
                        file_row,
                        &format!("var.{}", self.text(id_node)),
                        edge_kind_index("references").unwrap(),
                        node,
                    );
                }
                return true;
            }
            return false;
        }

        let Some(header) = self.read_block_header(node) else {
            return false;
        };
        let body = self.get_block_body(node);

        // locals: every attribute becomes its own constant.
        if header.block_type == "locals" && header.labels.is_empty() {
            self.emit_locals(body);
            return true;
        }

        // terraform { ... } settings block — no symbols, no refs.
        if header.block_type == "terraform" && header.labels.is_empty() {
            return true;
        }

        // moved / import / removed: state-migration blocks. Anchor refs to the
        // file node; scoped module refs are suppressed.
        if matches!(header.block_type.as_str(), "moved" | "import" | "removed")
            && header.labels.is_empty()
        {
            let file_row = self.file_row();
            if let Some(body) = body {
                self.emit_references_in_body(
                    body,
                    file_row,
                    EmitRefsOptions {
                        suppress_scoped: true,
                        ..EmitRefsOptions::default()
                    },
                );
            }
            return true;
        }

        // assert { condition = … } (inside check blocks): the condition's
        // references anchor to the file node.
        if header.block_type == "assert" && header.labels.is_empty() {
            let file_row = self.file_row();
            if let Some(body) = body {
                self.emit_references_in_body(
                    body,
                    file_row,
                    EmitRefsOptions {
                        suppress_scoped: true,
                        ..EmitRefsOptions::default()
                    },
                );
            }
            return true;
        }

        // resource / data / module / variable / output / provider.
        let Some(mut decl) = Self::describe_block(&header.block_type, &header.labels) else {
            return false;
        };

        // provider "aws" { alias = "east" } — carry the alias in the node.
        if header.block_type == "provider"
            && let Some(body) = body
            && let Some(alias) = header
                .labels
                .first()
                .and_then(|_| self.read_string_attr(body, "alias"))
        {
            decl.name = format!("{}.{alias}", header.labels[0]);
            decl.qualified_name = format!("provider.{}.{alias}", header.labels[0]);
            decl.signature = format!("provider \"{}\" alias=\"{alias}\"", header.labels[0]);
        }

        let Some(created) = self.create_node(
            decl.kind,
            &decl.name,
            node,
            Extra {
                qualified_name: Some(decl.qualified_name),
                signature: Some(decl.signature),
                is_exported: Some(decl.kind == "variable"),
            },
        ) else {
            return true;
        };

        if let Some(body) = body {
            self.stack.push(Scope {
                row: created,
                kind: decl.kind,
                name: decl.name.clone(),
            });
            let mut skip_top_attrs: Vec<String> = Vec::new();
            if header.block_type == "resource" || header.block_type == "data" {
                self.emit_provider_selection_ref(body, created);
                skip_top_attrs.push("provider".to_string());
            }
            if header.block_type == "module" {
                self.emit_module_providers_refs(body, created);
                skip_top_attrs.push("providers".to_string());
            }
            self.emit_references_in_body(
                body,
                created,
                EmitRefsOptions {
                    suppress_scoped: false,
                    skip_top_attrs,
                },
            );
            if header.block_type == "module"
                && let Some(label0) = header.labels.first()
            {
                let name = label0.clone();
                self.emit_module_wiring(&name, node, body, created);
            }
            self.stack.pop();
        }
        true
    }
}

/// qualifyReference (terraform.ts): map a head + attribute chain to the
/// qualified reference spellings the resolver matches against.
fn qualify_reference(head: &str, attrs: &[String]) -> Vec<String> {
    match head {
        "var" => attrs
            .first()
            .map(|a| format!("var.{a}"))
            .into_iter()
            .collect(),
        "local" => attrs
            .first()
            .map(|a| format!("local.{a}"))
            .into_iter()
            .collect(),
        "module" => {
            let Some(first) = attrs.first() else {
                return Vec::new();
            };
            let mut refs = vec![format!("module.{first}")];
            if let Some(second) = attrs.get(1) {
                refs.push(format!("module.{first}:output.{second}"));
            }
            if attrs.get(1).map(|s| s.as_str()) == Some("outputs")
                && let Some(third) = attrs.get(2)
            {
                refs.push(format!("module.{first}:remote-output.{third}"));
            }
            refs
        }
        "data" => match (attrs.first(), attrs.get(1)) {
            (Some(a), Some(b)) => vec![format!("data.{a}.{b}")],
            _ => Vec::new(),
        },
        _ => match attrs.first() {
            Some(a) => vec![format!("{head}.{a}")],
            None => Vec::new(),
        },
    }
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
        let emit = extract(path, source).expect("terraform extraction succeeds");
        crate::decode::decode_buffers(&emit, path, bohmian_code_review_core::Language::Terraform)
            .expect("terraform decode succeeds")
    }

    /// Collects references of one kind by name.
    fn refs_of(
        result: &crate::result::ExtractionResult,
        kind: bohmian_code_review_core::EdgeKind,
    ) -> Vec<String> {
        result
            .references
            .iter()
            .filter(|r| r.reference_kind == bohmian_code_review_core::ReferenceKind::Edge(kind))
            .map(|r| r.reference_name.clone())
            .collect()
    }

    #[test]
    /// Resource/data/module/variable blocks classify with their qualified names.
    fn blocks_classify_with_qualified_names() {
        let src = "variable \"region\" { type = string }\nresource \"aws_s3_bucket\" \"b\" { bucket = var.region }\nmodule \"vpc\" { source = \"./modules/vpc\" }\n";
        let result = extract_source("main.tf", src);
        let qns: Vec<(&str, &str, &str)> = result
            .nodes
            .iter()
            .filter(|n| n.kind != bohmian_code_review_core::NodeKind::File)
            .map(|n| {
                (
                    n.kind.wire_name(),
                    n.name.as_str(),
                    n.qualified_name.as_str(),
                )
            })
            .collect();
        assert!(qns.contains(&("variable", "region", "var.region")));
        assert!(qns.contains(&("class", "aws_s3_bucket.b", "aws_s3_bucket.b")));
        assert!(qns.contains(&("module", "vpc", "module.vpc")));
    }

    #[test]
    /// var/local/data/resource references qualify with their roots.
    fn references_qualify() {
        let src = "variable \"region\" {}\nlocals { name = var.region }\ndata \"aws_ami\" \"ubuntu\" {}\nresource \"aws_instance\" \"web\" { ami = data.aws_ami.ubuntu.id name = local.name }";
        let result = extract_source("main.tf", src);
        let refs = refs_of(&result, bohmian_code_review_core::EdgeKind::References);
        assert!(refs.iter().any(|r| r == "var.region"));
        assert!(refs.iter().any(|r| r == "local.name"));
        assert!(refs.iter().any(|r| r == "data.aws_ami.ubuntu"));
    }

    #[test]
    /// Module wiring emits `:file` imports and `:var.<in>` references.
    fn module_wiring_emits_scoped_refs() {
        let src = "module \"vpc\" { source = \"./modules/vpc\" cidr = var.region }";
        let result = extract_source("main.tf", src);
        let imports = refs_of(&result, bohmian_code_review_core::EdgeKind::Imports);
        let refs = refs_of(&result, bohmian_code_review_core::EdgeKind::References);
        assert!(imports.iter().any(|r| r == "module.vpc:file"));
        assert!(refs.iter().any(|r| r == "module.vpc:var.cidr"));
    }

    #[test]
    /// .tfvars top-level attributes reference the root variable from the file node.
    fn tfvars_attributes_reference_file() {
        let src = "region = \"us-east-1\"\nami = \"ami-123\"\n";
        let result = extract_source("terraform.tfvars", src);
        let refs = refs_of(&result, bohmian_code_review_core::EdgeKind::References);
        assert!(refs.iter().any(|r| r == "var.region"));
        assert!(refs.iter().any(|r| r == "var.ami"));
        for reference in result.references {
            assert!(reference.from_node_id.starts_with("file:"));
        }
    }
}
