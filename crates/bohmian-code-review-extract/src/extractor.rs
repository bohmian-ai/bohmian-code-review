//! [`Extractor`] — the synchronous native extraction owner.

use crate::composite::{CompositeExtractor, CompositeKind, DiscoveredSource};
use crate::config::ExtractionConfig;
use crate::decode::decode_buffers;
use crate::error::ExtractionError;
use crate::result::ExtractionResult;
use crate::routing::LanguageRouter;
use crate::source_file::SourceFile;
use crate::{
    arkts, ccpp, cfquery, cfscript, cobol, csharp, dart, erlang, go, java, kotlin, lua, nix, objc,
    pascal, php, python, rlang, ruby, rustlang, scala, solidity, swift, terraform, tsjs, vbnet,
};
use bohmian_code_review_core::{Language, RelativePath};

/// The synchronous native extraction owner.
///
/// Owns an [`ExtractionConfig`] and the closed [`LanguageRouter`], and exposes
/// the frozen kernel's per-language extraction as native Rust facts.
/// Construction validates the configuration; `extract` dispatches one
/// pre-classified file to its walker and decodes the walker's
/// flat-buffer tables into an [`ExtractionResult`]. `detect_language` and
/// `extract_detected` route an arbitrary path+source through the router first
/// — the ergonomic entry surface that does not require a pre-classified
/// language.
///
/// The extractor is stateless between calls and safe to share: callers
/// parallelize per file through their own bounded worker pool.
#[derive(Debug, Clone)]
pub struct Extractor {
    /// Validated extraction configuration controlling the enabled native-language set.
    config: ExtractionConfig,
    /// Closed language-routing owner for the detect-first entry surface.
    router: LanguageRouter,
}

impl Extractor {
    /// Construct an extractor over the given configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractionError::InvalidConfig`] when the configuration
    /// enables no languages at all.
    pub fn new(config: ExtractionConfig) -> Result<Self, ExtractionError> {
        if config.enabled_languages().is_empty() {
            return Err(ExtractionError::InvalidConfig {
                message: "enabled_languages must not be empty".to_string(),
            });
        }
        Ok(Extractor {
            config,
            router: LanguageRouter::new(),
        })
    }

    /// The configuration this extractor was constructed with.
    pub fn config(&self) -> &ExtractionConfig {
        &self.config
    }

    /// The language this extractor would attempt for `file_path` from the
    /// path alone, or `None` when the path is unregistered, routes to a
    /// language with no native or composite extractor, or is disabled.
    ///
    /// Intended for filtering before source is downloaded. A `.h` path
    /// reports [`Language::C`] here; content routing may still select C++
    /// or Objective-C at parse time.
    #[must_use]
    pub fn path_language(&self, file_path: &str) -> Option<Language> {
        let language = self.router.detect(file_path, None);
        let supported = CompositeKind::from_discovered(file_path, language).is_some()
            || (language.is_native() && self.config.enabled_languages().contains(&language));
        supported.then_some(language)
    }

    /// Routes a repository-relative path and its source to a language using
    /// the closed frozen routing contract (extension, path specials, and the
    /// first-8-KiB `.h` content heuristics).
    pub fn detect_language(&self, file_path: &str, source: &str) -> Language {
        self.router.detect(file_path, Some(source))
    }

    /// Extracts a repository-relative path and its UTF-8 source, routing the
    /// language first.
    ///
    /// The routed language is then admitted exactly like
    /// [`Extractor::extract`]: rejected with [`ExtractionError::DisabledLanguage`]
    /// when not enabled, [`ExtractionError::UnsupportedLanguage`] when the
    /// routed language has no native grammar, and
    /// [`ExtractionError::SourceNotUtf8`] when the bytes are invalid UTF-8.
    /// Walker failures surface as
    /// [`ExtractionError::ExtractionFailed`].
    ///
    /// # Errors
    ///
    /// Returns the same admission, walker, and decode errors as
    /// [`Extractor::extract`].
    pub fn extract_detected(
        &self,
        file_path: &str,
        source: &str,
    ) -> Result<ExtractionResult, ExtractionError> {
        let language = self.router.detect(file_path, Some(source));
        let path = bohmian_code_review_core::RelativePath::new(file_path).map_err(|error| {
            ExtractionError::InvalidPath {
                message: error.to_string(),
            }
        })?;
        let source_file = SourceFile::new(&path, source.as_bytes(), language);
        self.extract(source_file)
    }

