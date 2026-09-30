//! Closed production language-routing owner — a faithful Rust port of
//! `detectLanguage` / `EXTENSION_MAP` (src/extraction/grammars.ts) plus its
//! `isPlayRoutesFile` / `isShopifyLiquidJson` / `isErlangAppFile` path
//! specials and the first-8-KiB `.h` content heuristics.
//!
//! [`LanguageRouter`] is the single production owner of the frozen
//! extension/path/content routing precedence. Routing is deterministic and
//! IO-free; a caller may pre-classify a language explicitly (the
//! [`crate::Extractor::extract`] contract takes a pre-decided
//! [`bohmian_code_review_core::Language`]), while [`LanguageRouter::detect`] is the
//! routing authority that the extraction entry surface and corpus fixtures
//! consult.
//!
//! Precedence (upstream order): Play `conf/routes` → `yaml`; then the
//! lowercase last-dot extension; Shopify OS 2.0 `templates|sections/**/*.json`
//! → `liquid`; Erlang `.app`/`.app.src` → `erlang`; then the extension map
//! (with optional caller overrides first, keyed by the dot-prefixed
//! extension); finally `.h` content routing — C++ constructs before
//! Objective-C markers, both sampled from the first 8192 bytes only. Any
//! unregistered extension routes to [`bohmian_code_review_core::Language::Unknown`].
//!
//! The `.h` detector regexes are compiled with `unicode(false)` so that `\w`,
//! `\b`, and `\s` use JavaScript's ASCII semantics (JS `\w` is
//! `[A-Za-z0-9_]`; a non-ASCII letter such as `é` is a non-word character).
//! Upstream JS `\s` additionally includes non-ASCII whitespace
//! (`\u00A0 \u1680 \u2000-\u200A \u2028 \u2029 \u202F \u205F
//! \u3000 \uFEFF`), which the Rust regex crate cannot express under
//! `unicode(false)`; the sample is therefore pre-normalized by mapping
//! exactly that JavaScript whitespace set to ASCII space before matching, so
//! the ASCII `\s` class observes JavaScript-identical whitespace semantics.
//! The sample itself is the longest character-prefix whose characters all
//! start within the first 8192 UTF-16 code units (`substring(0, 8192)`
//! semantics at character granularity) — computed on the original string and
//! never panicking, so astral characters at or crossing the boundary cannot
//! split and no character starting at or after the boundary leaks in.

