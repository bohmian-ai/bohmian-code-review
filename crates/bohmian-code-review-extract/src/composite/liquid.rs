//! Liquid / Shopify extraction — a faithful Rust port of
//! `src/extraction/liquid-extractor.ts`.
//!
//! Liquid templates yield a hashed `file` node plus `import`/`component`
//! node pairs for `{% render %}`, `{% include %}`, and `{% section %}`
//! statements, a `constant` node per `{% schema %}` block (named from the
//! parsed schema JSON's `name`), and a `variable` node per
//! `{% assign %}`. Shopify OS 2.0 JSON templates/sections
//! (`templates/**/*.json`, `sections/**/*.json`) instead emit only the file
//! node and a `references` reference per `sections.<id>.type`, linking each
//! section to its `sections/<type>.liquid` file. Columns are computed from
//! the match offset within its line in UTF-16 code units (upstream
//! `match.index - this.getLineStart(line)`).

use crate::composite::util::{js_re, last_line_utf16_len, line_count, line_of, utf16_len};
use crate::error::ExtractionError;
use crate::result::ExtractionResult;
use bohmian_code_review_core::{
    Edge, EdgeKind, Language, Node, NodeKind, RawReference, ReferenceKind, RelativePath,
};
use regex::Regex;
use std::sync::OnceLock;

/// The `{% render 'name' %}` / `{% include 'name' %}` scanner.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn render_include_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r#"\{\%[-]?\s*(render|include)\s+['"]([^'"]+)['"]"#))
}

/// The `{% section 'name' %}` scanner.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn section_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r#"\{\%[-]?\s*section\s+['"]([^'"]+)['"]"#))
}

/// The `{% schema %}...{% endschema %}` scanner.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn schema_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        js_re(r#"\{\%[-]?\s*schema\s*[-]?%\}([\s\S]*?)\{%[-]?\s*endschema\s*[-]?%\}"#)
    })
}

/// The `{% assign var = ... %}` scanner.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn assign_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"\{\%[-]?\s*assign\s+(\w+)\s*="))
}

/// The owning state of one Liquid file extraction.
struct LiquidWalker<'a> {
    /// Repository-relative path (also the extraction file path).
    path: &'a RelativePath,
    /// Full source text.
    source: &'a str,
    /// The file node id (emitted first).
    file_id: String,
    /// Accumulated emitted facts in upstream order.
    result: ExtractionResult,
}

impl<'a> LiquidWalker<'a> {
    /// Creates the hashed `file` node for the template and pushes it first.
    fn new(path: &'a RelativePath, source: &'a str) -> LiquidWalker<'a> {
        let file_path = path.as_str();
        let id = crate::ids::node_id(file_path, "file", file_path, 1);
        let node = Node {
            id: id.clone(),
            kind: NodeKind::File,
            name: file_path
                .rsplit('/')
                .next()
                .unwrap_or(file_path)
                .to_string(),
            qualified_name: file_path.to_string(),
            file_path: file_path.to_string(),
            language: Language::Liquid,
            start_line: 1,
            end_line: line_count(source),
            start_column: 0,
            end_column: last_line_utf16_len(source),
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
        LiquidWalker {
            path,
            source,
            file_id: id,
            result: ExtractionResult {
                nodes: vec![node],
                edges: Vec::new(),
                references: Vec::new(),
                diagnostics: Vec::new(),
            },
        }
    }

    /// The 0-based column of `byte_index` within its line (upstream
    /// `match.index - this.getLineStart(line)` — UTF-16 code units).
    fn column_in_line(&self, byte_index: usize) -> u32 {
        let line_start = self.source[..byte_index]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        utf16_len(&self.source[line_start..byte_index]) as u32
    }

