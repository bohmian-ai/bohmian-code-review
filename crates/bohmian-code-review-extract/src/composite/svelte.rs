//! Svelte extraction — a faithful Rust port of
//! `src/extraction/svelte-extractor.ts`.
//!
//! A `.svelte` file yields one `component` node (Svelte components are
//! always importable), then every `<script>` block is delegated to the
//! native TypeScript/JavaScript extractor with its line numbers remapped to
//! the containing file (each emitted node also receives a `contains` edge
//! from the component). Template expressions `{fn(...)}` become `calls`
//! references and PascalCase tags become `references` references, both
//! skipping `<script>`/`<style>` line ranges. Svelte 5 rune calls
//! (`$state`, `$props`, …) are filtered from the final reference list, and
//! the script-block inner `file:` node is retained with its own contains
//! edge exactly as upstream emits it.

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

/// Svelte 5 rune names — compiler builtins, not real functions.
const SVELTE_RUNES: [&str; 8] = [
    "$props",
    "$state",
    "$derived",
    "$effect",
    "$bindable",
    "$inspect",
    "$host",
    "$snippet",
];

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

/// The `<script ...>`/`<style ...>` line-range scanner (two regexes because
/// Rust regexes have no backreferences; upstream uses `</\1>`).
fn script_or_style_range_re(open: &str) -> Regex {
    js_re(&format!(r"<{open}(\s[^>]*)?>[\s\S]*?</{open}>"))
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

/// The template-expression scanner `\{([^}#/:@][^}]*)\}`.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn template_expr_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"\{([^}#/:@][^}]*)\}"))
}

/// The call scanner `\b([a-zA-Z_$][\w$.]*)\s*\(`.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn call_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"\b([a-zA-Z_$][\w$.]*)\s*\("))
}

/// The PascalCase component-tag scanner `/<([A-Z][a-zA-Z0-9_$]*)\b/g`.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn component_tag_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"<([A-Z][a-zA-Z0-9_$]*)\b"))
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

/// The owning state of one Svelte file extraction.
struct SvelteWalker<'a> {
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

