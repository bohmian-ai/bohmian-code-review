//! CFScript extraction — a faithful Rust port of the CFScript slice of
//! `TreeSitterExtractor` (src/extraction/tree-sitter.ts) driven by
//! `languages/cfscript.ts`, plus the bare-script path of `cfml-extractor.ts`
//! (the `<anonymous>` component rename that pure `.cfs` files receive).
//!
//! CFScript is the modern bare-script CFML dialect: `component { ... }` /
//! `interface { ... }` declarations (the grammar reuses one `component` node
//! for both — classified by the first token), `function`/`method_definition`
//! routines, `import`/`include` statements, `call_expression` calls
//! (including the CFML scope-prefixed `variables.svc.save()` form), and
//! `variable_declaration`/`property_declaration` data members. The component
//! grammar node has no `name` field, so every component indexes as
//! `<anonymous>` and — exactly like `CfmlExtractor.extractBareScript` — the
//! walker renames `<anonymous>` class/interface nodes to the file-derived
//! component name (`Foo.cfs` → `Foo`) and rewrites their members' qualified
//! names (`<anonymous>::save` → `Foo::save`).
//!
//! The frozen oracle is the vendored `src/extraction/wasm/
//! tree-sitter-cfscript.wasm` (ABI 15), built from cfmleditor/tree-sitter-cfml
//! commit a224ff1 (MIT); the vendored C under
//! `crates/codegraph-extract/grammars/cfscript` is that commit's checked-in
//! parser.c + scanner.c. Behavior parity is asserted per fixture against the
//! oracle goldens. Positions are emitted in UTF-16 code units.

use crate::buffers::{
    Arena, BoolFlags, EdgeRow, EmitOut, FLAG_IS_EXPORTED, NONE, NONE_STR, NodeRow, RefRow, StrRef,
    Tables, build_meta, edge_kind_index, node_kind_index,
};
use crate::ids;
use crate::textutil as util;
use tree_sitter::{Node, Parser};

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

/// `/^(variables|this|local|arguments)\.[A-Za-z_][\w]*$/i` — the CFML
/// scope-prefixed member-call receiver gate.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn scope_prefix_re() -> &'static regex::Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::RegexBuilder::new(r"^(variables|this|local|arguments)\.[A-Za-z_][\w]*$")
            .case_insensitive(true)
            .build()
            .unwrap()
    })
}

/// One scope-stack entry so nested symbols receive stable qualified names and containment edges.
struct Scope {
    /// Emitted node row that owns the scope.
    row: u32,
    /// Closed upstream kind tag of the owning symbol (for the class-like gate).
    kind: &'static str,
    /// Source spelling retained for symbol names, qualified names, or reference matching.
    name: String,
}

/// Owns per-file CFScript traversal state, lexical scopes, and ordered fact buffers.
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
}

