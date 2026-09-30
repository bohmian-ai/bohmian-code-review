//! Grammar registry: codegraph `Language` string → native tree-sitter grammar.
//!
//! Mirrors the wasm side's `WASM_GRAMMAR_FILES` mapping (src/extraction/
//! grammars.ts): `tsx` and `jsx` reuse another language's grammar exactly the
//! way the wasm map does. The kernel-grammar-parity test asserts each entry is
//! built from the SAME grammar revision as the vendored wasm — bump the crate
//! and the wasm together.
//!
//! This is a direct port of the frozen upstream kernel's grammar registry
//! (`codegraph-kernel/src/langs.rs`). The vendored grammars — those with no
//! crates.io crate at the frozen upstream revision — are compiled from
//! vendored C in `build.rs`; the raw constructor symbols cross the C ABI
//! through the tiny [`ffi`] shim below, which is the crate's only `unsafe`
//! surface.

use tree_sitter::Language;

#[allow(unsafe_code)]
/// Contains the ffi domain types and algorithms re-exported through this crate boundary.
mod ffi {
    use tree_sitter::Language;
    // justification: tree-sitter's C ABI exposes vendored grammar parsers as
    // raw function-pointer symbols. The declarations below (and the
    // from_raw calls that wrap them) are the complete unsafe surface of this
    // crate: each grammar is a frozen upstream revision with no crates.io
    // crate at that revision, so the C symbols must be linked directly. All
    // callers go through the safe wrapper functions, never through the raw
    // pointers.
    unsafe extern "C" {
        /// Returns the vendored Kotlin grammar descriptor linked by the build script.
        pub fn tree_sitter_kotlin() -> *const ();
        /// Returns the vendored Lua grammar descriptor linked by the build script.
        pub fn tree_sitter_lua() -> *const ();
        /// Returns the vendored Scala grammar descriptor linked by the build script.
        pub fn tree_sitter_scala() -> *const ();
        /// Returns the vendored Dart grammar descriptor linked by the build script.
        pub fn tree_sitter_dart() -> *const ();
        /// Returns the vendored Objective-C grammar descriptor linked by the build script.
        pub fn tree_sitter_objc() -> *const ();
        /// Returns the vendored Solidity grammar descriptor linked by the build script.
        pub fn tree_sitter_solidity() -> *const ();
        /// Returns the vendored Terraform/HCL grammar descriptor linked by the build script.
        pub fn tree_sitter_hcl() -> *const ();
        /// Returns the vendored ArkTS grammar descriptor linked by the build script.
        pub fn tree_sitter_arkts() -> *const ();
        /// Returns the vendored Nix grammar descriptor linked by the build script.
        pub fn tree_sitter_nix() -> *const ();
        /// Returns the vendored Pascal grammar descriptor linked by the build script.
        pub fn tree_sitter_pascal() -> *const ();
        /// Returns the vendored CFScript grammar descriptor linked by the build script.
        pub fn tree_sitter_cfscript() -> *const ();
        /// Returns the vendored CFQuery grammar descriptor linked by the build script.
        pub fn tree_sitter_cfquery() -> *const ();
        /// Returns the vendored tag-based CFML grammar descriptor linked by the build script.
        pub fn tree_sitter_cfml() -> *const ();
        /// Returns the vendored COBOL grammar descriptor linked by the build script.
        pub fn tree_sitter_COBOL() -> *const ();
        /// Returns the vendored VB.NET grammar descriptor linked by the build script.
        pub fn tree_sitter_vbnet() -> *const ();
        /// Returns the vendored Erlang grammar descriptor linked by the build script.
        pub fn tree_sitter_erlang() -> *const ();
    }

