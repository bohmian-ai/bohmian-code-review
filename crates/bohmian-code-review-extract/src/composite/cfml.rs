//! CFML extraction — a faithful Rust port of `src/extraction/cfml-extractor.ts`.
//!
//! `.cfc`/`.cfm`/`.cfs` files dialect-switch between two grammars. A file
//! whose first real token is not `<` is bare-script CFML (modern style) and
//! is delegated wholesale to the native CFScript extractor with the
//! `<anonymous>` component rename and per-node/per-ref language stamp
//! (upstream `extractBareScript`). Otherwise the file is walked tag-by-tag
//! with the vendored `cfml` grammar: `<cfcomponent>`/`<cffunction>` become
//! class/method nodes with `extends`/`implements` refs, and every
//! `<cfscript>`/`<cfquery>` body is delegated to the native CFScript/CFQuery
//! extractors with its coordinates remapped to the containing file
//! (upstream `extractTagBased` and friends). The grammar revision is the
//! same cfmleditor/tree-sitter-cfml a224ff1 commit as the vendored
//! cfscript/cfquery grammars.

use crate::composite::util::{last_line_utf16_len, line_count};
use crate::error::ExtractionError;
use crate::extractor::Extractor;
use crate::result::ExtractionResult;
use crate::source_file::SourceFile;
use crate::textutil::{col16, line_starts};
use bohmian_code_review_core::{
    Edge, EdgeKind, ExtractionDiagnostic, Language, Node, NodeKind, RawReference, ReferenceKind,
    RelativePath, Severity, Visibility,
};
use std::collections::HashSet;
use tree_sitter::{Node as SyntaxNode, Parser};

/// One attribute value scan: the `cf_attribute` node types and the
/// `cf_attribute_name`/`attribute_value` leaf types are matched by kind
/// string in [`tag_attr`].
const CF_SCRIPT_CONTENT: &str = "cf_script_content";
/// The `<cfquery>` content node kind name (delegated to the CFQuery grammar).
const CF_QUERY_CONTENT: &str = "cf_query_content";

/// The owning state of one CFML file extraction.
struct CfmlWalker<'a> {
    /// Borrowed shared native extractor for script/query delegation.
    native: &'a Extractor,
    /// Repository-relative path (also the extraction file path).
    path: &'a RelativePath,
    /// Full source text.
    source: &'a str,
    /// The file's detected language (`cfml` for `.cfc`/`.cfm`, `cfscript`
    /// for `.cfs`) — stamped onto emitted nodes and refs.
    language: Language,
    /// Byte offsets of each line start, for UTF-16 column conversion.
    line_starts: Vec<usize>,
    /// Accumulated emitted facts in upstream order.
    result: ExtractionResult,
}

