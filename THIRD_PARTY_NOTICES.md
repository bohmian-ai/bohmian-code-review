# Third-party notices

## Extractor port

`bohmian-code-review-core` extraction facts and `bohmian-code-review-extract` are ported from `codegraph-rs-parity` commit `c757d1e776c074af58bb1131145cd3c82fa72718` (`crates/codegraph-core`, `crates/codegraph-extract`). That repository is a fork of [`colbymchenry/codegraph`](https://github.com/colbymchenry/codegraph) at `572d22bfbe82602080e457bec655f72e3314f9ef`, licensed under MIT:

> MIT License
>
> Copyright (c) 2026 Colby Mchenry
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

The fixture corpus, goldens, inventories, grammar provenance manifests, and provenance inputs under `tools/oracle/` and `docs/grammars/` are copied from the same commit. `tools/oracle/wasm/` holds the upstream `src/extraction/wasm/` grammar artifacts the provenance manifests pin by SHA-256.

## Vendored grammars

Each directory under `crates/bohmian-code-review-extract/grammars/` is compiled by `build.rs` and keeps its upstream `LICENSE` beside the sources. Upstream revisions and source hashes are recorded in `build.rs` and `tools/oracle/parity/extract/*-grammars.json`.

| Grammar | License |
|---|---|
| arkts, cfml, cfml-common, cfquery, cfscript, cobol, lua, nix, objc, pascal, scala, vbnet | MIT (see directory `LICENSE`) |
| dart, solidity | MIT-style (see directory `LICENSE`) |
| erlang, terraform | Apache-2.0 (see directory `LICENSE`) |
| kotlin (`fwcd/tree-sitter-kotlin` 0.3.8) | MIT; the donor vendored no license file |
| common (ArkTS shared scanner header) | Covered by the arkts grammar `LICENSE` |

## Grammar crates

`tree-sitter` 0.25.10, `tree-sitter-language` 0.1.7, `tree-sitter-typescript` 0.23.2, `tree-sitter-javascript` 0.25.0, `tree-sitter-java` 0.23.5, `tree-sitter-python` 0.23.6, `tree-sitter-go` 0.23.4, `tree-sitter-c` 0.24.2, `tree-sitter-cpp` 0.23.4, `tree-sitter-rust` 0.24.2, `tree-sitter-c-sharp` 0.23.5, `tree-sitter-ruby` 0.23.1, `tree-sitter-php` 0.24.2, `tree-sitter-swift` 0.7.3, `tree-sitter-r` 1.2.0, and `tree-sitter-luau` 1.2.0 are MIT-licensed crates.io dependencies pinned exactly in the workspace manifest.
