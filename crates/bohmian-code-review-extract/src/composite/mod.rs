//! Composite/template extraction — a faithful Rust port of the upstream
//! composite and special-file routes (`src/extraction/{svelte,vue,astro,
//! liquid,razor,cfml,dfm,mybatis}-extractor.ts`) plus the file-level-only
//! no-symbol routes (YAML, Twig, properties, Play routes).
//!
//! [`CompositeExtractor`] is the stateful dispatcher over the closed
//! [`CompositeKind`] set. Its `extract` reproduces the frozen
//! `extractFromSource` dispatch order from `src/extraction/tree-sitter.ts`
//! exactly: Svelte → Vue → Astro → Liquid → Razor → XML (MyBatis) →
//! CFML/CFScript → file-level-only no-symbol languages (YAML, Twig,
//! properties) → Pascal `.dfm`/`.fmx` form files → native kernel. Native
//! delegation (the `<script>`/frontmatter/`@code`/`<cfscript>`/`<cfquery>`
//! embedded fragments) runs through the shared native [`crate::Extractor`],
//! and every fragment span is remapped to the containing file's
//! coordinates with the upstream offset convention.
//!
//! The composite extractors replicate upstream regex/line behavior exactly:
//! JavaScript `\w`/`\b` semantics (compiled with `unicode(false)`), JS
//! string offsets counted in UTF-16 code units, `match.index`-based columns
//! against the containing line, and the upstream emission order for nodes,
//! edges, and unresolved references. `is_play_routes_file`,
//! `is_shopify_liquid_json`, and `is_erlang_app_file` ride the same frozen
//! predicates as [`crate::routing`].

use crate::error::ExtractionError;
use crate::extractor::Extractor;
use crate::result::ExtractionResult;
use crate::routing::is_erlang_app_file;
use bohmian_code_review_core::{Language, RelativePath};

/// The composite/Twig/YAML/properties extraction modules.
pub mod astro;
/// The CFML (tag-based + bare-script) extraction module.
pub mod cfml;
/// The Delphi DFM/FMX form-file extraction module.
pub mod dfm;
/// The OTP `.app`/`.app.src` extraction module.
pub mod erlang_app;
/// The file-level-only no-symbol extraction module.
pub mod file_level;
/// The Liquid / Shopify extraction module.
pub mod liquid;
/// The MyBatis mapper XML extraction module.
pub mod mybatis;
/// The Razor/Blazor extraction module.
pub mod razor;
/// The Svelte extraction module.
pub mod svelte;
/// Shared regex and coordinate helpers for the composite extractors.
mod util;
/// The Vue extraction module.
pub mod vue;

/// The closed set of non-native extraction routes this crate owns.
///
/// Mirrors the frozen upstream composite/special extractor routes: one value
/// per route, matching the closed extraction contract. `Cfscript` is deliberately
/// absent — `.cfs` files route through the CFML extractor's bare-script
/// delegation (the upstream `CfmlExtractor` contract), so the kind reuses
/// [`CompositeKind::Cfml`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompositeKind {
    /// `.svelte` components.
    Svelte,
    /// `.vue` single-file components.
    Vue,
    /// `.astro` components.
    Astro,
    /// `.liquid` templates and Shopify OS 2.0 JSON templates/sections.
    Liquid,
    /// `.cshtml`/`.razor` markup.
    Razor,
    /// YAML files (file-level tracking only).
    Yaml,
    /// Twig templates (file-level tracking only).
    Twig,
    /// Generic XML (MyBatis mapper or file-level only).
    Xml,
    /// Spring `.properties` files (file-level tracking only).
    Properties,
    /// CFML `.cfc`/`.cfm`/`.cfs` (tag-based or bare-script dialect).
    Cfml,
    /// Delphi `.dfm` form files.
    Dfm,
    /// Delphi `.fmx` form files.
    Fmx,
    /// MyBatis mapper XML.
    MyBatis,
    /// Play Framework `conf/routes` (file-level only — route extraction is
    /// the framework resolver's, not extraction's).
    PlayRoutes,
    /// OTP `.app`/`.app.src` resource files (native Erlang terms).
    ErlangApp,
}

