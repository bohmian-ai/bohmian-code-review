//! Compiles the vendored grammar C sources — those with no crates.io crate
//! at the frozen upstream revision — into the native tree-sitter grammars.
//!
//! This is a direct port of `codegraph-kernel/build.rs` minus the NAPI
//! setup: the kernel's napi_build::setup() exists only for the cdylib
//! boundary, and production extraction needs no Node ABI. Grammar source
//! revisions are frozen — the sha comments below record the upstream
//! kernel's provenance for each vendored artifact.

fn main() {
    // Kotlin grammar — vendored C, compiled here instead of a crate dep: the
    // crates.io tree-sitter-kotlin 0.3.8 pins `tree-sitter >= 0.21, < 0.23`
    // (the kernel links 0.25) and tree-sitter-kotlin-ng is a DIFFERENT
    // grammar (8 fields vs 0, renamed kinds — extractor-breaking). Sources
    // are the fwcd 0.3.8 tag's checked-in generated artifacts, sha-matched
    // against the crates.io tarball (kotlin checklist §Grammar prep):
    //   parser.c  54104a7ef1555c265b746c790e0f8bb953cc17806e9df0c3af82f7f62c06a70a
    //   scanner.c 27f73337ec357fc341fa57538f34c14277b0346980c3405dc30beab6202ec6d0
    let mut c = cc::Build::new();
    c.include("grammars/kotlin");
    c.file("grammars/kotlin/parser.c");
    c.file("grammars/kotlin/scanner.c");
    c.flag_if_supported("-Wno-unused-parameter");
    c.flag_if_supported("-Wno-unused-but-set-variable");
    c.flag_if_supported("-Wno-trigraphs");
    c.flag_if_supported("-utf-8"); // msvc
    c.compile("tree-sitter-kotlin");
    println!("cargo:rerun-if-changed=grammars/kotlin");

    // Lua grammar — vendored C (second vendored-grammar-C language): the
    // vendored wasm is tree-sitter-grammars/tree-sitter-lua v0.4.1 (tag
    // 816840c592), which is NOT on crates.io (only 0.1/0.2/0.5 exist; 0.5.0
    // adds Lua-5.5 `global` — a future bump with its own gate). Sources are
    // the v0.4.1 tag's checked-in generated artifacts, sha-recorded in the
    // lua-luau checklist §Grammar prep:
    //   parser.c  b34a362e43f0311f405721f3089e94f97f31da403b154d456d093e64609a4081
    //   scanner.c 35bbd630b5a7421d46d2e91185eeea09bf78565d44cb676b63ca20d0f1b54bbd
    let mut lua = cc::Build::new();
    lua.include("grammars/lua");
    lua.file("grammars/lua/parser.c");
    lua.file("grammars/lua/scanner.c");
    lua.flag_if_supported("-Wno-unused-parameter");
    lua.flag_if_supported("-Wno-unused-but-set-variable");
    lua.flag_if_supported("-Wno-trigraphs");
    lua.flag_if_supported("-utf-8"); // msvc
    lua.compile("tree-sitter-lua");
    println!("cargo:rerun-if-changed=grammars/lua");

    // Scala grammar — vendored C (third vendored-grammar-C language): the
    // vendored wasm is tree-sitter/tree-sitter-scala master@0aca5d0a6f (the
    // 2026-04-22 generation sync — 30 states PAST the v0.26.0 tag/crate, so a
    // crate pin would be a silent downgrade). Sources are that commit's
    // checked-in generated artifacts, sha-recorded in the scala checklist
    // §Grammar prep:
    //   parser.c  bc3c3c794f19461d99d04de6c31d57fa3e41243509b9ab023a9b88ed3273d102
    //   scanner.c e4ba242568ee3493015598997bf60f613802616eade62717c21109287ef64752
    let mut scala = cc::Build::new();
    scala.include("grammars/scala");
    scala.file("grammars/scala/parser.c");
    scala.file("grammars/scala/scanner.c");
    scala.flag_if_supported("-Wno-unused-parameter");
    scala.flag_if_supported("-Wno-unused-but-set-variable");
    scala.flag_if_supported("-Wno-trigraphs");
    scala.flag_if_supported("-utf-8"); // msvc
    scala.compile("tree-sitter-scala");
    println!("cargo:rerun-if-changed=grammars/scala");

    // Dart grammar — vendored C (fourth vendored-grammar-C language): the
    // production wasm is the tree-sitter-wasms 0.1.13 artifact, whose dart
    // dependency is an UNPINNED github:UserNobody14/tree-sitter-dart — the
    // vendored wasm byte-copy (src/extraction/wasm/) plus these same-commit
    // sources kill that hazard. crates.io tree-sitter-dart is the nielsenko
    // FORK (different lineage) — rejected. Sources are
    // UserNobody14/tree-sitter-dart master@d4d8f3e337d8 (dart checklist
    // §Grammar prep):
    //   parser.c  5a42b47abb4d494f125dbdee9138979248041689b1aa36355550fa3e28dcb8b8
    //   scanner.c 07a7b7818b175e9460523e705dd88d20f7b5141bac95c593d4426e6d52284996
    let mut dart = cc::Build::new();
    dart.include("grammars/dart");
    dart.file("grammars/dart/parser.c");
    dart.file("grammars/dart/scanner.c");
    dart.flag_if_supported("-Wno-unused-parameter");
    dart.flag_if_supported("-Wno-unused-but-set-variable");
    dart.flag_if_supported("-Wno-trigraphs");
    dart.flag_if_supported("-Wno-unused-function");
    dart.flag_if_supported("-utf-8"); // msvc
    dart.compile("tree-sitter-dart");
    println!("cargo:rerun-if-changed=grammars/dart");

    // Objective-C grammar — vendored C (fifth vendored-grammar-C language):
    // the frozen oracle wasm is tree-sitter-wasms 0.1.13's
    // out/tree-sitter-objc.wasm (sha256 7c1b5bfd…), built from npm
    // tree-sitter-objc 2.1.0 = amaanq/tree-sitter-objc dea2b2d6 (MIT). No
    // crates.io crate ships this revision; the wasm compiles THIS parser.c
    // (ABI 14, no external scanner), so the native table is identical by
    // construction. Sources sha-recorded in tools/oracle/inventories/
    // systems-config.json:
    //   parser.c  <recorded in systems-config inventory>
    let mut objc = cc::Build::new();
    objc.include("grammars/objc");
    objc.file("grammars/objc/parser.c");
    objc.flag_if_supported("-Wno-unused-parameter");
    objc.flag_if_supported("-Wno-unused-but-set-variable");
    objc.flag_if_supported("-Wno-trigraphs");
    objc.flag_if_supported("-utf-8"); // msvc
    objc.compile("tree-sitter-objc");
    println!("cargo:rerun-if-changed=grammars/objc");

    // Solidity grammar — vendored C (sixth vendored-grammar-C language): the
    // frozen oracle wasm is tree-sitter-wasms 0.1.13's
    // out/tree-sitter-solidity.wasm (sha256 160745e4…), built from
    // JoranHonig/tree-sitter-solidity b239a95f (npm 1.2.0, MIT). No crates.io
    // crate ships this revision; the wasm compiles THIS parser.c (ABI 14, no
    // external scanner). Sources sha-recorded in
    // tools/oracle/inventories/systems-config.json.
    let mut solidity = cc::Build::new();
    solidity.include("grammars/solidity");
    solidity.file("grammars/solidity/parser.c");
    solidity.flag_if_supported("-Wno-unused-parameter");
    solidity.flag_if_supported("-Wno-unused-but-set-variable");
    solidity.flag_if_supported("-Wno-trigraphs");
    solidity.flag_if_supported("-utf-8"); // msvc
    solidity.compile("tree-sitter-solidity");
    println!("cargo:rerun-if-changed=grammars/solidity");

    // Terraform/HCL grammar — vendored C (seventh vendored-grammar-C
    // language): the frozen oracle wasm is the vendored
    // src/extraction/wasm/tree-sitter-terraform.wasm (sha256 0d9ef3ae…),
    // byte-identical to @tree-sitter-grammars/tree-sitter-hcl 1.2.0's
    // package artifact, built from tree-sitter-grammars/tree-sitter-hcl
    // v1.2.0 = fad99186 (Apache-2.0). parser.c is ABI 15. Sources
    // sha-recorded in tools/oracle/inventories/systems-config.json.
    let mut hcl = cc::Build::new();
    hcl.include("grammars/terraform");
    hcl.file("grammars/terraform/parser.c");
    hcl.file("grammars/terraform/scanner.c");
    hcl.flag_if_supported("-Wno-unused-parameter");
    hcl.flag_if_supported("-Wno-unused-but-set-variable");
    hcl.flag_if_supported("-Wno-trigraphs");
    hcl.flag_if_supported("-Wno-unused-function");
    hcl.flag_if_supported("-utf-8"); // msvc
    hcl.compile("tree-sitter-hcl");
    println!("cargo:rerun-if-changed=grammars/terraform");

    // ArkTS grammar — vendored C (eighth vendored-grammar-C language): the
    // frozen oracle wasm is the vendored src/extraction/wasm/
    // tree-sitter-arkts.wasm (sha256 db081297…), byte-identical to npm
    // tree-sitter-arkts 0.2.0's package artifact, built from
    // harmony-contrib/tree-sitter-arkts 56f7fc28 (MIT). parser.c is ABI 14.
    // Sources sha-recorded in tools/oracle/inventories/systems-config.json.
    let mut arkts = cc::Build::new();
    arkts.include("grammars/arkts");
    arkts.file("grammars/arkts/parser.c");
    arkts.file("grammars/arkts/scanner.c");
    arkts.flag_if_supported("-Wno-unused-parameter");
    arkts.flag_if_supported("-Wno-unused-but-set-variable");
    arkts.flag_if_supported("-Wno-trigraphs");
    arkts.flag_if_supported("-Wno-unused-function");
    arkts.flag_if_supported("-utf-8"); // msvc
    arkts.compile("tree-sitter-arkts");
    println!("cargo:rerun-if-changed=grammars/arkts");

    // Nix grammar — vendored C (ninth vendored-grammar-C language): the
    // frozen oracle wasm is the vendored src/extraction/wasm/
    // tree-sitter-nix.wasm (sha256 4acffa1c…), built from
    // nix-community/tree-sitter-nix 3d0173d (MIT) by REGENERATING parser.c
    // with tree-sitter-cli 0.25.10 (the checked-in parser.c is ABI 13; the
    // regenerated parser is ABI 15, matching the wasm). scanner.c is the
    // checked-in hand-written external scanner (never regenerated).
    //   parser.c  0f185844da0e8e033127ebaa625554994eab6ef5d1a9629dd41b579fb41a8792
    //   scanner.c 45e12521e8be62ea47525417c64d9372fa34e9e12bff643c430b8b3607be9d56
    let mut nix = cc::Build::new();
    nix.include("grammars/nix");
    nix.file("grammars/nix/parser.c");
    nix.file("grammars/nix/scanner.c");
    nix.flag_if_supported("-Wno-unused-parameter");
    nix.flag_if_supported("-Wno-unused-but-set-variable");
    nix.flag_if_supported("-Wno-trigraphs");
    nix.flag_if_supported("-Wno-unused-function");
    nix.flag_if_supported("-utf-8"); // msvc
    nix.compile("tree-sitter-nix");
    println!("cargo:rerun-if-changed=grammars/nix");

    // Pascal grammar — vendored C (tenth vendored-grammar-C language): the
    // frozen oracle wasm is src/extraction/wasm/tree-sitter-pascal.wasm (ABI
    // 15), built from Isopod/tree-sitter-pascal v0.10.2 = 042119ec (MIT). The
    // repo's checked-in parser.c is ABI 14, so the vendored parser.c is the
    // REGENERATED ABI-15 parser from grammar.js at the pinned commit with
    // tree-sitter-cli 0.25.10 (same revision/CLI the wasm was built with).
    // Sources sha-recorded in tools/oracle/inventories/enterprise.json.
    let mut pascal = cc::Build::new();
    pascal.include("grammars/pascal");
    pascal.file("grammars/pascal/parser.c");
    pascal.flag_if_supported("-Wno-unused-parameter");
    pascal.flag_if_supported("-Wno-unused-but-set-variable");
    pascal.flag_if_supported("-Wno-trigraphs");
    pascal.flag_if_supported("-utf-8"); // msvc
    pascal.compile("tree-sitter-pascal");
    println!("cargo:rerun-if-changed=grammars/pascal");

    // CFScript grammar — vendored C (eleventh vendored-grammar-C language):
    // the frozen oracle wasm is src/extraction/wasm/tree-sitter-cfscript.wasm
    // (ABI 15), from cfmleditor/tree-sitter-cfml a224ff1 (MIT) — cfscript is
    // one of the monorepo's three grammars. The vendored parser.c + scanner.c
    // are the repo's checked-in ABI-15 artifacts (the upstream build was
    // verified bit-for-bit reproducible from the same commit). Sources
    // sha-recorded in tools/oracle/inventories/enterprise.json.
    let mut cfscript = cc::Build::new();
    cfscript.include("grammars/cfscript");
    cfscript.file("grammars/cfscript/parser.c");
    cfscript.file("grammars/cfscript/scanner.c");
    cfscript.flag_if_supported("-Wno-unused-parameter");
    cfscript.flag_if_supported("-Wno-unused-but-set-variable");
    cfscript.flag_if_supported("-Wno-unused-function");
    cfscript.flag_if_supported("-Wno-trigraphs");
    cfscript.flag_if_supported("-utf-8"); // msvc
    cfscript.compile("tree-sitter-cfscript");
    println!("cargo:rerun-if-changed=grammars/cfscript");

    // CFQuery grammar — vendored C (twelfth vendored-grammar-C language):
    // the frozen oracle wasm is src/extraction/wasm/tree-sitter-cfquery.wasm
    // (ABI 15), from cfmleditor/tree-sitter-cfml a224ff1 (MIT). Vendored
    // parser.c + scanner.c are the repo's checked-in ABI-15 artifacts; the
    // scanner's relative includes are adapted to the vendored layout
    // (cfml-common/ scanner.h + tag.h), a mechanical include-path fix
    // recorded in the enterprise inventory.
    let mut cfquery = cc::Build::new();
    cfquery.include("grammars/cfquery");
    cfquery.file("grammars/cfquery/parser.c");
    cfquery.file("grammars/cfquery/scanner.c");
    cfquery.flag_if_supported("-Wno-unused-parameter");
    cfquery.flag_if_supported("-Wno-unused-but-set-variable");
    cfquery.flag_if_supported("-Wno-unused-function");
    cfquery.flag_if_supported("-Wno-trigraphs");
    cfquery.flag_if_supported("-utf-8"); // msvc
    cfquery.compile("tree-sitter-cfquery");
    println!("cargo:rerun-if-changed=grammars/cfquery");

    // CFML grammar — vendored C (thirteenth vendored-grammar-C language): the
    // frozen oracle wasm is src/extraction/wasm/tree-sitter-cfml.wasm (ABI 15),
    // from cfmleditor/tree-sitter-cfml a224ff1 (MIT) — the tag-based `cfml`
    // subgrammar of the same monorepo commit as cfscript/cfquery. Vendored
    // parser.c + scanner.c + tag.c are the repo's checked-in ABI-15 artifacts;
    // the scanner's and tag.c's relative includes are adapted to the vendored
    // layout (cfml-common/ scanner.h + tag.h), the same mechanical include-path
    // fix recorded for cfquery in the enterprise grammar manifest.
    let mut cfml = cc::Build::new();
    cfml.include("grammars/cfml");
    cfml.file("grammars/cfml/parser.c");
    cfml.file("grammars/cfml/scanner.c");
    cfml.file("grammars/cfml/tag.c");
    cfml.flag_if_supported("-Wno-unused-parameter");
    cfml.flag_if_supported("-Wno-unused-but-set-variable");
    cfml.flag_if_supported("-Wno-unused-function");
    cfml.flag_if_supported("-Wno-trigraphs");
    cfml.flag_if_supported("-utf-8"); // msvc
    cfml.compile("tree-sitter-cfml");
    println!("cargo:rerun-if-changed=grammars/cfml");

    // COBOL grammar — vendored C (fourteenth vendored-grammar-C language):
    // the frozen oracle wasm is src/extraction/wasm/tree-sitter-cobol.wasm
    // (ABI 15), built from yutaro-sakamoto/tree-sitter-cobol e99dbdc3 with
    // docs/grammars/tree-sitter-cobol.patch applied (grammar.js + scanner.c)
    // and parser.c regenerated with tree-sitter-cli 0.25.10 (the upstream
    // checked-in parser.c is ABI 14). The vendored scanner.c is the patched
    // upstream scanner.c (never regenerated). Sources sha-recorded in
    // tools/oracle/inventories/enterprise.json.
    let mut cobol = cc::Build::new();
    cobol.include("grammars/cobol");
    cobol.file("grammars/cobol/parser.c");
    cobol.file("grammars/cobol/scanner.c");
    cobol.flag_if_supported("-Wno-unused-parameter");
    cobol.flag_if_supported("-Wno-unused-but-set-variable");
    cobol.flag_if_supported("-Wno-unused-function");
    cobol.flag_if_supported("-Wno-trigraphs");
    cobol.flag_if_supported("-utf-8"); // msvc
    cobol.compile("tree-sitter-cobol");
    println!("cargo:rerun-if-changed=grammars/cobol");

    // VB.NET grammar — vendored C (fifteenth vendored-grammar-C language):
    // the frozen oracle wasm is src/extraction/wasm/tree-sitter-vbnet.wasm
    // (ABI 15), built from govindbanura/tree-sitter-vbnet 538b708 with
    // docs/grammars/tree-sitter-vbnet.patch applied (grammar.js edits + a new
    // src/scanner.c) and parser.c regenerated with tree-sitter-cli 0.25.10
    // (required for the /u regex flag; the upstream repo checks in no
    // parser.c). Sources sha-recorded in
    // tools/oracle/inventories/enterprise.json.
    let mut vbnet = cc::Build::new();
    vbnet.include("grammars/vbnet");
    vbnet.file("grammars/vbnet/parser.c");
    vbnet.file("grammars/vbnet/scanner.c");
    vbnet.flag_if_supported("-Wno-unused-parameter");
    vbnet.flag_if_supported("-Wno-unused-but-set-variable");
    vbnet.flag_if_supported("-Wno-unused-function");
    vbnet.flag_if_supported("-Wno-trigraphs");
    vbnet.flag_if_supported("-utf-8"); // msvc
    vbnet.compile("tree-sitter-vbnet");
    println!("cargo:rerun-if-changed=grammars/vbnet");

    // Erlang grammar — vendored C (sixteenth vendored-grammar-C language):
    // the frozen oracle wasm is src/extraction/wasm/tree-sitter-erlang.wasm
    // (ABI 15), built from WhatsApp/tree-sitter-erlang v0.19.0 = 836aa2b (the
    // ELP grammar, Apache-2.0). The repo's checked-in parser.c is ABI 14, so
    // the vendored parser.c is the REGENERATED ABI-15 parser from grammar.js
    // at the pinned commit with tree-sitter-cli 0.25.10; scanner.c is the
    // checked-in hand-written external scanner (never regenerated). Sources
    // sha-recorded in tools/oracle/inventories/enterprise.json.
    let mut erlang = cc::Build::new();
    erlang.include("grammars/erlang");
    erlang.file("grammars/erlang/parser.c");
    erlang.file("grammars/erlang/scanner.c");
    erlang.flag_if_supported("-Wno-unused-parameter");
    erlang.flag_if_supported("-Wno-unused-but-set-variable");
    erlang.flag_if_supported("-Wno-unused-function");
    erlang.flag_if_supported("-Wno-trigraphs");
    erlang.flag_if_supported("-utf-8"); // msvc
    erlang.compile("tree-sitter-erlang");
    println!("cargo:rerun-if-changed=grammars/erlang");
}
