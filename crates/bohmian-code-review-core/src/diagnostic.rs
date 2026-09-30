//! Extraction diagnostics — the typed replacement for the upstream
//! `ExtractionError` value. A ready snapshot may carry warnings without
//! aborting unrelated files.

use serde::{Deserialize, Serialize};

/// Severity of an extraction diagnostic (upstream `severity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Error.
    Error,
    /// Warning.
    Warning,
}

/// One file-local extraction diagnostic, mirroring the upstream
/// `ExtractionError` interface. Recorded without aborting unrelated files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionDiagnostic {
    /// Error message.
    pub message: String,
    /// File path where the error occurred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Line number if available (1-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Column number if available (0-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// Severity.
    pub severity: Severity,
    /// Error code for categorization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// Pins serialized enum and field names to the frozen upstream JSON contract.
    ///
    /// # Panics
    ///
    /// Panics if JSON serialization fails or produces a spelling outside the asserted wire contract.
    fn serde_uses_upstream_names() {
        let diagnostic = ExtractionDiagnostic {
            message: "boom".into(),
            file_path: Some("a.ts".into()),
            line: Some(2),
            column: Some(0),
            severity: Severity::Warning,
            code: Some("E1".into()),
        };
        let wire = serde_json::to_value(&diagnostic).unwrap();
        assert_eq!(wire["message"], "boom");
        assert_eq!(wire["filePath"], "a.ts");
        assert_eq!(wire["severity"], "warning");
        let back: ExtractionDiagnostic = serde_json::from_value(wire).unwrap();
        assert_eq!(back, diagnostic);
    }
}
