//! Delphi DFM/FMX form-file extraction — the composite route over the
//! native DFM/FMX extractor (`crates/codegraph-extract/src/pascal.rs::dfm`,
//! a faithful port of `src/extraction/dfm-extractor.ts`).
//!
//! DFM/FMX files describe the visual component hierarchy and event-handler
//! bindings with a simple `object`/`end` block format; the extractor emits
//! `component` nodes, `contains` edges, and `references` references per
//! `OnXxx = Method` event handler. Repository-wide Pascal companion
//! association and missing-companion edges are resolution-synthesis work
//! (owned elsewhere) and are deliberately not implemented here.

use crate::decode::decode_buffers;
use crate::error::ExtractionError;
use crate::result::ExtractionResult;
use bohmian_code_review_core::{Language, RelativePath};

/// Extract DFM/FMX form facts from a `.dfm`/`.fmx` file.
///
/// # Errors
///
/// Returns the DFM walker's buffer-emission error, or
/// [`ExtractionError::Decode`] when the emitted buffers are malformed.
pub fn extract(
    _native: &crate::extractor::Extractor,
    path: &RelativePath,
    source: &str,
) -> Result<ExtractionResult, ExtractionError> {
    let emit = crate::pascal::dfm::extract_dfm(path.as_str(), source).map_err(|message| {
        ExtractionError::ExtractionFailed {
            language: Language::Pascal,
            message,
        }
    })?;
    decode_buffers(&emit, path.as_str(), Language::Pascal)
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;
    use crate::config::ExtractionConfig;
    use crate::extractor::Extractor;
    use bohmian_code_review_core::NodeKind;

    /// Builds a native extractor over the full native set.
    ///
    /// # Panics
    ///
    /// Panics if extractor construction fails.
    fn native() -> Extractor {
        Extractor::new(ExtractionConfig::all_native()).expect("extractor builds")
    }

    #[test]
    /// DFM forms emit components, containment edges, and event refs.
    /// # Panics
    ///
    /// Panics if a hard-coded regular expression literal fails to compile.
    fn dfm_components_and_events() {
        let source = "object MainForm: TMainForm\n  object Panel1: TPanel\n    object Button1: TButton\n      OnClick = Button1Click\n    end\n  end\nend\n";
        let path = RelativePath::new("Form1.dfm").expect("valid path");
        let result = extract(&native(), &path, source).expect("dfm extraction succeeds");
        assert_eq!(result.nodes[0].kind, NodeKind::File);
        let components: Vec<&str> = result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Component)
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(components, vec!["MainForm", "Panel1", "Button1"]);
        assert!(
            result
                .references
                .iter()
                .any(|r| r.reference_name == "Button1Click")
        );
        assert_eq!(result.edges.len(), 3);
    }
}