impl CompositeKind {
    /// The upstream wire spelling of the kind's containing language (the
    /// `Language` value the kind routes through; `dfm`/`fmx`/`playRoutes`/
    /// `erlangApp` spell their containing language).
    pub fn wire_name(self) -> &'static str {
        match self {
            CompositeKind::Svelte => "svelte",
            CompositeKind::Vue => "vue",
            CompositeKind::Astro => "astro",
            CompositeKind::Liquid => "liquid",
            CompositeKind::Razor => "razor",
            CompositeKind::Yaml => "yaml",
            CompositeKind::Twig => "twig",
            CompositeKind::Xml => "xml",
            CompositeKind::Properties => "properties",
            CompositeKind::Cfml => "cfml",
            CompositeKind::Dfm | CompositeKind::Fmx => "pascal",
            CompositeKind::MyBatis => "xml",
            CompositeKind::PlayRoutes => "yaml",
            CompositeKind::ErlangApp => "erlang",
        }
    }

    /// Classify a discovered repository-relative path + routed language into
    /// the closed composite kind, or `None` when the file is a native-language
    /// file (or an unregistered extension).
    ///
    /// Path specials (Play routes, Shopify JSON, Erlang app files, DFM/FMX
    /// forms) are recognized before the language falls through to the plain
    /// composite languages, mirroring the frozen routing precedence. Without
    /// source content, an XML file classifies as the generic
    /// [`CompositeKind::Xml`] kind; [`CompositeKind::classify`] distinguishes
    /// mapper-shaped XML ([`CompositeKind::MyBatis`]) when content is given.
    pub fn from_discovered(path: &str, language: Language) -> Option<CompositeKind> {
        Self::classify(path, language, None)
    }

    /// Classify a discovered path + routed language into the closed
    /// composite kind, using the source content when available to
    /// distinguish mapper-shaped XML ([`CompositeKind::MyBatis`]) from
    /// generic XML ([`CompositeKind::Xml`]).
    pub fn classify(path: &str, language: Language, source: Option<&str>) -> Option<CompositeKind> {
        let lower = path.to_ascii_lowercase();
        let ext = lower.rfind('.').map(|dot| &lower[dot..]).unwrap_or("");
        match language {
            Language::Svelte => Some(CompositeKind::Svelte),
            Language::Vue => Some(CompositeKind::Vue),
            Language::Astro => Some(CompositeKind::Astro),
            Language::Liquid => Some(CompositeKind::Liquid),
            Language::Razor => Some(CompositeKind::Razor),
            Language::Xml => {
                if source.is_some_and(mybatis::is_mapper_xml) {
                    Some(CompositeKind::MyBatis)
                } else {
                    Some(CompositeKind::Xml)
                }
            }
            Language::Cfml | Language::Cfscript => Some(CompositeKind::Cfml),
            Language::Yaml => {
                if crate::routing::is_play_routes_file(path) {
                    Some(CompositeKind::PlayRoutes)
                } else {
                    Some(CompositeKind::Yaml)
                }
            }
            Language::Twig => Some(CompositeKind::Twig),
            Language::Properties => Some(CompositeKind::Properties),
            Language::Pascal if ext == ".dfm" => Some(CompositeKind::Dfm),
            Language::Pascal if ext == ".fmx" => Some(CompositeKind::Fmx),
            Language::Erlang if is_erlang_app_file(path) => Some(CompositeKind::ErlangApp),
            _ => None,
        }
    }
}

/// One discovered source file: the validated repository-relative path, the
/// routed language, and the SHA-256 digest of the file's content.
///
/// `relative_path` is root-relative, slash-normalized, escape-free, and
/// deterministically ordered (see `codegraph::source::SourceDiscovery`);
/// `language` is the frozen routing outcome; `content_sha256` is the
/// lower-case hex digest used for change detection and manifest identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSource {
    /// Validated repository-relative path (slash-normalized, escape-free).
    relative_path: RelativePath,
    /// Routed language of the file.
    language: Language,
    /// Lower-case hex SHA-256 of the file content.
    content_sha256: String,
}

impl DiscoveredSource {
    /// Construct a discovered source.
    pub fn new(
        relative_path: RelativePath,
        language: Language,
        content_sha256: String,
    ) -> DiscoveredSource {
        DiscoveredSource {
            relative_path,
            language,
            content_sha256,
        }
    }

