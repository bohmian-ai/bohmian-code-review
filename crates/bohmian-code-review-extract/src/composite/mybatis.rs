//! MyBatis mapper XML extraction — a faithful Rust port of
//! `src/extraction/mybatis-extractor.ts`.
//!
//! MyBatis splits a DAO across a Java interface (native extraction) and an
//! XML mapper file keyed by `<namespace>` and statement `id`. This
//! extractor emits one `method`-shaped node per `<select|insert|update|
//! delete|sql>` statement, qualified as `<namespace>::<id>` so the MyBatis
//! framework synthesizer can suffix-match the Java method; `<include
//! refid="...">` inside a statement yields an unresolved reference keyed
//! the same way. Both dialects are covered: MyBatis 3 `<mapper
//! namespace="...">` and legacy iBatis 2 `<sqlMap>` (with its extra
//! `<statement>`/`<procedure>` verbs). Non-mapper XML returns just the
//! (hashed) file node so the file row is tracked without emitting symbols.
//!
//! XML comments are blanked up front (length-preserving — newlines kept)
//! so commented-out statements produce no phantom nodes, and
//! `<![CDATA[ ... ]]>` content is left intact.

use crate::composite::util::js_re;
use crate::error::ExtractionError;
use crate::result::ExtractionResult;
use bohmian_code_review_core::{
    Edge, EdgeKind, Language, Node, NodeKind, RawReference, ReferenceKind, RelativePath,
};
use regex::Regex;
use std::sync::OnceLock;

/// The `<mapper\b([^>]*)>` root detector.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn mapper_root_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"<mapper\b([^>]*)>"))
}

/// The `<sqlMap\b([^>]*)>` root detector.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn sqlmap_root_re() -> &'static Regex {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| js_re(r"<sqlMap\b([^>]*)>"))
}

/// Extracts the value of an attribute `name="value"` (or `name='value'`)
/// requiring the closing quote to MATCH the opening quote and the value to
/// be non-empty and contain neither quote — the upstream JS
/// `(["'])([^"']+)\1` backreference semantics, which the Rust regex crate
/// cannot express.
fn quoted_attr(attrs: &str, name: &str) -> Option<String> {
    let pattern = format!(r#"\b{}\s*=\s*(["'])"#, regex::escape(name));
    let captures = js_re(&pattern).captures(attrs)?;
    let whole = captures.get(0)?;
    let opening = captures.get(1)?.as_str().chars().next()?;
    let rest = &attrs[whole.end()..];
    // The value ends at the FIRST quote of either kind; the closer must be
    // the SAME kind as the opener (a mismatched or missing closer rejects).
    let mut value_end = None;
    for (index, c) in rest.char_indices() {
        if c == '"' || c == '\'' {
            if c == opening {
                value_end = Some(index);
            }
            break;
        }
    }
    let end = value_end?;
    if end == 0 {
        return None; // empty value — `[^"']+` requires at least one char
    }
    Some(rest[..end].to_string())
}

/// The owning state of one MyBatis mapper extraction.
struct MybatisWalker<'a> {
    /// Repository-relative path (also the extraction file path).
    path: &'a RelativePath,
    /// Comment-stripped source (length-preserving).
    source: String,
    /// Byte offsets of each line start in the stripped source.
    line_starts: Vec<usize>,
    /// Accumulated emitted facts in upstream order.
    result: ExtractionResult,
}

impl<'a> MybatisWalker<'a> {
    /// Construct the walker over a `.xml` mapper source.
    fn new(path: &'a RelativePath, source: &str) -> MybatisWalker<'a> {
        let stripped = Self::strip_xml_comments(source);
        let line_starts = crate::textutil::line_starts(&stripped);
        let file_path = path.as_str();
        let lines: Vec<&str> = stripped.split('\n').collect();
        let id = crate::ids::node_id(file_path, "file", file_path, 1);
        let file_node = Node {
            id: id.clone(),
            kind: NodeKind::File,
            name: file_path
                .rsplit('/')
                .next()
                .unwrap_or(file_path)
                .to_string(),
            qualified_name: file_path.to_string(),
            file_path: file_path.to_string(),
            language: Language::Xml,
            start_line: 1,
            end_line: lines.len().max(1) as u32,
            start_column: 0,
            end_column: lines
                .last()
                .map(|l| crate::composite::util::utf16_len(l) as u32)
                .unwrap_or(0),
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
        MybatisWalker {
            path,
            source: stripped,
            line_starts,
            result: ExtractionResult {
                nodes: vec![file_node],
                edges: Vec::new(),
                references: Vec::new(),
                diagnostics: Vec::new(),
            },
        }
    }

    /// Blank out XML comments up front — length-preserving (comment bytes
    /// become spaces, newlines are kept); `<![CDATA[ ... ]]>` is left intact.
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn strip_xml_comments(source: &str) -> String {
        let bytes = source.as_bytes();
        let mut out: Vec<u8> = bytes.to_vec();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i..].starts_with(b"<![CDATA[") {
                match find_bytes(&bytes[i + 9..], b"]]>") {
                    Some(off) => i += 9 + off + 3,
                    None => i = bytes.len(),
                }
                continue;
            }
            if bytes[i..].starts_with(b"<!--") {
                let stop = match find_bytes(&bytes[i + 4..], b"-->") {
                    Some(off) => i + 4 + off + 3,
                    None => bytes.len(),
                };
                for j in i..stop {
                    if bytes[j] != b'\n' {
                        out[j] = b' ';
                    }
                }
                i = stop;
                continue;
            }
            i += 1;
        }
        // Every replaced byte is ASCII space and every kept byte is a
        // valid source byte, so the output is valid UTF-8 of equal length.
        String::from_utf8(out).expect("comment stripping preserves UTF-8 and length")
    }