use bohmian_code_review_core::Language;
use regex::bytes::Regex as BytesRegex;
use regex::{Regex, RegexBuilder};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The frozen extension → language map (grammars.ts `EXTENSION_MAP`).
///
/// Extension keys are lower-case and include the leading dot; values are the
/// closed upstream language spellings. Order is irrelevant (lookup by key);
/// precedence over the whole map is fixed by the routing algorithm.
const EXTENSION_MAP: [(&str, Language); 78] = [
    (".ts", Language::Typescript),
    (".tsx", Language::Tsx),
    // ESM/CJS TypeScript module extensions — parsed as TS (no JSX). (#366)
    (".mts", Language::Typescript),
    (".cts", Language::Typescript),
    // ArkTS (HarmonyOS / OpenHarmony) — a TypeScript superset with
    // declarative UI; plain `.ts` in an ArkTS project stays TypeScript. (#648)
    (".ets", Language::Arkts),
    (".js", Language::Javascript),
    (".mjs", Language::Javascript),
    (".cjs", Language::Javascript),
    // SAP HANA XS Classic server-side JavaScript. (#556)
    (".xsjs", Language::Javascript),
    (".xsjslib", Language::Javascript),
    (".jsx", Language::Jsx),
    (".py", Language::Python),
    (".pyw", Language::Python),
    (".go", Language::Go),
    (".rs", Language::Rust),
    (".java", Language::Java),
    (".c", Language::C),
    // Could also be C++, defaulting to C; content routing disambiguates.
    (".h", Language::C),
    (".cpp", Language::Cpp),
    (".cc", Language::Cpp),
    (".cxx", Language::Cpp),
    (".hpp", Language::Cpp),
    (".hxx", Language::Cpp),
    (".cs", Language::Csharp),
    // ASP.NET Razor / Blazor markup — custom RazorExtractor.
    (".cshtml", Language::Razor),
    (".razor", Language::Razor),
    (".php", Language::Php),
    // Drupal-specific PHP file extensions.
    (".module", Language::Php),
    (".install", Language::Php),
    (".theme", Language::Php),
    (".inc", Language::Php),
    // YAML (Drupal routing; file-level tracking only).
    (".yml", Language::Yaml),
    (".yaml", Language::Yaml),
    // Twig templates (file-level tracking only).
    (".twig", Language::Twig),
    (".rb", Language::Ruby),
    (".rake", Language::Ruby),
    (".swift", Language::Swift),
    (".kt", Language::Kotlin),
    (".kts", Language::Kotlin),
    (".dart", Language::Dart),
    (".liquid", Language::Liquid),
    (".svelte", Language::Svelte),
    (".vue", Language::Vue),
    (".astro", Language::Astro),
    (".r", Language::R),
    (".pas", Language::Pascal),
    (".dpr", Language::Pascal),
    (".dpk", Language::Pascal),
    (".lpr", Language::Pascal),
    (".dfm", Language::Pascal),
    (".fmx", Language::Pascal),
    (".scala", Language::Scala),
    (".sc", Language::Scala),
    (".lua", Language::Lua),
    (".luau", Language::Luau),
    (".m", Language::Objc),
    (".mm", Language::Objc),
    (".sol", Language::Solidity),
    // CFML: .cfc/.cfm use the tag-aware cfml grammar; .cfs is pure CFScript.
    (".cfc", Language::Cfml),
    (".cfm", Language::Cfml),
    (".cfs", Language::Cfscript),
    // Metal Shading Language ≈ C++14; CUDA ≈ C++ (#1121, #387).
    (".metal", Language::Cpp),
    (".cu", Language::Cpp),
    (".cuh", Language::Cpp),
    (".nix", Language::Nix),
    // XML: file-level tracking; MyBatis mapper extraction.
    (".xml", Language::Xml),
    // COBOL programs and copybooks.
    (".cbl", Language::Cobol),
    (".cob", Language::Cobol),
    (".cobol", Language::Cobol),
    (".cpy", Language::Cobol),
    // VB.NET.
    (".vb", Language::Vbnet),
    // Erlang modules and header files.
    (".erl", Language::Erlang),
    (".hrl", Language::Erlang),
    (".escript", Language::Erlang),
    // Spring config properties (file-level tracking).
    (".properties", Language::Properties),
    // Terraform / OpenTofu / HCL config.
    (".tf", Language::Terraform),
    (".tfvars", Language::Terraform),
    (".tofu", Language::Terraform),
];

/// Looks up the extension map, falling back to `Unknown` for unregistered extensions.
fn extension_language(ext: &str) -> Language {
    EXTENSION_MAP
        .iter()
        .find(|(candidate, _)| *candidate == ext)
        .map(|(_, language)| *language)
        .unwrap_or(Language::Unknown)
}

/// The longest character-prefix of `s` whose characters all start within the
/// first `limit` UTF-16 code units — JavaScript
/// `String.prototype.substring(0, limit)` semantics at character granularity.
///
/// A character is included in full iff its UTF-16 start offset plus its width
/// does not exceed `limit`. A character that would be cut by the boundary (an
/// astral pair straddling it) is excluded entirely: a lone high surrogate in
/// the JavaScript sample is a non-word byte that can never join an ASCII word,
/// so excluding it cannot change any ASCII-detector match, and no character
/// starting at or after `limit` ever leaks into the prefix. Never panics.
fn first_utf16_units(s: &str, limit: usize) -> &str {
    let mut units = 0usize;
    for (index, c) in s.char_indices() {
        let width = c.len_utf16();
        if units + width > limit {
            return &s[..index];
        }
        units += width;
        if units == limit {
            return &s[..index + c.len_utf8()];
        }
    }
    s
}

