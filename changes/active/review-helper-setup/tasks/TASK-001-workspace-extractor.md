---
id: CR-SETUP-T01
kind: implementation
status: proposed
spec: SPEC-review-helper-setup
spec_revision: 2
requirements: [REQ-001, REQ-002, INV-001, INV-005, INV-006, AC-001, AC-002]
depends_on: []
---

# Independent workspace and complete extractor

## Outcome and Value

A standalone six-crate Rust workspace exposes the complete donor extractor over caller-supplied in-memory source. All supported routes and the unchanged fixture corpus work without the donor checkout. Required execution skill: `$cr-implement`.

## Current Repository Facts

No Cargo workspace, lockfile, toolchain file, or crate exists. `mise.toml` has only skill-sync tasks. The donor's extraction-relevant core values and complete extractor source, grammars, fixtures, and provenance are available locally as port inputs. Existing contributor documents are task 04's authority surface.

## Owners, Scope, Consumers, and Prohibited Changes

`bohmian-code-review-core` owns IO-free facts and validated paths; `bohmian-code-review-extract` owns routing, native/composite walkers, decoding, preprocessing, diagnostics, bounded synchronous parsing, and grammar resources. The index, selector, and facade consume their outputs in later tasks. Establish all six manifests in the approved downward dependency direction, while implementing only core and extract behavior here. Preserve all donor routes, exact grammar pins, checked-in fixtures/goldens, provenance, and applicable licenses. No runtime Git, network, disk source read, storage, model, donor dependency, local `SourceTree`, or golden rewrite.

## Approach

1. Bootstrap Rust 2024, Rust 1.94, the six-package workspace, lockfile, repository artifacts, and core/extractor owner lanes.
2. Port extraction-relevant core facts and canonical path validation.
3. Port the full extractor, vendored grammars, corpus, and provenance with rehomed imports.
4. Close native/composite route and file-local failure behavior against unchanged evidence.

## Ordered Implementation Scenarios

### Scenario 1 — Validated shared facts

**Behavior.** Core accepts canonical repository-relative UTF-8 paths, rejects invalid/over-limit paths, and exposes donor extraction facts with one-based positions. Covers REQ-001, REQ-002, INV-001, INV-005.

**RED.** After a compiling workspace scaffold exists, add a focused core test for valid/invalid paths and fact positions; run `mise exec -- cargo test --locked -p bohmian-code-review-core --lib`. Record a failing assertion for missing behavior, not a setup/compile failure. The implementer records the exact selector once the test is named.

**GREEN.** Port only the needed core values and make the focused check pass; rerun prior core checks.

**REFACTOR.** Keep one canonical path/fact owner and remove only redundant adaptation while checks stay green.

### Scenario 2 — Complete route and corpus behavior

**Behavior.** Every supported native/composite donor route produces the approved nodes, edges, references, coordinates, and diagnostics from supplied bytes; the local copied corpus passes unchanged. Covers REQ-002, INV-001, INV-006, AC-002.

**RED.** Once the local corpus harness compiles, run `mise run test:extract` and observe a missing route/behavior against an unchanged fixture. A missing target, donor path, or compile error is a prerequisite failure to repair, not RED evidence.

**GREEN.** Port the corresponding extractor behavior and rerun the affected check and prior green routes until the full corpus passes.

**REFACTOR.** Keep donor semantics and pins; simplify only import/build adaptation.

### Scenario 3 — File-local failure and bounded parsing

**Behavior.** Malformed or unsupported input has typed file-local outcomes where the extractor permits recovery; a later valid file still parses, and synchronous batch work stays bounded. Covers REQ-002, INV-001, AC-002.

**RED.** Add a focused extractor test with a bad file and a valid file; run `mise run test:extract` and record the missing isolation/outcome assertion.

**GREEN.** Restore donor routing, error, and batch behavior; rerun earlier scenarios and the corpus.

**REFACTOR.** Keep parser state on its concrete owner; do not add a provider abstraction.

## Acceptance Criteria

- Six packages, Rust 2024/1.94, checked-in lockfile, README, license, notices, and approved dependency direction exist and compile independently.
- Core validates paths and exposes extraction facts; every donor route and unchanged local fixture/golden passes.
- Vendored grammar sources, exact pins, provenance, and `OUT_DIR`-only generated output are present; runtime code needs no checkout or network service.

## Expected Write Set and Consumer Closure

Likely surfaces: root manifests/toolchain/lockfile, `.gitignore`, `README.md`, `LICENSE`, notices; `crates/bohmian-code-review-core/**`, `crates/bohmian-code-review-extract/**`, minimal roots/manifests for the four later crates, and `fmt`, `fmt:check`, `lints`, `test:extract` in `mise.toml`. These paths guide ownership, not private layout. Tasks 02–03 consume the real facts/extractor; task 03 proves them through the public journey.

## Verification and Evidence

Run the focused core command, `mise run test:extract`, `mise run fmt:check`, `mise run lints`, workspace type-check, and `git diff --check`; inspect manifests, pins, fixtures, notices, and tracked/untracked changes. Bootstrap artifacts use static proof rather than manufactured RED. Record each behavioral RED/GREEN cycle and acceptance evidence. An absent lane or donor-dependent fixture is failure.

## Material Stop Conditions

Return `SPEC_REVISION_REQUIRED` if complete extraction requires changing approved grammar behavior, shared facts, or crate direction. Do not reduce route coverage, edit goldens, or import excluded donor systems.