    /// Push an `import` node + `component` node pair and their contains
    /// edges plus the snippet/section reference (upstream
    /// `extractSnippetReferences` / `extractSectionReferences` shared
    /// shape).
    fn push_reference_pair(
        &mut self,
        tag_type: &str,
        name: &str,
        full_match: &str,
        match_index: usize,
        reference_name: String,
    ) {
        let line = line_of(self.source, match_index);
        let column = self.column_in_line(match_index);
        let end_column = column + utf16_len(full_match) as u32;
        let file_path = self.path.as_str().to_string();
        let import_id = crate::ids::node_id(&file_path, "import", name, line);
        let import_node = Node {
            id: import_id.clone(),
            kind: NodeKind::Import,
            name: name.to_string(),
            qualified_name: format!("{file_path}::import:{name}"),
            file_path: file_path.clone(),
            language: Language::Liquid,
            signature: Some(full_match.to_string()),
            start_line: line,
            end_line: line,
            start_column: column,
            end_column,
            docstring: None,
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
        self.result.nodes.push(import_node);
        self.result.edges.push(Edge {
            source: self.file_id.clone(),
            target: import_id.clone(),
            kind: EdgeKind::Contains,
            metadata: None,
            line: None,
            column: None,
            provenance: None,
        });
        let node_id =
            crate::ids::node_id(&file_path, "component", &format!("{tag_type}:{name}"), line);
        let node = Node {
            id: node_id.clone(),
            kind: NodeKind::Component,
            name: name.to_string(),
            qualified_name: format!("{file_path}::{tag_type}:{name}"),
            file_path: file_path.clone(),
            language: Language::Liquid,
            start_line: line,
            end_line: line,
            start_column: column,
            end_column,
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
        self.result.nodes.push(node);
        self.result.edges.push(Edge {
            source: self.file_id.clone(),
            target: node_id,
            kind: EdgeKind::Contains,
            metadata: None,
            line: None,
            column: None,
            provenance: None,
        });
        self.result.references.push(RawReference {
            from_node_id: self.file_id.clone(),
            reference_name,
            reference_kind: ReferenceKind::Edge(EdgeKind::References),
            line,
            column,
            file_path: None,
            language: None,
            candidates: None,
        });
    }

    /// Extract `{% render %}` / `{% include %}` snippet references.
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn extract_snippet_references(&mut self) {
        for captures in render_include_re().captures_iter(self.source) {
            let whole = captures.get(0).expect("whole match");
            let tag_type = captures.get(1).expect("tag type").as_str();
            let snippet_name = captures.get(2).expect("snippet name").as_str();
            self.push_reference_pair(
                tag_type,
                snippet_name,
                whole.as_str(),
                whole.start(),
                format!("snippets/{snippet_name}.liquid"),
            );
        }
    }

    /// Extract `{% section 'name' %}` references.
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn extract_section_references(&mut self) {
        for captures in section_re().captures_iter(self.source) {
            let whole = captures.get(0).expect("whole match");
            let section_name = captures.get(1).expect("section name").as_str();
            self.push_reference_pair(
                "section",
                section_name,
                whole.as_str(),
                whole.start(),
                format!("sections/{section_name}.liquid"),
            );
        }
    }

    /// Extract `{% schema %}` blocks.
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn extract_schema(&mut self) {
        for captures in schema_re().captures_iter(self.source) {
            let whole = captures.get(0).expect("whole match");
            let schema_content = captures.get(1).expect("schema content").as_str();
            let start_line = line_of(self.source, whole.start());
            let end_line = line_of(self.source, whole.start() + whole.as_str().len());
            let mut schema_name = "schema".to_string();
            if let Ok(schema_json) = serde_json::from_str::<serde_json::Value>(schema_content)
                && let Some(name) = schema_json.get("name")
            {
                if let Some(name_str) = name.as_str() {
                    schema_name = name_str.to_string();
                } else if let Some(obj) = name.as_object()
                    && let Some(en) = obj.get("en").and_then(|v| v.as_str())
                {
                    schema_name = en.to_string();
                } else if let Some(obj) = name.as_object()
                    && let Some(first) = obj.values().next().and_then(|v| v.as_str())
                {
                    schema_name = first.to_string();
                }
            }
            let file_path = self.path.as_str().to_string();
            let qualified = format!("{file_path}::schema:{schema_name}");
            let node_id = crate::ids::node_id(
                &file_path,
                "constant",
                &format!("schema:{schema_name}"),
                start_line,
            );
            let node = Node {
                id: node_id.clone(),
                kind: NodeKind::Constant,
                name: schema_name,
                qualified_name: qualified,
                file_path: file_path.clone(),
                language: Language::Liquid,
                start_line,
                end_line,
                start_column: 0,
                end_column: 0,
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
            self.result.nodes.push(node);
            self.result.edges.push(Edge {
                source: self.file_id.clone(),
                target: node_id,
                kind: EdgeKind::Contains,
                metadata: None,
                line: None,
                column: None,
                provenance: None,
            });
        }
    }

    /// Extract `{% assign var = value %}` statements.
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn extract_assignments(&mut self) {
        for captures in assign_re().captures_iter(self.source) {
            let whole = captures.get(0).expect("whole match");
            let variable_name = captures.get(1).expect("variable name").as_str();
            let line = line_of(self.source, whole.start());
            let column = self.column_in_line(whole.start());
            let file_path = self.path.as_str().to_string();
            let node_id = crate::ids::node_id(&file_path, "variable", variable_name, line);
            let node = Node {
                id: node_id.clone(),
                kind: NodeKind::Variable,
                name: variable_name.to_string(),
                qualified_name: format!("{file_path}::{variable_name}"),
                file_path: file_path.clone(),
                language: Language::Liquid,
                start_line: line,
                end_line: line,
                start_column: column,
                end_column: column + utf16_len(whole.as_str()) as u32,
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
            self.result.nodes.push(node);
            self.result.edges.push(Edge {
                source: self.file_id.clone(),
                target: node_id,
                kind: EdgeKind::Contains,
                metadata: None,
                line: None,
                column: None,
                provenance: None,
            });
        }
    }

    /// Extract Shopify OS 2.0 JSON section references (upstream
    /// `extractShopifyJsonSections`).
    fn extract_shopify_json_sections(&mut self) {
        let parsed: serde_json::Value = match serde_json::from_str(self.source) {
            Ok(value) => value,
            Err(_) => return, // not valid JSON (or a partial) — nothing to link
        };
        let Some(sections) = parsed.get("sections").and_then(|v| v.as_object()) else {
            return;
        };
        let mut seen = std::collections::BTreeSet::new();
        for (_, entry) in sections {
            let Some(type_name) = entry.get("type").and_then(|v| v.as_str()) else {
                continue;
            };
            if !seen.insert(type_name.to_string()) {
                continue;
            }
            self.result.references.push(RawReference {
                from_node_id: self.file_id.clone(),
                reference_name: format!("sections/{type_name}.liquid"),
                reference_kind: ReferenceKind::Edge(EdgeKind::References),
                line: 1,
                column: 0,
                file_path: None,
                language: None,
                candidates: None,
            });
        }
    }
}

/// Extract Liquid facts from a `.liquid` template or Shopify JSON file.
///
/// The upstream JSON branch (`filePath.endsWith('.json')`) emits only the
/// file node and section references; every other branch runs the
/// render/include, section, schema, and assign scans in upstream order.
/// # Errors
///
/// Returns the documented typed error.
pub fn extract(
    _native: &crate::extractor::Extractor,
    path: &RelativePath,
    source: &str,
) -> Result<ExtractionResult, ExtractionError> {
    let mut walker = LiquidWalker::new(path, source);
    if path.as_str().ends_with(".json") {
        walker.extract_shopify_json_sections();
    } else {
        walker.extract_snippet_references();
        walker.extract_section_references();
        walker.extract_schema();
        walker.extract_assignments();
    }
    Ok(walker.result)
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;
    use crate::config::ExtractionConfig;
    use crate::extractor::Extractor;

    /// Builds a native extractor over the full native set.
    ///
    /// # Panics
    ///
    /// Panics if extractor construction fails.
    fn native() -> Extractor {
        Extractor::new(ExtractionConfig::all_native()).expect("extractor builds")
    }

    /// Extracts a liquid fixture by path and source.
    ///
    /// # Panics
    ///
    /// Panics if the path is invalid or extraction fails.
    fn extract_src(file_path: &str, source: &str) -> ExtractionResult {
        let path = RelativePath::new(file_path).expect("valid path");
        extract(&native(), &path, source).expect("liquid extraction succeeds")
    }

    #[test]
    /// Render/include/section pairs, schema constant, and assign variable
    /// are emitted in upstream order.
    fn template_facts_in_upstream_order() {
        let source = "{% section 'header' %}\n{% render 'card', x: 1 %}\n{% assign total = 0 %}\n{% schema %}\n{\"name\": \"Product\"}\n{% endschema %}\n";
        let result = extract_src("t.liquid", source);
        assert_eq!(result.nodes.len(), 7);
        assert_eq!(result.nodes[0].kind, NodeKind::File);
        assert_eq!(result.nodes[1].name, "card");
        assert_eq!(result.nodes[1].qualified_name, "t.liquid::import:card");
        assert_eq!(result.nodes[2].name, "card");
        assert_eq!(result.nodes[2].qualified_name, "t.liquid::render:card");
        assert_eq!(result.nodes[3].name, "header");
        assert_eq!(result.nodes[4].name, "header");
        assert_eq!(result.nodes[5].kind, NodeKind::Constant);
        assert_eq!(result.nodes[5].name, "Product");
        assert_eq!(result.nodes[6].kind, NodeKind::Variable);
        assert_eq!(result.nodes[6].name, "total");
        assert!(
            result
                .references
                .iter()
                .any(|r| r.reference_name == "snippets/card.liquid")
        );
        assert!(
            result
                .references
                .iter()
                .any(|r| r.reference_name == "sections/header.liquid")
        );
    }

    #[test]
    /// Shopify JSON templates emit section references keyed by `type`.
    fn shopify_json_sections() {
        let source = r#"{"sections":{"a":{"type":"product-main"},"b":{"type":"product-main"},"c":{"type":"related"}}}"#;
        let result = extract_src("templates/product.json", source);
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].kind, NodeKind::File);
        let names: Vec<&str> = result
            .references
            .iter()
            .map(|r| r.reference_name.as_str())
            .collect();
        assert_eq!(
            names,
            vec!["sections/product-main.liquid", "sections/related.liquid"]
        );
    }
}