/// Returns true when `c` is a member of the JavaScript `\s` set
/// (`[\t\n\v\f\r \u00A0 \u1680 \u2000-\u200A \u2028 \u2029
/// \u202F \u205F \u3000 \uFEFF]`).
fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\x0B' | '\x0C' | '\r' | ' ' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// Maps exactly the JavaScript `\s` characters to ASCII space so the
/// `unicode(false)` `\s` class observes JavaScript-identical whitespace
/// semantics (the Rust regex crate cannot express non-ASCII escapes under
/// `unicode(false)`).
fn normalize_js_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if is_js_whitespace(c) {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// Play Framework routes file: the extensionless `conf/routes` (and included
/// `conf/*.routes`) — grammars.ts `isPlayRoutesFile`.
pub fn is_play_routes_file(file_path: &str) -> bool {
    file_path == "conf/routes"
        || file_path.ends_with("/conf/routes")
        || file_path.ends_with(".routes")
}

/// Shopify OS 2.0 JSON template (`templates/*.json`) or section group
/// (`sections/*.json`) — grammars.ts `isShopifyLiquidJson`.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
pub fn is_shopify_liquid_json(file_path: &str) -> bool {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<BytesRegex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::bytes::RegexBuilder::new(r"(^|/)(templates|sections)/.+\.json$")
            .unicode(false)
            .case_insensitive(true)
            .build()
            .unwrap()
    })
    .is_match(file_path.as_bytes())
}

/// OTP application resource file: `<app>.app.src` or its compiled
/// `<app>.app` — grammars.ts `isErlangAppFile`.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
pub fn is_erlang_app_file(file_path: &str) -> bool {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<BytesRegex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::bytes::RegexBuilder::new(r"\.app(?:\.src)?$")
            .unicode(false)
            .case_insensitive(true)
            .build()
            .unwrap()
    })
    .is_match(file_path.as_bytes())
}

/// The first-8-KiB C++ marker scan — grammars.ts `looksLikeCpp`.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn looks_like_cpp(sample: &str) -> bool {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        RegexBuilder::new(
            r"\bnamespace\b|\bclass\s+\w+\s*[:{]|\b(?:class|struct)\s+[A-Z][A-Z0-9_]+\s+\w+\s*(?:final\s*)?[:{]|\btemplate\s*<|\b(?:public|private|protected)\s*:|\bvirtual\b|\busing\s+(?:namespace\b|\w+\s*=)",
        )
        .unicode(false)
        .build()
        .unwrap()
    })
    .is_match(sample)
}

/// The first-8-KiB Objective-C marker scan — grammars.ts `looksLikeObjc`.
///
/// # Panics
///
/// Panics if the hard-coded regular expression literal fails to compile.
fn looks_like_objc(sample: &str) -> bool {
    /// Lazily compiled literal pattern used by the enclosing classifier; compilation is an internal invariant.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        RegexBuilder::new(r"@(?:interface|implementation|protocol|synthesize)\b")
            .unicode(false)
            .build()
            .unwrap()
    })
    .is_match(sample)
}

/// The production closed language-routing owner.
///
/// Owns the frozen extension/path/content routing contract and an optional
/// caller-supplied extension override map (the `codegraph.json` custom
/// extension → language mapping). Routing is stateless between calls and
/// safe to share; construction with [`LanguageRouter::new`] reproduces the
/// zero-config upstream behavior byte-for-byte.
#[derive(Debug, Clone, Default)]
pub struct LanguageRouter {
    /// Caller-supplied extension → language overrides (keys include the dot).
    overrides: BTreeMap<String, Language>,
}

impl LanguageRouter {
    /// Constructs the router over the frozen routing contract with no overrides.
    pub fn new() -> Self {
        LanguageRouter::default()
    }

    /// Constructs the router with the given extension → language overrides.
    ///
    /// Override keys must include the leading dot (`.foo`), mirroring the
    /// upstream `overrides` map consumed by `detectLanguage`.
    pub fn with_overrides(overrides: BTreeMap<String, Language>) -> Self {
        LanguageRouter { overrides }
    }

    /// Detects a source file's language from its path and optional source.
    ///
    /// Reproduces the frozen upstream precedence: Play routes → `yaml`;
    /// Shopify template/section JSON → `liquid`; Erlang `.app`/`.app.src` →
    /// `erlang`; then the lower-cased last-dot extension via the override map
    /// first and the frozen extension map second; finally `.h` content
    /// routing (C++ markers before Objective-C markers, sampled from the
    /// first 8192 bytes only, requiring `source`). Unregistered extensions
    /// route to [`Language::Unknown`].
    /// Whether a file is one the engine can parse, based purely on its
    /// path — grammars.ts `isSourceFile` (the single source of truth for
    /// "should we index this file", derived from the extension map so
    /// parser support and indexing selection never drift).
    ///
    /// Play `conf/routes` (extensionless), Shopify OS 2.0 `templates|sections`
    /// JSON, and OTP `.app`/`.app.src` resource files count as source files;
    /// then the lower-cased last-dot extension via the caller overrides first
    /// and the frozen extension map second.
    pub fn is_source_file(&self, file_path: &str) -> bool {
        if is_play_routes_file(file_path) {
            return true; // Play `conf/routes` is extensionless
        }
        if is_shopify_liquid_json(file_path) {
            return true; // Shopify OS 2.0 JSON templates / section groups
        }
        if is_erlang_app_file(file_path) {
            return true; // OTP `.app`/`.app.src` resource files
        }
        let Some(dot) = file_path.rfind('.') else {
            return false;
        };
        let ext = file_path[dot..].to_ascii_lowercase();
        self.overrides.contains_key(&ext) || extension_language(&ext) != Language::Unknown
    }