impl<'a> CfmlWalker<'a> {
    /// Construct the walker over a CFML source file.
    fn new(
        native: &'a Extractor,
        path: &'a RelativePath,
        source: &'a str,
        language: Language,
    ) -> CfmlWalker<'a> {
        CfmlWalker {
            native,
            path,
            source,
            language,
            line_starts: line_starts(source),
            result: ExtractionResult {
                nodes: Vec::new(),
                edges: Vec::new(),
                references: Vec::new(),
                diagnostics: Vec::new(),
            },
        }
    }

    /// Push an error diagnostic with the upstream `parse_error` spelling.
    fn push_error(&mut self, message: String) {
        self.result.diagnostics.push(ExtractionDiagnostic {
            message,
            file_path: None,
            line: None,
            column: None,
            severity: Severity::Error,
            code: Some("parse_error".to_string()),
        });
    }

    /// The file-derived component name (`Foo.cfc` → `Foo`).
    fn component_name_from_path(&self) -> String {
        let file_name = self
            .path
            .as_str()
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(self.path.as_str());
        let mut stripped = file_name.to_string();
        for ext in [".cfc", ".cfm", ".cfs"] {
            if stripped.len() >= ext.len()
                && stripped[stripped.len() - ext.len()..].eq_ignore_ascii_case(ext)
            {
                stripped.truncate(stripped.len() - ext.len());
                break;
            }
        }
        stripped
    }

    /// The tag-based path: parse with the vendored cfml grammar and walk
    /// the tree (upstream `extractTagBased`). Failures are recorded as
    /// diagnostics, never propagated.
    fn extract_tag_based(&mut self) {
        let Some(grammar) = crate::langs::grammar_for("cfml") else {
            self.result.diagnostics.push(ExtractionDiagnostic {
                message: "cfml grammar not loaded".to_string(),
                file_path: None,
                line: None,
                column: None,
                severity: Severity::Error,
                code: Some("unsupported_language".to_string()),
            });
            return;
        };
        let mut parser = Parser::new();
        if parser.set_language(&grammar).is_err() {
            self.result.diagnostics.push(ExtractionDiagnostic {
                message: "cfml grammar not loaded".to_string(),
                file_path: None,
                line: None,
                column: None,
                severity: Severity::Error,
                code: Some("unsupported_language".to_string()),
            });
            return;
        }
        let Some(tree) = parser.parse(self.source, None) else {
            self.result.diagnostics.push(ExtractionDiagnostic {
                message: "Failed to parse CFML source".to_string(),
                file_path: None,
                line: None,
                column: None,
                severity: Severity::Error,
                code: Some("parse_error".to_string()),
            });
            return;
        };
        let file_node_id = self.create_file_node();
        self.walk_program(tree.root_node(), &file_node_id);
    }

    /// Modern bare-script `.cfc`/`.cfm`: delegate the whole file to the
    /// native CFScript extractor (upstream `extractBareScript`).
    ///
    /// # Errors
    ///
    /// Returns the native extractor's typed error when the file fails
    /// native extraction.
    fn extract_bare_script(&mut self) -> Result<(), ExtractionError> {
        let inner = self.native.extract(SourceFile::new(
            self.path,
            self.source.as_bytes(),
            Language::Cfscript,
        ))?;
        let component_name = self.component_name_from_path();
        let file_path = self.path.as_str().to_string();
        for mut node in inner.nodes {
            node.language = self.language;
            if node.name == "<anonymous>"
                && (node.kind == NodeKind::Class || node.kind == NodeKind::Interface)
            {
                node.name = component_name.clone();
                node.qualified_name = format!("{file_path}::{component_name}");
            } else if node.qualified_name == "<anonymous>"
                || node.qualified_name.starts_with("<anonymous>::")
            {
                node.qualified_name = format!(
                    "{component_name}{}",
                    &node.qualified_name["<anonymous>".len()..]
                );
            }
            self.result.nodes.push(node);
        }
        self.result.edges.extend(inner.edges);
        for mut reference in inner.references {
            reference.language = Some(self.language);
            self.result.references.push(reference);
        }
        self.result.diagnostics.extend(inner.diagnostics);
        Ok(())
    }

    /// The tag-based file's own `file`-kind node (upstream
    /// `createFileNode`).
    fn create_file_node(&mut self) -> String {
        let file_path = self.path.as_str();
        let id = crate::ids::node_id(file_path, "file", file_path, 1);
        let node = Node {
            id: id.clone(),
            kind: NodeKind::File,
            name: file_path
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(file_path)
                .to_string(),
            qualified_name: file_path.to_string(),
            file_path: file_path.to_string(),
            language: self.language,
            start_line: 1,
            end_line: line_count(self.source),
            start_column: 0,
            end_column: last_line_utf16_len(self.source),
            docstring: None,
            signature: None,
            visibility: None,
            is_exported: None,
            is_async: None,
            is_static: None,
            is_abstract: None,
            decorators: None,
            type_parameters: None,
            return_type: None,
            extra: None,
        };
        let id_for_return = id.clone();
        self.result.nodes.push(node);
        id_for_return
    }

    /// Walk `program`'s children with a single forward cursor — upstream
    /// `walkProgram` (extractComponent consumes a variable run of following
    /// siblings, so the walk resumes from whatever it last consumed).
    fn walk_program(&mut self, root: SyntaxNode, file_node_id: &str) {
        let mut child = root.named_child(0);
        while let Some(node) = child {
            match node.kind() {
                "cf_component_open_tag" => {
                    child = self.extract_component(node, file_node_id).next_sibling();
                    continue;
                }
                "cf_function_tag" => {
                    self.extract_function_tag(node, None, Some(file_node_id), None);
                }
                "cf_script_tag" => {
                    self.delegate_script_tag(node, Some(file_node_id), None);
                }
                "cf_query_tag" => {
                    self.delegate_query_tag(node, Some(file_node_id));
                }
                _ => {
                    self.delegate_nested_tags(node, Some(file_node_id), None);
                }
            }
            child = node.next_sibling();
        }
    }

    /// Extract `<cfcomponent>` (upstream `extractComponent`). The returned
    /// close-tag node's lifetime is the parse tree's, not `self`'s, so the
    /// program walk can resume from it across method calls.
    fn extract_component<'t>(
        &mut self,
        open_tag: SyntaxNode<'t>,
        container_id: &str,
    ) -> SyntaxNode<'t> {
        let file_path = self.path.as_str().to_string();
        let name = self
            .tag_attr(open_tag, "name")
            .unwrap_or_else(|| self.component_name_from_path());
        let start_row = open_tag.start_position().row;
        let start_column = col16(
            self.source,
            &self.line_starts,
            start_row,
            open_tag.start_byte(),
        );
        let end_column = col16(
            self.source,
            &self.line_starts,
            start_row,
            open_tag.end_byte(),
        );
        let id = crate::ids::node_id(&file_path, "class", &name, start_row as u32 + 1);
        let class_node = Node {
            id: id.clone(),
            kind: NodeKind::Class,
            name: name.clone(),
            qualified_name: format!("{file_path}::{name}"),
            file_path: file_path.clone(),
            language: self.language,
            start_line: start_row as u32 + 1,
            end_line: start_row as u32 + 1, // extended once the close tag is found
            start_column,
            end_column,
            docstring: None,
            signature: None,
            visibility: None,
            is_exported: Some(true),
            is_async: None,
            is_static: None,
            is_abstract: None,
            decorators: None,
            type_parameters: None,
            return_type: None,
            extra: None,
        };
        self.result.nodes.push(class_node);
        if !container_id.is_empty() {
            self.result.edges.push(Edge {
                source: container_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                metadata: None,
                line: None,
                column: None,
                provenance: None,
            });
        }

        let row = start_row as u32 + 1;
        let column = start_column;
        if let Some(extends_name) = self.tag_attr(open_tag, "extends").filter(|n| !n.is_empty()) {
            self.result.references.push(RawReference {
                from_node_id: id.clone(),
                reference_name: extends_name,
                reference_kind: ReferenceKind::Edge(EdgeKind::Extends),
                file_path: Some(file_path.clone()),
                line: row,
                column,
                language: Some(self.language),
                candidates: None,
            });
        }
        if let Some(implements_attr) = self.tag_attr(open_tag, "implements") {
            for iface in implements_attr
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                self.result.references.push(RawReference {
                    from_node_id: id.clone(),
                    reference_name: iface.to_string(),
                    reference_kind: ReferenceKind::Edge(EdgeKind::Implements),
                    file_path: Some(file_path.clone()),
                    line: row,
                    column,
                    language: Some(self.language),
                    candidates: None,
                });
            }
        }

        // Walk siblings between the open tag and its close tag.
        let mut sibling = open_tag.next_sibling();
        let mut last_node = open_tag;
        while let Some(node) = sibling {
            if node.kind() == "cf_component_close_tag" {
                last_node = node;
                break;
            }
            match node.kind() {
                "cf_function_tag" => {
                    self.extract_function_tag(node, Some(&id), Some(&id), Some(&name));
                }
                "cf_script_tag" => {
                    self.delegate_script_tag(node, Some(&id), Some(&name));
                }
                "cf_query_tag" => {
                    self.delegate_query_tag(node, Some(&id));
                }
                _ => {
                    self.delegate_nested_tags(node, Some(&id), Some(&name));
                }
            }
            last_node = node;
            sibling = node.next_sibling();
        }
        if let Some(class_node) = self.result.nodes.iter_mut().find(|n| n.id == id) {
            class_node.end_line = last_node.end_position().row as u32 + 1;
        }
        last_node
    }

    /// Extract `<cffunction>` (upstream `extractFunctionTag`).
    fn extract_function_tag(
        &mut self,
        tag: SyntaxNode,
        parent_class_id: Option<&str>,
        container_id: Option<&str>,
        parent_class_name: Option<&str>,
    ) {
        let Some(name) = self.tag_attr(tag, "name").filter(|n| !n.is_empty()) else {
            return;
        };
        let kind = if parent_class_id.is_some() {
            NodeKind::Method
        } else {
            NodeKind::Function
        };
        let file_path = self.path.as_str().to_string();
        let start_row = tag.start_position().row;
        let id = crate::ids::node_id(&file_path, kind.wire_name(), &name, start_row as u32 + 1);
        let access = self.tag_attr(tag, "access");
        let visibility = match access.as_deref() {
            Some("private") => Some(Visibility::Private),
            Some("package") => Some(Visibility::Internal),
            Some(_) => Some(Visibility::Public),
            None => None,
        };
        let qualified_name = match parent_class_name {
            Some(class_name) => format!("{class_name}::{name}"),
            None => format!("{file_path}::{name}"),
        };
        let fn_node = Node {
            id: id.clone(),
            kind,
            name,
            qualified_name,
            file_path: file_path.clone(),
            language: self.language,
            start_line: start_row as u32 + 1,
            end_line: tag.end_position().row as u32 + 1,
            start_column: col16(self.source, &self.line_starts, start_row, tag.start_byte()),
            end_column: col16(
                self.source,
                &self.line_starts,
                tag.end_position().row,
                tag.end_byte(),
            ),
            docstring: None,
            signature: None,
            visibility,
            is_exported: None,
            is_async: None,
            is_static: None,
            is_abstract: None,
            decorators: None,
            type_parameters: None,
            return_type: self.tag_attr(tag, "returntype"),
            extra: None,
        };
        self.result.nodes.push(fn_node);
        if let Some(container_id) = container_id {
            self.result.edges.push(Edge {
                source: container_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                metadata: None,
                line: None,
                column: None,
                provenance: None,
            });
        }
        self.delegate_nested_tags(tag, Some(&id), None);
    }

    /// Recursively delegate `<cfscript>`/`<cfquery>` found within `node`'s
    /// subtree — upstream `delegateNestedTags` (does not descend into a
    /// nested `cf_function_tag`).
    fn delegate_nested_tags(
        &mut self,
        node: SyntaxNode,
        container_id: Option<&str>,
        parent_class_name: Option<&str>,
    ) {
        for i in 0..node.named_child_count() {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            match child.kind() {
                "cf_script_tag" => {
                    self.delegate_script_tag(child, container_id, parent_class_name);
                }
                "cf_query_tag" => {
                    self.delegate_query_tag(child, container_id);
                }
                "cf_function_tag" => {
                    continue;
                }
                _ => {
                    self.delegate_nested_tags(child, container_id, parent_class_name);
                }
            }
        }
    }

    /// Delegate a `<cfscript>` tag body to the native CFScript extractor
    /// (upstream `delegateScriptTag`). A native failure is recorded as a
    /// diagnostic, matching the upstream try/catch recovery.
    fn delegate_script_tag(
        &mut self,
        script_tag: SyntaxNode,
        parent_id: Option<&str>,
        parent_class_name: Option<&str>,
    ) {
        let content = (0..script_tag.named_child_count()).find_map(|i| {
            script_tag
                .named_child(i)
                .filter(|child| child.kind() == CF_SCRIPT_CONTENT)
        });
        let Some(content) = content else {
            return;
        };
        let inner = self.source[content.start_byte()..content.end_byte()].to_string();
        let start_line = content.start_position().row as u32;

        let inner_result = self.native.extract(SourceFile::new(
            self.path,
            inner.as_bytes(),
            Language::Cfscript,
        ));
        match inner_result {
            Ok(inner_result) => {
                self.merge_delegated(inner_result, start_line, parent_id, parent_class_name);
            }
            Err(error) => self.push_error(format!("CFML script delegation failed: {error}")),
        }
    }

    /// Merge one delegated native result into this walker with the upstream
    /// offset/kind/qualified-name/ref-redirect rules shared by the
    /// `<cfscript>` delegation paths.
    fn merge_delegated(
        &mut self,
        inner: ExtractionResult,
        start_line: u32,
        parent_id: Option<&str>,
        parent_class_name: Option<&str>,
    ) {
        let file_path = self.path.as_str().to_string();
        let inner_file_node_id = inner
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::File)
            .map(|n| n.id.clone());
        let top_level_ids: HashSet<String> = inner
            .edges
            .iter()
            .filter(|e| {
                e.kind == EdgeKind::Contains && Some(&e.source) == inner_file_node_id.as_ref()
            })
            .map(|e| e.target.clone())
            .collect();
        for mut node in inner.nodes {
            if node.kind == NodeKind::File {
                continue;
            }
            node.start_line += start_line;
            node.end_line += start_line;
            node.language = self.language;
            if let Some(class_name) = parent_class_name {
                if node.kind == NodeKind::Function && top_level_ids.contains(&node.id) {
                    node.kind = NodeKind::Method;
                }
                node.qualified_name = format!("{class_name}::{}", node.qualified_name);
            }
            let id = node.id.clone();
            self.result.nodes.push(node);
            if let Some(parent_id) = parent_id {
                self.result.edges.push(Edge {
                    source: parent_id.to_string(),
                    target: id,
                    kind: EdgeKind::Contains,
                    metadata: None,
                    line: None,
                    column: None,
                    provenance: None,
                });
            }
        }
        for mut edge in inner.edges {
            if let Some(inner_file_id) = &inner_file_node_id
                && (edge.source == *inner_file_id || edge.target == *inner_file_id)
            {
                continue;
            }
            if let Some(line) = edge.line {
                edge.line = Some(line + start_line);
            }
            self.result.edges.push(edge);
        }
        for mut reference in inner.references {
            reference.line += start_line;
            reference.file_path = Some(file_path.clone());
            reference.language = Some(self.language);
            if (reference.from_node_id.is_empty()
                || Some(&reference.from_node_id) == inner_file_node_id.as_ref())
                && let Some(parent_id) = parent_id
            {
                reference.from_node_id = parent_id.to_string();
            }
            self.result.references.push(reference);
        }
        for mut diagnostic in inner.diagnostics {
            if let Some(line) = diagnostic.line {
                diagnostic.line = Some(line + start_line);
            }
            self.result.diagnostics.push(diagnostic);
        }
    }

    /// Delegate a `<cfquery>` tag's SQL body to the native CFQuery
    /// extractor (upstream `delegateQueryTag`). A native failure is recorded
    /// as a diagnostic, matching the upstream try/catch recovery.
    fn delegate_query_tag(&mut self, query_tag: SyntaxNode, parent_id: Option<&str>) {
        let content = (0..query_tag.named_child_count()).find_map(|i| {
            query_tag
                .named_child(i)
                .filter(|child| child.kind() == CF_QUERY_CONTENT)
        });
        let Some(content) = content else {
            return;
        };
        let sql = self.source[content.start_byte()..content.end_byte()].to_string();
        let start_line = content.start_position().row as u32;

        let inner_result = self.native.extract(SourceFile::new(
            self.path,
            sql.as_bytes(),
            Language::Cfquery,
        ));
        let inner_result = match inner_result {
            Ok(inner_result) => inner_result,
            Err(error) => {
                self.push_error(format!("CFML query delegation failed: {error}"));
                return;
            }
        };
        let file_path = self.path.as_str().to_string();
        let inner_file_node_id = inner_result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::File)
            .map(|n| n.id.clone());
        for mut reference in inner_result.references {
            reference.line += start_line;
            reference.file_path = Some(file_path.clone());
            reference.language = Some(self.language);
            if (reference.from_node_id.is_empty()
                || Some(&reference.from_node_id) == inner_file_node_id.as_ref())
                && let Some(parent_id) = parent_id
            {
                reference.from_node_id = parent_id.to_string();
            }
            self.result.references.push(reference);
        }
        for mut diagnostic in inner_result.diagnostics {
            if let Some(line) = diagnostic.line {
                diagnostic.line = Some(line + start_line);
            }
            self.result.diagnostics.push(diagnostic);
        }
    }

    /// Read a `cf_attribute`'s value by name (upstream `tagAttr`).
    fn tag_attr(&self, tag: SyntaxNode, attr_name: &str) -> Option<String> {
        let mut attrs: Vec<SyntaxNode> = Vec::new();
        for i in 0..tag.named_child_count() {
            let Some(child) = tag.named_child(i) else {
                continue;
            };
            if child.kind() == "cf_attribute" {
                attrs.push(child);
            } else if child.kind() == "cf_tag_attributes" {
                for j in 0..child.named_child_count() {
                    if let Some(inner) = child.named_child(j)
                        && inner.kind() == "cf_attribute"
                    {
                        attrs.push(inner);
                    }
                }
            }
        }
        for attr in attrs {
            let name_node = (0..attr.named_child_count()).find_map(|i| {
                attr.named_child(i)
                    .filter(|child| child.kind() == "cf_attribute_name")
            });
            let Some(name_node) = name_node else {
                continue;
            };
            let text = &self.source[name_node.start_byte()..name_node.end_byte()];
            if !text.eq_ignore_ascii_case(attr_name) {
                continue;
            }
            let value_wrapper = (0..attr.named_child_count()).find_map(|i| {
                attr.named_child(i).filter(|child| {
                    child.kind() == "quoted_cf_attribute_value"
                        || child.kind() == "cf_attribute_value"
                })
            });
            let value_wrapper = value_wrapper?;
            let value_node = (0..value_wrapper.named_child_count()).find_map(|i| {
                value_wrapper
                    .named_child(i)
                    .filter(|child| child.kind() == "attribute_value")
            });
            let Some(value_node) = value_node else {
                return Some(String::new());
            };
            return Some(self.source[value_node.start_byte()..value_node.end_byte()].to_string());
        }
        None
    }
}

