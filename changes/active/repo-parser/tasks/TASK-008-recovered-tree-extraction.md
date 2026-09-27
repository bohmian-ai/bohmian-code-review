---
id: TASK-008
kind: implementation
status: proposed
spec: SPEC-repo-parser@14
depends_on: [TASK-001, TASK-003]
maps: [REQ-002, REQ-005, REQ-006, INV-002, INV-003, AC-001, AC-004]
---

## Outcome and Value

A file with a syntax error yields the declarations tree-sitter recovered, including any inside an error region, instead of being rejected whole (REQ-002). A verified file with no extractable facts is a recorded content skip that does not stop the snapshot from being complete (Complete snapshot term, REQ-005, REQ-006). Real repositories whose only blockers were syntax-error files, such as `bohmian-ai/wyrd`, now publish a reusable snapshot.

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

**Behavior.** Rust source with an attribute on a field inside a destructuring pattern (a tree-sitter-rust 0.24.2 grammar gap, as in wyrd `crates/wyrd/wyrd-server/src/boot/mod.rs`) extracts the declarations in its recovered tree. TypeScript source with a raw NUL byte inside a string literal (as in wyrd `wyrd-ui/src/lib/server/wyrd.ts`) does the same. Neither returns `ExtractionError::Deferred` (REQ-002).

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

Stop if a language's recovered tree makes a `match` fixture differ from its golden. Stop if removing the deferral would change a public error contract that consumers other than the facade and pipeline depend on.

## Authority Links

[Revision 14 spec](../spec.md) (REQ-002, Complete snapshot), [AGENTS.md](../../../../AGENTS.md), [library design](../../../../architecture/bohmian-code-review-design.md).

## Implementation Evidence

Environment: macOS 15 (Darwin 24.6.0), Apple silicon, 2026-09-27. The live run used a read-only `gh` user token and only read GitHub. The [report](../evidence/TASK-008/s20-wyrd-pr1.json) carries paths, ranges and identities, never source text.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Syntax-error files in every ported language return recovered facts, not a deferral; valid-file fixtures and goldens unchanged | `has_error()` rejection deleted from all 25 walkers; `ExtractionError::Deferred` and `CANONICAL_DEFER_MESSAGE` removed; `extractor.rs` `recovered_tree_*` tests (Rust destructured-field attribute, TypeScript NUL in string); the 21 malformed corpus fixtures are now `recover` entries (`manifest.json`), and `recover_fixtures_keep_valid_regions_per_fixture` asserts that native extraction keeps each fixture's valid-region declarations. No golden or `match` fixture changed | `cargo test ... recovered_tree` failed with `Deferred` before the change; `mise run test:extract` (142 + 27 passed) | PASS |
| `EXTRACTION_VERSION` differs from the prior value; old-version facts are rejected | `lib.rs` → `codegraph-kernel-572d22bf.tree-sitter-0.25.10.recovered-tree.facts-2`; journey loads facts stamped `…facts-1` and expects `ExtractionVersionMismatch` | `mise run test:symbolic-context:journey` failed before the bump and passed after it (6 passed) | PASS |
| No-facts file is a recorded skip with blob identity and does not block a complete snapshot; fetch/verification skips still block | `SkipReason::NoFacts`, added to the single `is_benign` set; `Ingest::ingest` removes a file-node-only upsert and records the skip. Cold build, update, and PR overlay share that path. `no_facts_file_is_a_recorded_skip_in_a_published_build`; Scenario 2's broken-file segment now asserts publication with a `NoFacts` skip; `NotFetched`/`FetchFailed` incompleteness assertions unchanged | `cargo test ... --lib no_facts` failed before the change; `mise run test:pipeline` (68 + 2 passed) | PASS |
| Live cold build of `bohmian-ai/wyrd` indexes `crates/wyrd/wyrd-server/src/boot/mod.rs` and `wyrd-ui/src/lib/server/wyrd.ts` | `examples/live_pr.rs` now reports the cold build's skipped files, not just a count | `mise run live:pr` with `BCR_REPO_ID=1383739221 BCR_PR=1`: commit `2855eac`, eligible 1,681, indexed 1,538, skipped 143 (all `NoFacts`; 31 are the empty blob, the rest are `mod`/re-export-only Rust, key-less YAML and empty `__init__.py`), outcome `Published`. Neither named file is skipped. PR context: `full_coverage: true`, 50 spans, `read_back_equal: true`, exit 0, 29.0 s, 135 MB peak RSS | PASS |

Before this change, the TASK-007 run on the same commit indexed 1,678 files and left 3 files unindexed (`Incomplete`). The 140 files that now show as `NoFacts` were counted as indexed before even though they held only a file node. Under the revision 14 Complete-snapshot term they are content skips. PR #91 still exhausts its review budget, which is unrelated to this task.

Non-goals stayed excluded: no grammar pin, vendored C, `match` fixture or golden changed. No facts come from outside the recovered tree, and fetch, verification and extractor failures remain blocking. The only public additions are `SymbolicContext::has_declarations` and `SkipReason::NoFacts` (TASK-008-R2 replaced the earlier `NodeKind` re-export).

Commands: `mise run test:extract`, `mise run test:symbolic-context:journey`, `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `mise run live:pr`, `mise run gate`, `git diff --check`.