    /// The 1-based line number of a byte offset in the stripped source.
    fn line_of(&self, offset: usize) -> u32 {
        // Binary search over line starts.
        let mut lo = 0usize;
        let mut hi = self.line_starts.len() - 1;
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if self.line_starts[mid] <= offset {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo as u32 + 1
    }

    /// Find the mapper root and its dialect (upstream `findMapperRoot`).
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn find_mapper_root(&self) -> Option<(String, bool, usize, usize)> {
        if let Some(captures) = mapper_root_re().captures(&self.source) {
            let whole = captures.get(0).expect("whole mapper match");
            let attrs = captures.get(1).map(|m| m.as_str()).unwrap_or("");
            if let Some(namespace) = quoted_attr(attrs, "namespace") {
                let body_start = whole.start() + whole.as_str().len();
                let close = self.source[body_start..].find("</mapper>");
                let body_end = close.map(|i| body_start + i).unwrap_or(self.source.len());
                return Some((namespace, true, body_start, body_end));
            }
        }
        if let Some(captures) = sqlmap_root_re().captures(&self.source) {
            let whole = captures.get(0).expect("whole sqlMap match");
            let attrs = captures.get(1).map(|m| m.as_str()).unwrap_or("");
            let namespace = quoted_attr(attrs, "namespace").unwrap_or_default();
            let body_start = whole.start() + whole.as_str().len();
            let close = self.source[body_start..].find("</sqlMap>");
            let body_end = close.map(|i| body_start + i).unwrap_or(self.source.len());
            return Some((namespace, false, body_start, body_end));
        }
        None
    }

    /// Extract mapper statements (upstream `extractMapper`).
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn extract_mapper(
        &mut self,
        file_node_id: &str,
        namespace: &str,
        mybatis: bool,
        body_start: usize,
        body_end: usize,
    ) {
        let body = &self.source[body_start..body_end];
        let verbs = if mybatis {
            "select|insert|update|delete|sql"
        } else {
            "select|insert|update|delete|sql|statement|procedure"
        };
        let open_re = js_re(&format!(r"<({verbs})\b([^>]*)>"));
        let file_path = self.path.as_str().to_string();
        for captures in open_re.captures_iter(body) {
            let whole = captures.get(0).expect("whole statement match");
            let elem_type = captures.get(1).expect("verb").as_str();
            let attrs = captures.get(2).map(|m| m.as_str()).unwrap_or("");
            // Find the matching close tag for THIS verb (upstream `</\1>`).
            let open_end = whole.start() + whole.as_str().len();
            let close_tag = format!("</{elem_type}>");
            let close_rel = body[open_end..].find(&close_tag);
            let Some(close_rel) = close_rel else {
                continue;
            };
            let elem_body = &body[open_end..open_end + close_rel];
            let Some(id) = quoted_attr(attrs, "id") else {
                continue;
            };
            let absolute_index = body_start + whole.start();
            let open_len = whole.as_str().len();
            let start_line = self.line_of(absolute_index);
            let end_line = self.line_of(absolute_index + open_len + close_rel + close_tag.len());
            let is_sql_fragment = elem_type == "sql";
            let (qualified, name) = self.qualify_statement(namespace, &id);
            let node_id =
                crate::ids::node_id(&file_path, "method", &qualified, absolute_index as u32);
            let node = Node {
                id: node_id.clone(),
                kind: NodeKind::Method,
                name,
                qualified_name: qualified.clone(),
                file_path: file_path.clone(),
                language: Language::Xml,
                signature: Some(self.build_signature(elem_type, attrs, is_sql_fragment)),
                start_line,
                end_line,
                start_column: 0,
                end_column: 0,
                docstring: Some(self.preview_sql(elem_body)),
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
                source: file_node_id.to_string(),
                target: node_id.clone(),
                kind: EdgeKind::Contains,
                metadata: None,
                line: None,
                column: None,
                provenance: None,
            });

            let include_tag_re = js_re(r"<include\b([^>]*)>");
            for inc in include_tag_re.captures_iter(elem_body) {
                let inc_whole = inc.get(0).expect("whole include match");
                let inc_attrs = inc.get(1).map(|m| m.as_str()).unwrap_or("");
                let Some(refid) = quoted_attr(inc_attrs, "refid") else {
                    continue;
                };
                let ref_qualified = if refid.contains('.') {
                    refid.replace('.', "::")
                } else if !namespace.is_empty() {
                    format!("{namespace}::{refid}")
                } else {
                    refid.to_string()
                };
                // Upstream folds the include offset as
                // `absoluteIndex + (m[0].length - m[3].length - closeLen) +
                // inc.index`, which equals the statement open end plus the
                // include's offset within the body.
                let absolute_include = absolute_index + open_len + inc_whole.start();
                let line = self.line_of(absolute_include);
                self.result.references.push(RawReference {
                    from_node_id: node_id.clone(),
                    reference_name: ref_qualified,
                    reference_kind: ReferenceKind::Edge(EdgeKind::References),
                    line,
                    column: 0,
                    file_path: None,
                    language: None,
                    candidates: None,
                });
            }
        }
    }

