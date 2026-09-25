//! OTP application resource extraction — the `.app`/`.app.src` composite
//! route.
//!
//! Erlang application resource files are Erlang TERMS the native Erlang
//! grammar parses as top-level expressions; the native Erlang walker's
//! application-tuple handler turns `{mod, {Mod, _}}` and
//! `{applications, [...]}` into entry-module and dependency references. The
//! composite route simply delegates the whole file to the native Erlang
//! extractor (upstream routes `.app`/`.app.src` through the erlang grammar
//! path).

use crate::error::ExtractionError;
use crate::extractor::Extractor;
use crate::result::ExtractionResult;
use crate::source_file::SourceFile;
use bohmian_code_review_core::{Language, RelativePath};

/// Extract Erlang application-resource facts from an `.app`/`.app.src` file.
///
/// # Errors
///
/// Returns the native Erlang extractor's typed error when the file fails
/// native extraction.
pub fn extract(
    native: &Extractor,
    path: &RelativePath,
    source: &str,
) -> Result<ExtractionResult, ExtractionError> {
    native.extract(SourceFile::new(path, source.as_bytes(), Language::Erlang))
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

    #[test]
    /// Application dependency and entry-module references are emitted.
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn app_resource_references() {
        let source = "{application, demo,\n [{description, \"Demo\"},\n  {applications, [kernel, stdlib]},\n  {mod, {demo_app, []}}]}.\n";
        let path = RelativePath::new("src/demo.app.src").expect("valid path");
        let result = extract(&native(), &path, source).expect("erlang app extraction succeeds");
        assert_eq!(result.nodes[0].kind, bohmian_code_review_core::NodeKind::File);
        let names: Vec<&str> = result
            .references
            .iter()
            .map(|r| r.reference_name.as_str())
            .collect();
        assert!(names.contains(&"kernel"));
        assert!(names.contains(&"stdlib"));
        assert!(names.contains(&"demo_app"));
    }
}