    /// Detects a source file's language from its path and optional source.
    pub fn detect(&self, file_path: &str, source: Option<&str>) -> Language {
        if is_play_routes_file(file_path) {
            return Language::Yaml;
        }
        let dot = file_path.rfind('.');
        let raw_ext = match dot {
            Some(index) => &file_path[index..],
            None => file_path,
        };
        let ext = raw_ext.to_ascii_lowercase();
        if is_shopify_liquid_json(file_path) {
            return Language::Liquid;
        }
        if is_erlang_app_file(file_path) {
            return Language::Erlang;
        }
        let language = self
            .overrides
            .get(&ext)
            .copied()
            .unwrap_or_else(|| extension_language(&ext));

        // `.h` files could be C, C++, or Objective-C — check source content.
        if language == Language::C
            && ext == ".h"
            && let Some(source) = source
        {
            // `substring(0, 8192)` counts UTF-16 code units; characters are
            // included atomically (cut astral pairs excluded — see
            // `first_utf16_units`), then JavaScript whitespace is normalized
            // so the ASCII detector classes match JS semantics exactly.
            let sample = normalize_js_whitespace(first_utf16_units(source, 8192));
            if looks_like_cpp(&sample) {
                return Language::Cpp;
            }
            if looks_like_objc(&sample) {
                return Language::Objc;
            }
        }
        language
    }
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen routing behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// Routes every native extension to its frozen language, case-insensitively.
    fn native_extensions_route_exactly() {
        let router = LanguageRouter::new();
        let cases = [
            ("a.ts", Language::Typescript),
            ("a.tsx", Language::Tsx),
            ("a.mts", Language::Typescript),
            ("a.cts", Language::Typescript),
            ("a.ets", Language::Arkts),
            ("a.js", Language::Javascript),
            ("a.mjs", Language::Javascript),
            ("a.cjs", Language::Javascript),
            ("a.jsx", Language::Jsx),
            ("a.py", Language::Python),
            ("a.go", Language::Go),
            ("a.rs", Language::Rust),
            ("a.java", Language::Java),
            ("a.c", Language::C),
            ("a.h", Language::C), // content routing decides C/C++/objc
            ("a.cpp", Language::Cpp),
            ("a.cc", Language::Cpp),
            ("a.hpp", Language::Cpp),
            ("a.cs", Language::Csharp),
            ("a.php", Language::Php),
            ("a.rb", Language::Ruby),
            ("a.swift", Language::Swift),
            ("a.kt", Language::Kotlin),
            ("a.dart", Language::Dart),
            ("a.r", Language::R),
            ("a.scala", Language::Scala),
            ("a.lua", Language::Lua),
            ("a.luau", Language::Luau),
            ("a.m", Language::Objc),
            ("a.mm", Language::Objc),
            ("a.sol", Language::Solidity),
            ("a.tf", Language::Terraform),
            ("a.tfvars", Language::Terraform),
            ("a.tofu", Language::Terraform),
            ("a.nix", Language::Nix),
            ("A.TS", Language::Typescript),
            ("dir/FILE.MM", Language::Objc),
        ];
        for (path, expected) in cases {
            assert_eq!(router.detect(path, None), expected, "routing {path}");
        }
    }

    #[test]
    /// Unregistered extensions and extensionless paths route to Unknown.
    fn unknown_routes_to_unknown() {
        let router = LanguageRouter::new();
        assert_eq!(router.detect("a.xyz", None), Language::Unknown);
        assert_eq!(router.detect("Makefile", None), Language::Unknown);
        assert_eq!(router.detect(".gitignore", None), Language::Unknown);
    }