    /// Build the statement signature (upstream `buildSignature`).
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn build_signature(&self, elem_type: &str, attrs: &str, is_sql_fragment: bool) -> String {
        if is_sql_fragment {
            return "<sql>".to_string();
        }
        let verb = elem_type.to_ascii_uppercase();
        let result = quoted_attr(attrs, "resultType");
        let param = quoted_attr(attrs, "parameterType");
        let db_id = quoted_attr(attrs, "databaseId");
        let mut parts = vec![verb];
        if let Some(param) = param {
            parts.push(format!("param={param}"));
        }
        if let Some(result) = result {
            parts.push(format!("result={result}"));
        }
        if let Some(db_id) = db_id {
            parts.push(format!("databaseId={db_id}"));
        }
        parts.join(" ")
    }

    /// Build the `<namespace>::<id>` qualified name (upstream
    /// `qualifyStatement`).
    fn qualify_statement(&self, namespace: &str, id: &str) -> (String, String) {
        if !namespace.is_empty() {
            return (format!("{namespace}::{id}"), id.to_string());
        }
        match id.rfind('.') {
            Some(dot) => (
                format!("{}::{}", &id[..dot], &id[dot + 1..]),
                id[dot + 1..].to_string(),
            ),
            None => (id.to_string(), id.to_string()),
        }
    }

    /// The SQL preview docstring (upstream `previewSql`): tags → spaces,
    /// whitespace collapsed, first 200 UTF-16 units.
    fn preview_sql(&self, body: &str) -> String {
        let stripped = js_re(r"<[^>]+>").replace_all(body, " ");
        let collapsed = js_re(r"\s+").replace_all(&stripped, " ");
        let trimmed = collapsed.trim();
        crate::textutil::slice_utf16(trimmed, 200).0
    }
}

/// Finds a byte subslice in a byte slice, returning its start offset.
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Extract MyBatis mapper facts from an `.xml` file.
/// # Errors
///
/// Returns the documented typed error.
pub fn extract(
    _native: &crate::extractor::Extractor,
    path: &RelativePath,
    source: &str,
) -> Result<ExtractionResult, ExtractionError> {
    let mut walker = MybatisWalker::new(path, source);
    if let Some((namespace, mybatis, body_start, body_end)) = walker.find_mapper_root() {
        let file_id = walker.result.nodes[0].id.clone();
        walker.extract_mapper(&file_id, &namespace, mybatis, body_start, body_end);
    }
    Ok(walker.result)
}