    /// The validated repository-relative path.
    pub fn relative_path(&self) -> &RelativePath {
        &self.relative_path
    }

    /// The routed language.
    pub fn language(&self) -> Language {
        self.language
    }

    /// The lower-case hex SHA-256 of the file content.
    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }
}

/// The stateful composite/Twig extraction dispatcher.
///
/// Owns the shared native [`Extractor`] used to parse every embedded
/// fragment (Svelte/Vue/Astro `<script>` and frontmatter blocks, Razor
/// `@code` blocks, CFML `<cfscript>`/`<cfquery>` bodies, Erlang app terms).
/// `extract` reproduces the frozen upstream `extractFromSource` dispatch
/// over the closed [`CompositeKind`] set and the native fallback, so one
/// call classifies and extracts any discovered source file.
#[derive(Debug, Clone)]
pub struct CompositeExtractor {
    /// Shared native extractor for embedded-fragment and native-file delegation.
    native: Extractor,
}

impl CompositeExtractor {
    /// Construct the composite dispatcher over the given native extractor.
    pub fn new(native: Extractor) -> CompositeExtractor {
        CompositeExtractor { native }
    }

    /// The shared native extractor.
    pub fn native(&self) -> &Extractor {
        &self.native
    }

    /// Extract one discovered source file, mirroring the frozen upstream
    /// `extractFromSource` dispatch order.
    ///
    /// The upstream dispatch order is preserved exactly: Svelte, Vue, Astro,
    /// Liquid, Razor, XML (MyBatis mapper), CFML/CFScript, file-level-only
    /// languages (YAML, Twig, properties → empty result), Pascal
    /// `.dfm`/`.fmx` form files, then the native kernel. Embedded fragment
    /// spans are remapped to the containing file with the upstream offset
    /// convention; emitted facts preserve upstream IDs, names, qualified
    /// names, signatures, metadata, positions, edges, raw references, and
    /// ordering.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractionError::UnsupportedLanguage`] for an unregistered
    /// or unsupported language, and the native extractor's typed errors
    /// ([`ExtractionError::DisabledLanguage`],
    /// [`ExtractionError::SourceNotUtf8`], [`ExtractionError::Deferred`],
    /// [`ExtractionError::ExtractionFailed`]) when the routed file or an
    /// embedded fragment fails native extraction.
    pub fn extract(
        &self,
        source: &DiscoveredSource,
        text: &str,
    ) -> Result<ExtractionResult, ExtractionError> {
        let file_path = source.relative_path.as_str();
        let detected = source.language;
        // Dispatch on the closed composite kind (content-aware for XML so
        // mapper-shaped XML routes to MyBatis and generic XML routes to the
        // generic Xml behavior); native files fall through to the kernel.
        match CompositeKind::classify(file_path, detected, Some(text)) {
            Some(CompositeKind::Svelte) => {
                svelte::extract(&self.native, &source.relative_path, text)
            }
            Some(CompositeKind::Vue) => vue::extract(&self.native, &source.relative_path, text),
            Some(CompositeKind::Astro) => astro::extract(&self.native, &source.relative_path, text),
            Some(CompositeKind::Liquid) => {
                liquid::extract(&self.native, &source.relative_path, text)
            }
            Some(CompositeKind::Razor) => razor::extract(&self.native, &source.relative_path, text),
            // Both XML kinds run the MyBatis module: mapper-shaped XML emits
            // statements; generic XML emits only the file node.
            Some(CompositeKind::Xml) | Some(CompositeKind::MyBatis) => {
                mybatis::extract(&self.native, &source.relative_path, text)
            }
            Some(CompositeKind::Cfml) => {
                cfml::extract(&self.native, &source.relative_path, text, detected)
            }
            Some(CompositeKind::Yaml)
            | Some(CompositeKind::Twig)
            | Some(CompositeKind::Properties)
            | Some(CompositeKind::PlayRoutes) => Ok(file_level::empty_result()),
            Some(CompositeKind::Dfm) | Some(CompositeKind::Fmx) => {
                dfm::extract(&self.native, &source.relative_path, text)
            }
            Some(CompositeKind::ErlangApp) => {
                erlang_app::extract(&self.native, &source.relative_path, text)
            }
            None if detected.is_native() => self.native.extract_file(file_path, text, detected),
            None => Err(ExtractionError::UnsupportedLanguage { language: detected }),
        }
    }
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    /// Extracts a composite kind classification from a path+language pair.
    #[test]
    fn classifies_every_composite_kind() {
        let cases = [
            ("a.svelte", Language::Svelte, CompositeKind::Svelte),
            ("a.vue", Language::Vue, CompositeKind::Vue),
            ("a.astro", Language::Astro, CompositeKind::Astro),
            ("a.liquid", Language::Liquid, CompositeKind::Liquid),
            (
                "templates/product.json",
                Language::Liquid,
                CompositeKind::Liquid,
            ),
            (
                "sections/group.json",
                Language::Liquid,
                CompositeKind::Liquid,
            ),
            ("a.cshtml", Language::Razor, CompositeKind::Razor),
            ("a.razor", Language::Razor, CompositeKind::Razor),
            ("a.yaml", Language::Yaml, CompositeKind::Yaml),
            ("conf/routes", Language::Yaml, CompositeKind::PlayRoutes),
            (
                "src/main/conf/routes",
                Language::Yaml,
                CompositeKind::PlayRoutes,
            ),
            ("conf/api.routes", Language::Yaml, CompositeKind::PlayRoutes),
            ("a.twig", Language::Twig, CompositeKind::Twig),
            ("a.xml", Language::Xml, CompositeKind::Xml),
            (
                "a.properties",
                Language::Properties,
                CompositeKind::Properties,
            ),
            ("A.cfc", Language::Cfml, CompositeKind::Cfml),
            ("A.cfm", Language::Cfml, CompositeKind::Cfml),
            ("A.cfs", Language::Cfscript, CompositeKind::Cfml),
            ("Form.dfm", Language::Pascal, CompositeKind::Dfm),
            ("Form.fmx", Language::Pascal, CompositeKind::Fmx),
            (
                "src/app.app.src",
                Language::Erlang,
                CompositeKind::ErlangApp,
            ),
            ("src/app.app", Language::Erlang, CompositeKind::ErlangApp),
        ];
        for (path, language, expected) in cases {
            assert_eq!(
                CompositeKind::from_discovered(path, language),
                Some(expected),
                "classify {path} ({language})"
            );
        }
        // Mapper-shaped XML classifies as MyBatis with content; generic XML
        // stays Xml.
        assert_eq!(
            CompositeKind::classify(
                "a.xml",
                Language::Xml,
                Some("<mapper namespace=\"com.a.B\"><select id=\"x\"/></mapper>"),
            ),
            Some(CompositeKind::MyBatis)
        );
        assert_eq!(
            CompositeKind::classify("a.xml", Language::Xml, Some("<project/>")),
            Some(CompositeKind::Xml)
        );
        // Native and unknown files are not composite kinds.
        assert_eq!(
            CompositeKind::from_discovered("a.ts", Language::Typescript),
            None
        );
        assert_eq!(
            CompositeKind::from_discovered("a.erl", Language::Erlang),
            None
        );
        assert_eq!(
            CompositeKind::from_discovered("a.xyz", Language::Unknown),
            None
        );
        // A `.dfm` file whose routed language is not pascal is not a form.
        assert_eq!(
            CompositeKind::from_discovered("a.dfm", Language::Unknown),
            None
        );
    }

    #[test]
    /// Pins the composite kind wire spellings to the frozen language registry.
    fn wire_names_are_contract_spellings() {
        for kind in [
            CompositeKind::Svelte,
            CompositeKind::Vue,
            CompositeKind::Astro,
            CompositeKind::Liquid,
            CompositeKind::Razor,
            CompositeKind::Yaml,
            CompositeKind::Twig,
            CompositeKind::Xml,
            CompositeKind::Properties,
            CompositeKind::Cfml,
            CompositeKind::Dfm,
            CompositeKind::Fmx,
            CompositeKind::MyBatis,
            CompositeKind::PlayRoutes,
            CompositeKind::ErlangApp,
        ] {
            assert!(Language::from_name(kind.wire_name()).is_some());
        }
    }
}