/// Sniff whether CFML source is bare-script vs tag-based (upstream
/// `isBareScriptCfml`): skip a leading UTF-8 BOM, whitespace, and
/// `//`/`/* */` comments; tag-based files start with `<`.
fn is_bare_script_cfml(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b'\n' | b'\r' | 0xEF => {
                // 0xEF starts the UTF-8 BOM (EF BB BF); treat the full BOM
                // as skippable whitespace exactly like upstream's `\uFEFF`.
                if bytes[i] == 0xEF
                    && bytes.get(i + 1) == Some(&0xBB)
                    && bytes.get(i + 2) == Some(&0xBF)
                {
                    i += 3;
                } else {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => match source[i..].find('\n') {
                Some(newline) => i += newline + 1,
                None => i = bytes.len(),
            },
            b'/' if bytes.get(i + 1) == Some(&b'*') => match source[i + 2..].find("*/") {
                Some(end) => i += 2 + end + 2,
                None => i = bytes.len(),
            },
            _ => return bytes[i] != b'<',
        }
    }
    true // empty/whitespace-only file — treat as script (no-op extraction either way)
}

/// Extract CFML facts from a `.cfc`/`.cfm`/`.cfs` file.
///
/// `language` is the file's detected language — `cfml` for `.cfc`/`.cfm`,
/// `cfscript` for `.cfs`; both dialect-switch internally and this only
/// controls the language tag stamped onto emitted nodes and references.
///
/// # Errors
///
/// Returns the native extractor's typed error when a delegated
/// bare-script/`<cfscript>`/`<cfquery>` block fails native extraction.
/// Tag-parse failures are recorded as diagnostics (upstream behavior),
/// never as errors.
pub fn extract(
    native: &Extractor,
    path: &RelativePath,
    source: &str,
    language: Language,
) -> Result<ExtractionResult, ExtractionError> {
    let mut walker = CfmlWalker::new(native, path, source, language);
    if is_bare_script_cfml(source) {
        walker.extract_bare_script()?;
    } else {
        walker.extract_tag_based();
    }
    Ok(walker.result)
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;
    use crate::config::ExtractionConfig;

    /// Builds a native extractor over the full native set.
    ///
    /// # Panics
    ///
    /// Panics if extractor construction fails.
    fn native() -> Extractor {
        Extractor::new(ExtractionConfig::all_native()).expect("extractor builds")
    }

    /// Extracts a cfml fixture by path and source.
    ///
    /// # Panics
    ///
    /// Panics if the path is invalid or extraction fails.
    fn extract_src(file_path: &str, source: &str) -> ExtractionResult {
        let path = RelativePath::new(file_path).expect("valid path");
        extract(&native(), &path, source, Language::Cfml).expect("cfml extraction succeeds")
    }

    #[test]
    /// Bare-script CFCs rename the anonymous component to the file name.
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn bare_script_anonymous_rename() {
        let source = "component extends=\"Base\" {\n\tfunction go() {}\n}\n";
        let result = extract_src("Svc.cfc", source);
        let class = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Class)
            .expect("class");
        assert_eq!(class.name, "Svc");
        assert_eq!(class.qualified_name, "Svc.cfc::Svc");
        assert!(result.references.iter().any(|r| r.reference_name == "Base"
            && r.reference_kind == ReferenceKind::Edge(EdgeKind::Extends)));
        for reference in &result.references {
            assert_eq!(reference.language, Some(Language::Cfml));
        }
    }

    #[test]
    /// Tag-based CFCs emit a hashed file node, class/method nodes, and
    /// extends/implements refs; `<cfscript>` bodies are delegated.
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn tag_based_component_walk() {
        let source = "<cfcomponent extends=\"Base\" implements=\"IUser\">\n  <cffunction name=\"save\" access=\"public\" returntype=\"User\">\n    <cfscript>\n      var svc = new UserDao();\n      return svc.save(user);\n    </cfscript>\n  </cffunction>\n</cfcomponent>\n";
        let result = extract_src("UserService.cfc", source);
        assert_eq!(result.nodes[0].kind, NodeKind::File);
        assert!(
            !result.nodes[0].id.starts_with("file:UserService.cfc"),
            "tag-based file node uses the hashed id, not the file: special"
        );
        let class = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Class)
            .expect("class");
        assert_eq!(class.name, "UserService");
        assert_eq!(class.qualified_name, "UserService.cfc::UserService");
        assert_eq!(class.end_line, 8);
        let method = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Method)
            .expect("method");
        assert_eq!(method.name, "save");
        assert_eq!(method.qualified_name, "UserService::save");
        assert_eq!(method.return_type.as_deref(), Some("User"));
        assert_eq!(method.visibility, Some(Visibility::Public));
        let variable = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Variable)
            .expect("variable");
        assert_eq!(variable.name, "svc");
        assert_eq!(variable.start_line, 4);
        assert!(
            result
                .references
                .iter()
                .any(|r| r.reference_name == "UserDao"
                    && r.reference_kind == ReferenceKind::Edge(EdgeKind::Instantiates)
                    && r.line == 4)
        );
        assert!(
            result
                .references
                .iter()
                .any(|r| r.reference_name == "svc.save"
                    && r.reference_kind == ReferenceKind::Edge(EdgeKind::Calls)
                    && r.line == 5)
        );
        for reference in &result.references {
            assert_eq!(reference.file_path.as_deref(), Some("UserService.cfc"));
            assert_eq!(reference.language, Some(Language::Cfml));
        }
    }

    #[test]
    /// The bare-script/tag-based sniff matches upstream's first-token rule.
    fn bare_script_sniffing() {
        assert!(!is_bare_script_cfml("<cfcomponent>"));
        assert!(!is_bare_script_cfml("  \n\t<cfif x>"));
        assert!(is_bare_script_cfml("component {}"));
        assert!(is_bare_script_cfml("// comment\ncomponent {}"));
        assert!(is_bare_script_cfml("/* c */ component {}"));
        assert!(is_bare_script_cfml("\u{FEFF}component {}"));
        assert!(is_bare_script_cfml(""));
    }
}