/// Whether an `.xml` source has a MyBatis mapper root (`<mapper
/// namespace="...">` or an iBatis `<sqlMap>`), classifying it as a mapper
/// rather than generic XML.
///
/// # Panics
///
/// Panics if a hard-coded regular expression literal fails to compile.
pub fn is_mapper_xml(source: &str) -> bool {
    let stripped = MybatisWalker::strip_xml_comments(source);
    if let Some(captures) = mapper_root_re().captures(&stripped) {
        let attrs = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        if quoted_attr(attrs, "namespace").is_some() {
            return true;
        }
    }
    sqlmap_root_re().is_match(&stripped)
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    /// Extracts an xml fixture by path and source.
    ///
    /// # Panics
    ///
    /// Panics if the path is invalid or extraction fails.
    fn extract_src(file_path: &str, source: &str) -> ExtractionResult {
        let path = RelativePath::new(file_path).expect("valid path");
        extract(
            &crate::extractor::Extractor::new(crate::config::ExtractionConfig::all_native())
                .expect("extractor builds"),
            &path,
            source,
        )
        .expect("xml extraction succeeds")
    }

    #[test]
    /// Non-mapper XML returns only the hashed file node.
    fn non_mapper_xml_is_file_only() {
        let result = extract_src("pom.xml", "<project><groupId>com.x</groupId></project>\n");
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].kind, NodeKind::File);
        assert!(result.nodes[0].id.starts_with("file:"));
        assert!(!result.nodes[0].id.starts_with("file:pom.xml"), "hashed id");
        assert!(result.edges.is_empty());
        assert!(result.references.is_empty());
    }

    #[test]
    /// Mapper statements, includes, and signatures match the frozen shape.
    fn mapper_statements_and_includes() {
        let source = "<mapper namespace=\"com.a.B\">\n  <select id=\"find\" resultType=\"com.a.User\">\n    SELECT <include refid=\"cols\"/> FROM t\n  </select>\n  <sql id=\"cols\">id, name</sql>\n</mapper>\n";
        let result = extract_src("B.xml", source);
        let methods: Vec<&Node> = result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Method)
            .collect();
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0].name, "find");
        assert_eq!(methods[0].qualified_name, "com.a.B::find");
        assert_eq!(
            methods[0].signature.as_deref(),
            Some("SELECT result=com.a.User")
        );
        assert_eq!(methods[1].signature.as_deref(), Some("<sql>"));
        assert!(
            result
                .references
                .iter()
                .any(|r| r.reference_name == "com.a.B::cols")
        );
    }

    #[test]
    /// Commented-out statements produce no phantom nodes.
    fn commented_statements_are_ignored() {
        let source = "<mapper namespace=\"com.a.B\">\n  <!-- <select id=\"old\">SELECT 1</select> -->\n  <select id=\"new\">SELECT 2</select>\n</mapper>\n";
        let result = extract_src("B.xml", source);
        let methods: Vec<&Node> = result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Method)
            .collect();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "new");
    }

    #[test]
    /// Mismatched attribute quotes are rejected exactly like the upstream
    /// `(["'])([^"']+)\\1` backreference: a `<mapper namespace='x">` is not a
    /// mapper (classification and extraction both reject it), and a
    /// statement whose `id` quotes do not match emits nothing.
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn mismatched_quotes_are_rejected() {
        // Mismatched namespace quote — not a mapper, generic file-level XML.
        assert!(!is_mapper_xml("<mapper namespace='com.a.B\">"));
        assert!(!is_mapper_xml("<mapper namespace=\"com.a.B'>"));
        assert!(is_mapper_xml("<mapper namespace='com.a.B'>"));
        assert!(is_mapper_xml("<mapper namespace=\"com.a.B\">"));
        // Mismatched statement id — no method node emitted.
        let source = "<mapper namespace=\"com.a.B\">\n  <select id='x\">SELECT 1</select>\n  <select id=\"y\">SELECT 2</select>\n</mapper>\n";
        let result = extract_src("B.xml", source);
        let methods: Vec<&Node> = result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Method)
            .collect();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "y");
        // Mismatched resultType quote — the statement still emits but the
        // signature omits the malformed attribute.
        let source2 = "<mapper namespace=\"com.a.B\">\n  <select id=\"x\" resultType='com.a.User\">SELECT 1</select>\n</mapper>\n";
        let result2 = extract_src("B.xml", source2);
        let method = result2
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Method)
            .expect("method emitted");
        assert_eq!(method.signature.as_deref(), Some("SELECT"));
        // Mismatched include refid — no reference emitted.
        let source3 = "<mapper namespace=\"com.a.B\">\n  <sql id=\"cols\">id</sql>\n  <select id=\"x\">SELECT <include refid='cols\"/> FROM t</select>\n</mapper>\n";
        let result3 = extract_src("B.xml", source3);
        assert!(
            result3
                .references
                .iter()
                .all(|r| r.reference_name != "com.a.B::cols"),
            "mismatched include quotes must not emit a reference"
        );
    }

    #[test]
    /// Comment stripping preserves offsets and line numbers.
    fn comment_stripping_is_length_preserving() {
        let source = "<!-- c -->\n<select id=\"x\">SELECT 1</select>\n";
        let stripped = MybatisWalker::strip_xml_comments(source);
        assert_eq!(stripped.len(), source.len());
        assert!(stripped.contains('\n'));
    }
}