    /// Extract one source file into native facts.
    ///
    /// The file is rejected with [`ExtractionError::DisabledLanguage`] when
    /// its language is not enabled by the configuration, with
    /// [`ExtractionError::UnsupportedLanguage`] when the language has no
    /// native grammar, and with [`ExtractionError::SourceNotUtf8`] when the
    /// bytes are not valid UTF-8. Walker failures surface as
    /// [`ExtractionError::ExtractionFailed`].
    ///
    /// # Errors
    ///
    /// Returns [`ExtractionError::DisabledLanguage`],
    /// [`ExtractionError::UnsupportedLanguage`], or
    /// [`ExtractionError::SourceNotUtf8`] during admission. Walker failures
    /// map to [`ExtractionError::ExtractionFailed`]; malformed ABI tables
    /// return [`ExtractionError::Decode`].
    pub fn extract(&self, source: SourceFile<'_>) -> Result<ExtractionResult, ExtractionError> {
        let language = source.language;
        if !self.config.enabled_languages().contains(&language) {
            return Err(ExtractionError::DisabledLanguage { language });
        }
        if !language.is_native() {
            return Err(ExtractionError::UnsupportedLanguage { language });
        }
        let source_str =
            std::str::from_utf8(source.bytes).map_err(|error| ExtractionError::SourceNotUtf8 {
                message: error.to_string(),
            })?;
        let path = source.path.as_str();

        let emit = Self::run_walker(path, source_str, language)
            .map_err(|message| ExtractionError::ExtractionFailed { language, message })?;

        let mut result = decode_buffers(&emit, path, language)?;
        // CfmlExtractor.extractBareScript stamps `language` on every ref of a
        // `.cfs` file (the wasm oracle does the same for the cfscript and cfml
        // languages); the native cfscript surface reproduces that on its refs.
        if language == Language::Cfscript {
            for reference in &mut result.references {
                reference.language = Some(language);
            }
        }
        Ok(result)
    }

    /// Extract one pre-classified native source file from its raw path and
    /// UTF-8 text (a thin [`SourceFile`] convenience wrapper over
    /// [`Extractor::extract`]).
    ///
    /// # Errors
    ///
    /// Returns [`ExtractionError::InvalidPath`] when `file_path` is not a
    /// valid repository-relative path, and the same admission, walker, and
    /// decode errors as [`Extractor::extract`].
    pub fn extract_file(
        &self,
        file_path: &str,
        source: &str,
        language: Language,
    ) -> Result<ExtractionResult, ExtractionError> {
        let path = RelativePath::new(file_path).map_err(|error| ExtractionError::InvalidPath {
            message: error.to_string(),
        })?;
        self.extract(SourceFile::new(&path, source.as_bytes(), language))
    }

    /// Extract one discovered repository-relative file, routing the language
    /// from its path and source and dispatching native or composite routes
    /// exactly like the frozen upstream `extractFromSource`.
    ///
    /// Play routes and Shopify OS 2.0 JSON route through the same path
    /// specials as [`crate::routing::LanguageRouter`]; composite/Twig routes
    /// delegate to [`CompositeExtractor`]; native files go through
    /// [`Extractor::extract`]. This is the extraction entry the indexing
    /// pipeline and the composite differential corpus use.
    ///
    /// # Errors
    ///
    /// Returns the routed extractor's typed error — see
    /// [`CompositeExtractor::extract`] and [`Extractor::extract`].
    pub fn extract_from_source(
        &self,
        file_path: &str,
        source: &str,
    ) -> Result<ExtractionResult, ExtractionError> {
        let language = self.router.detect(file_path, Some(source));
        self.extract_from_source_with_language(file_path, source, language)
    }

