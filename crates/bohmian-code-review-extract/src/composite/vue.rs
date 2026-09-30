//! Vue extraction — a faithful Rust port of `src/extraction/vue-extractor.ts`.
//!
//! A `.vue` single-file component yields one `component` node, then every
//! `<script>`/`<script setup>` block is delegated to the native
//! TypeScript/JavaScript extractor with line numbers remapped to the
//! containing file. Template component usages become `references`
//! references: PascalCase tags pass through, kebab-case tags convert to
//! PascalCase (`my-button` → `MyButton`), Vue built-in components
//! (`Transition`, `KeepAlive`, …) and lowercase native HTML elements are
//! skipped, and `<script>`/`<style>` line ranges are excluded from the scan.

use crate::composite::util::{
    count_newlines, js_re, last_line_utf16_len, line_count, utf16_col_in_line, zero_indexed_line,
};
use crate::error::ExtractionError;
use crate::extractor::Extractor;
use crate::result::ExtractionResult;
use crate::source_file::SourceFile;
use bohmian_code_review_core::{
    Edge, EdgeKind, Language, Node, NodeKind, RawReference, ReferenceKind, RelativePath,
};
use regex::Regex;
use std::sync::OnceLock;

/// Vue built-in components — skipped so a `<Transition>`/`<KeepAlive>` in the
/// template doesn't become a phantom reference.
const VUE_BUILTIN_COMPONENTS: [&str; 7] = [
    "Transition",
    "TransitionGroup",
    "KeepAlive",
    "Suspense",
    "Teleport",
    "Component",
    "Slot",
];

/// `my-component` → `MyComponent` (upstream `kebabToPascal`).
fn kebab_to_pascal(name: &str) -> String {
    name.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = first.to_uppercase().collect::<String>();
                    out.push_str(chars.as_str());
                    out
                }
                None => String::new(),
            }
        })
        .collect()
}

/// The `<script ...>...</script>` block scanner.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn script_block_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"<script(\s[^>]*)?>(?P<content>[\s\S]*?)</script>"))
}

/// The `lang="ts"` attribute detector.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn ts_lang_attr_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r#"lang\s*=\s*["'](ts|typescript)["']"#))
}

/// The `<script ...>`/`<style ...>` line-range scanner (two regexes because
/// Rust regexes have no backreferences).
fn script_or_style_range_re(open: &str) -> Regex {
    js_re(&format!(r"<{open}(\s[^>]*)?>[\s\S]*?</{open}>"))
}

/// The template tag scanner `/<([A-Za-z][A-Za-z0-9_-]*)\b/g`.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn tag_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"<([A-Za-z][A-Za-z0-9_-]*)\b"))
}

/// One extracted `<script>` block.
struct ScriptBlock {
    /// The block content (starts right after the opening tag's `>`).
    content: String,
    /// The 0-indexed line where the content begins.
    start_line: u32,
    /// Whether the block declares `lang="ts"`.
    is_typescript: bool,
}

/// The owning state of one Vue file extraction.
struct VueWalker<'a> {
    /// Borrowed shared native extractor for script-block delegation.
    native: &'a Extractor,
    /// Repository-relative path (also the extraction file path).
    path: &'a RelativePath,
    /// Full source text.
    source: &'a str,
    /// The component node id (emitted first).
    component_id: String,
    /// Accumulated emitted facts in upstream order.
    result: ExtractionResult,
}

