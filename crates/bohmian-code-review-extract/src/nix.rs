//! Nix extraction — a faithful Rust port of `languages/nix.ts` (the
//! visitNode-driven Nix path of src/extraction/tree-sitter.ts).
//!
//! The frozen oracle is the vendored `src/extraction/wasm/
//! tree-sitter-nix.wasm`, built from nix-community/tree-sitter-nix 3d0173d
//! (MIT) by REGENERATING parser.c with tree-sitter-cli 0.25.10 (ABI 15). The
//! vendored C under `crates/codegraph-extract/grammars/nix` is that exact
//! regenerated parser.c plus the checked-in hand-written scanner.c — table
//! identity by construction with the wasm. Behavior parity is asserted per
//! fixture against the oracle goldens.
//!
//! Ported quirks preserved on purpose: bindings whose value is a
//! `function_expression` become `function` nodes (with curried-parameter
//! signatures such as `(x: y)`), other bindings become `variable` nodes whose
//! initializer subtree is still walked for calls; `inherit`/`inherit_from`
//! mint one `variable` per inherited attribute; `import ./x` and
//! `callPackage ./x` emit `import` nodes plus `imports` refs for static
//! project paths only; every other application emits a `calls` ref for its
//! callee (identifier or select chain); bindings inside a returned attrset
//! (and inherited attributes there) are exported. Only literal
//! `path_expression` entries of `imports = [ … ]` / `modules = [ … ]` lists
//! count as file imports. Nix is NOT in TYPE_ANNOTATION_LANGUAGES,
//! VALUE_REF_LANGS, or FN_REF_SPECS. Positions are emitted in UTF-16 code
//! units.

use crate::buffers::{
    Arena, BoolFlags, EdgeRow, EmitOut, FLAG_IS_EXPORTED, NONE, NONE_STR, NodeRow, RefRow, StrRef,
    Tables, build_meta, edge_kind_index, node_kind_index,
};
use crate::ids;
use crate::textutil as util;
use tree_sitter::{Node, Parser};

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
    /// Source signature text preserved on the emitted symbol.
    signature: Option<String>,
    /// Whether the declaration is exported, encoded in the node flag bit-pair.
    is_exported: Option<bool>,
}

/// Owns per-file Nix traversal state, lexical scopes, and ordered fact buffers.
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