/// Parses one CFScript source file and returns its flat facts, diagnostics, and measured kernel duration.
///
/// # Errors
///
/// Returns an error when the native grammar is unavailable, tree-sitter rejects grammar installation, parsing returns no tree, or the tree contains errors and must defer to canonical wasm recovery.
///
/// # Panics
///
/// Panics if a hard-coded node-kind spelling is absent from the frozen `NODE_KINDS` registry.
pub fn extract(file_path: &str, source: &str) -> Result<EmitOut, String> {
    let grammar = crate::langs::grammar_for("cfscript").ok_or("no cfscript grammar")?;
    let t0 = std::time::Instant::now();
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .map_err(|e| format!("set_language(cfscript) failed: {e}"))?;
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

    w.visit_node(tree.root_node());
    w.stack.pop();

    // CfmlExtractor.extractBareScript: rename the grammar's `<anonymous>`
    // component (and every member scoped under it) to the file-derived
    // component name. The renames are applied to the flat tables before they
    // are returned, exactly like the TS loop over the inner extractor's nodes.
    w.rename_anonymous_component();

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
    /// Whether the top of the scope stack is a class-like symbol (the
    /// `isInsideClassLikeNode` gate: class/struct/interface/trait/enum/module).
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
        let name_ref = self.arena.put(name);
        let qn_ref = self.arena.put(&qualified);
        let id_ref = self.arena.put(&id);
        let sig_ref = opt_str(&mut self.arena, extra.signature.as_deref());
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

    // --- the dispatcher (visitNode) ------------------------------------------------

    /// Traverses node syntax and dispatches each recognized construct exactly once.
    fn visit_node(&mut self, node: Node<'t>) {
        let kind = node.kind();
        // functionTypes
        if matches!(
            kind,
            "function_declaration" | "function_expression" | "arrow_function"
        ) {
            if self.inside_class_like()
                && matches!(kind, "function_declaration" | "method_definition")
            {
                self.extract_method(node);
            } else {
                self.extract_function(node);
            }
            return;
        }
        // classTypes (component)
        if kind == "component" {
            // classifyClassNode (cfscript.ts): the first child — named or
            // unnamed token — is 'interface' or 'component'.
            let classification = if node.child(0).map(|c| c.kind()) == Some("interface") {
                "interface"
            } else {
                "class"
            };
            self.extract_class(node, classification);
            return;
        }
        // methodTypes not already handled
        if kind == "method_definition" {
            self.extract_method(node);
            return;
        }
        // propertyTypes + class-like
        if kind == "property_declaration" && self.inside_class_like() {
            self.extract_property(node);
            return;
        }
        // variableTypes: top-level only (cfscript has no class-scope constant gate)
        if kind == "variable_declaration" && !self.inside_class_like() {
            self.extract_variable(node);
            return;
        }
        // importTypes
        if matches!(kind, "import_statement" | "include_statement") {
            self.extract_import(node);
            return;
        }
        // callTypes
        if kind == "call_expression" {
            self.extract_call(node);
            // fall through: children still walked (nested calls in args)
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_node(child);
        }
    }

    // --- extractFunction / extractMethod ------------------------------------------

    /// extractFunction (tree-sitter.ts): a routine with the caller-supplied
    /// scope as parent; walks its body for calls.
    fn extract_function(&mut self, node: Node<'t>) {
        let Some(name) = self.node_name(node) else {
            return;
        };
        if name == "<anonymous>" {
            return;
        }
        let signature = self.node_signature(node);
        let visibility = self.node_visibility(node);
        let row = self.create_node(
            "function",
            &name,
            node,
            NodeExtra {
                signature,
                visibility,
                ..NodeExtra::default()
            },
        );
        let Some(row) = row else { return };
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

    /// extractMethod (tree-sitter.ts): a routine inside a class-like scope;
    /// its calls attribute to the method node.
    fn extract_method(&mut self, node: Node<'t>) {
        let Some(name) = self.node_name(node) else {
            return;
        };
        if name == "<anonymous>" {
            return;
        }
        let signature = self.node_signature(node);
        let visibility = self.node_visibility(node);
        let row = self.create_node(
            "method",
            &name,
            node,
            NodeExtra {
                signature,
                visibility,
                ..NodeExtra::default()
            },
        );
        let Some(row) = row else { return };
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

    /// extractName: the `name` field, or `<anonymous>` when absent.
    fn node_name(&self, node: Node<'t>) -> Option<String> {
        if let Some(name) = node.child_by_field_name("name") {
            let text = self.text(name).to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
        Some("<anonymous>".to_string())
    }

    /// getSignature (cfscript.ts): the `parameters` field text.
    fn node_signature(&self, node: Node<'t>) -> Option<String> {
        node.child_by_field_name("parameters")
            .map(|p| self.text(p).to_string())
    }

    /// cfmlVisibility (cfscript.ts): `public`/`private`/`package`/`remote`
    /// access modifiers on the routine.
    fn node_visibility(&self, node: Node<'t>) -> Option<&'static str> {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "access_type" {
                let text = self.text(child);
                return match text {
                    "public" => Some("public"),
                    "private" => Some("private"),
                    "package" => Some("internal"),
                    "remote" => Some("public"),
                    _ => None,
                };
            }
        }
        None
    }

    // --- extractClass --------------------------------------------------------------

    /// extractClass (tree-sitter.ts): the `component` node — named by the
    /// caller (the class-like dispatch classifies `interface` first), with
    /// inheritance from `component_attribute` labels.
    fn extract_class(&mut self, node: Node<'t>, classification: &'static str) {
        let Some(name) = self.node_name(node) else {
            return;
        };
        let kind = match classification {
            "interface" => "interface",
            _ => "class",
        };
        let visibility = self.node_visibility(node);
        let row = self.create_node(
            kind,
            &name,
            node,
            NodeExtra {
                visibility,
                ..NodeExtra::default()
            },
        );
        let Some(row) = row else { return };
        self.extract_inheritance(node, row);
        self.stack.push(Scope { row, kind, name });
        let body = node.child_by_field_name("body").unwrap_or(node);
        let mut cursor = body.walk();
        for child in body.named_children(&mut cursor) {
            self.visit_node(child);
        }
        self.stack.pop();
    }

    /// # Panics
    /// extractInheritance (tree-sitter.ts): CFML `component extends="Base"`
    /// / `implements="IFoo,IBar"` attributes — labeled identifier/value pairs.
    fn extract_inheritance(&mut self, node: Node<'t>, class_row: u32) {
        if node.kind() != "component" {
            return;
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() != "component_attribute" {
                continue;
            }
            let mut attr_cursor = child.walk();
            let children: Vec<Node<'t>> = child.named_children(&mut attr_cursor).collect();
            let label = children.iter().find(|c| c.kind() == "identifier");
            let value = children.iter().find(|c| c.kind() != "identifier");
            let (Some(label), Some(value)) = (label, value) else {
                continue;
            };
            let label_text = self.text(*label).to_lowercase();
            if label_text != "extends" && label_text != "implements" {
                continue;
            }
            let value_text = strip_quotes(self.text(*value));
            let names: Vec<String> = if label_text == "implements" {
                value_text
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            } else {
                let trimmed = value_text.trim().to_string();
                if trimmed.is_empty() {
                    Vec::new()
                } else {
                    vec![trimmed]
                }
            };
            let kind_code = if label_text == "implements" {
                edge_kind_index("implements").unwrap()
            } else {
                edge_kind_index("extends").unwrap()
            };
            for name in names {
                self.push_ref_at(class_row, &name, kind_code, *value);
            }
        }
    }

    // --- extractImport -------------------------------------------------------------

    /// # Panics
    /// extractImport (cfscript.ts hook): `include "path.cfm"` and
    /// `import com.foo.Bar;` / `import "java:java.util.ArrayList";`.
    fn extract_import(&mut self, node: Node<'t>) {
        let import_text = self.text(node).trim().to_string();
        let mut module_name: Option<String> = None;

        if node.kind() == "include_statement" {
            let mut cursor = node.walk();
            let expr = node
                .named_children(&mut cursor)
                .find(|c| c.kind() == "string");
            if let Some(expr) = expr {
                let inner = strip_quotes(self.text(expr));
                if !inner.is_empty() {
                    module_name = Some(inner);
                }
            }
        } else {
            // import_statement
            let source = node.child_by_field_name("source");
            if let Some(source) = source {
                if source.kind() == "import_path" {
                    let mut cursor = source.walk();
                    let parts: Vec<String> = source
                        .named_children(&mut cursor)
                        .map(|c| self.text(c).to_string())
                        .collect();
                    if !parts.is_empty() {
                        module_name = Some(parts.join("."));
                    }
                } else {
                    let inner = strip_quotes(self.text(source));
                    if !inner.is_empty() {
                        module_name = Some(inner);
                    }
                }
            }
        }

        if let Some(module_name) = module_name {
            let row = self.create_node(
                "import",
                &module_name,
                node,
                NodeExtra {
                    signature: Some(import_text),
                    ..NodeExtra::default()
                },
            );
            let _ = row;
            let from = self.top_row();
            self.push_ref_at(
                from,
                &module_name,
                edge_kind_index("imports").unwrap(),
                node,
            );
        }
    }

    // --- extractVariable -----------------------------------------------------------

    /// extractVariable (tree-sitter.ts, the TS/JS/cfscript branch): each
    /// `variable_declarator` child becomes a variable node (functions stay
    /// functions, destructures are skipped); the initializer is walked for
    /// calls unless it is an object literal.
    fn extract_variable(&mut self, node: Node<'t>) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() != "variable_declarator" {
                continue;
            }
            let name_node = child.child_by_field_name("name");
            let value_node = child.child_by_field_name("value");
            let Some(name_node) = name_node else { continue };
            if matches!(name_node.kind(), "object_pattern" | "array_pattern") {
                continue;
            }
            let name = self.text(name_node).to_string();
            if let Some(value_node) = value_node
                && matches!(value_node.kind(), "arrow_function" | "function_expression")
            {
                self.extract_function(value_node);
                continue;
            }
            let init_value = value_node.map(|v| util::slice_utf16(self.text(v), 100).0);
            let init_signature = init_value.as_ref().map(|v| {
                let len = util::utf16_len(v);
                if len >= 100 {
                    format!("= {v}...")
                } else {
                    format!("= {v}")
                }
            });
            let row = self.create_node(
                "variable",
                &name,
                child,
                NodeExtra {
                    signature: init_signature,
                    is_exported: Some(false),
                    ..NodeExtra::default()
                },
            );
            let _ = row;
            if let Some(value_node) = value_node
                && !matches!(value_node.kind(), "object" | "object_expression")
            {
                self.visit_body(value_node);
            }
        }
    }

    // --- extractProperty -----------------------------------------------------------

    /// extractProperty (tree-sitter.ts): a `property_declaration` inside a
    /// component — named by the `name` field or first identifier, signed by
    /// the type text plus the name.
    fn extract_property(&mut self, node: Node<'t>) {
        let name_node = node
            .child_by_field_name("name")
            .or_else(|| node.child_by_field_name("property"))
            .or_else(|| {
                let mut cursor = node.walk();
                node.named_children(&mut cursor)
                    .find(|c| c.kind() == "identifier")
            });
        let Some(name_node) = name_node else { return };
        let name = self.text(name_node).to_string();
        if name.is_empty() {
            return;
        }
        let type_node = {
            let mut cursor = node.walk();
            node.named_children(&mut cursor).find(|c| {
                !matches!(
                    c.kind(),
                    "modifier"
                        | "modifiers"
                        | "identifier"
                        | "accessor_list"
                        | "accessors"
                        | "equals_value_clause"
                )
            })
        };
        let type_text = type_node.map(|t| self.text(t).trim().to_string());
        let signature = type_text
            .map(|t| format!("{t} {name}"))
            .unwrap_or_else(|| name.clone());
        let visibility = self.node_visibility(node);
        self.create_node(
            "property",
            &name,
            node,
            NodeExtra {
                signature: Some(signature),
                visibility,
                ..NodeExtra::default()
            },
        );
    }

    // --- extractCall ---------------------------------------------------------------

    /// # Panics
    /// extractCall (tree-sitter.ts): the generic callee resolution plus the
    /// CFML scope-prefixed member-call branch.
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
                } else if receiver.kind() == "member_expression"
                    && scope_prefix_re().is_match(self.text(receiver).trim())
                {
                    // CFML scope-prefixed member call — keep the full receiver
                    // chain so resolution can strip the scope prefix.
                    callee_name = format!("{}.{method_name}", self.text(receiver).trim());
                } else {
                    callee_name = method_name;
                }
            } else {
                callee_name = method_name;
            }
        } else {
            callee_name = self.text(func).to_string();
        }

        if !callee_name.is_empty() {
            self.push_ref_at(from, &callee_name, edge_kind_index("calls").unwrap(), node);
        }
    }

    // --- body walk -----------------------------------------------------------------

    /// visitFunctionBody (tree-sitter.ts): the body walker visits calls,
    /// instantiations, nested named functions, and structural symbols — but
    /// never method/variable/import dispatch (locals stay out of the graph).
    fn visit_body(&mut self, node: Node<'t>) {
        let kind = node.kind();
        if kind == "call_expression" {
            self.extract_call(node);
        } else if kind == "new_expression" || kind == "object_creation_expression" {
            self.extract_instantiation(node);
        } else if kind == "component" {
            let classification = if node.child(0).map(|c| c.kind()) == Some("interface") {
                "interface"
            } else {
                "class"
            };
            self.extract_class(node, classification);
            return;
        } else if matches!(
            kind,
            "function_declaration" | "function_expression" | "arrow_function"
        ) {
            let name = self.node_name(node).unwrap_or_default();
            if name != "<anonymous>" {
                self.extract_function(node);
                return;
            }
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_body(child);
        }
    }

    /// # Panics
    /// extractInstantiation (tree-sitter.ts): `new Foo()` inside a body emits
    /// an `instantiates` reference to the class name.
    fn extract_instantiation(&mut self, node: Node<'t>) {
        let from = self.top_row();
        let ctor = node
            .child_by_field_name("constructor")
            .or_else(|| node.child_by_field_name("type"))
            .or_else(|| node.child_by_field_name("name"))
            .or_else(|| node.named_child(0));
        let Some(ctor) = ctor else { return };
        let mut class_name = self.text(ctor).to_string();
        if let Some(lt) = class_name.find('<') {
            class_name.truncate(lt);
        }
        let last_dot = class_name.rfind('.');
        let last_colon = class_name.rfind("::");
        let last = last_dot.max(last_colon);
        if let Some(last) = last {
            class_name = class_name[last + 1..].to_string();
            while class_name.starts_with('.') || class_name.starts_with(':') {
                class_name.remove(0);
            }
        }
        let class_name = class_name.trim().to_string();
        if !class_name.is_empty() {
            self.push_ref_at(
                from,
                &class_name,
                edge_kind_index("instantiates").unwrap(),
                node,
            );
        }
    }

    // --- CfmlExtractor.extractBareScript rename ------------------------------------

    /// Applies the bare-script component rename over the flat tables: every
    /// emitted `<anonymous>` class/interface node is renamed to the
    /// file-derived component name and re-qualified as `filePath::Name`, and
    /// every other node whose qualified name was scoped under `<anonymous>`
    /// gets the prefix rewritten to the component name. The rename patches
    /// the NAME/QUALIFIED_NAME string refs in place (appending replacement
    /// strings to the arena, which never invalidates earlier refs).
    fn rename_anonymous_component(&mut self) {
        let component_name = component_name_from_path(self.file_path);
        if component_name.is_empty() {
            return;
        }
        let anonymous = "<anonymous>";
        let node_count = self.tables.node_count as usize;
        for index in 0..node_count {
            let row_start = index * crate::buffers::NODE_ROW_SIZE;
            let row = &self.tables.nodes[row_start..row_start + crate::buffers::NODE_ROW_SIZE];
            let kind_index = row[crate::buffers::node::KIND];
            let name = self.arena_str(row, crate::buffers::node::NAME);
            let qn = self.arena_str(row, crate::buffers::node::QUALIFIED_NAME);
            let kind_name = crate::buffers::NODE_KINDS
                .get(kind_index as usize)
                .copied()
                .unwrap_or("");
            let (name_out, qn_out);
            if name == anonymous && matches!(kind_name, "class" | "interface") {
                name_out = component_name.clone();
                qn_out = format!("{}::{}", self.file_path, component_name);
            } else if qn == anonymous || qn.starts_with(&format!("{anonymous}::")) {
                name_out = name.clone();
                qn_out = format!("{component_name}{}", &qn[anonymous.len()..]);
            } else {
                continue;
            }
            if name_out != name {
                let name_ref = self.arena.put(&name_out);
                self.patch_str_ref(row_start, crate::buffers::node::NAME, name_ref);
            }
            if qn_out != qn {
                let qn_ref = self.arena.put(&qn_out);
                self.patch_str_ref(row_start, crate::buffers::node::QUALIFIED_NAME, qn_ref);
            }
        }
    }

    /// Overwrites one str-ref slot (offset + length) in an encoded node row.
    fn patch_str_ref(&mut self, row_start: usize, slot: usize, reference: StrRef) {
        let bytes = &mut self.tables.nodes[row_start..row_start + crate::buffers::NODE_ROW_SIZE];
        bytes[slot..slot + 4].copy_from_slice(&u32::to_le_bytes(reference.0));
        bytes[slot + 4..slot + 8].copy_from_slice(&u32::to_le_bytes(reference.1));
    }

    /// # Panics
    /// Reads an arena string from a fixed-width row's str-ref slot.
    fn arena_str(&self, row: &[u8], slot: usize) -> String {
        let off = u32::from_le_bytes(row[slot..slot + 4].try_into().unwrap()) as usize;
        let len = u32::from_le_bytes(row[slot + 4..slot + 8].try_into().unwrap()) as usize;
        if off == crate::buffers::NONE as usize {
            return String::new();
        }
        self.arena.get((off as u32, len as u32)).to_string()
    }
}

/// The component name a bare-script file derives from its path (componentNameFromPath)./// The component name a bare-script file derives from its path (componentNameFromPath).
fn component_name_from_path(file_path: &str) -> String {
    let file_name = file_path.rsplit(['/', '\\']).next().unwrap_or(file_path);
    let mut stripped = file_name.to_string();
    for ext in [".cfc", ".cfm", ".cfs"] {
        if stripped.ends_with(ext) {
            stripped.truncate(stripped.len() - ext.len());
            break;
        }
    }
    stripped
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

/// Strips matching leading/trailing quotes from a source slice.
fn strip_quotes(text: &str) -> String {
    let bytes = text.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        text[1..text.len() - 1].to_string()
    } else {
        text.to_string()
    }
}

/// Converts the arena absent sentinel into `None` and preserves present strings without normalization.
fn opt_str(arena: &mut Arena, s: Option<&str>) -> StrRef {
    match s {
        Some(s) => arena.put(s),
        None => NONE_STR,
    }
}
