//! Astro extraction — a faithful Rust port of `src/extraction/astro-extractor.ts`.
//!
//! A `.astro` file yields one `component` node, then the `---`-fenced
//! frontmatter block (first non-blank line must be the opening fence) and
//! every `<script>` block are delegated to the native TypeScript extractor
//! with line numbers remapped to the containing file (Astro processes both
//! as TypeScript by default). Template expressions `{fn(...)}` become
//! `calls` references and PascalCase tags become `references` references,
//! with the frontmatter and `<script>`/`<style>` line ranges excluded. An
//! unclosed frontmatter fence is treated as "no frontmatter", and an
//! expression group left open at end-of-line contributes its opening line's
//! calls exactly as upstream does.

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

/// Astro built-in components — compiler-provided (`<Fragment>`) or shipped
/// by `astro:components` (`<Code>`, `<Debug>`), not user code.
const ASTRO_BUILTIN_COMPONENTS: [&str; 3] = ["Fragment", "Code", "Debug"];

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

/// The `<script ...>`/`<style ...>` line-range scanner.
fn script_or_style_range_re(open: &str) -> Regex {
    js_re(&format!(r"<{open}(\s[^>]*)?>[\s\S]*?</{open}>"))
}

/// The template-expression scanner `\{([^}/][^}]*)\}` (excludes JSX
/// comments `{/* ... */}`).
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn template_expr_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"\{([^}/][^}]*)\}"))
}

/// The open-at-end-of-line expression scanner `\{([^}/][^}]*)$`.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn open_expr_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"\{([^}/][^}]*)$"))
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

/// One extracted script/frontmatter content block.
struct ScriptBlock {
    /// The block content.
    content: String,
    /// The 0-indexed line where the content begins.
    start_line: u32,
    /// The 0-indexed line where the block ends (frontmatter: the closing
    /// fence line; script blocks: unused).
    end_line: u32,
}

/// The owning state of one Astro file extraction.
struct AstroWalker<'a> {
    /// Borrowed shared native extractor for block delegation.
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