/// Parses one Nix source file and returns its flat facts, diagnostics, and measured kernel duration.
///
/// # Errors
///
/// Returns an error when the native grammar is unavailable, tree-sitter rejects grammar installation, parsing returns no tree, or the tree contains errors and must defer to canonical wasm recovery.
///
/// # Panics
///
/// Panics if a hard-coded node-kind spelling is absent from the frozen `NODE_KINDS` registry.
pub fn extract(file_path: &str, source: &str) -> Result<EmitOut, String> {
    let grammar = crate::langs::grammar_for("nix").ok_or("no nix grammar")?;
    let t0 = std::time::Instant::now();
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .map_err(|e| format!("set_language(nix) failed: {e}"))?;
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

    // --- nix.ts helpers -----------------------------------------------------------

    /// unwrapVariableExpression (nix.ts): a `variable_expression` wraps its
    /// identifier/select child.
    fn unwrap_variable_expression(&self, node: Node<'t>) -> Node<'t> {
        if node.kind() != "variable_expression" {
            return node;
        }
        node.named_child(0).unwrap_or(node)
    }

    /// getCalleeName (nix.ts): the innermost function of a curried apply chain.
    fn get_callee_name(&self, node: Node<'t>) -> Option<String> {
        let mut current = node;
        while current.kind() == "apply_expression" {
            let func = current
                .child_by_field_name("function")
                .or_else(|| current.named_child(0));
            let Some(func) = func else { break };
            current = func;
        }
        let current = self.unwrap_variable_expression(current);
        if matches!(current.kind(), "identifier" | "select_expression") {
            return Some(self.text(current).trim().to_string());
        }
        None
    }

    /// getDirectCalleeName (nix.ts): the immediate function child of an apply.
    fn get_direct_callee_name(&self, node: Node<'t>) -> Option<String> {
        let func = node
            .child_by_field_name("function")
            .or_else(|| node.named_child(0))?;
        let func = self.unwrap_variable_expression(func);
        Some(self.text(func).trim().to_string())
    }

    /// isStaticProjectPath (nix.ts): a literal `./`/`../` path with no
    /// dynamic characters.
    fn is_static_project_path(value: &str) -> bool {
        (value.starts_with("./") || value.starts_with("../"))
            && !value.chars().any(|c| {
                matches!(
                    c,
                    ' ' | '\t'
                        | '\n'
                        | '{'
                        | '}'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | ';'
                        | '"'
                        | '\''
                        | '<'
                        | '>'
                        | '$'
                )
            })
    }

    /// getStaticImportPath (nix.ts): unwrap parens and quotes, keep only
    /// static project paths.
    fn get_static_import_path(&self, mut current: Node<'t>) -> Option<String> {
        while current.kind() == "parenthesized_expression" {
            let inner = current.named_child(0)?;
            current = inner;
        }

        let mut text = self.text(current).trim().to_string();
        if ((text.starts_with('"') && text.ends_with('"'))
            || (text.starts_with('\'') && text.ends_with('\'')))
            && text.len() >= 2
        {
            text = text[1..text.len() - 1].to_string();
        }

        if Self::is_static_project_path(&text) {
            Some(text)
        } else {
            None
        }
    }

    /// isReturnedAttrsetMember (nix.ts): is this node inside an attrset that
    /// is the value being returned by the enclosing chain?
    fn is_returned_attrset_member(&self, node: Node<'t>) -> bool {
        let mut current: Option<Node<'t>> = Some(node);
        let mut seen_returned_attrset = false;

        while let Some(cur) = current {
            let Some(parent) = cur.parent() else { break };

            if parent.kind() == "let_expression" {
                let body_node = parent
                    .child_by_field_name("body")
                    .or_else(|| parent.child_by_field_name("expression"));
                if let Some(body_node) = body_node
                    && !body_node.eq(&cur)
                {
                    return false;
                }
            }

            if parent.kind() == "binding" && !cur.eq(&node) {
                return false;
            }
            if matches!(parent.kind(), "formal_parameters" | "formals") {
                return false;
            }

            if matches!(
                parent.kind(),
                "attrset" | "rec_attrset" | "attrset_expression" | "rec_attrset_expression"
            ) {
                seen_returned_attrset = true;
            }

            current = Some(parent);
        }

        seen_returned_attrset
    }

    /// getCurriedParamsAndBody (nix.ts): the parameter prefix text and body
    /// node of a curried function expression.
    fn get_curried_params_and_body(&self, node: Node<'t>) -> (Vec<String>, Option<Node<'t>>) {
        let mut params: Vec<String> = Vec::new();
        let mut current = node;

        while current.kind() == "function_expression" && current.named_child_count() > 0 {
            let body_node = current.named_child(current.named_child_count() - 1);
            let Some(body_node) = body_node else { break };

            let mut param_part = self.text(current)
                [..body_node.start_byte() - current.start_byte()]
                .trim()
                .to_string();
            if param_part.ends_with(':') {
                param_part = param_part[..param_part.len() - 1].trim().to_string();
            }
            if !param_part.is_empty() {
                params.push(param_part);
            }

            if body_node.kind() == "function_expression" {
                current = body_node;
            } else {
                return (params, Some(body_node));
            }
        }

        let last = if current.named_child_count() > 0 {
            current.named_child(current.named_child_count() - 1)
        } else {
            None
        };
        (params, last)
    }

    /// formatFunctionSignature (nix.ts): the stored function signature shape.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn format_function_signature(params: &[String]) -> String {
        match params.len() {
            0 => "()".to_string(),
            n if n > 1 => params.join(" : "),
            _ => {
                let param = &params[0];
                if param.starts_with('(') || param.contains('{') || param.contains('@') {
                    param.clone()
                } else {
                    format!("({param})")
                }
            }
        }
    }

    /// inheritedAttrs (nix.ts): the `inherited_attrs` child of inherit/inherit_from.
    fn inherited_attrs(&self, node: Node<'t>) -> Option<Node<'t>> {
        (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .find(|c| c.kind() == "inherited_attrs")
    }

    /// isCallPackageName (nix.ts): the nixpkgs auto-wiring idiom.
    fn is_call_package_name(name: &str) -> bool {
        name == "callPackage"
            || name == "callPackages"
            || name.ends_with(".callPackage")
            || name.ends_with(".callPackages")
    }

    /// getFirstApplyArgument (nix.ts): the innermost argument of a curried apply chain.
    fn get_first_apply_argument(&self, node: Node<'t>) -> Option<Node<'t>> {
        let mut inner = node;
        loop {
            let func = inner
                .child_by_field_name("function")
                .or_else(|| inner.named_child(0));
            if let Some(func) = func
                && func.kind() == "apply_expression"
            {
                inner = func;
                continue;
            }
            break;
        }
        inner
            .child_by_field_name("argument")
            .or_else(|| inner.named_child(1))
    }

    /// emitFileImport (nix.ts): an import node plus an `imports` ref for a
    /// static project path.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn emit_file_import(&mut self, import_path: &str, anchor: Node<'t>) {
        let sig = util::slice_utf16(self.text(anchor).trim(), 100).0;
        let Some(import_row) = self.create_node(
            "import",
            import_path,
            anchor,
            Extra {
                signature: Some(sig),
                ..Extra::default()
            },
        ) else {
            return;
        };
        let _ = import_row;
        let from = self.top_row();
        self.push_ref_at(
            from,
            import_path,
            edge_kind_index("imports").unwrap(),
            anchor,
        );
    }

    // --- the dispatcher (visitNode) ------------------------------------------------

    /// Traverses node syntax and dispatches each recognized construct exactly once.
    fn visit_node(&mut self, node: Node<'t>) {
        if self.visit_nix_node(node) {
            return;
        }
        for i in 0..node.named_child_count() {
            if let Some(c) = node.named_child(i) {
                self.visit_node(c);
            }
        }
    }

    /// visitNode (languages/nix.ts): binding/function/inherit/apply-driven
    /// extraction. Returns true when the node was fully handled.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded node-kind or edge-kind spelling is absent from the frozen ABI registries.
    fn visit_nix_node(&mut self, node: Node<'t>) -> bool {
        if node.kind() == "binding" {
            let attrpath = node
                .child_by_field_name("attrpath")
                .or_else(|| node.named_child(0));
            let Some(attrpath) = attrpath else {
                return false;
            };
            let name = self.text(attrpath).trim().to_string();
            if name.is_empty() {
                return false;
            }
            let value_node = node
                .child_by_field_name("expression")
                .or_else(|| node.child_by_field_name("value"))
                .or_else(|| node.named_child(1));
            let Some(value_node) = value_node else {
                return false;
            };

            if value_node.kind() == "function_expression" {
                let (params, body_node) = self.get_curried_params_and_body(value_node);
                let is_exported = self.is_returned_attrset_member(node);
                if let Some(func_row) = self.create_node(
                    "function",
                    &name,
                    node,
                    Extra {
                        signature: Some(Self::format_function_signature(&params)),
                        is_exported: Some(is_exported),
                    },
                ) {
                    self.stack.push(Scope {
                        row: func_row,
                        kind: "function",
                        name: name.clone(),
                    });
                    if let Some(body_node) = body_node {
                        self.visit_node(body_node);
                    }
                    self.stack.pop();
                }
            } else {
                let init_value = util::slice_utf16(self.text(value_node), 100).0;
                let signature = if init_value.is_empty() {
                    None
                } else {
                    let len = util::utf16_len(&init_value);
                    Some(if len >= 100 {
                        format!("= {init_value}...")
                    } else {
                        format!("= {init_value}")
                    })
                };
                let is_exported = self.is_returned_attrset_member(node);
                self.create_node(
                    "variable",
                    &name,
                    node,
                    Extra {
                        signature,
                        is_exported: Some(is_exported),
                    },
                );

                // NixOS/home-manager module lists: `imports = [ ./hardware.nix … ]`.
                let final_segment = name.rsplit('.').next().unwrap_or("");
                if (final_segment == "imports" || final_segment == "modules")
                    && value_node.kind() == "list_expression"
                {
                    for i in 0..value_node.named_child_count() {
                        if let Some(child) = value_node.named_child(i)
                            && child.kind() == "path_expression"
                        {
                            let entry_path = self.text(child).trim().to_string();
                            if Self::is_static_project_path(&entry_path) {
                                self.emit_file_import(&entry_path, child);
                            }
                        }
                    }
                }

                self.visit_node(value_node);
            }

            return true;
        }

        if node.kind() == "function_expression" {
            let body_node = node.named_child(node.named_child_count() - 1);
            if let Some(body_node) = body_node {
                self.visit_node(body_node);
            }
            return true;
        }

        if matches!(node.kind(), "inherit" | "inherit_from") {
            let attrs = self.inherited_attrs(node);
            if let Some(attrs) = attrs {
                for i in 0..attrs.named_child_count() {
                    let Some(child) = attrs.named_child(i) else {
                        continue;
                    };
                    let name = self.text(child).trim().to_string();
                    if !name.is_empty() {
                        let is_exported = self.is_returned_attrset_member(child);
                        self.create_node(
                            "variable",
                            &name,
                            child,
                            Extra {
                                is_exported: Some(is_exported),
                                ..Extra::default()
                            },
                        );
                    }
                }
            }

            for i in 0..node.named_child_count() {
                if let Some(child) = node.named_child(i)
                    && child.kind() != "inherited_attrs"
                {
                    self.visit_node(child);
                }
            }
            return true;
        }

        if node.kind() == "apply_expression" {
            let direct_callee = self.get_direct_callee_name(node);
            let is_direct_import = matches!(
                direct_callee.as_deref(),
                Some("import") | Some("builtins.import")
            );
            // Wrapper objects are re-created per access — compare with eq().
            let parent_fn = match node.parent() {
                Some(parent) if parent.kind() == "apply_expression" => parent
                    .child_by_field_name("function")
                    .or_else(|| parent.named_child(0)),
                _ => None,
            };
            let is_callee_of_parent = parent_fn.map(|f| f.eq(&node)).unwrap_or(false);

            if !is_callee_of_parent || is_direct_import {
                if is_direct_import {
                    let arg_node = node
                        .child_by_field_name("argument")
                        .or_else(|| node.named_child(1));
                    if let Some(arg_node) = arg_node
                        && let Some(import_path) = self.get_static_import_path(arg_node)
                    {
                        self.emit_file_import(&import_path, node);
                    }
                } else {
                    let callee_name = self.get_callee_name(node);
                    if let Some(callee_name) = &callee_name
                        && callee_name != "import"
                        && callee_name != "builtins.import"
                        && !self.stack.is_empty()
                    {
                        let from = self.top_row();
                        self.push_ref_at(
                            from,
                            callee_name,
                            edge_kind_index("calls").unwrap(),
                            node,
                        );
                    }

                    if let Some(callee_name) = &callee_name
                        && Self::is_call_package_name(callee_name)
                        && let Some(first_arg) = self.get_first_apply_argument(node)
                        && let Some(import_path) = self.get_static_import_path(first_arg)
                    {
                        self.emit_file_import(&import_path, node);
                    }
                }
            }

            for i in 0..node.named_child_count() {
                if let Some(child) = node.named_child(i) {
                    self.visit_node(child);
                }
            }
            return true;
        }

        false
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
        let emit = extract(path, source).expect("nix extraction succeeds");
        crate::decode::decode_buffers(&emit, path, bohmian_code_review_core::Language::Nix)
            .expect("nix decode succeeds")
    }

    #[test]
    /// Function-valued bindings become function nodes with curried signatures.
    fn function_bindings_extract_with_signatures() {
        let src =
            "{ foo = x: y: x + y; bar = { pkgs }: pkgs.stdenv.mkDerivation { name = \"a\"; }; }";
        let result = extract_source("s.nix", src);
        let fns: Vec<(&str, Option<&str>)> = result
            .nodes
            .iter()
            .filter(|n| n.kind == bohmian_code_review_core::NodeKind::Function)
            .map(|n| (n.name.as_str(), n.signature.as_deref()))
            .collect();
        assert!(fns.contains(&("foo", Some("x : y"))));
        assert!(fns.contains(&("bar", Some("{ pkgs }"))));
    }

    #[test]
    /// Value bindings become variables whose initializers still emit calls.
    fn value_bindings_extract_variables_and_calls() {
        let src = "let x = pkgs.stdenv.mkDerivation { name = \"a\"; }; in x";
        let result = extract_source("s.nix", src);
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Variable && n.name == "x")
        );
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
        assert!(calls.contains(&"pkgs.stdenv.mkDerivation"));
    }

    #[test]
    /// `import ./x` emits an import node and an imports ref.
    fn import_emits_file_import() {
        let src = "{ foo = import ./foo.nix; }";
        let result = extract_source("s.nix", src);
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Import
                    && n.name == "./foo.nix")
        );
        assert!(result.references.iter().any(|r| {
            r.reference_kind
                == bohmian_code_review_core::ReferenceKind::Edge(
                    bohmian_code_review_core::EdgeKind::Imports,
                )
                && r.reference_name == "./foo.nix"
        }));
    }

    #[test]
    /// `inherit x` inside a returned attrset mints an exported variable.
    fn inherit_mints_variables() {
        let src = "{ inherit x; y = 1; }";
        let result = extract_source("s.nix", src);
        let vars: Vec<(&str, Option<bool>)> = result
            .nodes
            .iter()
            .filter(|n| n.kind == bohmian_code_review_core::NodeKind::Variable)
            .map(|n| (n.name.as_str(), n.is_exported))
            .collect();
        assert!(vars.contains(&("x", Some(true))));
        assert!(vars.contains(&("y", Some(true))));
    }

    #[test]
    /// Dynamic imports and non-static paths stay silent.
    fn dynamic_imports_stay_silent() {
        let src = "let p = \"../x.nix\"; in import p";
        let result = extract_source("s.nix", src);
        assert!(
            !result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Import)
        );
    }
}