impl<'a> VueWalker<'a> {
    /// Creates the `component` node for the file and pushes it first.
    fn new(native: &'a Extractor, path: &'a RelativePath, source: &'a str) -> VueWalker<'a> {
        let file_path = path.as_str();
        let file_name = file_path.rsplit(['/', '\\']).next().unwrap_or(file_path);
        let component_name = file_name.strip_suffix(".vue").unwrap_or(file_name);
        let id = crate::ids::node_id(file_path, "component", component_name, 1);
        let node = Node {
            id: id.clone(),
            kind: NodeKind::Component,
            name: component_name.to_string(),
            qualified_name: format!("{file_path}::{component_name}"),
            file_path: file_path.to_string(),
            language: Language::Vue,
            start_line: 1,
            end_line: line_count(source),
            start_column: 0,
            end_column: last_line_utf16_len(source),
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
        VueWalker {
            native,
            path,
            source,
            component_id: id,
            result: ExtractionResult {
                nodes: vec![node],
                edges: Vec::new(),
                references: Vec::new(),
                diagnostics: Vec::new(),
            },
        }
    }

    /// Extract `<script>` blocks (upstream `extractScriptBlocks`).
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn script_blocks(&self) -> Vec<ScriptBlock> {
        let mut blocks = Vec::new();
        for captures in script_block_re().captures_iter(self.source) {
            let whole = captures.get(0).expect("whole script match");
            let attrs = captures.get(1).map(|m| m.as_str()).unwrap_or("");
            let content = captures
                .name("content")
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let is_typescript = ts_lang_attr_re().is_match(attrs);
            let before_script = &self.source[..whole.start()];
            let script_tag_line = count_newlines(before_script);
            let opening_tag = &self.source[whole.start()
                ..whole.start()
                    + whole
                        .as_str()
                        .find('>')
                        .expect("script tag has a closing >")
                    + 1];
            let opening_tag_lines = count_newlines(opening_tag);
            blocks.push(ScriptBlock {
                content,
                start_line: script_tag_line + opening_tag_lines,
                is_typescript,
            });
        }
        blocks
    }

    /// Delegate one script block to the native extractor and remap its
    /// coordinates (upstream `processScriptBlock`).
    ///
    /// # Errors
    ///
    /// Returns the native extractor's typed error when the embedded block
    /// fails native extraction.
    fn process_script_block(&mut self, block: &ScriptBlock) -> Result<(), ExtractionError> {
        let script_language = if block.is_typescript {
            Language::Typescript
        } else {
            Language::Javascript
        };
        let inner = self.native.extract(SourceFile::new(
            self.path,
            block.content.as_bytes(),
            script_language,
        ))?;
        let file_path = self.path.as_str().to_string();
        for mut node in inner.nodes {
            node.start_line += block.start_line;
            node.end_line += block.start_line;
            node.language = Language::Vue;
            let id = node.id.clone();
            self.result.edges.push(Edge {
                source: self.component_id.clone(),
                target: id,
                kind: EdgeKind::Contains,
                metadata: None,
                line: None,
                column: None,
                provenance: None,
            });
            self.result.nodes.push(node);
        }
        for mut edge in inner.edges {
            if let Some(line) = edge.line {
                edge.line = Some(line + block.start_line);
            }
            self.result.edges.push(edge);
        }
        for mut reference in inner.references {
            reference.line += block.start_line;
            reference.file_path = Some(file_path.clone());
            reference.language = Some(Language::Vue);
            self.result.references.push(reference);
        }
        for mut diagnostic in inner.diagnostics {
            if let Some(line) = diagnostic.line {
                diagnostic.line = Some(line + block.start_line);
            }
            self.result.diagnostics.push(diagnostic);
        }
        Ok(())
    }

    /// The `<script>`/`<style>` 0-indexed line ranges the template scan must
    /// skip (upstream `coveredRanges`).
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn covered_ranges(&self) -> Vec<(u32, u32)> {
        let mut ranges = Vec::new();
        for open in ["script", "style"] {
            let re = script_or_style_range_re(open);
            for captures in re.captures_iter(self.source) {
                let whole = captures.get(0).expect("whole range match");
                let start_line = zero_indexed_line(self.source, whole.start());
                let end_line = start_line + count_newlines(whole.as_str());
                ranges.push((start_line, end_line));
            }
        }
        ranges
    }

    /// Extract component usages from the template (upstream
    /// `extractTemplateComponents`).
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn extract_template_components(&mut self, covered: &[(u32, u32)]) {
        let lines: Vec<&str> = self.source.split('\n').collect();
        for (line_idx, line) in lines.iter().enumerate() {
            let line_idx = line_idx as u32;
            if covered
                .iter()
                .any(|(start, end)| line_idx >= *start && line_idx <= *end)
            {
                continue;
            }
            for tag_match in tag_re().captures_iter(line) {
                let raw = tag_match.get(1).expect("tag name").as_str();
                let component_name: String;
                if raw.starts_with(|c: char| c.is_ascii_uppercase()) {
                    component_name = raw.to_string();
                } else if raw.contains('-') {
                    component_name = kebab_to_pascal(raw);
                } else {
                    continue; // lowercase, no hyphen → native HTML element
                }
                if VUE_BUILTIN_COMPONENTS.contains(&component_name.as_str()) {
                    continue;
                }
                let column = utf16_col_in_line(line, tag_match.get(0).expect("whole").start()) + 1;
                self.result.references.push(RawReference {
                    from_node_id: self.component_id.clone(),
                    reference_name: component_name,
                    reference_kind: ReferenceKind::Edge(EdgeKind::References),
                    line: line_idx + 1,
                    column,
                    file_path: Some(self.path.as_str().to_string()),
                    language: Some(Language::Vue),
                    candidates: None,
                });
            }
        }
    }
}

/// Extract Vue facts from a `.vue` file.
///
/// # Errors
///
/// Returns the native extractor's typed error when an embedded `<script>`
/// block fails native extraction.
pub fn extract(
    native: &Extractor,
    path: &RelativePath,
    source: &str,
) -> Result<ExtractionResult, ExtractionError> {
    let mut walker = VueWalker::new(native, path, source);
    let blocks = walker.script_blocks();
    for block in &blocks {
        walker.process_script_block(block)?;
    }
    let covered = walker.covered_ranges();
    walker.extract_template_components(&covered);
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

    /// Extracts a vue fixture by path and source.
    ///
    /// # Panics
    ///
    /// Panics if the path is invalid or extraction fails.
    fn extract_src(file_path: &str, source: &str) -> ExtractionResult {
        let path = RelativePath::new(file_path).expect("valid path");
        extract(&native(), &path, source).expect("vue extraction succeeds")
    }

    #[test]
    /// Kebab-case tags convert to PascalCase, builtins and HTML are skipped.
    fn kebab_and_builtin_component_routing() {
        let result = extract_src(
            "comp.vue",
            "<template>\n<my-button />\n<Modal />\n<Transition>\n<x></x>\n</template>\n",
        );
        let names: Vec<&str> = result
            .references
            .iter()
            .map(|r| r.reference_name.as_str())
            .collect();
        assert!(names.contains(&"MyButton"));
        assert!(names.contains(&"Modal"));
        assert!(!names.contains(&"Transition"), "builtin must be skipped");
        assert!(
            !names.contains(&"x"),
            "lowercase HTML element must be skipped"
        );
    }

    #[test]
    /// The script block emits the inner file node and remapped coordinates.
    fn script_block_delegation() {
        let result = extract_src(
            "comp.vue",
            "<script>\nimport x from './x';\n</script>\n<template><div/></template>\n",
        );
        assert_eq!(result.nodes[0].kind, NodeKind::Component);
        assert_eq!(result.nodes[1].id, "file:comp.vue");
        assert_eq!(result.nodes[1].language, Language::Vue);
        // import on content line 2 → line 2 after the 0-offset block.
        assert!(
            result
                .references
                .iter()
                .any(|r| r.reference_name == "./x" && r.line == 2)
        );
    }
}
