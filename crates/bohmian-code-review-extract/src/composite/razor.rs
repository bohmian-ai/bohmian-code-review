//! Razor/Blazor extraction — a faithful Rust port of
//! `src/extraction/razor-extractor.ts`.
//!
//! A `.cshtml`/`.razor` file yields exactly one `component` node; markup
//! directives become `references` to the C# types they name
//! (`@model`/`@inherits`/`@inject`/`@typeof`), Blazor PascalCase component
//! tags become `references` (framework built-ins skipped), and the C# in
//! `@code {}`/`@functions {}`/`@{}` blocks is delegated to the native C#
//! extractor wrapped in a synthetic `class __RazorCode__ { ... }` so the
//! component's external references (service/DTO instantiations and calls)
//! are attributed to the component with the upstream line mapping
//! (`ref.line + lineOffset - 1`).

use crate::composite::util::{js_re, last_line_utf16_len, line_count, utf16_col_in_line};
use crate::error::ExtractionError;
use crate::extractor::Extractor;
use crate::result::ExtractionResult;
use crate::source_file::SourceFile;
use bohmian_code_review_core::{
    Language, Node, NodeKind, RawReference, ReferenceKind, RelativePath,
};
use regex::Regex;
use std::sync::OnceLock;

/// Blazor framework-provided components — invoked by the runtime, not
/// defined in-repo, so references to them never resolve.
const BLAZOR_BUILTIN_COMPONENTS: [&str; 36] = [
    "Router",
    "Found",
    "NotFound",
    "RouteView",
    "AuthorizeRouteView",
    "LayoutView",
    "CascadingValue",
    "CascadingAuthenticationState",
    "AuthorizeView",
    "Authorized",
    "NotAuthorized",
    "Authorizing",
    "EditForm",
    "DataAnnotationsValidator",
    "ValidationSummary",
    "ValidationMessage",
    "InputText",
    "InputNumber",
    "InputCheckbox",
    "InputSelect",
    "InputDate",
    "InputTextArea",
    "InputRadio",
    "InputRadioGroup",
    "InputFile",
    "PageTitle",
    "HeadContent",
    "HeadOutlet",
    "Virtualize",
    "DynamicComponent",
    "ErrorBoundary",
    "SectionContent",
    "SectionOutlet",
    "FocusOnNavigate",
    "NavLink",
    "Microsoft",
];

/// The `@model`/`@inherits` directive scanner.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn model_inherits_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"^\s*@(?:model|inherits)\s+([A-Za-z_][\w.]*(?:\s*<[^>]+>)?)"))
}

/// The `@inject` directive scanner.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn inject_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"^\s*@inject\s+([A-Za-z_][\w.]*(?:\s*<[^>]+>)?)\s+[A-Za-z_]"))
}

/// The `@typeof(...)` scanner.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn typeof_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"@typeof\(\s*([A-Za-z_][\w.]*)\s*\)"))
}

/// The PascalCase Blazor component tag scanner.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn component_tag_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"<([A-Z][A-Za-z0-9_]*)\b([^>]*)>"))
}

/// The generic type-argument scanner inside a component tag.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn type_arg_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r#"\bT[A-Za-z]*\s*=\s*"([A-Za-z_][\w.]*)""#))
}

/// The `@code`/`@functions`/`@{}` block scanner.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn code_block_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"@(?:code|functions)\b\s*\{|@\{"))
}

/// The owning state of one Razor file extraction.
struct RazorWalker<'a> {
    /// Borrowed shared native extractor for `@code` block delegation.
    native: &'a Extractor,
    /// Repository-relative path (also the extraction file path).
    path: &'a RelativePath,
    /// Full source text.
    source: &'a str,
    /// The component node id (the file's only node).
    component_id: String,
    /// Accumulated emitted facts in upstream order.
    result: ExtractionResult,
}

