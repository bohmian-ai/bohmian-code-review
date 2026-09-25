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
