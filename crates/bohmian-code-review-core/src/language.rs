//! The closed language contract inherited without additions from upstream.
//!
//! The 42 values mirror `LANGUAGES` in the frozen upstream `src/types.ts` at
//! `572d22bf`; the array order is part of the contract. Of these, the 31
//! "native" languages have a Rust tree-sitter grammar in `bohmian-code-review-extract`;
//! the remainder are composite/template formats or languages whose native
//! grammar ports are intentionally absent. Extending this enum is a
//! planning decision, never a local edit.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The complete upstream language contract (42 values, upstream order).
///
/// Serializes to the upstream lowercase wire name (`typescript`, `tsx`,
/// `csharp`, `objc`, `r`, …). See the module docs for provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// TypeScript.
    Typescript,
    /// JavaScript.
    Javascript,
    /// TSX (TypeScript + JSX).
    Tsx,
    /// JSX.
    Jsx,
    /// ArkTS.
    Arkts,
    /// Python.
    Python,
    /// Go.
    Go,
    /// Rust.
    Rust,
    /// Java.
    Java,
    /// C.
    C,
    /// C++.
    Cpp,
    /// C#.
    Csharp,
    /// ASP.NET Razor markup.
    Razor,
    /// PHP.
    Php,
    /// Ruby.
    Ruby,
    /// Swift.
    Swift,
    /// Kotlin.
    Kotlin,
    /// Dart.
    Dart,
    /// Svelte.
    Svelte,
    /// Vue.
    Vue,
    /// Astro.
    Astro,
    /// Liquid.
    Liquid,
    /// Pascal.
    Pascal,
    /// Scala.
    Scala,
    /// Lua.
    Lua,
    /// Luau.
    Luau,
    /// Objective-C.
    Objc,
    /// R.
    R,
    /// Solidity.
    Solidity,
    /// Nix.
    Nix,
    /// YAML.
    Yaml,
    /// Twig.
    Twig,
    /// XML.
    Xml,
    /// Java properties.
    Properties,
    /// CFML.
    Cfml,
    /// CFScript.
    Cfscript,
    /// CFQuery.
    Cfquery,
    /// COBOL.
    Cobol,
    /// VB.NET.
    Vbnet,
    /// Erlang.
    Erlang,
    /// Terraform.
    Terraform,
    /// Unknown (undetected).
    Unknown,
}

/// The 31 languages with a native Rust tree-sitter grammar in
/// `bohmian-code-review-extract` (the frozen kernel's original 20-language table plus
/// the five systems/configuration grammar ports — Objective-C, Solidity,
/// Terraform, ArkTS, Nix — and the six enterprise/legacy grammar ports —
/// Pascal, CFScript, CFQuery, COBOL, VB.NET, Erlang).
pub const NATIVE_LANGUAGES: [Language; 31] = [
    Language::Typescript,
    Language::Tsx,
    Language::Javascript,
    Language::Jsx,
    Language::Java,
    Language::Python,
    Language::Go,
    Language::C,
    Language::Cpp,
    Language::Rust,
    Language::Csharp,
    Language::Ruby,
    Language::Php,
    Language::Swift,
    Language::Kotlin,
    Language::R,
    Language::Lua,
    Language::Luau,
    Language::Scala,
    Language::Dart,
    Language::Objc,
    Language::Solidity,
    Language::Terraform,
    Language::Arkts,
    Language::Nix,
    Language::Pascal,
    Language::Cfscript,
    Language::Cfquery,
    Language::Cobol,
    Language::Vbnet,
    Language::Erlang,
];

/// The complete upstream wire-name list, in contract order.
pub const LANGUAGE_NAMES: [&str; 42] = [
    "typescript",
    "javascript",
    "tsx",
    "jsx",
    "arkts",
    "python",
    "go",
    "rust",
    "java",
    "c",
    "cpp",
    "csharp",
    "razor",
    "php",
    "ruby",
    "swift",
    "kotlin",
    "dart",
    "svelte",
    "vue",
    "astro",
    "liquid",
    "pascal",
    "scala",
    "lua",
    "luau",
    "objc",
    "r",
    "solidity",
    "nix",
    "yaml",
    "twig",
    "xml",
    "properties",
    "cfml",
    "cfscript",
    "cfquery",
    "cobol",
    "vbnet",
    "erlang",
    "terraform",
    "unknown",
];