    /// Extract one discovered repository-relative file whose language the
    /// caller already resolved, dispatching native or composite routes
    /// exactly like the frozen upstream `extractFromSource(filePath, content,
    /// language)`.
    ///
    /// This is the entry the indexing handoff uses: discovery resolves the
    /// language once — honoring the project's custom extension overrides,
    /// which this crate's built-in router deliberately does not carry — and
    /// that exact decision selects the walker and reaches persistence
    /// unchanged. Re-deriving the language here would silently drop an
    /// override and route the file to a different grammar.
    ///
    /// # Errors
    ///
    /// Returns the routed extractor's typed error — see
    /// [`CompositeExtractor::extract`] and [`Extractor::extract`], plus
    /// [`ExtractionError::InvalidPath`] when `file_path` is not a valid
    /// repository-relative path.
    pub fn extract_from_source_with_language(
        &self,
        file_path: &str,
        source: &str,
        language: Language,
    ) -> Result<ExtractionResult, ExtractionError> {
        let path = RelativePath::new(file_path).map_err(|error| ExtractionError::InvalidPath {
            message: error.to_string(),
        })?;
        let content_sha256 = hex_digest(source.as_bytes());
        let discovered = DiscoveredSource::new(path, language, content_sha256);
        CompositeExtractor::new(self.clone()).extract(&discovered, source)
    }

    /// Dispatch one file to the frozen kernel's per-language walker.
    /// # Errors
    ///
    /// Returns the selected walker’s grammar-installation, parser-setup,
    /// or null-tree failure without changing its message.
    fn run_walker(
        path: &str,
        source: &str,
        language: Language,
    ) -> Result<crate::buffers::EmitOut, String> {
        match language {
            Language::Java => java::extract(path, source),
            Language::Python => python::extract(path, source),
            Language::Go => go::extract(path, source),
            Language::C | Language::Cpp => ccpp::extract(path, source, language.as_str()),
            Language::Rust => rustlang::extract(path, source),
            Language::Csharp => csharp::extract(path, source),
            Language::Ruby => ruby::extract(path, source),
            Language::Php => php::extract(path, source),
            Language::Swift => swift::extract(path, source),
            Language::Kotlin => kotlin::extract(path, source),
            Language::R => rlang::extract(path, source),
            Language::Lua | Language::Luau => lua::extract(path, source, language.as_str()),
            Language::Scala => scala::extract(path, source),
            Language::Dart => dart::extract(path, source),
            Language::Typescript | Language::Tsx | Language::Javascript | Language::Jsx => {
                tsjs::extract(path, source, language.as_str())
            }
            Language::Objc => objc::extract(path, source),
            Language::Solidity => solidity::extract(path, source),
            Language::Terraform => terraform::extract(path, source),
            Language::Arkts => arkts::extract(path, source),
            Language::Nix => nix::extract(path, source),
            Language::Pascal => pascal::extract(path, source),
            Language::Cfscript => cfscript::extract(path, source),
            Language::Cfquery => cfquery::extract(path, source),
            Language::Cobol => cobol::extract(path, source),
            Language::Vbnet => vbnet::extract(path, source),
            Language::Erlang => erlang::extract(path, source),
            _ => Err(format!("no native walker for {language}")),
        }
    }
}

