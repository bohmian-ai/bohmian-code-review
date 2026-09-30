//! File-level-only no-symbol extraction — YAML, Twig, properties, and Play
//! routes.
//!
//! These languages are tracked at the file-record level only: extraction
//! emits zero symbol nodes, edges, and references (upstream
//! `isFileLevelOnlyLanguage`). Framework extractors (Drupal routing YAML,
//! Spring `@Value` resolution) run later and add per-file nodes when they
//! apply; Play `conf/routes` route extraction is the Play framework
//! resolver's, not extraction's.

use crate::result::ExtractionResult;

/// The empty extraction result for a file-level-only language.
pub fn empty_result() -> ExtractionResult {
    ExtractionResult {
        nodes: Vec::new(),
        edges: Vec::new(),
        references: Vec::new(),
        diagnostics: Vec::new(),
    }
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// File-level-only extraction is always empty.
    fn empty_result_is_empty() {
        let result = empty_result();
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
        assert!(result.references.is_empty());
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.fact_count(), 0);
    }
}