    #[test]
    /// `.h` content routing prefers C++ markers before Objective-C markers.
    fn header_content_routing_prefers_cpp() {
        let router = LanguageRouter::new();
        let c_header = "int add(int a, int b);\n";
        assert_eq!(router.detect("x.h", Some(c_header)), Language::C);
        let cpp_header = "#pragma once\nnamespace engine { class Foo { public: int run(); }; }\n";
        assert_eq!(router.detect("x.h", Some(cpp_header)), Language::Cpp);
        // A header carrying BOTH C++ and Objective-C constructs routes to C++.
        let both =
            "@interface Wrapper\n@end\nnamespace engine { template <typename T> class Box {}; }\n";
        assert_eq!(router.detect("x.h", Some(both)), Language::Cpp);
        let objc_header =
            "#import <Foundation/Foundation.h>\n@interface Greeter : NSObject\n@end\n";
        assert_eq!(router.detect("x.h", Some(objc_header)), Language::Objc);
    }

    #[test]
    /// The `.h` content scan is limited to the first 8192 bytes.
    fn header_routing_samples_only_first_8kib() {
        let router = LanguageRouter::new();
        let mut prefix = "int filler;\n".repeat(900); // > 8 KiB of plain C
        prefix.push_str("@interface LateObjc\n@end\n");
        assert_eq!(router.detect("x.h", Some(&prefix)), Language::C);
        let mut early = "@interface EarlyObjc\n@end\n".to_string();
        early.push_str(&"int filler;\n".repeat(100));
        assert_eq!(router.detect("x.h", Some(&early)), Language::Objc);
    }

    #[test]
    /// Path specials route before the extension map, in upstream order.
    fn path_specials_route_first() {
        let router = LanguageRouter::new();
        assert_eq!(router.detect("conf/routes", None), Language::Yaml);
        assert_eq!(router.detect("src/main/conf/routes", None), Language::Yaml);
        assert_eq!(router.detect("conf/api.routes", None), Language::Yaml);
        assert_eq!(
            router.detect("templates/product.json", None),
            Language::Liquid
        );
        assert_eq!(router.detect("sections/group.json", None), Language::Liquid);
        assert_eq!(router.detect("src/app.app.src", None), Language::Erlang);
        assert_eq!(router.detect("src/app.app", None), Language::Erlang);
        // Non-special JSON routes by extension (unknown).
        assert_eq!(router.detect("config/app.json", None), Language::Unknown);
    }

    #[test]
    /// Caller overrides take precedence over the frozen extension map.
    fn overrides_take_precedence() {
        let mut overrides = BTreeMap::new();
        overrides.insert(".foo".to_string(), Language::Rust);
        let router = LanguageRouter::with_overrides(overrides);
        assert_eq!(router.detect("a.foo", None), Language::Rust);
        assert_eq!(router.detect("a.rs", None), Language::Rust);
        assert_eq!(router.detect("a.py", None), Language::Python);
    }
}

#[cfg(test)]
/// Contains focused regression tests for the UTF-16-boundary and
/// JavaScript-regex-semantics behavior of the routing owner.
mod js_semantics_tests {
    use super::*;

    #[test]
    /// The 8192-unit prefix includes BMP characters atomically and excludes
    /// astral characters that would be cut, without later leakage.
    fn utf16_prefix_is_char_atomic() {
        assert_eq!(first_utf16_units("a😀b", 2), "a"); // 😀 (2 units) cut at 2
        assert_eq!(first_utf16_units("a😀b", 3), "a😀"); // fits exactly
        assert_eq!(first_utf16_units("aé", 2), "aé"); // BMP é (1 unit)
        assert_eq!(first_utf16_units("aé", 1), "a");
        assert_eq!(first_utf16_units("abc", 2), "ab");
        assert_eq!(first_utf16_units("abc", 10), "abc");
        // No later ASCII leakage: the astral at the boundary is excluded,
        // and characters starting at or after the limit never appear.
        assert!(!first_utf16_units("a😀b", 2).contains('b'));
        assert!(!first_utf16_units("a😀b", 3).contains('b'));
    }

