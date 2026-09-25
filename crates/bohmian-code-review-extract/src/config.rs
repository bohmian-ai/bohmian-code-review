//! [`ExtractionConfig`] — the extractor's configuration.

use crate::error::ExtractionError;
use bohmian_code_review_core::Language;
use std::collections::BTreeSet;

/// Configuration for the synchronous native extractor.
///
/// Owns the set of languages the [`crate::Extractor`] may be asked to
/// extract. Languages outside the set fail with a typed
/// [`ExtractionError::DisabledLanguage`] before any parsing; languages with
/// no native grammar fail with [`ExtractionError::UnsupportedLanguage`].
/// The default enables every native language compiled into this kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionConfig {
    /// Validated closed set of languages the extractor is permitted to parse.
    enabled_languages: BTreeSet<Language>,
}

impl ExtractionConfig {
    /// Build a configuration over the given enabled languages.
    ///
    /// The set is stored in [`BTreeSet`] order for deterministic iteration.
    pub fn new(enabled_languages: BTreeSet<Language>) -> Self {
        ExtractionConfig { enabled_languages }
    }

    /// The configuration that enables every native language compiled into this kernel.
    pub fn all_native() -> Self {
        ExtractionConfig {
            enabled_languages: bohmian_code_review_core::NATIVE_LANGUAGES.iter().copied().collect(),
        }
    }

    /// The enabled languages (deterministically ordered).
    pub fn enabled_languages(&self) -> &BTreeSet<Language> {
        &self.enabled_languages
    }
}

impl Default for ExtractionConfig {
    /// Enables exactly the thirty-one native languages compiled into this kernel.
    fn default() -> Self {
        ExtractionConfig::all_native()
    }
}

impl TryFrom<BTreeSet<Language>> for ExtractionConfig {
    /// Names the Error representation used by this extraction stage.
    type Error = ExtractionError;

    /// Reject an empty language set: an extractor that can never extract
    /// anything is a configuration error.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractionConfigError::NoLanguagesEnabled`] when the set is empty.
    fn try_from(languages: BTreeSet<Language>) -> Result<Self, Self::Error> {
        if languages.is_empty() {
            return Err(ExtractionError::InvalidConfig {
                message: "enabled_languages must not be empty".to_string(),
            });
        }
        Ok(ExtractionConfig::new(languages))
    }
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// Ensures default configuration enables exactly the frozen native language set.
    fn default_enables_all_native_languages() {
        let config = ExtractionConfig::default();
        assert_eq!(config.enabled_languages().len(), 31);
        for language in bohmian_code_review_core::NATIVE_LANGUAGES {
            assert!(config.enabled_languages().contains(&language));
        }
        assert!(!config.enabled_languages().contains(&Language::Svelte));
    }

    #[test]
    /// Rejects configurations that cannot route any source file.
    fn empty_set_is_rejected() {
        let result = ExtractionConfig::try_from(BTreeSet::new());
        assert!(matches!(result, Err(ExtractionError::InvalidConfig { .. })));
    }
}