    /// The kotlin grammar (fwcd 0.3.8 vendored C, see build.rs).
    pub fn kotlin() -> Language {
        // justification: LanguageFn::from_raw is tree-sitter's documented
        // safe construction path for vendored grammar C; the symbol is
        // linked from build.rs-compiled grammars/kotlin.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_kotlin as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The lua grammar (tree-sitter-grammars v0.4.1 vendored C, see
    /// build.rs).
    pub fn lua() -> Language {
        // justification: as kotlin(); symbol linked from grammars/lua.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_lua as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The scala grammar (master@0aca5d0a6f vendored C, see build.rs).
    pub fn scala() -> Language {
        // justification: as kotlin(); symbol linked from grammars/scala.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_scala as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The dart grammar (UserNobody14 d4d8f3e vendored C, see build.rs).
    pub fn dart() -> Language {
        // justification: as kotlin(); symbol linked from grammars/dart.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_dart as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The objective-c grammar (amaanq 2.1.0 dea2b2d6 vendored C, see build.rs).
    pub fn objc() -> Language {
        // justification: as kotlin(); symbol linked from grammars/objc.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_objc as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The solidity grammar (JoranHonig b239a95f vendored C, see build.rs).
    pub fn solidity() -> Language {
        // justification: as kotlin(); symbol linked from grammars/solidity.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_solidity as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The terraform/hcl grammar (tree-sitter-grammars v1.2.0 vendored C, see build.rs).
    pub fn hcl() -> Language {
        // justification: as kotlin(); symbol linked from grammars/terraform.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_hcl as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The arkts grammar (harmony-contrib 56f7fc28 vendored C, see build.rs).
    pub fn arkts() -> Language {
        // justification: as kotlin(); symbol linked from grammars/arkts.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_arkts as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The nix grammar (nix-community 3d0173d regenerated vendored C, see build.rs).
    pub fn nix() -> Language {
        // justification: as kotlin(); symbol linked from grammars/nix.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_nix as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The pascal grammar (Isopod 042119ec regenerated vendored C, see build.rs).
    pub fn pascal() -> Language {
        // justification: as kotlin(); symbol linked from grammars/pascal.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_pascal as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The cfscript grammar (cfmleditor a224ff1 vendored C, see build.rs).
    pub fn cfscript() -> Language {
        // justification: as kotlin(); symbol linked from grammars/cfscript.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_cfscript as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The cfquery grammar (cfmleditor a224ff1 vendored C, see build.rs).
    pub fn cfquery() -> Language {
        // justification: as kotlin(); symbol linked from grammars/cfquery.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_cfquery as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The tag-based cfml grammar (cfmleditor a224ff1 vendored C, see build.rs).
    pub fn cfml() -> Language {
        // justification: as kotlin(); symbol linked from grammars/cfml.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_cfml as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The cobol grammar (yutaro-sakamoto e99dbdc3 patched + regenerated vendored C, see build.rs).
    pub fn cobol() -> Language {
        // justification: as kotlin(); symbol linked from grammars/cobol.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_COBOL as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The vbnet grammar (govindbanura 538b708 patched + generated vendored C, see build.rs).
    pub fn vbnet() -> Language {
        // justification: as kotlin(); symbol linked from grammars/vbnet.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_vbnet as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }

    /// The erlang grammar (WhatsApp 836aa2b regenerated vendored C, see build.rs).
    pub fn erlang() -> Language {
        // justification: as kotlin(); symbol linked from grammars/erlang.
        unsafe {
            tree_sitter_language::LanguageFn::from_raw(
                tree_sitter_erlang as unsafe extern "C" fn() -> *const (),
            )
        }
        .into()
    }
}

/// Languages this kernel binary can extract (reported by contractInfo;
/// TS-side routing policy decides what actually routes).
pub const LANGUAGES: [&str; 31] = [
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
];

/// Returns the frozen native tree-sitter grammar for a supported language spelling.
pub fn grammar_for(language: &str) -> Option<Language> {
    match language {
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "javascript" | "jsx" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        // `.metal`/`.cu`/`.cuh` map to language 'cpp' at detectLanguage, so the
        // dialects ride this grammar too (their blanking pre-passes stay
        // TS-side — the route point applies preParse before the kernel call).
        "c" => Some(tree_sitter_c::LANGUAGE.into()),
        "cpp" => Some(tree_sitter_cpp::LANGUAGE.into()),
        // R7b: v0.24.2, sha-matched with the vendored wasm (grammars.ts).
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        // R7b: 0.23.5, table-identical to the vendored ABI-15 wasm (#717;
        // verified against the crates.io tarball — csharp checklist header).
        "csharp" => Some(tree_sitter_c_sharp::LANGUAGE.into()),
        // R7b: v0.23.1, sha-matched with the vendored wasm (grammars.ts).
        // Content bump only — the tag's parser.c is still ABI 14.
        "ruby" => Some(tree_sitter_ruby::LANGUAGE.into()),
        // R7b: v0.24.2, the full HTML-interleaving variant — LANGUAGE_PHP,
        // NEVER LANGUAGE_PHP_ONLY (which errors on leading HTML).
        "php" => Some(tree_sitter_php::LANGUAGE_PHP.into()),
        // R7b: crate 0.7.3 — the vendored wasm is built from this crate's own
        // tarball src/ (table identity by construction; see grammars.ts).
        "swift" => Some(tree_sitter_swift::LANGUAGE.into()),
        // R7b: fwcd 0.3.8, vendored C compiled in build.rs (crate unusable).
        "kotlin" => Some(ffi::kotlin()),
        // R7b batch 4: crate =1.2.0, sha-identical to the r-lib v1.2.0 tag
        // the vendored wasm was built from (r checklist §Grammar prep).
        "r" => Some(tree_sitter_r::LANGUAGE.into()),
        // R7b batch 4: v0.4.1 vendored C compiled in build.rs (revision not
        // on crates.io — the wasm is the v0.4.1 tag, table-identical).
        "lua" => Some(ffi::lua()),
        // R7b batch 4: crate =1.2.0, sha-identical to the v1.2.0 tag the
        // vendored wasm was built from (lua-luau checklist §Grammar prep).
        "luau" => Some(tree_sitter_luau::LANGUAGE.into()),
        // R7b batch 4: master@0aca5d0a6f vendored C compiled in build.rs (the
        // revision is not a release — crate 0.26.0 is 30 states behind).
        "scala" => Some(ffi::scala()),
        // R7b batch 4: UserNobody14 d4d8f3e vendored C compiled in build.rs
        // (same commit as the byte-copied tree-sitter-wasms 0.1.13 artifact).
        "dart" => Some(ffi::dart()),
        // Vendored C compiled in build.rs. The frozen oracle wasms are
        // tree-sitter-wasms 0.1.13 out/ artifacts (objc 2.1.0 = amaanq
        // dea2b2d6; solidity = JoranHonig b239a95f) and the vendored
        // src/extraction/wasm/ artifacts (hcl v1.2.0 fad99186; arkts 0.2.0 =
        // harmony-contrib 56f7fc28; nix 3d0173d regenerated with
        // tree-sitter-cli 0.25.10). All revisions, licenses, and hashes are
        // recorded in tools/oracle/inventories/systems-config.json.
        "objc" => Some(ffi::objc()),
        "solidity" => Some(ffi::solidity()),
        "terraform" => Some(ffi::hcl()),
        "arkts" => Some(ffi::arkts()),
        "nix" => Some(ffi::nix()),
        // Vendored C compiled in build.rs. The frozen oracle wasms are the
        // vendored src/extraction/wasm/ artifacts: pascal (Isopod 042119ec,
        // regenerated ABI-15 parser), cfscript + cfquery (cfmleditor a224ff1,
        // checked-in ABI-15 parser.c + scanner.c), cobol (yutaro-sakamoto
        // e99dbdc3 + docs/grammars/tree-sitter-cobol.patch, regenerated),
        // vbnet (govindbanura 538b708 + docs/grammars/tree-sitter-vbnet.patch,
        // generated), and erlang (WhatsApp 836aa2b, regenerated ABI-15
        // parser + checked-in scanner). All revisions, licenses, and hashes are
        // recorded in tools/oracle/inventories/enterprise.json.
        "pascal" => Some(ffi::pascal()),
        "cfscript" => Some(ffi::cfscript()),
        "cfquery" => Some(ffi::cfquery()),
        // The tag-based cfml grammar is not a native kernel language: it is
        // loaded only by the composite CFML extractor (which dialect-switches
        // between the bare-script cfscript grammar and this tag grammar).
        "cfml" => Some(ffi::cfml()),
        "cobol" => Some(ffi::cobol()),
        "vbnet" => Some(ffi::vbnet()),
        "erlang" => Some(ffi::erlang()),
        _ => None,
    }
}
