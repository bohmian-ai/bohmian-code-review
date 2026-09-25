//! VB.NET extraction — a faithful Rust port of the VB.NET slice of
//! `TreeSitterExtractor` (src/extraction/tree-sitter.ts) driven by
//! `languages/vbnet.ts`.
//!
//! The vendored grammar has no true end-of-file token, so a file whose last
//! line lacks a trailing newline ends with a MISSING-newline error; the
//! `preParse` appends a newline (offset-preserving for all existing
//! content). The walker parses and emits positions from the preprocessed
//! bytes — identical to the oracle — so emitted ranges carry the frozen
//! oracle's observable coordinates (the appended byte is past every emitted
//! range, so content coordinates are the original source coordinates).
//!
//! Extracted shapes: classes/modules as `class`, interfaces, structures,
//! enums (+ members), delegates as `type_alias`, `Imports` statements,
//! `Dim` declaration statements as variables, fields, properties, methods
//! (incl. constructors named `New`, P/Invoke `Declare`, `MustOverride`
//! abstract members, interface members), and events as `field` nodes so
//! `RaiseEvent`/`Handles` flows have findable declarations. Calls treat
//! `invocation_expression` / `array_access_expression` /
//! `generic_invocation_expression` as call sites (VB uses parentheses for
//! both calls and indexing), and a constructor-shaped `New T(args)` parses
//! as an `array_creation_expression` that is emitted as an `instantiates`
//! reference.
//!
//! The frozen oracle is the vendored `src/extraction/wasm/
//! tree-sitter-vbnet.wasm` (ABI 15), built from govindbanura/tree-sitter-vbnet
//! commit 538b708 with docs/grammars/tree-sitter-vbnet.patch applied and the
//! parser regenerated with tree-sitter-cli 0.25.10 (the patch's `/u` regex
//! flag requires ≥0.25); the vendored C under
//! `crates/codegraph-extract/grammars/vbnet` is that generated parser.c +
//! the patched scanner.c. Behavior parity is asserted per fixture against
//! the oracle goldens. Positions are emitted in UTF-16 code units.

use crate::buffers::{
    Arena, BoolFlags, EdgeRow, EmitOut, FLAG_IS_EXPORTED, NONE, NONE_STR, NodeRow, RefRow, StrRef,
    Tables, build_meta, edge_kind_index, node_kind_index,
};
use crate::ids;
use crate::textutil as util;
use regex::Regex;
use std::borrow::Cow;
use std::sync::OnceLock;
use tree_sitter::{Node, Parser};

/// ensureTrailingNewline (vbnet.ts): append a newline when the source does
/// not end with one — offset-preserving for all existing content.
fn ensure_trailing_newline(source: &str) -> Cow<'_, str> {
    if source.ends_with('\n') {
        Cow::Borrowed(source)
    } else {
        let mut owned = source.to_owned();
        owned.push('\n');
        Cow::Owned(owned)
    }
}

/// `/^private$/i` — the private member-modifier test.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn private_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^private$").unwrap())
}

/// `/^protected(\s+friend)?$/i` — the protected member-modifier test.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn protected_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^protected(\s+friend)?$").unwrap())
}

/// `/^friend$/i` — the friend (internal) member-modifier test.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn friend_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^friend$").unwrap())
}

/// `/^shared$/i` — the Shared (static) member-modifier test.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn shared_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^shared$").unwrap())
}

/// `/^async$/i` — the Async member-modifier test.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn async_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^async$").unwrap())
}

/// One scope-stack entry so nested symbols receive stable qualified names and containment edges.
#[derive(Clone)]
struct Scope {
    /// Emitted node row that owns the scope.
    row: u32,
    /// Closed upstream kind tag of the owning symbol (for the class-like gate).
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
    /// Visibility wire value ('public' | 'private' | 'protected' | 'internal').
    visibility: Option<&'static str>,
    /// Whether the symbol is static, encoded in the node flag bit-pair.
    is_static: Option<bool>,
    /// Whether the symbol is async, encoded in the node flag bit-pair.
    is_async: Option<bool>,
    /// Declared return type retained for the node payload and type references.
    return_type: Option<String>,
    /// Qualified name override (the `New` constructor is not scoped).
    qualified_name: Option<String>,
}

