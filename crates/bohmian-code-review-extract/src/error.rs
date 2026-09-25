//! [`ExtractionError`] — the typed error contract of the native extractor.

use bohmian_code_review_core::Language;
use thiserror::Error;

/// Errors produced by [`crate::Extractor::new`] and
/// [`crate::Extractor::extract`].
///
/// The variants preserve the frozen kernel's failure distinctions: an
/// unsupported language is a routing error, a parse-tree-with-errors file is
/// [`ExtractionError::Deferred`] (upstream deferred to the wasm fallback with
/// a `"defer:"` signal), and any other walker failure is
/// [`ExtractionError::ExtractionFailed`]. Malformed buffers and invalid
/// configuration are separate typed errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExtractionError {
    /// The language has no native grammar in this crate (a non-native member
    /// of the 42-value contract, e.g. `svelte` or `objc`).
    #[error("language `{language}` is not supported by the native extractor")]
    UnsupportedLanguage {
        /// The rejected language.
        language: Language,
    },
    /// The language is native but not enabled in the extraction
    /// configuration.
    #[error("language `{language}` is disabled by the extraction configuration")]
    DisabledLanguage {
        /// The disabled language.
        language: Language,
    },
    /// The source bytes are not valid UTF-8; the kernel seam consumed
    /// UTF-8 strings.
    #[error("source is not valid UTF-8: {message}")]
    SourceNotUtf8 {
        /// The UTF-8 conversion error message.
        message: String,
    },
    /// The file's parse tree contains errors and extraction was deferred
    /// (upstream would fall back to the wasm extractor's error recovery).
    #[error("extraction deferred for `{language}`: {message}")]
    Deferred {
        /// The deferred language.
        language: Language,
        /// The walker's deferral message.
        message: String,
    },
    /// The walker failed for another reason (grammar setup, null parse).
    #[error("extraction failed for `{language}`: {message}")]
    ExtractionFailed {
        /// The failing language.
        language: Language,
        /// The walker's error message.
        message: String,
    },
    /// The extraction configuration is invalid.
    #[error("invalid extraction configuration: {message}")]
    InvalidConfig {
        /// Why the configuration is invalid.
        message: String,
    },
    /// The flat buffer tables are malformed or do not match the wire ABI.
    #[error("malformed extraction buffers: {message}")]
    Decode {
        /// Why decoding failed.
        message: String,
    },
    /// A repository-relative path is not valid for the routed entry surface.
    #[error("invalid repository-relative path: {message}")]
    InvalidPath {
        /// Why the path was rejected.
        message: String,
    },
}

/// The canonical native deferral message: a parse tree containing errors is
/// routed to the wasm recovery path. This exact spelling is a stable wire
/// contract — the defer contract tests assert it verbatim.
pub const CANONICAL_DEFER_MESSAGE: &str =
    "defer: parse tree contains errors — wasm recovery is canonical";

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// Pins extraction errors as thread-safe values with stable human-readable messages.
    fn errors_are_send_sync_and_display() {
        let error = ExtractionError::UnsupportedLanguage {
            language: Language::Svelte,
        };
        assert!(error.to_string().contains("svelte"));
        /// Requires extraction errors to satisfy `Send + Sync` at compile time.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ExtractionError>();
    }
}
