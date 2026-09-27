---
id: TASK-008
kind: implementation
status: proposed
spec: SPEC-repo-parser@14
depends_on: [TASK-001, TASK-003]
maps: [REQ-002, REQ-005, REQ-006, INV-002, INV-003, AC-001, AC-004]
---

## Outcome and Value

A file with a syntax error yields the declarations tree-sitter recovered outside the error region instead of being rejected whole (REQ-002). A verified file with no extractable facts is a recorded content skip that does not stop the snapshot from being complete (Complete snapshot term, REQ-005, REQ-006). Real repositories whose only blockers were syntax-error files, such as `bohmian-ai/wyrd`, now publish a reusable snapshot.

## Owners, Scope, Consumers, and Prohibited Changes

`bohmian-code-review-extract` owns the change: every language module that ports codegraph's `has_error()` "wasm recovery is canonical" defer rule loses it and extracts from the recovered tree. `EXTRACTION_VERSION` changes, so snapshots built under the old version are never reused (INV-003). The facade (`bohmian-code-review`) and the outer pipeline crate are consumers. The pipeline's cold build, update and PR overlay record a no-facts file as a skip with its blob identity. Cold build and update count that skip as a content skip, like oversize and LFS pointers.

Prohibited: changing grammar pins or vendored grammar C; changing any checked-in fixture or golden for a file that parses without errors; producing facts that the recovered tree does not contain; treating a fetch, verification, or other extractor failure as a benign skip.

## Approach

1. Remove the whole-file error rejection from every language module and let extraction walk the recovered tree.
2. Replace the corpus's `defer` expectation for error fixtures with the new recovered-tree outcome. Leave every `match` fixture and golden untouched.
3. Bump `EXTRACTION_VERSION`.
4. In the pipeline, record an indexed file with no extractable facts as a content skip that keeps the snapshot complete.
5. Re-run cold build against `bohmian-ai/wyrd` and record that the two named files index.

## Ordered Implementation Scenarios

### Scenario 1 — Syntax error keeps recovered declarations

**Behavior.** Rust source with an attribute on a field inside a destructuring pattern (a tree-sitter-rust 0.24.2 grammar gap, as in wyrd `crates/wyrd/wyrd-server/src/boot/mod.rs`) extracts its declarations outside the error region. TypeScript source with a raw NUL byte inside a string literal (as in wyrd `wyrd-ui/src/lib/server/wyrd.ts`) does the same. Neither returns `ExtractionError::Deferred` (REQ-002).

**RED.** Add focused extractor tests with minimal in-memory reproductions of both constructs, run with `cargo test --locked -p bohmian-code-review-extract --all-features recovered_tree`. They fail with `ExtractionError::Deferred`.

**GREEN.** Delete the defer rule in every language module that ports it. Rerun the full `mise run test:extract`: every `match` fixture must stay byte-identical to its golden. Update `defer` corpus fixtures to assert successful recovered-tree extraction, not a deferral. They must not assert equality with the oracle's wasm-recovered golden, and they must not rewrite it.

**REFACTOR.** Remove the deferral path that nothing produces anymore, including docs that describe wasm deferral, if no remaining caller needs it. Keep error variants that other failures still use.

### Scenario 2 — Extraction version changes

**Behavior.** Facts exported under the previous version fail to load (`FactsVersionMismatch` or equivalent), and a snapshot key under the old version is not reused (REQ-002, INV-003).

**RED.** Assert in the public journey that `EXTRACTION_VERSION` differs from the previous value `codegraph-kernel-572d22bf.tree-sitter-0.25.10.facts-1`: `mise run test:symbolic-context:journey`. It fails until the constant changes.

**GREEN.** Bump the constant.

**REFACTOR.** None beyond keeping one source of the version string.

### Scenario 3 — No-facts file is a non-blocking recorded skip

**Behavior.** In a cold build, a verified eligible file that extracts to no facts is recorded in `coverage.skipped` with its path, blob, and a reason that carries no source. The build is still complete and publishes. A file skipped because it could not be fetched or verified still makes the build incomplete (Complete snapshot term, INV-002). Update reaches the same result through the shared ingest path.

**RED.** Add a pipeline test through cold build over the fake provider with one normal file and one file that yields no facts, run with `cargo test --locked -p bohmian-code-review-pipeline --all-features --lib no_facts`. It fails because the build is not complete or records no skip. Update the existing assertion that a deferred parse failure makes the build incomplete, near `crates/bohmian-code-review-pipeline/src/snapshot.rs:1591`, to the new behavior. Do not delete it.

**GREEN.** Record the no-facts outcome as a content skip that counts toward completeness. Rerun `mise run test:pipeline`.

**REFACTOR.** Keep one definition of which skip reasons a complete snapshot may carry.

## Acceptance Criteria

- Syntax-error files in every ported language return recovered facts, not a deferral. Valid-file fixtures and goldens are unchanged, and `test:extract` passes.
- `EXTRACTION_VERSION` differs from the prior value, and old-version facts are rejected.
- A no-facts file is a recorded skip with blob identity and does not block a complete snapshot. Fetch or verification skips still block it.
- A live cold build of `bohmian-ai/wyrd` indexes `crates/wyrd/wyrd-server/src/boot/mod.rs` and `wyrd-ui/src/lib/server/wyrd.ts`.

## Expected Write Set and Consumer Closure

`crates/bohmian-code-review-extract/src/**` (language modules, `error.rs`, `extractor.rs`, `lib.rs`, `corpus_tests.rs` and the `defer`-mode fixture expectations); the facade journey; `crates/bohmian-code-review-pipeline/src/snapshot.rs` and its tests; an evidence note under `changes/active/repo-parser/evidence/TASK-008/`.

## Verification and Evidence

`mise run test:extract`, `mise run test:symbolic-context:journey`, `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`. Record the live `bohmian-ai/wyrd` cold-build coverage (commit, eligible and indexed counts, skips by reason) using the existing `mise run live:pr` entrypoint or an equivalent explicitly invoked run.

## Material Stop Conditions

Stop if a language's recovered tree makes a `match` fixture differ from its golden. Stop if a grammar produces facts from inside an error region that the spec's "drops only the declarations inside its error region" rule cannot accept. Stop if removing the deferral would change a public error contract that consumers other than the facade and pipeline depend on.

## Authority Links

[Revision 14 spec](../spec.md) (REQ-002, Complete snapshot), [AGENTS.md](../../../../AGENTS.md), [library design](../../../../architecture/bohmian-code-review-design.md).
