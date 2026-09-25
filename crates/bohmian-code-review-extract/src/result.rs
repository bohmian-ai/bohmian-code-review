//! [`ExtractionResult`] — the native facts for one extracted file.

use bohmian_code_review_core::{Edge, ExtractionDiagnostic, Node, RawReference};
use serde::{Deserialize, Serialize};

/// The synchronous native extraction result for one source file.
///
/// Collections preserve the upstream kernel's emission order exactly; the
/// oracle comparison code (never production extraction) may normalize them.
/// Serde emits the upstream wire names (`errors` for diagnostics,
/// `unresolvedReferences` for references) so native JSON dumps compare
/// directly with oracle artifacts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractionResult {
    /// Extracted nodes (the `file` node first).
    pub nodes: Vec<Node>,
    /// Extracted edges.
    pub edges: Vec<Edge>,
    /// References that could not be resolved yet (wire name
    /// `unresolvedReferences`).
    #[serde(rename = "unresolvedReferences")]
    pub references: Vec<RawReference>,
    /// File-local extraction diagnostics, empty for a clean file (wire name
    /// `errors`).
    #[serde(rename = "errors")]
    pub diagnostics: Vec<ExtractionDiagnostic>,
}

impl ExtractionResult {
    /// The number of extracted facts (nodes + edges + references).
    pub fn fact_count(&self) -> usize {
        self.nodes.len() + self.edges.len() + self.references.len()
    }
}