## Authority Links

`changes/active/review-helper-setup/spec.md`; `AGENTS.md`; `architecture/agent-rules.md`; `architecture/bohmian-code-review-design.md`; `architecture/references/domain/symbolic-context.md`; `architecture/references/languages/spec-driven-development.md`; `architecture/references/languages/implementation-execution.md`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md`.

## Implementation Evidence

Status: `IMPLEMENTED` on branch `review-helper-setup` (commits `0fc361e`, `d71d5eb`, `9ca27dc`, plus this record). Donor: `codegraph-rs-parity` `c757d1e776c074af58bb1131145cd3c82fa72718`.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Six packages, Rust 2024/1.94, lockfile, README, license, notices, downward direction, independent compile | `Cargo.toml`, `rust-toolchain.toml`, `Cargo.lock`, `crates/*/Cargo.toml`, `README.md`, `LICENSE`, `THIRD_PARTY_NOTICES.md`, `.gitignore` | `cargo check --locked -p <each of 6> --all-targets` PASS; `cargo tree -p bohmian-code-review` shows only the five lower crates; no `codegraph`/`../` path in any manifest | PASS |
| Core validates paths and exposes extraction facts | `crates/bohmian-code-review-core/src/{relative_path,position,diagnostic,edge,language,node,reference,limits}.rs` | `cargo test --locked -p bohmian-code-review-core --lib` 25 passed | PASS |
| Every donor route and unchanged fixture/golden passes | `crates/bohmian-code-review-extract/src/**`, `tests/`, `tools/oracle/{fixtures,goldens,inventories}` | `mise run test:extract`: extract lib 138 passed (incl. corpus/golden differentials, defer contract, route tests), `ccpp_fixture_probe` 1 passed; `diff -rq` vs donor for fixtures, goldens, inventories, provenance, grammars, `build.rs`: identical | PASS |
| Vendored grammars, exact pins, provenance, `OUT_DIR`-only output; no checkout/network at runtime | `crates/bohmian-code-review-extract/{grammars,build.rs}`, workspace `=` pins, `tools/oracle/parity/extract/**`, `docs/grammars/`, `tools/oracle/wasm/` | grammar/enterprise manifest tests verify source, patch, output, and wasm SHA-256; runtime `src` (excluding `corpus_tests.rs`) has no `std::fs`, `std::net`, or process use | PASS |

RED/GREEN record:

- Scenario 1 — RED: `mise exec -- cargo test --locked -p bohmian-code-review-core --lib relative_path` against a permissive `RelativePath::new` stub failed 3 assertions (`invalid_paths_are_rejected`: `Ok(RelativePath(""))` vs `Err(Empty)`; `noncanonical_segments_are_rejected`; `deserialization_enforces_the_canonical_invariant`). GREEN: donor validation ported; core lib 25 passed.
- Scenario 2 — no behavioral RED observed: the verbatim port matched every fixture once prerequisite harness paths were repaired (below). GREEN: `mise run test:extract` all passed.
- Scenario 3 — `mise exec -- cargo test --locked -p bohmian-code-review-extract --lib parse_pool::tests::malformed_file_defers_alone_and_later_valid_file_parses -- --exact` passed on first run: the ported pool already isolates a `Deferred` TypeScript file between two valid files. Recorded as characterization, not RED. Donor `one_failing_file_does_not_abort_the_batch`, `bounded_workers_do_not_change_batch_results`, and `cancellation_leaves_unattempted_requests_unparsed` cover unroutable input, bounded workers, and cancellation.

Verification (sequential): `mise run test:extract` PASS; `mise run fmt:check` PASS; `mise run lints` (`clippy --workspace --all-targets --all-features -D warnings`) PASS; `cargo check --locked --workspace --all-targets` PASS; `git diff --check` PASS. Features: default (no crate features exist).

Execution updates:

- 2026-09-25, bounded correction: corpus harness hard-coded donor `crates/codegraph-extract/grammars`, and manifests record `crates/codegraph-extract/...` committed outputs. Added `grammars_root()` and `manifest_recorded_path()` in `corpus_tests.rs` mapping that prefix to this crate; manifests unchanged. Three grammar-manifest tests went from prerequisite failure to PASS.
- 2026-09-25, bounded correction: arkts, terraform, nix, and cfml oracle wasm hashes fall back to the donor's upstream `src/extraction/wasm/`. Copied those four files to `tools/oracle/wasm/` and repointed the fallback; hashes are still verified against the unchanged manifests.
- 2026-09-25, local: `Cargo.lock` seeded from the donor lock so shared crates keep donor-resolved versions; rehomed crate names required `cargo fmt` line re-wrapping only.

Non-goals: storage, snapshots, `SourceTree`, query facade, full resolver, search ranking, identity/config/lifecycle core modules, Node tooling (`napi-adapter`, `.mjs` scripts), and index/resolve/select/facade behavior — EXCLUDED (later crates hold only a crate doc line).

Risks: the donor vendored no `LICENSE` file for the kotlin grammar (MIT, `fwcd/tree-sitter-kotlin` 0.3.8); noted in `THIRD_PARTY_NOTICES.md`, text not fabricated. Donor walkers read `CODEGRAPH_VALUE_REFS`/`COBOL_DBG` env vars (preserved donor behavior; no IO). The repository grows by ~380 MB of vendored grammar and provenance input, as accepted by the plan. `IMPLEMENTATION_PLAN.md` carries a pre-existing uncommitted edit that is not part of this task.