impl Language {
    /// The locked contract tag: this language's index in the upstream order.
    ///
    /// The tag is the durable one-byte encoding used by identity hashes, so it
    /// is fixed by the enum's declaration order and is append-only.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// The upstream wire name (`"typescript"`, `"tsx"`, …).
    pub fn as_str(self) -> &'static str {
        LANGUAGE_NAMES[self as usize]
    }

    /// Whether this language has a native Rust tree-sitter grammar in
    /// `bohmian-code-review-extract` (the frozen kernel's original 20-language set
    /// plus the five systems/configuration ports and the six
    /// enterprise/legacy ports).
    pub fn is_native(self) -> bool {
        NATIVE_LANGUAGES.contains(&self)
    }

    /// Resolve a wire name into a [`Language`], or `None` for unknown names.
    pub fn from_name(name: &str) -> Option<Language> {
        LANGUAGE_NAMES
            .iter()
            .position(|candidate| *candidate == name)
            .and_then(Language::from_index)
    }

    /// Maps a frozen language ordinal back to its closed enum variant, returning no value for out-of-range bytes.
    fn from_index(index: usize) -> Option<Language> {
        Language::ALL.get(index).copied()
    }

    /// All 42 contract values in upstream order.
    pub const ALL: [Language; 42] = [
        Language::Typescript,
        Language::Javascript,
        Language::Tsx,
        Language::Jsx,
        Language::Arkts,
        Language::Python,
        Language::Go,
        Language::Rust,
        Language::Java,
        Language::C,
        Language::Cpp,
        Language::Csharp,
        Language::Razor,
        Language::Php,
        Language::Ruby,
        Language::Swift,
        Language::Kotlin,
        Language::Dart,
        Language::Svelte,
        Language::Vue,
        Language::Astro,
        Language::Liquid,
        Language::Pascal,
        Language::Scala,
        Language::Lua,
        Language::Luau,
        Language::Objc,
        Language::R,
        Language::Solidity,
        Language::Nix,
        Language::Yaml,
        Language::Twig,
        Language::Xml,
        Language::Properties,
        Language::Cfml,
        Language::Cfscript,
        Language::Cfquery,
        Language::Cobol,
        Language::Vbnet,
        Language::Erlang,
        Language::Terraform,
        Language::Unknown,
    ];
}

impl fmt::Display for Language {
    /// Implements the standard fmt boundary using the canonical wire spelling and validation rules.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Language {
    /// Names the Err representation used by this extraction stage.
    type Err = InvalidLanguage;

    /// Implements the standard from_str boundary using the canonical wire spelling and validation rules.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidLanguage`] when `name` is not one of the 42 frozen wire spellings.
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Language::from_name(name).ok_or_else(|| InvalidLanguage(name.to_string()))
    }
}

/// Error returned when a string is not a member of the closed language
/// contract.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown language `{0}`; the contract has 42 closed values")]
pub struct InvalidLanguage(
    /// Unrecognized serialized language name supplied by the caller.
    pub String,
);

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// Pins every language wire name and ordinal to the frozen language registry.
    fn wire_names_match_upstream_contract() {
        for (language, name) in Language::ALL.iter().zip(LANGUAGE_NAMES.iter()) {
            assert_eq!(language.as_str(), *name, "wire name for {language:?}");
            assert_eq!(Language::from_name(name), Some(*language));
        }
        assert_eq!(Language::ALL.len(), 42);
        assert_eq!(LANGUAGE_NAMES.len(), 42);
    }

    #[test]
    /// Serializes and deserializes every language variant without changing its wire spelling.
    ///
    /// # Panics
    ///
    /// Panics if serialization, deserialization, or parsing fails for a closed language variant.
    fn every_value_round_trips_through_serde() {
        for language in Language::ALL {
            let wire = serde_json::to_string(&language).unwrap();
            let back: Language = serde_json::from_str(&wire).unwrap();
            assert_eq!(back, language);
        }
    }

    #[test]
    /// Pins the native-language subset to the thirty-one grammars compiled into the kernel.
    ///
    /// # Panics
    ///
    /// Panics if a native-language name cannot be parsed or the asserted thirty-one-entry set changes.
    fn native_set_is_exactly_the_frozen_kernel_languages() {
        let mut native = NATIVE_LANGUAGES.to_vec();
        native.sort();
        let expected: Vec<Language> = [
            "typescript",
            "tsx",
            "javascript",
            "jsx",
            "java",
            "python",
            "go",
            "c",
            "cpp",
            "rust",
            "csharp",
            "ruby",
            "php",
            "swift",
            "kotlin",
            "r",
            "lua",
            "luau",
            "scala",
            "dart",
            "objc",
            "solidity",
            "terraform",
            "arkts",
            "nix",
            "pascal",
            "cfscript",
            "cfquery",
            "cobol",
            "vbnet",
            "erlang",
        ]
        .iter()
        .map(|name| Language::from_name(name).unwrap())
        .collect();
        let mut expected = expected;
        expected.sort();
        assert_eq!(native, expected);
        for language in Language::ALL {
            assert_eq!(language.is_native(), expected.contains(&language));
        }
    }

    #[test]
    /// Rejects language spellings outside the closed serialized registry.
    fn unknown_names_are_rejected() {
        assert_eq!(Language::from_name("not-a-language"), None);
        assert_eq!(
            "nope".parse::<Language>(),
            Err(InvalidLanguage("nope".into()))
        );
        assert!(serde_json::from_str::<Language>("\"cobol++\"").is_err());
    }

    #[test]
    /// Round-trips every language through its display spelling and parser.
    ///
    /// # Panics
    ///
    /// Panics if a displayed closed language spelling cannot be parsed back.
    fn display_and_parse_round_trip() {
        for language in Language::ALL {
            assert_eq!(language.to_string().parse::<Language>().unwrap(), language);
        }
    }
}
