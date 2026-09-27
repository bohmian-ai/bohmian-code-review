//! ArkTS extraction — the ArkTS slice of the shared TS/JS walker
//! (`crate::tsjs`) plus `languages/arkts.ts`.
//!
//! ArkTS (HarmonyOS / OpenHarmony, `.ets`) is a TypeScript superset whose
//! headline feature is declarative UI: an `@Component struct` with a
//! `build()` method. The vendored grammar (harmony-contrib/tree-sitter-arkts
//! 0.2.0 = 56f7fc28, MIT) extends tree-sitter-javascript exactly the way
//! tree-sitter-typescript does, so the entire TS/JS walker applies verbatim;
//! upstream composes the ArkTS extractor as `{ ...typescriptExtractor, … }`
//! and this module mirrors that composition by routing the ArkTS variant
//! through the shared walker.
//!
//! ArkTS-specific shapes: `struct_declaration` / `struct_body` (the
//! `@Component struct`, same `name:`/`body:` fields as class_declaration);
//! `arkui_component_expression` build()-DSL component instantiations with
//! chained `property:`/`arguments:` field pairs on the SAME node (the
//! walker's arkts extractCall branch); `leading_dot_expression` detached
//! chains; and first-class decorators (`@Builder`, `@Entry`, `@State`, …)
//! surfaced on the node's `decorators` list. ArkTS IS in VALUE_REF_LANGS and
//! TYPE_ANNOTATION_LANGUAGES but NOT in FN_REF_SPECS (no function-as-value
//! capture).
//!
//! The frozen oracle is the vendored `src/extraction/wasm/
//! tree-sitter-arkts.wasm`, byte-identical to npm tree-sitter-arkts 0.2.0's
//! package artifact; the parser.c + scanner.c are vendored under
//! `crates/codegraph-extract/grammars/arkts` and compiled natively — table
//! identity by construction (ABI 14). Positions are emitted in UTF-16 code
//! units.

use crate::buffers::EmitOut;

/// Parses one ArkTS source file and returns its flat facts, diagnostics, and measured kernel duration.
///
/// # Errors
///
/// Returns an error when the native grammar is unavailable, tree-sitter rejects grammar installation, or parsing returns no tree.
pub fn extract(file_path: &str, source: &str) -> Result<EmitOut, String> {
    crate::tsjs::extract(file_path, source, "arkts")
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
        let emit = extract(path, source).expect("arkts extraction succeeds");
        crate::decode::decode_buffers(&emit, path, bohmian_code_review_core::Language::Arkts)
            .expect("arkts decode succeeds")
    }

    #[test]
    /// `@Component struct` extracts as a struct node with its decorators.
    ///
    /// # Panics
    ///
    /// Panics if arkts extraction fails or an expected node is missing.
    fn component_struct_extracts_with_decorators() {
        let src = "@Component\nstruct Hello {\n  @State count: number = 0\n  build() {}\n}\n";
        let result = extract_source("s.ets", src);
        let hello = result
            .nodes
            .iter()
            .find(|n| n.kind == bohmian_code_review_core::NodeKind::Struct && n.name == "Hello")
            .expect("Hello struct node");
        assert_eq!(
            hello.decorators.as_deref(),
            Some(&["Component".to_string()][..])
        );
        let count = result
            .nodes
            .iter()
            .find(|n| n.name == "count")
            .expect("count property node");
        assert_eq!(
            count.decorators.as_deref(),
            Some(&["State".to_string()][..])
        );
    }

    #[test]
    /// `@Builder` functions extract with their decorator surfaced.
    ///
    /// # Panics
    ///
    /// Panics if arkts extraction fails or an expected node is missing.
    fn builder_function_surfaces_decorator() {
        let src = "@Builder\nfunction helloBuilder() {}\n";
        let result = extract_source("s.ets", src);
        let f = result
            .nodes
            .iter()
            .find(|n| {
                n.kind == bohmian_code_review_core::NodeKind::Function && n.name == "helloBuilder"
            })
            .expect("helloBuilder node");
        assert_eq!(f.decorators.as_deref(), Some(&["Builder".to_string()][..]));
    }

    #[test]
    /// build()-DSL chained attributes emit dot-prefixed calls; the component
    /// itself stays a plain call.
    fn build_dsl_chains_emit_dot_prefixed_calls() {
        let src = "@Component\nstruct Hello {\n  build() {\n    Column({ space: 8 }) {\n      Text(\"Hi\").fontSize(20)\n    }.width(\"100%\")\n  }\n}\n";
        let result = extract_source("s.ets", src);
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
        assert!(calls.contains(&"Column"));
        assert!(calls.contains(&"Text"));
        assert!(calls.contains(&".fontSize"));
        assert!(calls.contains(&".width"));
    }

    #[test]
    /// `@Entry @Component` decorations land on the struct (sibling walk).
    ///
    /// # Panics
    ///
    /// Panics if arkts extraction fails or an expected node is missing.
    fn entry_component_sibling_decorators() {
        let src = "@Entry\n@Component\nstruct Page {}\n";
        let result = extract_source("s.ets", src);
        let page = result
            .nodes
            .iter()
            .find(|n| n.kind == bohmian_code_review_core::NodeKind::Struct && n.name == "Page")
            .expect("Page struct node");
        let decorators: Vec<&str> = page
            .decorators
            .as_ref()
            .map(|d| d.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();
        assert_eq!(decorators, vec!["Entry", "Component"]);
    }
}