impl<'a> RazorWalker<'a> {
    /// Creates the `component` node for the file and pushes it.
    fn new(native: &'a Extractor, path: &'a RelativePath, source: &'a str) -> RazorWalker<'a> {
        let file_path = path.as_str();
        let file_name = file_path.rsplit(['/', '\\']).next().unwrap_or(file_path);
        let component_name = strip_razor_extension(file_name);
        let id = crate::ids::node_id(file_path, "component", component_name, 1);
        let node = Node {
            id: id.clone(),
            kind: NodeKind::Component,
            name: component_name.to_string(),
            qualified_name: format!("{file_path}::{component_name}"),
            file_path: file_path.to_string(),
            language: Language::Razor,
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
        RazorWalker {
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

    /// The last `.`-segment of a dotted type name.
    fn last_segment<'t>(&self, s: &'t str) -> &'t str {
        match s.rfind('.') {
            Some(index) => &s[index + 1..],
            None => s,
        }
    }

    /// Split a type expression into capitalized type names (base + generic
    /// arguments), each reduced to its last namespace segment.
    fn type_names<'t>(&self, expr: &'t str, out: &mut Vec<&'t str>) {
        for raw in expr.split(|c: char| c == '<' || c == '>' || c == ',' || c.is_whitespace()) {
            let seg = self.last_segment(raw.trim());
            if is_capitalized(seg) {
                out.push(seg);
            }
        }
    }

    /// Push one reference attributed to the component.
    fn push_ref(&mut self, name: &str, line: u32, column: u32) {
        self.result.references.push(RawReference {
            from_node_id: self.component_id.clone(),
            reference_name: name.to_string(),
            reference_kind: ReferenceKind::Edge(bohmian_code_review_core::EdgeKind::References),
            line,
            column,
            file_path: Some(self.path.as_str().to_string()),
            language: Some(Language::Razor),
            candidates: None,
        });
    }

    /// Extract `@model`/`@inherits`/`@inject`/`@typeof` directives.
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn extract_directives(&mut self) {
        let lines: Vec<&str> = self.source.split('\n').collect();
        for (i, line) in lines.iter().enumerate() {
            let i = i as u32;
            if let Some(captures) = model_inherits_re().captures(line) {
                let mut names = Vec::new();
                self.type_names(
                    captures.get(1).expect("type expression").as_str(),
                    &mut names,
                );
                for name in names {
                    self.push_ref(name, i + 1, 0);
                }
            }
            if let Some(captures) = inject_re().captures(line) {
                let mut names = Vec::new();
                self.type_names(
                    captures.get(1).expect("type expression").as_str(),
                    &mut names,
                );
                for name in names {
                    self.push_ref(name, i + 1, 0);
                }
            }
            for captures in typeof_re().captures_iter(line) {
                let seg = self.last_segment(captures.get(1).expect("type name").as_str());
                if seg.starts_with(|c: char| c.is_ascii_uppercase()) {
                    self.push_ref(
                        seg,
                        i + 1,
                        utf16_col_in_line(line, captures.get(0).expect("whole").start()),
                    );
                }
            }
        }
    }

    /// Extract Blazor component tags (`.razor` only).
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn extract_component_tags(&mut self) {
        let lines: Vec<&str> = self.source.split('\n').collect();
        for (i, line) in lines.iter().enumerate() {
            let i = i as u32;
            for captures in component_tag_re().captures_iter(line) {
                let name = captures.get(1).expect("component name").as_str();
                if BLAZOR_BUILTIN_COMPONENTS.contains(&name) {
                    continue;
                }
                self.push_ref(
                    name,
                    i + 1,
                    utf16_col_in_line(line, captures.get(0).expect("whole").start()) + 1,
                );
                let attrs = captures.get(2).map(|m| m.as_str()).unwrap_or("");
                for arg in type_arg_re().captures_iter(attrs) {
                    let seg = self.last_segment(arg.get(1).expect("type arg").as_str());
                    if seg.starts_with(|c: char| c.is_ascii_uppercase()) {
                        self.push_ref(seg, i + 1, 0);
                    }
                }
            }
        }
    }

    /// Find the matching `}` for the `{` at `open_index`, skipping string
    /// literals and comments (upstream `matchBrace`).
    fn match_brace(&self, open_index: usize) -> Option<usize> {
        let bytes = self.source.as_bytes();
        let mut depth = 0i32;
        let mut i = open_index;
        while i < bytes.len() {
            match bytes[i] {
                b'"' | b'\'' => {
                    let quote = bytes[i];
                    i += 1;
                    while i < bytes.len() && bytes[i] != quote {
                        if bytes[i] == b'\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                }
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    while i < bytes.len() && bytes[i] != b'\n' {
                        i += 1;
                    }
                }
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    i += 2;
                    while i < bytes.len() && !(bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/'))
                    {
                        i += 1;
                    }
                    i += 1;
                }
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// Extract `@code`/`@functions`/`@{}` blocks (upstream
    /// `extractCodeBlocks`).
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn code_blocks(&self) -> Vec<(String, u32)> {
        let mut blocks = Vec::new();
        let mut search_from = 0;
        while let Some(captures) = code_block_re().captures_at(self.source, search_from) {
            let whole = captures.get(0).expect("whole match");
            let open_index = self.source[whole.start()..]
                .find('{')
                .map(|i| i + whole.start());
            let Some(open_index) = open_index else {
                break;
            };
            let Some(close) = self.match_brace(open_index) else {
                break;
            };
            let content = self.source[open_index + 1..close].to_string();
            let line_offset =
                crate::composite::util::count_newlines(&self.source[..open_index + 1]);
            blocks.push((content, line_offset));
            search_from = close;
        }
        blocks
    }

    /// Delegate each `@code` block's C# to the native C# extractor and
    /// attribute the block's external references to the component (upstream
    /// `processCodeBlocks`).
    fn process_code_blocks(&mut self) {
        let blocks = self.code_blocks();
        for (content, line_offset) in blocks {
            if content.trim().is_empty() {
                continue;
            }
            let wrapped = format!("class __RazorCode__ {{\n{content}\n}}");
            let inner = match self.native.extract(SourceFile::new(
                self.path,
                wrapped.as_bytes(),
                Language::Csharp,
            )) {
                Ok(inner) => inner,
                Err(_) => continue, // grammar not loaded / parse failure — skip this block
            };
            for mut reference in inner.references {
                reference.from_node_id = self.component_id.clone();
                reference.line = reference.line.saturating_add(line_offset).saturating_sub(1);
                reference.file_path = Some(self.path.as_str().to_string());
                reference.language = Some(Language::Razor);
                self.result.references.push(reference);
            }
        }
    }
}

/// Whether a type-name segment is capitalized (`/^[A-Z][A-Za-z0-9_]*$/`).
fn is_capitalized(seg: &str) -> bool {
    let mut chars = seg.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Strips a case-insensitive `.razor`/`.cshtml` extension from a file name
/// (upstream `fileName.replace(/\.(razor|cshtml)$/i, '')`).
fn strip_razor_extension(file_name: &str) -> &str {
    for ext in [".razor", ".cshtml"] {
        if file_name.len() >= ext.len()
            && file_name[file_name.len() - ext.len()..].eq_ignore_ascii_case(ext)
        {
            return &file_name[..file_name.len() - ext.len()];
        }
    }
    file_name
}

/// Extract Razor facts from a `.cshtml`/`.razor` file.
///
/// # Errors
///
/// Returns the native extractor's typed error when an embedded `@code`
/// block fails native extraction (upstream skips the block on failure; the
/// port mirrors that by skipping the block and never failing).
pub fn extract(
    native: &Extractor,
    path: &RelativePath,
    source: &str,
) -> Result<ExtractionResult, ExtractionError> {
    let mut walker = RazorWalker::new(native, path, source);
    walker.extract_directives();
    if path.as_str().to_ascii_lowercase().ends_with(".razor") {
        walker.extract_component_tags();
    }
    walker.process_code_blocks();
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

    /// Extracts a razor fixture by path and source.
    ///
    /// # Panics
    ///
    /// Panics if the path is invalid or extraction fails.
    fn extract_src(file_path: &str, source: &str) -> ExtractionResult {
        let path = RelativePath::new(file_path).expect("valid path");
        extract(&native(), &path, source).expect("razor extraction succeeds")
    }

    #[test]
    /// Directives, component tags, and generic type args become references.
    fn directives_and_component_tags() {
        let result = extract_src(
            "Index.cshtml",
            "@model App.ViewModels.RegisterModel\n@inject IUserService users\n@typeof(MainLayout)\n",
        );
        let names: Vec<&str> = result
            .references
            .iter()
            .map(|r| r.reference_name.as_str())
            .collect();
        assert_eq!(names, vec!["RegisterModel", "IUserService", "MainLayout"]);
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].kind, NodeKind::Component);
    }

    #[test]
    /// Razor files do not scan component tags; `.razor` files do.
    fn cshtml_skips_component_tags() {
        let source = "<EditForm Model=\"m\" />\n";
        let cshtml = extract_src("Index.cshtml", source);
        assert!(
            cshtml
                .references
                .iter()
                .all(|r| r.reference_name != "EditForm"),
            ".cshtml must not scan Blazor component tags"
        );
        let razor = extract_src("Counter.razor", "<EditForm Model=\"m\" />\n");
        assert!(
            razor
                .references
                .iter()
                .all(|r| r.reference_name != "EditForm"),
            "builtin EditForm must be skipped"
        );
        let generic = extract_src("Counter.razor", "<Grid TItem=\"CatalogItem\" />\n");
        assert!(
            generic
                .references
                .iter()
                .any(|r| r.reference_name == "CatalogItem")
        );
    }

    #[test]
    /// The synthetic class wrapper maps `@code` block lines back to the file.
    fn code_block_line_mapping() {
        let source = "@code {\n  private void Go() {\n    var svc = new OrderService();\n  }\n}\n";
        let result = extract_src("Counter.razor", source);
        assert!(
            result
                .references
                .iter()
                .any(|r| r.reference_name == "OrderService" && r.line == 3),
            "OrderService must be attributed at the original line 3"
        );
    }
}
