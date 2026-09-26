---
id: TASK-001
kind: implementation
status: review
spec: SPEC-repo-parser@10
depends_on: []
maps: [REQ-001, REQ-002, REQ-003, INV-001, INV-002, INV-004, AC-001]
---

## Outcome and Value

The existing public library can export and reload source-free symbol facts, identify extraction compatibility, rank a multi-file PR, and pack candidate-only spans. The existing single-file journey still works.

## Owners, Scope, Consumers, and Prohibited Changes

The six current crates retain their owners and IO-free boundaries; the public `bohmian-code-review` facade is the consumer contract for the outer pipeline. Update `AGENTS.md`, `architecture/agent-rules.md`, and `architecture/bohmian-code-review-design.md` to distinguish those six crates from the newly approved outer IO-capable library before implementation. Do not add GitHub, storage, database, or model code to the six crates; do not change grammar pins or goldens to obtain a pass.

## Approach

1. Record the approved outer-crate boundary in the repository authorities.
2. Add the minimum persistable facts, extraction identity, and path eligibility behavior.
3. Extend ranking and packing through the existing owners, then exercise both public journeys.

## Ordered Implementation Scenarios

### Scenario 1 — Source-free facts round trip

**Behavior.** Exported facts contain no source fragments; loading them in a fresh index reproduces lookup and ranking, while invalid facts fail explicitly.

**RED.** Extend the public facade journey with export/load equivalence and invalid-input assertions; `mise run test:symbolic-context:journey` fails before the capability exists.

**GREEN.** Add the required projection and load path through core/index/facade; rerun the same journey.

**REFACTOR.** Reuse existing path validation and postings replacement while preserving the journey result.

### Scenario 2 — Multi-file candidate context

**Behavior.** Changed-line names from multiple files contribute once per distinct name, every changed path is excluded, ties are deterministic, and candidate-only packing obeys file/token limits without changed hunks.

**RED.** Extend `crates/bohmian-code-review/tests/symbolic_context_journey.rs` with a two-file case and a budget failure; `mise run test:symbolic-context:journey` fails on missing behavior.

**GREEN.** Add path eligibility/version exposure, multi-file ranking, and candidate-only packing through the existing owners; keep the single-file flow green.

**REFACTOR.** Share name collection and scoring with the single-file path instead of duplicating selector logic.

## Acceptance Criteria

REQ-001–REQ-003 and the public journey pass; loaded facts reproduce results, unsupported paths are identified before download, extraction changes invalidate old facts, and existing APIs retain their behavior.

## Expected Write Set and Consumer Closure

Current six crate owners, their nearest tests, the public journey, repository architecture documents, workspace manifest and lockfile only as needed. The outer pipeline consumes only the public facade.

## Verification and Evidence

Run `mise run test:extract`, `mise run test:index`, `mise run test:select`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `mise run gate`; record RED/GREEN and the public journey result.

## Material Stop Conditions

Stop if source-free facts cannot reproduce required selection without persisting source text, or if a grammar behavior change requires altering pinned fixtures.

## Authority Links

[Revision 10 spec](../spec.md), [library design](../../../../architecture/bohmian-code-review-design.md), [agent rules](../../../../architecture/agent-rules.md).

## Implementation Evidence

Commits `a59e51d..HEAD` on `review-helper-setup`. RED: the extended journey failed to compile (missing `export`, `load`, `path_language`, `rank_files`, `build_candidate_context`, `EXTRACTION_VERSION`, `IndexedFileFacts`, `Error::InvalidFacts`, `Error::ExtractionVersionMismatch`). GREEN: every lane below exited 0.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Authorities distinguish six IO-free crates from the outer pipeline crate | `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md` | `mise run gate` (`check:skills-sync`, docs unchanged elsewhere) | PASS |
| REQ-001 source-free facts export/load, invalid facts fail without corrupting others | `core::facts` (`DeclarationFact`, `IndexedFileFacts`, `InvalidFact`), `SymbolIndex::export`/`load`, `SymbolicContext::export`/`load` | `exported_facts_reload_without_source_and_reject_invalid_records` (journey), `export_round_trips_through_load_and_rejects_invalid_records` (index), core `facts::tests` | PASS |
| REQ-002 extraction version invalidates old facts; path-only eligibility | `extract::EXTRACTION_VERSION`, `Extractor::path_language`, `SymbolicContext::path_language`, `Error::ExtractionVersionMismatch` | `path_language_identifies_eligible_paths_before_download` (journey), `path_language_reports_eligibility_without_source` (extract), version mismatch assertion in the facts journey | PASS |
| REQ-003 multi-file ranking, all changed paths excluded, distinct-name scoring, deterministic ties, candidate-only packing with omissions | `select::ChangedFile`, `rank_files`, `pack_candidates` (shared by `rank`/`pack`), `SymbolicContext::rank_files`, `build_candidate_context` | `multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only` (journey), `multi_file_ranking_unions_names_and_excludes_every_changed_path`, `candidate_packing_counts_only_selected_candidates` (select) | PASS |
| AC-001 existing single-file journey and APIs unchanged | `rank`/`pack` delegate to the shared functions; no public signature changed | Pre-existing journey and owner tests pass unchanged | PASS |

Commands run in this session, each exit 0: `mise run test:extract`, `mise run test:index`, `mise run test:select`, `mise run test:resolve`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`.

Focused commands (current GREEN closure, TASK-001-R2; each run sequentially in this session):

| Command | Observed |
|---|---|
| `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey exported_facts_reload_without_source_and_reject_invalid_records -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey path_language_identifies_eligible_paths_before_download -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-index --lib tests::export_round_trips_through_load_and_rejects_invalid_records -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-extract --lib extractor::tests::path_language_reports_eligibility_without_source -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-select --lib tests::multi_file_ranking_unions_names_and_excludes_every_changed_path -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-select --lib tests::candidate_packing_counts_only_selected_candidates -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-core --lib facts::tests::projection_is_source_free_and_rebuilds_a_node -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-core --lib facts::tests::validation_rejects_empty_identity_and_bad_spans -- --exact` | 1 passed, exit 0 |

Non-goals stayed excluded: no GitHub, storage, database, or model code entered the six crates; no grammar pin, golden, or fixture changed. `Cargo.lock` changed only to add the facade's `serde_json` dev-dependency. Limit: `EXTRACTION_VERSION` is a maintained constant; the grammar-parity fixtures are the evidence a bump is due, not an automatic check.
