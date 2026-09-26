---
id: CR-SETUP-T02
kind: implementation
status: proposed
spec: SPEC-review-helper-setup
spec_revision: 2
requirements: [REQ-003, REQ-004, REQ-010, INV-001, INV-002, INV-003, INV-004, INV-005, AC-003]
depends_on: [CR-SETUP-T01]
---

# In-memory symbol index and candidate lookup

## Outcome and Value

Extracted facts enter an exact symbol-to-file index one file at a time; reference and declaration names yield all plausible candidate hits with deterministic order and visible ambiguity. Required execution skill: `$cr-implement`.

## Current Repository Facts

Task 01 provides validated core facts and the extractor. Index/resolver behavior and their owner lanes do not yet exist. The resolver's useful behavior depends directly on index postings, so both owners close one candidate-lookup outcome here.

## Owners, Scope, Consumers, and Prohibited Changes

`bohmian-code-review-index` owns per-file facts, byte-length metadata, exact simple/qualified postings, replacement, removal, and lookup; it consumes core, not extract. `bohmian-code-review-resolve` owns matching extracted `reference_name`, qualified candidates, and changed declaration names to index hits; it consumes core and index. Selector/facade consume candidates. Preserve case sensitivity, all ambiguous hits, path/location/ID order, atomic replacement, idempotent removal, and partial-coverage honesty. No source blobs, parser dependency in index, persistence, fuzzy match, or proven-definition claim. Facade transaction behavior after extraction failure closes in task 03.

## Approach

1. Establish package-scoped `test:index` and `test:resolve` lanes.
2. Build exact postings and deterministic lookup from core facts.
3. Implement complete per-file replacement/removal and fact-only retention.
4. Resolve all approved name sources through the real index, preserving ambiguity.

## Ordered Implementation Scenarios

### Scenario 1 — Exact symbol hits

**Behavior.** Simple/qualified lookup returns every matching declaration, including duplicates, in canonical path/location/ID order; unknown names return no hit. Covers REQ-003, INV-002, INV-004, AC-003.

**RED.** Add a focused index owner test using core facts for duplicates, qualified names, and unknown names; run `mise run test:index` and record incorrect/missing lookup after the target compiles.

**GREEN.** Implement exact postings and ordered lookup; rerun the focused check.

**REFACTOR.** Keep one posting owner and reuse its order instead of duplicating sorting downstream.

### Scenario 2 — Complete replacement and one-file retention

**Behavior.** Upsert replaces all old postings for one file as a complete visible state, other files stay intact, removal is idempotent, and successive one-file updates retain facts/size metadata without source bytes. Covers REQ-003, REQ-010, INV-003, AC-003.

**RED.** Add a focused index test for stale postings, repeated removal, and successive file facts; run `mise run test:index` and record the missing state behavior.

**GREEN.** Implement coherent per-file mutation and metadata retention; rerun Scenario 1.

**REFACTOR.** Keep state on the concrete index; do not add a generic transaction/storage layer.

### Scenario 3 — Ambiguity-preserving candidate matching

**Behavior.** Raw reference names, extractor-provided qualified candidates, and changed declaration names produce all exact plausible index hits; repeated or unknown names do not fabricate targets. Covers REQ-004, INV-002, INV-004, AC-003.

**RED.** Add a focused resolver test across the real index boundary with simple, qualified, ambiguous, and unknown names; run `mise run test:resolve` and record the missing candidate behavior.

**GREEN.** Match approved name sources through index lookup; rerun both owner lanes.

**REFACTOR.** Keep matching in resolve and postings/order in index; remove duplicate name-processing code only while results remain stable.

## Acceptance Criteria

- Exact case-sensitive lookup includes duplicate simple/qualified hits in deterministic order; unknown names are empty.
- Per-file replacement/removal leaves complete postings and unrelated files intact; only facts/byte lengths remain.
- Resolver preserves every plausible/ambiguous hit from all approved name sources and makes no exact-definition or completeness claim.

## Expected Write Set and Consumer Closure

Likely surfaces: `crates/bohmian-code-review-index/**`, `crates/bohmian-code-review-resolve/**`, their manifests/tests, and `test:index`/`test:resolve` in `mise.toml`. Task 03 uses these real owners through the facade to prove malformed/unsupported extraction cannot corrupt old or unrelated index state.

## Verification and Evidence

Run `mise run test:index`, `mise run test:resolve`, `mise run fmt:check`, `mise run lints`, and `git diff --check`; inspect crate dependencies and tracked/untracked diff. Record RED/GREEN per scenario and an acceptance-to-evidence map. Task 03 supplies the required higher-tier public journey.

## Material Stop Conditions

Return `SPEC_REVISION_REQUIRED` if correctness requires parser coupling in index, retained source, a completeness claim, fuzzy matching, or exact semantic resolution. Do not choose that behavior privately.