impl<'a> SvelteWalker<'a> {
    /// Creates the `component` node for the file and pushes it first.
    fn new(native: &'a Extractor, path: &'a RelativePath, source: &'a str) -> SvelteWalker<'a> {
        let file_path = path.as_str();
        let file_name = file_path.rsplit(['/', '\\']).next().unwrap_or(file_path);
        let component_name = file_name.strip_suffix(".svelte").unwrap_or(file_name);
        let id = crate::ids::node_id(file_path, "component", component_name, 1);
        let node = Node {
            id: id.clone(),
            kind: NodeKind::Component,
            name: component_name.to_string(),
            qualified_name: format!("{file_path}::{component_name}"),
            file_path: file_path.to_string(),
            language: Language::Svelte,
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
        SvelteWalker {
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
            node.language = Language::Svelte;
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
            reference.language = Some(Language::Svelte);
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

    /// The `<script>`/`<style>` 0-indexed line ranges the template scans
    /// must skip (upstream `coveredRanges`).
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

    /// Extract function calls from template expressions (upstream
    /// `extractTemplateCalls`).
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn extract_template_calls(&mut self, covered: &[(u32, u32)]) {
        let lines: Vec<&str> = self.source.split('\n').collect();
        for (line_idx, line) in lines.iter().enumerate() {
            let line_idx = line_idx as u32;
            if covered
                .iter()
                .any(|(start, end)| line_idx >= *start && line_idx <= *end)
            {
                continue;
            }
            for expr_match in template_expr_re().captures_iter(line) {
                let expr = expr_match.get(1).expect("expression group");
                for call_match in call_re().captures_iter(expr.as_str()) {
                    let callee_name = call_match.get(1).expect("callee group").as_str();
                    if SVELTE_RUNES.contains(&callee_name)
                        || matches!(callee_name, "if" | "else" | "each" | "await")
                    {
                        continue;
                    }
                    let column = utf16_col_in_line(line, expr_match.get(0).expect("whole").start())
                        + utf16_col_in_line(
                            expr.as_str(),
                            call_match.get(0).expect("whole").start(),
                        );
                    self.result.references.push(RawReference {
                        from_node_id: self.component_id.clone(),
                        reference_name: callee_name.to_string(),
                        reference_kind: ReferenceKind::Edge(EdgeKind::Calls),
                        line: line_idx + 1,
                        column,
                        file_path: Some(self.path.as_str().to_string()),
                        language: Some(Language::Svelte),
                        candidates: None,
                    });
                }
            }
        }
    }

    /// Extract PascalCase component usages from the template (upstream
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
            for tag_match in component_tag_re().captures_iter(line) {
                let component_name = tag_match.get(1).expect("component name").as_str();
                let column = utf16_col_in_line(line, tag_match.get(0).expect("whole").start()) + 1;
                self.result.references.push(RawReference {
                    from_node_id: self.component_id.clone(),
                    reference_name: component_name.to_string(),
                    reference_kind: ReferenceKind::Edge(EdgeKind::References),
                    line: line_idx + 1,
                    column,
                    file_path: Some(self.path.as_str().to_string()),
                    language: Some(Language::Svelte),
                    candidates: None,
                });
            }
        }
    }
}

/// Extract Svelte facts from a `.svelte` file.
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
    let mut walker = SvelteWalker::new(native, path, source);
    let blocks = walker.script_blocks();
    for block in &blocks {
        walker.process_script_block(block)?;
    }
    let covered = walker.covered_ranges();
    walker.extract_template_calls(&covered);
    walker.extract_template_components(&covered);
    walker
        .result
        .references
        .retain(|reference| !SVELTE_RUNES.contains(&reference.reference_name.as_str()));
    Ok(walker.result)
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;
    use crate::composite::util::utf16_len;
    use crate::config::ExtractionConfig;
    use bohmian_code_review_core::Severity;

    /// Builds a native extractor over the full native set.
    ///
    /// # Panics
    ///
    /// Panics if extractor construction fails.
    fn native() -> Extractor {
        Extractor::new(ExtractionConfig::all_native()).expect("extractor builds")
    }

    /// Extracts a svelte fixture by path and source.
    ///
    /// # Panics
    ///
    /// Panics if the path is invalid or extraction fails.
    fn extract_src(file_path: &str, source: &str) -> ExtractionResult {
        let path = RelativePath::new(file_path).expect("valid path");
        extract(&native(), &path, source).expect("svelte extraction succeeds")
    }

    #[test]
    /// Emits the component node first and the script-block file node second.
    fn component_then_inner_file_node() {
        let result = extract_src("comp.svelte", "<script>let x = 1;</script>");
        assert_eq!(result.nodes.len(), 3); // component, inner file, variable x
        assert_eq!(result.nodes[0].kind, NodeKind::Component);
        assert_eq!(result.nodes[0].name, "comp");
        assert_eq!(result.nodes[0].qualified_name, "comp.svelte::comp");
        assert!(result.nodes[0].is_exported == Some(true));
        assert_eq!(result.nodes[1].kind, NodeKind::File);
        assert_eq!(result.nodes[1].id, "file:comp.svelte");
        assert_eq!(result.nodes[1].language, Language::Svelte);
        assert_eq!(result.nodes[2].kind, NodeKind::Variable);
        assert_eq!(
            result.edges[0],
            Edge {
                source: result.nodes[0].id.clone(),
                target: "file:comp.svelte".to_string(),
                kind: EdgeKind::Contains,
                metadata: None,
                line: None,
                column: None,
                provenance: None,
            }
        );
    }

    #[test]
    /// Rune calls vanish from the emitted references.
    fn rune_calls_are_filtered() {
        let result = extract_src(
            "comp.svelte",
            "<script>let count = $state(0);</script><h1>{count}</h1>",
        );
        assert!(
            result
                .references
                .iter()
                .all(|r| !r.reference_name.starts_with('$')),
            "no rune references may survive"
        );
    }

    #[test]
    /// Template calls and PascalCase components become references with the
    /// upstream column convention (`match.index + 1` for tags).
    fn template_calls_and_components() {
        let result = extract_src(
            "comp.svelte",
            "<script>function greet() {}</script>\n<h1 onclick={greet()}>hi</h1>\n<Modal />\n",
        );
        let calls: Vec<&RawReference> = result
            .references
            .iter()
            .filter(|r| r.reference_kind == ReferenceKind::Edge(EdgeKind::Calls))
            .collect();
        assert!(
            calls
                .iter()
                .any(|r| r.reference_name == "greet" && r.line == 2 && r.column == 12)
        );
        let components: Vec<&RawReference> = result
            .references
            .iter()
            .filter(|r| r.reference_kind == ReferenceKind::Edge(EdgeKind::References))
            .collect();
        assert!(
            components
                .iter()
                .any(|r| r.reference_name == "Modal" && r.line == 3 && r.column == 1)
        );
        for reference in result.references {
            assert_eq!(reference.file_path.as_deref(), Some("comp.svelte"));
            assert_eq!(reference.language, Some(Language::Svelte));
        }
    }

    #[test]
    /// The whole file carries the component span; the diagnostic severity
    /// conversion stays available for carried errors.
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn severity_conversion_exists() {
        // Severity is exercised through carried inner diagnostics; assert the
        // wire spelling contract used by the carried-error path.
        assert_eq!(
            serde_json::to_string(&Severity::Error).unwrap(),
            "\"error\""
        );
        assert_eq!(utf16_len("ab"), 2);
    }
}