/// Owns per-file VB.NET traversal state, lexical scopes, and ordered fact buffers.
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
    /// Lexical containment stack used to form qualified names and containment edges.
    stack: Vec<Scope>,
}

/// Parses one VB.NET source file and returns its flat facts, diagnostics, and measured kernel duration.
///
/// # Errors
///
/// Returns an error when the native grammar is unavailable, tree-sitter rejects grammar installation, parsing returns no tree, or the tree contains errors and must defer to canonical wasm recovery.
///
/// # Panics
///
/// Panics if a hard-coded node-kind spelling is absent from the frozen `NODE_KINDS` registry.
pub fn extract(file_path: &str, source: &str) -> Result<EmitOut, String> {
    let grammar = crate::langs::grammar_for("vbnet").ok_or("no vbnet grammar")?;
    // preParse (vbnet.ts): append a trailing newline before parsing; emitted
    // ranges carry the prepared source's coordinates exactly as the frozen
    // oracle emits them (the appended byte is past every emitted range).
    let prepared = ensure_trailing_newline(source);
    extract_prepared(file_path, &prepared, &grammar)
}

/// Parses the preprocessed VB.NET bytes and runs the walker.
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
        .map_err(|e| format!("set_language(vbnet) failed: {e}"))?;
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

    // extractFilePackage: the `namespace_declaration` wraps the file in an
    // implicit namespace node.
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
    /// Whether the top of the scope stack is a class-like symbol (the
    /// `isInsideClassLikeNode` gate).
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
        if let Some(v) = extra.is_static {
            flags.set(crate::buffers::FLAG_IS_STATIC, v);
        }
        if let Some(v) = extra.is_async {
            flags.set(crate::buffers::FLAG_IS_ASYNC, v);
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

    /// extractFilePackage: the first `namespace_declaration` child becomes an
    /// implicit namespace node (packageTypes/extractPackage in vbnet.ts).
    fn extract_file_package(&mut self, root: Node<'t>) -> Option<(u32, String)> {
        let mut cursor = root.walk();
        let namespace = root
            .named_children(&mut cursor)
            .find(|c| c.kind() == "namespace_declaration")?;
        let name = namespace.child_by_field_name("name")?;
        let text = self.text(name).to_string();
        if text.is_empty() {
            return None;
        }
        let row = self.create_node("namespace", &text, namespace, NodeExtra::default())?;
        Some((row, text))
    }

    /// extractName: the `name` field, or `<anonymous>` when absent.
    fn node_name(&self, node: Node<'t>) -> String {
        node.child_by_field_name("name")
            .map(|n| self.text(n).to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "<anonymous>".to_string())
    }

    /// hasModifier (vbnet.ts): scan the member_modifier children case-insensitively.
    fn has_modifier(&self, node: Node<'t>, re: &Regex) -> bool {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "member_modifier" && re.is_match(self.text(child)) {
                return true;
            }
        }
        false
    }

    /// getVisibility (vbnet.ts): VB members default to Public.
    fn node_visibility(&self, node: Node<'t>) -> Option<&'static str> {
        if self.has_modifier(node, private_re()) {
            Some("private")
        } else if self.has_modifier(node, protected_re()) {
            Some("protected")
        } else if self.has_modifier(node, friend_re()) {
            Some("internal")
        } else {
            Some("public")
        }
    }

    /// extractVbnetReturnType (vbnet.ts): the bare class name a chained
    /// factory call could be called on, from the method's `as_clause`.
    fn vbnet_return_type(&self, node: Node<'t>) -> Option<String> {
        let mut cursor = node.walk();
        let as_clause = node
            .named_children(&mut cursor)
            .find(|c| c.kind() == "as_clause")?;
        let type_node = as_clause.child_by_field_name("declared_type")?;
        if matches!(type_node.kind(), "predefined_type" | "array_type") {
            return None;
        }
        let mut t = self.text(type_node).trim().to_string();
        while t.ends_with('?') {
            t.pop();
        }
        // Strip generics `List(Of Foo)` → `List`.
        if let Some(open) = t.find('(') {
            t.truncate(open);
        }
        let last = t.rsplit('.').next().unwrap_or("").trim().to_string();
        if last.is_empty() || !is_simple_ident(&last) {
            return None;
        }
        Some(last)
    }

    // --- the dispatcher (visitNode) ------------------------------------------------

    /// Traverses node syntax and dispatches each recognized construct exactly once.
    fn visit_node(&mut self, node: Node<'t>) {
        let kind = node.kind();

        // vbnet.ts visitNode hook.
        if matches!(kind, "event_declaration" | "custom_event_declaration") {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = self.text(name_node).to_string();
                self.create_node("field", &name, node, NodeExtra::default());
            }
            return;
        }
        if kind == "constructor_declaration" {
            let row = self.create_node("method", "New", node, NodeExtra::default());
            if let Some(row) = row {
                self.stack.push(Scope {
                    row,
                    kind: "method",
                    name: "New".to_string(),
                });
                self.visit_body(node, row);
                self.stack.pop();
            }
            return;
        }

        // classTypes
        if matches!(kind, "class_declaration" | "module_declaration") {
            self.extract_class(node, "class");
            return;
        }
        // interfaceTypes
        if kind == "interface_declaration" {
            self.extract_interface(node);
            return;
        }
        // structTypes
        if kind == "structure_declaration" {
            self.extract_struct(node);
            return;
        }
        // enumTypes
        if kind == "enum_declaration" {
            self.extract_enum(node);
            return;
        }
        // typeAliasTypes
        if kind == "delegate_declaration" {
            let name = self.node_name(node);
            if name != "<anonymous>" {
                self.create_node("type_alias", &name, node, NodeExtra::default());
            }
            return;
        }
        // methodTypes (functionTypes is empty for vbnet)
        if matches!(
            kind,
            "method_declaration"
                | "external_method_declaration"
                | "interface_method_declaration"
                | "abstract_method_declaration"
        ) {
            self.extract_method(node);
            return;
        }
        // propertyTypes + class-like
        if matches!(
            kind,
            "property_declaration"
                | "interface_property_declaration"
                | "abstract_property_declaration"
        ) && self.inside_class_like()
        {
            self.extract_property(node);
            return;
        }
        // fieldTypes + class-like
        if kind == "field_declaration" && self.inside_class_like() {
            self.extract_field(node);
            return;
        }
        // variableTypes
        if kind == "declaration_statement" && !self.inside_class_like() {
            self.extract_variable(node);
            return;
        }
        // importTypes
        if kind == "imports_statement" {
            self.extract_import(node);
            return;
        }
        // callTypes
        if matches!(
            kind,
            "invocation_expression" | "array_access_expression" | "generic_invocation_expression"
        ) {
            self.extract_call(node);
        }
        // instantiations: `New T(args)` parses as array_creation_expression
        if kind == "array_creation_expression" && self.is_vbnet_constructor_shaped(node) {
            self.extract_instantiation(node);
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_node(child);
        }
    }

    /// isVbnetConstructorShapedArrayCreation (tree-sitter.ts): a
    /// `New T(args)` array_creation_expression with a user-defined type and
    /// no brace initializer is really a constructor call.
    fn is_vbnet_constructor_shaped(&self, node: Node<'t>) -> bool {
        let type_node = node.child_by_field_name("type");
        let Some(type_node) = type_node else {
            return false;
        };
        if matches!(type_node.kind(), "predefined_type" | "array_type") {
            return false;
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "array_initializer" {
                return false;
            }
        }
        true
    }

    // --- extractClass / extractInterface / extractStruct / extractEnum -------------

    /// extractClass (tree-sitter.ts): classes and modules (modules index as
    /// classes so their members get normal containment).
    fn extract_class(&mut self, node: Node<'t>, kind: &'static str) {
        let name = self.node_name(node);
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
        // vbnet resolveBody returns the node itself — the declaration node
        // IS its body (no wrapper field), so every member child is walked.
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_node(child);
        }
        self.stack.pop();
    }

    /// extractInterface (tree-sitter.ts): interfaces contain their member
    /// declarations.
    fn extract_interface(&mut self, node: Node<'t>) {
        let name = self.node_name(node);
        let visibility = self.node_visibility(node);
        let row = self.create_node(
            "interface",
            &name,
            node,
            NodeExtra {
                visibility,
                ..NodeExtra::default()
            },
        );
        let Some(row) = row else { return };
        self.extract_inheritance(node, row);
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

    /// extractStruct (tree-sitter.ts): structure declarations.
    fn extract_struct(&mut self, node: Node<'t>) {
        let name = self.node_name(node);
        let visibility = self.node_visibility(node);
        let row = self.create_node(
            "struct",
            &name,
            node,
            NodeExtra {
                visibility,
                ..NodeExtra::default()
            },
        );
        let Some(row) = row else { return };
        self.extract_inheritance(node, row);
        self.stack.push(Scope {
            row,
            kind: "struct",
            name,
        });
        // Upstream extractStruct reads the `body` FIELD only (not resolveBody):
        // the first body: child — so sibling members after it are never walked.
        let body = node.child_by_field_name("body").unwrap_or(node);
        let mut cursor = body.walk();
        for child in body.named_children(&mut cursor) {
            self.visit_node(child);
        }
        self.stack.pop();
    }

    /// extractEnum (tree-sitter.ts): enums with enum_member_declaration members.
    fn extract_enum(&mut self, node: Node<'t>) {
        // vbnet resolveBody returns the node itself — the declaration node IS
        // its body (the enum members are its direct children).
        let name = self.node_name(node);
        let visibility = self.node_visibility(node);
        let row = self.create_node(
            "enum",
            &name,
            node,
            NodeExtra {
                visibility,
                ..NodeExtra::default()
            },
        );
        let Some(row) = row else { return };
        self.stack.push(Scope {
            row,
            kind: "enum",
            name,
        });
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "enum_member_declaration" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let member_name = self.text(name_node).to_string();
                    self.create_node("enum_member", &member_name, child, NodeExtra::default());
                }
            } else {
                self.visit_node(child);
            }
        }
        self.stack.pop();
    }

    /// # Panics
    /// extractInheritance (tree-sitter.ts): VB `Inherits Base` /
    /// `Implements IFoo, IBar(Of T)` are STATEMENTS inside the class body.
    /// Each simple/qualified/generic/global name unwraps generics and keeps
    /// the trailing dotted segment.
    fn extract_inheritance(&mut self, node: Node<'t>, class_row: u32) {
        let mut cursor = node.walk();
        let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
        for child in children {
            let kind_code = match child.kind() {
                "inherits_statement" => edge_kind_index("extends").unwrap(),
                "implements_statement" => edge_kind_index("implements").unwrap(),
                _ => continue,
            };
            let mut inner = child.walk();
            for ref_node in child.named_children(&mut inner) {
                if !matches!(
                    ref_node.kind(),
                    "simple_name" | "qualified_name" | "generic_name" | "global_qualified_name"
                ) {
                    continue;
                }
                let mut name = self.text(ref_node).to_string();
                // Strip `(Of T)` generics.
                if let Some(open) = name.find('(') {
                    name.truncate(open);
                }
                if let Some(last_dot) = name.rfind('.') {
                    name = name[last_dot + 1..].to_string();
                }
                let name = name.trim().to_string();
                if name.is_empty() {
                    continue;
                }
                self.push_ref_at(class_row, &name, kind_code, ref_node);
            }
        }
    }

    // --- extractMethod / extractFunction ------------------------------------------

    /// extractMethod (tree-sitter.ts): a member routine whose body is the
    /// node itself (this grammar has no body wrapper — resolveBody returns
    /// the node). A method outside a class-like scope falls back to function.
    fn extract_method(&mut self, node: Node<'t>) {
        if !self.inside_class_like() {
            self.extract_function(node);
            return;
        }
        let name = self.node_name(node);
        if name == "<anonymous>" {
            return;
        }
        let visibility = self.node_visibility(node);
        let is_static = Some(self.has_modifier(node, shared_re()));
        let is_async = Some(self.has_modifier(node, async_re()));
        let return_type = self.vbnet_return_type(node);
        let row = self.create_node(
            "method",
            &name,
            node,
            NodeExtra {
                visibility,
                is_static,
                is_async,
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
        self.visit_children_body(node);
        self.stack.pop();
    }

    /// extractFunction (tree-sitter.ts): a top-level routine.
    fn extract_function(&mut self, node: Node<'t>) {
        let name = self.node_name(node);
        if name == "<anonymous>" {
            return;
        }
        let visibility = self.node_visibility(node);
        let is_static = Some(self.has_modifier(node, shared_re()));
        let is_async = Some(self.has_modifier(node, async_re()));
        let return_type = self.vbnet_return_type(node);
        let row = self.create_node(
            "function",
            &name,
            node,
            NodeExtra {
                visibility,
                is_static,
                is_async,
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
        self.visit_children_body(node);
        self.stack.pop();
    }

    // --- extractProperty / extractField / extractVariable --------------------------

    /// extractProperty (tree-sitter.ts): properties, signed `Type Name`.
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
        let is_static = Some(self.has_modifier(node, shared_re()));
        self.create_node(
            "property",
            &name,
            node,
            NodeExtra {
                signature: Some(signature),
                visibility,
                is_static,
                ..NodeExtra::default()
            },
        );
    }

    /// extractField (tree-sitter.ts): each `variable_declarator` of a field
    /// declaration becomes a field node. The signature is the first
    /// non-excluded named child's text plus the name (for vbnet's
    /// `field_declaration` that is the member_modifier, so `Public X` reads
    /// as "Public X" — the upstream generic-extractor quirk, ported as-is).
    fn extract_field(&mut self, node: Node<'t>) {
        let visibility = self.node_visibility(node);
        let is_static = Some(self.has_modifier(node, shared_re()));
        let type_node = {
            let mut cursor = node.walk();
            node.named_children(&mut cursor).find(|c| {
                !matches!(
                    c.kind(),
                    "modifiers"
                        | "modifier"
                        | "variable_declarator"
                        | "variable_declaration"
                        | "marker_annotation"
                        | "annotation"
                )
            })
        };
        let type_text = type_node.map(|t| self.text(t).to_string());
        let mut cursor = node.walk();
        let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
        let declarators: Vec<Node<'t>> = children
            .iter()
            .copied()
            .filter(|c| c.kind() == "variable_declarator")
            .collect();
        for declarator in declarators {
            let Some(name_node) = declarator.child_by_field_name("name") else {
                continue;
            };
            let name = self.text(name_node).to_string();
            if name.is_empty() {
                continue;
            }
            let signature = type_text
                .as_ref()
                .map(|t| format!("{t} {name}"))
                .unwrap_or_else(|| name.clone());
            self.create_node(
                "field",
                &name,
                declarator,
                NodeExtra {
                    signature: Some(signature),
                    visibility,
                    is_static,
                    ..NodeExtra::default()
                },
            );
        }
    }

    /// extractVariable (tree-sitter.ts): the generic fallback — each direct
    /// `identifier` or `variable_declarator` child becomes a variable node
    /// (no signature for vbnet).
    fn extract_variable(&mut self, node: Node<'t>) {
        let mut cursor = node.walk();
        let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
        for child in children {
            let name = if child.kind() == "identifier" {
                let text = self.text(child).to_string();
                if text.is_empty() {
                    continue;
                }
                text
            } else if child.kind() == "variable_declarator" {
                let name = child
                    .child_by_field_name("name")
                    .map(|n| self.text(n).to_string())
                    .unwrap_or_default();
                if name.is_empty() || name == "<anonymous>" {
                    continue;
                }
                name
            } else {
                continue;
            };
            self.create_node("variable", &name, child, NodeExtra::default());
        }
    }

    // --- extractImport -------------------------------------------------------------

    /// # Panics
    /// extractImport (vbnet.ts hook): `Imports A.B.C` / `Imports Alias = X` /
    /// `Imports Global.X` — the last qualified/simple/global name child.
    fn extract_import(&mut self, node: Node<'t>) {
        let import_text = self.text(node).trim().to_string();
        let mut cursor = node.walk();
        let children: Vec<Node<'t>> = node.named_children(&mut cursor).collect();
        let name_node = children
            .iter()
            .rev()
            .find(|c| {
                matches!(
                    c.kind(),
                    "qualified_name" | "simple_name" | "global_qualified_name" | "identifier"
                )
            })
            .copied();
        let Some(name_node) = name_node else { return };
        let module_name = self.text(name_node).to_string();
        if module_name.is_empty() {
            return;
        }
        self.create_node(
            "import",
            &module_name,
            node,
            NodeExtra {
                signature: Some(import_text),
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
    }

    // --- extractCall ---------------------------------------------------------------

    /// # Panics
    /// extractCall (tree-sitter.ts vbnet branch): `foo(args)` is ambiguous
    /// between call and index, so all three shapes are call sites; the callee
    /// is the member/identifier under the function/array field.
    fn extract_call(&mut self, node: Node<'t>) {
        let from = self.top_row();
        let func = node
            .child_by_field_name("function")
            .or_else(|| node.child_by_field_name("array"));
        let Some(func) = func else { return };
        let callee_name;
        if func.kind() == "member_access_expression" {
            let member = func.child_by_field_name("member");
            let Some(member) = member else { return };
            let member_name = self.text(member).to_string();
            if member_name.is_empty() {
                return;
            }
            let receiver = func.child_by_field_name("object");
            if let Some(receiver) = receiver
                && receiver.kind() == "identifier"
            {
                let receiver_name = self.text(receiver).to_string();
                if !matches!(
                    receiver_name.to_lowercase().as_str(),
                    "me" | "mybase" | "myclass"
                ) {
                    callee_name = format!("{receiver_name}.{member_name}");
                } else {
                    callee_name = member_name;
                }
            } else {
                callee_name = member_name;
            }
        } else if func.kind() == "identifier" {
            callee_name = self.text(func).to_string();
        } else {
            return;
        }
        if !callee_name.is_empty() {
            self.push_ref_at(from, &callee_name, edge_kind_index("calls").unwrap(), node);
        }
    }

    /// # Panics
    /// extractInstantiation (tree-sitter.ts): `New T(args)` → instantiates.
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
        // VB.NET spells generics with parentheses: `New List(Of String)`.
        if let Some(paren) = class_name.find('(') {
            class_name.truncate(paren);
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

    // --- body walk -----------------------------------------------------------------

    /// Walks a routine node's CHILDREN as its body (the vbnet resolveBody
    /// returns the declaration node itself, so the body walk never
    /// re-dispatches the routine).
    fn visit_children_body(&mut self, node: Node<'t>) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_body(child, 0);
        }
    }

    /// visitFunctionBody (tree-sitter.ts): a VB routine's body IS the node
    /// (resolveBody returns it); the body walker visits calls,
    /// instantiations, nested named functions, and structural symbols — never
    /// method/variable/import dispatch (locals stay out of the graph).
    fn visit_body(&mut self, node: Node<'t>, _function_row: u32) {
        let kind = node.kind();
        if matches!(
            kind,
            "invocation_expression" | "array_access_expression" | "generic_invocation_expression"
        ) {
            self.extract_call(node);
        } else if kind == "object_creation_expression"
            || (kind == "array_creation_expression" && self.is_vbnet_constructor_shaped(node))
        {
            self.extract_instantiation(node);
        } else if matches!(kind, "class_declaration" | "module_declaration") {
            self.extract_class(node, "class");
            return;
        } else if kind == "structure_declaration" {
            self.extract_struct(node);
            return;
        } else if kind == "enum_declaration" {
            self.extract_enum(node);
            return;
        } else if kind == "interface_declaration" {
            self.extract_interface(node);
            return;
        } else if matches!(
            kind,
            "method_declaration"
                | "external_method_declaration"
                | "interface_method_declaration"
                | "abstract_method_declaration"
                | "constructor_declaration"
        ) {
            let name = self.node_name(node);
            if name != "<anonymous>" && name != "New" {
                self.extract_function(node);
                return;
            }
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_body(child, 0);
        }
    }
}

/// `/^[A-Za-z_]\w*$/` with JS's ASCII `\w` — the return-type gate.
fn is_simple_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
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
        let emit = extract(path, source).expect("vbnet extraction succeeds");
        crate::decode::decode_buffers(&emit, path, bohmian_code_review_core::Language::Vbnet)
            .expect("vbnet decode succeeds")
    }

    #[test]
    /// # Panics
    /// Classes, methods, properties, fields, imports, and enums extract.
    fn class_members_and_imports_extract() {
        let src = "Imports System.Collections.Generic\n\nPublic Class Foo\n    Public Sub Bar()\n        Baz()\n    End Sub\n    Public Property Name As String\n    Private _items As New List(Of String)\nEnd Class\n";
        let result = extract_source("s.vb", src);
        let classes: Vec<&str> = result
            .nodes
            .iter()
            .filter(|n| n.kind == bohmian_code_review_core::NodeKind::Class)
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(classes, vec!["Foo"]);
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Method && n.name == "Bar")
        );
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Property && n.name == "Name")
        );
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Import
                    && n.name == "System.Collections.Generic")
        );
        // The `Baz()` call attributes to Bar.
        let bar = result
            .nodes
            .iter()
            .find(|n| n.name == "Bar")
            .expect("Bar node");
        assert!(result.references.iter().any(|r| {
            r.from_node_id == bar.id
                && r.reference_name == "Baz"
                && r.reference_kind
                    == bohmian_code_review_core::ReferenceKind::Edge(
                        bohmian_code_review_core::EdgeKind::Calls,
                    )
        }));
    }

    #[test]
    /// # Panics
    /// A file without a trailing newline still parses clean (preParse).
    fn trailing_newline_preparse_parses_clean() {
        let src = "Public Class Foo\nEnd Class";
        let result = extract_source("s.vb", src);
        assert_eq!(
            result
                .nodes
                .iter()
                .find(|n| n.kind == bohmian_code_review_core::NodeKind::File)
                .expect("file node")
                .end_line,
            3, // the appended newline makes the file node span 3 lines, like the oracle
        );
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Class && n.name == "Foo")
        );
    }

    #[test]
    /// # Panics
    /// The trailing-newline preprocessing is offset-preserving: a file
    /// without a trailing newline parses cleanly, the appended byte lands
    /// past every emitted range, and the file node's endLine carries the
    /// prepared-source coordinate the frozen oracle emits (the appended
    /// newline makes it 3 for a two-line file).
    fn trailing_newline_preparse_observable_coordinates() {
        let source = "Public Class Foo\nEnd Class";
        let prepared = ensure_trailing_newline(source);
        assert!(prepared.ends_with('\n'));
        // All original content is byte-identical; only a terminal newline is
        // appended.
        assert!(prepared.starts_with(source));
        let result = extract_source("s.vb", source);
        assert_eq!(
            result
                .nodes
                .iter()
                .find(|n| n.kind == bohmian_code_review_core::NodeKind::File)
                .expect("file node")
                .end_line,
            3, // the appended newline makes the file node span 3 lines, like the oracle
        );
        assert!(
            result
                .nodes
                .iter()
                .any(|n| n.kind == bohmian_code_review_core::NodeKind::Class && n.name == "Foo")
        );
    }
}