/// The lower-case hex SHA-256 digest of `bytes` (upstream
/// `hashContent`/`crypto.createHash('sha256')`).
fn hex_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;
    use bohmian_code_review_core::RelativePath;
    use std::collections::BTreeSet;

    /// Returns the validated source bytes borrowed by this extraction request.
    fn source<'a>(path: &'a RelativePath, bytes: &'a [u8], language: Language) -> SourceFile<'a> {
        SourceFile::new(path, bytes, language)
    }

    #[test]
    /// Path-only routing reports native and composite languages, honors the
    /// enabled set, and reports nothing for unregistered or extractor-less
    /// languages.
    fn path_language_reports_eligibility_without_source() {
        let all = Extractor::new(ExtractionConfig::all_native()).expect("valid config");
        assert_eq!(all.path_language("src/a.ts"), Some(Language::Typescript));
        assert_eq!(all.path_language("src/a.h"), Some(Language::C));
        assert_eq!(all.path_language("src/App.svelte"), Some(Language::Svelte));
        assert_eq!(all.path_language("conf/routes"), Some(Language::Yaml));
        assert_eq!(all.path_language("README.md"), None);
        assert_eq!(all.path_language("Makefile"), None);
        let python_only = Extractor::new(ExtractionConfig::new(BTreeSet::from([Language::Python])))
            .expect("valid config");
        assert_eq!(python_only.path_language("a.py"), Some(Language::Python));
        assert_eq!(python_only.path_language("a.ts"), None);
    }

    #[test]
    /// Rejects extractor construction when no native language is enabled.
    fn empty_configuration_is_rejected() {
        let error = Extractor::new(ExtractionConfig::new(BTreeSet::new())).unwrap_err();
        assert!(matches!(error, ExtractionError::InvalidConfig { .. }));
    }

    #[test]
    /// Returns the typed disabled-language error before parser initialization.
    ///
    /// # Panics
    ///
    /// Panics if extractor setup fails or extraction does not return `DisabledLanguage`.
    fn disabled_language_is_rejected() {
        let mut languages = BTreeSet::new();
        languages.insert(Language::Java);
        let extractor = Extractor::new(ExtractionConfig::new(languages)).unwrap();
        let path = RelativePath::new("a.py").unwrap();
        let error = extractor
            .extract(source(&path, b"x = 1\n", Language::Python))
            .unwrap_err();
        assert!(matches!(error, ExtractionError::DisabledLanguage { .. }));
    }

    #[test]
    /// Returns the typed unsupported-language error for non-native contract values.
    ///
    /// # Panics
    ///
    /// Panics if extractor setup fails or extraction does not return `UnsupportedLanguage`.
    fn unsupported_language_is_rejected() {
        // A non-native language passes the enabled check only when the
        // configuration explicitly enables it; then the extractor must
        // reject it as unsupported.
        let mut languages = BTreeSet::new();
        languages.insert(Language::Svelte);
        let extractor = Extractor::new(ExtractionConfig::new(languages)).unwrap();
        let path = RelativePath::new("a.svelte").unwrap();
        let error = extractor
            .extract(source(&path, b"<script>let x</script>", Language::Svelte))
            .unwrap_err();
        assert!(matches!(error, ExtractionError::UnsupportedLanguage { .. }));
    }

    #[test]
    /// Rejects non-UTF-8 source bytes before tree-sitter parsing.
    ///
    /// # Panics
    ///
    /// Panics if extractor setup fails or invalid UTF-8 does not return `SourceNotUtf8`.
    fn invalid_utf8_is_rejected() {
        let extractor = Extractor::new(ExtractionConfig::all_native()).unwrap();
        let path = RelativePath::new("a.py").unwrap();
        let error = extractor
            .extract(source(&path, &[0xff, 0xfe, 0x00], Language::Python))
            .unwrap_err();
        assert!(matches!(error, ExtractionError::SourceNotUtf8 { .. }));
    }

    /// Extracts `bytes` at `path` and returns every recovered declaration name.
    ///
    /// # Panics
    ///
    /// Panics if the path is invalid or extraction fails.
    fn recovered_names(path: &str, bytes: &[u8], language: Language) -> BTreeSet<String> {
        let extractor = Extractor::new(ExtractionConfig::all_native()).unwrap();
        let path = RelativePath::new(path).unwrap();
        extractor
            .extract(source(&path, bytes, language))
            .expect("a syntax error keeps the recovered tree")
            .nodes
            .into_iter()
            .map(|node| node.name)
            .collect()
    }

    #[test]
    /// A Rust attribute on a destructured field, which tree-sitter-rust
    /// cannot parse, drops only that region; the declarations around it stay.
    ///
    /// # Panics
    ///
    /// Panics if extraction fails or a surrounding declaration is missing.
    fn recovered_tree_keeps_rust_declarations_around_a_grammar_gap() {
        let source = b"pub struct Boot { pub port: u16 }\n\npub fn start(boot: Boot) -> u16 {\n    let Boot {\n        #[cfg(feature = \"x\")]\n        port,\n    } = boot;\n    port\n}\n\npub fn stop() {}\n";
        let names = recovered_names("src/boot/mod.rs", source, Language::Rust);
        for name in ["Boot", "start", "stop"] {
            assert!(names.contains(name), "missing `{name}` in {names:?}");
        }
    }

    #[test]
    /// A raw NUL byte inside a TypeScript string literal drops only that
    /// region; the declarations around it stay.
    ///
    /// # Panics
    ///
    /// Panics if extraction fails or a surrounding declaration is missing.
    fn recovered_tree_keeps_typescript_declarations_around_a_nul_byte() {
        let source = b"export function before(): string {\n  return \"a\";\n}\n\nconst SEP = \"\x00\";\n\nexport function after(): string {\n  return SEP;\n}\n";
        let names = recovered_names("src/lib/server/wyrd.ts", source, Language::Typescript);
        for name in ["before", "after"] {
            assert!(names.contains(name), "missing `{name}` in {names:?}");
        }
    }
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod routing_tests {
    use super::*;

    #[test]
    /// The detect-first entry surface routes `.h` headers by content.
    ///
    /// # Panics
    ///
    /// Panics if extractor setup fails or the routed extraction fails.
    fn extract_detected_routes_header_content() {
        let extractor = Extractor::new(ExtractionConfig::all_native()).unwrap();
        let objc_header =
            "#import <Foundation/Foundation.h>\n@interface Greeter : NSObject\n@end\n";
        let result = extractor
            .extract_detected("Greeter.h", objc_header)
            .expect("routed extraction succeeds");
        let classes: Vec<&str> = result
            .nodes
            .iter()
            .filter(|n| n.kind == bohmian_code_review_core::NodeKind::Class)
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(classes, vec!["Greeter"]);
    }

    #[test]
    /// The detect-first entry surface routes the systems/configuration extensions to their grammars.
    ///
    /// # Panics
    ///
    /// Panics if extractor setup fails or a routed extraction fails.
    fn extract_detected_routes_native_extensions() {
        let extractor = Extractor::new(ExtractionConfig::all_native()).unwrap();
        let cases = [
            ("A.m", "@interface A\n@end\n", "class"),
            ("B.sol", "contract B {}\n", "class"),
            ("C.tf", "variable \"c\" {}\n", "variable"),
            ("D.ets", "@Component\nstruct D {}\n", "struct"),
            ("E.nix", "{ e = 1; }\n", "variable"),
        ];
        for (path, source, expected_kind) in cases {
            let result = extractor
                .extract_detected(path, source)
                .unwrap_or_else(|error| panic!("{path}: {error}"));
            assert!(
                result
                    .nodes
                    .iter()
                    .any(|n| n.kind.wire_name() == expected_kind),
                "{path} must route to a {expected_kind}"
            );
        }
        // An unregistered extension routes to Unknown → typed rejection
        // (Unknown is not in the enabled native set).
        let error = extractor.extract_detected("F.xyz", "whatever").unwrap_err();
        assert!(matches!(error, ExtractionError::DisabledLanguage { .. }));
    }
}