impl<'a> AstroWalker<'a> {
    /// Creates the `component` node for the file and pushes it first.
    fn new(native: &'a Extractor, path: &'a RelativePath, source: &'a str) -> AstroWalker<'a> {
        let file_path = path.as_str();
        let file_name = file_path.rsplit(['/', '\\']).next().unwrap_or(file_path);
        let component_name = file_name.strip_suffix(".astro").unwrap_or(file_name);
        let id = crate::ids::node_id(file_path, "component", component_name, 1);
        let node = Node {
            id: id.clone(),
            kind: NodeKind::Component,
            name: component_name.to_string(),
            qualified_name: format!("{file_path}::{component_name}"),
            file_path: file_path.to_string(),
            language: Language::Astro,
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
        AstroWalker {
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

    /// Extract the frontmatter block (upstream `extractFrontmatter`).
    fn frontmatter(&self) -> Option<ScriptBlock> {
        let lines: Vec<&str> = self.source.split('\n').collect();
        let mut open_idx: Option<usize> = None;
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed == "---" {
                open_idx = Some(i);
            }
            break;
        }
        let open_idx = open_idx?;
        let mut close_idx = None;
        for (i, line) in lines.iter().enumerate().skip(open_idx + 1) {
            if line.trim() == "---" {
                close_idx = Some(i);
                break;
            }
        }
        let close_idx = close_idx?;
        Some(ScriptBlock {
            content: lines[open_idx + 1..close_idx].join("\n"),
            start_line: open_idx as u32 + 1,
            end_line: close_idx as u32,
        })
    }

    /// Extract `<script>` blocks from the template portion (upstream
    /// `extractScriptBlocks`).
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn script_blocks(&self) -> Vec<ScriptBlock> {
        let mut blocks = Vec::new();
        for captures in script_block_re().captures_iter(self.source) {
            let whole = captures.get(0).expect("whole script match");
            let content = captures
                .name("content")
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
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
                end_line: script_tag_line + opening_tag_lines,
            });
        }
        blocks
    }

    /// Delegate one block to the native TypeScript extractor and remap its
    /// coordinates (upstream `processScriptContent`).
    ///
    /// # Errors
    ///
    /// Returns the native extractor's typed error when the embedded block
    /// fails native extraction.
    fn process_script_content(&mut self, block: &ScriptBlock) -> Result<(), ExtractionError> {
        let inner = self.native.extract(SourceFile::new(
            self.path,
            block.content.as_bytes(),
            Language::Typescript,
        ))?;
        let file_path = self.path.as_str().to_string();
        for mut node in inner.nodes {
            node.start_line += block.start_line;
            node.end_line += block.start_line;
            node.language = Language::Astro;
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
            reference.language = Some(Language::Astro);
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

    /// The line ranges the template scans must skip (upstream
    /// `getCoveredRanges`).
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn covered_ranges(&self, frontmatter: Option<&ScriptBlock>) -> Vec<(u32, u32)> {
        let mut ranges = Vec::new();
        if let Some(block) = frontmatter {
            // Cover from the opening fence line through the closing fence line.
            ranges.push((block.start_line - 1, block.end_line));
        }
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
            let mut exprs: Vec<(String, usize)> = Vec::new();
            for expr_match in template_expr_re().captures_iter(line) {
                exprs.push((
                    expr_match
                        .get(1)
                        .expect("expression group")
                        .as_str()
                        .to_string(),
                    expr_match.get(0).expect("whole").start(),
                ));
            }
            let replaced = template_expr_re().replace_all(line, "");
            if let Some(open_match) = open_expr_re().captures(&replaced) {
                let text = open_match.get(1).expect("open expression group").as_str();
                exprs.push((text.to_string(), line.rfind('{').expect("open brace")));
            }
            for (expr, expr_offset) in exprs {
                for call_match in call_re().captures_iter(&expr) {
                    let callee_name = call_match.get(1).expect("callee group").as_str();
                    if matches!(callee_name, "if" | "await" | "function") {
                        continue;
                    }
                    let column = utf16_col_in_line(line, expr_offset)
                        + utf16_col_in_line(&expr, call_match.get(0).expect("whole").start());
                    self.result.references.push(RawReference {
                        from_node_id: self.component_id.clone(),
                        reference_name: callee_name.to_string(),
                        reference_kind: ReferenceKind::Edge(EdgeKind::Calls),
                        line: line_idx + 1,
                        column,
                        file_path: Some(self.path.as_str().to_string()),
                        language: Some(Language::Astro),
                        candidates: None,
                    });
                }
            }
        }
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
            for tag_match in component_tag_re().captures_iter(line) {
                let component_name = tag_match.get(1).expect("component name").as_str();
                if ASTRO_BUILTIN_COMPONENTS.contains(&component_name) {
                    continue;
                }
                let column = utf16_col_in_line(line, tag_match.get(0).expect("whole").start()) + 1;
                self.result.references.push(RawReference {
                    from_node_id: self.component_id.clone(),
                    reference_name: component_name.to_string(),
                    reference_kind: ReferenceKind::Edge(EdgeKind::References),
                    line: line_idx + 1,
                    column,
                    file_path: Some(self.path.as_str().to_string()),
                    language: Some(Language::Astro),
                    candidates: None,
                });
            }
        }
    }
}

/// Extract Astro facts from a `.astro` file.
///
/// # Errors
///
/// Returns the native extractor's typed error when the frontmatter or an
/// embedded `<script>` block fails native extraction.
pub fn extract(
    native: &Extractor,
    path: &RelativePath,
    source: &str,
) -> Result<ExtractionResult, ExtractionError> {
    let mut walker = AstroWalker::new(native, path, source);
    let frontmatter = walker.frontmatter();
    if let Some(block) = &frontmatter {
        walker.process_script_content(block)?;
    }
    let script_blocks = walker.script_blocks();
    for block in &script_blocks {
        walker.process_script_content(block)?;
    }
    let covered = walker.covered_ranges(frontmatter.as_ref());
    walker.extract_template_calls(&covered);
    walker.extract_template_components(&covered);
    Ok(walker.result)
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;
    use crate::composite::util::utf16_len;
    use crate::config::ExtractionConfig;

    /// Builds a native extractor over the full native set.
    ///
    /// # Panics
    ///
    /// Panics if extractor construction fails.
    fn native() -> Extractor {
        Extractor::new(ExtractionConfig::all_native()).expect("extractor builds")
    }

    /// Extracts an astro fixture by path and source.
    ///
    /// # Panics
    ///
    /// Panics if the path is invalid or extraction fails.
    fn extract_src(file_path: &str, source: &str) -> ExtractionResult {
        let path = RelativePath::new(file_path).expect("valid path");
        extract(&native(), &path, source).expect("astro extraction succeeds")
    }

    #[test]
    /// Frontmatter and script blocks both emit inner file nodes; template
    /// calls and components become references.
    fn frontmatter_script_and_template() {
        let source = "---\nconst x = 1;\n---\n<h1>{fmt(x)}</h1>\n<Card />\n";
        let result = extract_src("page.astro", source);
        assert_eq!(result.nodes[0].kind, NodeKind::Component);
        assert_eq!(result.nodes[1].id, "file:page.astro");
        assert_eq!(result.nodes[1].start_line, 2);
        let calls: Vec<&RawReference> = result
            .references
            .iter()
            .filter(|r| r.reference_kind == ReferenceKind::Edge(EdgeKind::Calls))
            .collect();
        assert!(
            calls
                .iter()
                .any(|r| r.reference_name == "fmt" && r.line == 4)
        );
        let components: Vec<&RawReference> = result
            .references
            .iter()
            .filter(|r| r.reference_kind == ReferenceKind::Edge(EdgeKind::References))
            .collect();
        assert!(
            components
                .iter()
                .any(|r| r.reference_name == "Card" && r.line == 5)
        );
    }

    #[test]
    /// An unclosed frontmatter fence is treated as no frontmatter.
    fn unclosed_frontmatter_is_absent() {
        let result = extract_src("page.astro", "---\nconst x = 1;\n<h1>hi</h1>\n");
        // No frontmatter → the only delegated block is absent; the component
        // node still exists and no inner file node is emitted.
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].kind, NodeKind::Component);
        assert_eq!(utf16_len("ab"), 2);
    }
}