## Authority Links

`changes/active/review-helper-setup/spec.md`; `AGENTS.md`; `architecture/agent-rules.md`; `architecture/bohmian-code-review-design.md`; `architecture/references/domain/symbolic-context.md`; `architecture/references/languages/spec-driven-development.md`; `architecture/references/languages/implementation-execution.md`; `architecture/references/languages/testing-workflows.md`.

## Implementation Evidence

### RED / GREEN

| Scenario | RED (stub API compiled) | GREEN |
|---|---|---|
| 1 — Exact symbol hits | `mise run test:index`: `exact_lookup_returns_all_duplicates_in_canonical_order` FAILED (stub lookup empty) | ok after postings + ordered lookup |
| 2 — Replacement/retention | `upsert_replaces_postings_and_removal_is_idempotent`, `successive_upserts_retain_facts_and_byte_lengths` FAILED | ok; Scenario 1 re-run ok |
| 3 — Candidate matching | `mise run test:resolve`: `all_name_sources_resolve_to_every_plausible_hit` FAILED (stub resolve empty) | ok; `test:index` re-run ok |

### Acceptance map

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Exact case-sensitive simple/qualified lookup with duplicates in path/location/ID order; unknown empty | `crates/bohmian-code-review-index/src/lib.rs` (`SymbolIndex::lookup`, `Posting` order, `is_declaration`) | `index/src/lib.rs` `tests::exact_lookup_returns_all_duplicates_in_canonical_order` | PASS |
| Per-file replacement/removal leaves complete postings, unrelated files intact; only facts/byte lengths retained | `SymbolIndex::upsert`/`remove` (`&mut self`, remove-before-insert), `IndexedFile { byte_len, nodes }` | `index/src/lib.rs` `tests::upsert_replaces_postings_and_removal_is_idempotent`, `tests::successive_upserts_retain_facts_and_byte_lengths` | PASS |
| Resolver keeps every plausible/ambiguous hit from reference names, extractor candidates, and declaration names; no exact-definition/completeness claim | `crates/bohmian-code-review-resolve/src/lib.rs` (`reference_names`, `declaration_names`, `resolve`) | `resolve/src/lib.rs` `tests::all_name_sources_resolve_to_every_plausible_hit` | PASS |

### Verification

`mise run test:index` (3 passed), `mise run test:resolve` (1 passed), `mise run fmt:check`, `mise run lints`, `git diff --check` — all pass. `cargo tree -e normal`: index → core only; resolve → core, index. No `std::fs`/`net`/`process` or tree-sitter use in either crate. Added `test:index` and `test:resolve` lanes to `mise.toml`.

### Non-goals and limits

No source retention, parser dependency in index, persistence, fuzzy match, or proven-definition claim. File, import, export, and parameter nodes are not posted as declarations. Malformed/unsupported extraction isolation through the facade closes in TASK-003 per this task's scope. `IMPLEMENTATION_PLAN.md` carries a pre-existing maintainer edit and was left out.

### Remediation TASK-002-R1 (FIND-TASK-002-1, FIND-TASK-002-2)

The four owner tests, their helpers, and every assertion moved unchanged from `crates/bohmian-code-review-{index,resolve}/tests/*.rs` into `#[cfg(test)] mod tests` in each owning `src/lib.rs`; the external test files were deleted. The resolver test still calls the real `SymbolIndex`. The RED rows above are historical lane-level observations from before the move; no focused RED was replayed.

Current GREEN closure, one exact command per named test:

| Command | Observed |
|---|---|
| `mise exec -- cargo test --locked -p bohmian-code-review-index --lib tests::exact_lookup_returns_all_duplicates_in_canonical_order -- --exact` | `1 passed; 0 failed; … 2 filtered out` |
| `mise exec -- cargo test --locked -p bohmian-code-review-index --lib tests::upsert_replaces_postings_and_removal_is_idempotent -- --exact` | `1 passed; 0 failed; … 2 filtered out` |
| `mise exec -- cargo test --locked -p bohmian-code-review-index --lib tests::successive_upserts_retain_facts_and_byte_lengths -- --exact` | `1 passed; 0 failed; … 2 filtered out` |
| `mise exec -- cargo test --locked -p bohmian-code-review-resolve --lib tests::all_name_sources_resolve_to_every_plausible_hit -- --exact` | `1 passed; 0 failed; … 0 filtered out` |

Each index command selects exactly its one named test; `2 filtered out` is the other two tests in the same `--lib` target, so the remediation's literal `0 filtered out` is only attainable for the single-test resolver target. Broader checks rerun after the move: `mise run test:index` (3 passed), `mise run test:resolve` (1 passed), `mise run fmt:check`, `mise run lints`, `git diff --check` — all pass. No production code, fixture, grammar, or `IMPLEMENTATION_PLAN.md` change.