    #[test]
    /// A multi-byte character straddling byte 8192 must not panic and must
    /// route like the JavaScript sample (the token after the boundary is
    /// absent).
    fn byte_8192_multibyte_split_does_not_panic() {
        let router = LanguageRouter::new();
        // 8191 ASCII 'a' (8191 bytes/units) + 'é' (bytes 8191-8192, unit 8191)
        // + "namespace" starting at unit 8192 — excluded from the sample.
        let mut src = "a".repeat(8191);
        src.push('é');
        src.push_str("namespace X {}");
        assert_eq!(router.detect("x.h", Some(&src)), Language::C);
        // A C++ marker fully inside the first 8192 units (with a leading
        // non-word separator so `\bnamespace\b` matches) still routes to C++.
        let mut src2 = "a".repeat(8182);
        src2.push_str(" namespace X {}");
        assert_eq!(router.detect("x.h", Some(&src2)), Language::Cpp);
    }

    #[test]
    /// The boundary counts UTF-16 units, so a non-ASCII BMP character counts
    /// one unit and an astral character counts two; the detector sees the
    /// same tokens the JavaScript sample would.
    fn utf16_boundary_bmp_and_astral() {
        // 8182 units of filler + " namespace" (10 units, ends at 8192) → cpp.
        let mut cpp_src = "a".repeat(8182);
        cpp_src.push_str(" namespace X {}");
        // Insert an astral char (2 units) BEFORE the marker so the marker
        // starts at unit 8185: still fully inside.
        let mut cpp_src2 = "a".repeat(8180);
        cpp_src2.push('😀');
        cpp_src2.push_str(" namespace X {}");
        let router = LanguageRouter::new();
        assert_eq!(router.detect("x.h", Some(&cpp_src2)), Language::Cpp);
        // Pushing the marker one unit past the boundary hides it.
        let mut c_src = "a".repeat(8183);
        c_src.push_str(" namespace X {}");
        assert_eq!(router.detect("x.h", Some(&c_src)), Language::C);
    }

    #[test]
    /// Non-ASCII letters are non-word characters under JavaScript semantics:
    /// `class é` is not a C++ marker, while `énamespace` still matches
    /// `\bnamespace\b` (ASCII boundary).
    fn non_ascii_letters_are_non_word() {
        let router = LanguageRouter::new();
        assert_eq!(router.detect("x.h", Some("class é { }")), Language::C);
        assert_eq!(router.detect("x.h", Some("class Foo { }")), Language::Cpp);
        assert_eq!(router.detect("x.h", Some("énamespace X {}")), Language::Cpp);
        assert_eq!(router.detect("x.h", Some("namespace中 {}")), Language::Cpp);
    }

    #[test]
    /// Non-ASCII JavaScript whitespace (U+00A0 and friends) separates tokens
    /// exactly like ASCII space under the normalized detector.
    fn js_whitespace_members_separate_tokens() {
        let router = LanguageRouter::new();
        let nbsp = '\u{00A0}';
        let narrow_nbsp = '\u{202F}';
        let em_space = '\u{2003}';
        assert_eq!(
            router.detect("x.h", Some(&format!("class{nbsp}Foo {{ }}"))),
            Language::Cpp
        );
        assert_eq!(
            router.detect("x.h", Some(&format!("class{narrow_nbsp}Foo {{ }}"))),
            Language::Cpp
        );
        assert_eq!(
            router.detect("x.h", Some(&format!("class{em_space}Foo {{ }}"))),
            Language::Cpp
        );
    }

    #[test]
    /// Precedence when C++ and Objective-C markers straddle the boundary:
    /// the C++ scan runs first over the same normalized 8192-unit sample.
    fn boundary_straddling_markers_precedence() {
        let router = LanguageRouter::new();
        // ObjC marker fully inside, then a C++ marker at the boundary: C++
        // wins (checked first) only when fully inside.
        // The 22-unit "@interface Early\n@end\n" prefix participates in the
        // UTF-16 budget, so the filler is reduced accordingly.
        let mut both = "@interface Early\n@end\n".to_string();
        both.push_str(&"a".repeat(8160));
        both.push_str(" namespace X {}");
        assert_eq!(router.detect("x.h", Some(&both)), Language::Cpp);
        // Same but the C++ marker hidden past the boundary: objc wins.
        let mut objc_only = "@interface Early\n@end\n".to_string();
        objc_only.push_str(&"a".repeat(8161));
        objc_only.push_str(" namespace X {}");
        assert_eq!(router.detect("x.h", Some(&objc_only)), Language::Objc);
    }
}
