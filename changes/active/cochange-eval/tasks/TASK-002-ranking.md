---
id: TASK-002
kind: implementation
status: proposed
spec: SPEC-cochange-eval
spec_revision: 2
requirements: [INV-001, INV-002, INV-003, INV-004, AC-003, AC-004]
depends_on: [TASK-001]
---

## Outcome and Value

Every sampled PR supplies up to five deterministic leave-one-out trials. C, E, and F rank the same base-snapshot candidate universe, and each trial reports only the withheld file's rank or `unranked` and hit@10. The experiment measures ranking without packing.

## Owners, Scope, Consumers, and Prohibited Changes

The existing pipeline example's trial owner is likely owner; the report owner consumes trial results. Reuse the existing C directional rarity scoring and real public symbolic facade for base facts and changed-line input. Do not move experimental ranking into the production selector, change public APIs or grammar pins, report packing/model exposure, retain source in results, or add other arms or outcomes.

## Approach

1. Keep C's existing score rule, document-frequency ceiling of 100, full base candidate universe, and changed-path exclusions.
2. Select at most five eligible withheld paths per PR by a reproducible rule keyed by PR number and path; use all when five or fewer qualify.
3. Score E from the time-eligible history supplied by TASK-001, excluding paths missing from that trial's base snapshot.
4. Fuse C and E ranks using the fixed reciprocal-rank constant of 60; order all arms by descending score then canonical path, with zero-score files unranked.
5. Emit only the rank and hit@10 data needed by the report while keeping fetched source transient.

## Ordered Implementation Scenarios

### Scenario 1 — Deterministic withheld trials

**Behavior.** A PR with at most five withholdable modified files uses all of them; a larger PR uses exactly five selected reproducibly from its number and paths. Each remaining changed path is excluded and the withheld base file stays rankable. (INV-002, AC-003)

**RED.** Add a focused in-memory trial check in the existing example target for both sides of the five-file boundary, repeated selection, and changed-path exclusion. Run `mise run test:pipeline`; expect the new selection check to fail.

**GREEN.** Apply bounded selection and rerun the new and existing trial checks.

**REFACTOR.** Reuse the current withholding and exclusion flow, removing only obsolete variant or packing work while checks stay green.

### Scenario 2 — Co-change and fused rank

**Behavior.** For every remaining changed file with nonzero history participation, E sums co-change proportions over candidate files present in the trial's base snapshot. F adds `1/(60 + rank_C)` and `1/(60 + rank_E)` for ranked sides. Each arm uses descending score, canonical-path ties, and treats zero-score files as unranked; history cannot leak future PRs. (INV-002, INV-003, AC-003)

**RED.** Add focused in-memory trial checks covering nonzero denominators, multiple changed files, absent base paths, C-only and E-only candidates, reciprocal-rank fusion, ties, zero scores, and a future history PR that must not affect ranks. Run `mise run test:pipeline`; expect the E/F checks to fail.

**GREEN.** Add the minimum evaluation-only scoring and ranks; rerun both scenario checks and existing C regression checks.

**REFACTOR.** Share only established ranking order and candidate facts where it shortens the evaluator without altering C.

### Scenario 3 — Ranking-only trial record

**Behavior.** Each arm reports the withheld path's one-based rank or `unranked` and hit@10. All three arms use the same trial and base snapshot, and no packing or model-exposure result is emitted. (INV-001, INV-004, AC-003, AC-004)

**RED.** Add a focused serialization and paired-trial check in the existing example target, including a rank-10 hit, rank-11 miss, and unranked miss. Run `mise run test:pipeline`; expect the new record assertion to fail.

**GREEN.** Narrow the trial record to the specified outcomes and rerun all earlier scenario checks.

**REFACTOR.** Remove now-unused experimental packing and legacy arm code while preserving the historical checked-in reports.

## Acceptance Criteria

- At most five trials per PR follow the deterministic rule and all qualifying paths are used when there are five or fewer.
- C matches its approved rarity-weighted definition; E and F implement the specified formulas, path ties, and unranked rule over the same candidates.
- Trial records contain withheld paths and only ranking/hit@10 outcomes for C, E, and F; no production ranking or packing behavior changes.
- No-network tests prove time isolation and a complete base-snapshot candidate universe.

## Expected Write Set and Consumer Closure

Likely `examples/context_relevance/trial.rs` and its inline tests, with small command integration where history is passed into trials. The report owner must consume the final paired records. Paths guide ownership, not implementation shape.

## Verification and Evidence

Run `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Record focused selection/scoring evidence and C regression evidence. Run `mise run gate` after integration.

## Material Stop Conditions

Stop for spec authority if the approved candidate universe, C definition, history timing, or ranking-only result cannot be preserved without changing production contracts or the approved formulas.

## Authority Links

- [Approved revision 2](../spec.md)
- [Contributor rules](../../../../AGENTS.md)
- [Design](../../../../architecture/bohmian-code-review-design.md)
- [Symbolic context](../../../../architecture/references/domain/symbolic-context.md)
- [Testing workflow](../../../../architecture/references/languages/testing-workflows.md)

## Implementation Evidence

Candidate: `24fe346` (implementation) on `change/cochange-eval`. Write set: `crates/bohmian-code-review-pipeline/examples/context_relevance/{trial.rs,main.rs,report.rs}`.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| At most five trials per PR, deterministic rule, all used when five or fewer | `trial.rs` `MAX_TRIALS`, `TRIAL_RULE`, `draw` (FNV-1a 64 of `"<pr>/<path>"`), `chosen`, `evaluate` | `trial_selection_is_bounded_and_deterministic` (5 → all, 7 → 5 by draw, repeated runs equal, listing order kept) | PASS |
| C matches its approved rarity definition; E and F formulas, path ties, unranked rule over the same candidates | `rank_c` (unchanged rule, `DF_CEILING` 100), `rank_e`, `rank_f` (`FUSION_CONSTANT` 60), shared `ranked`, `excluded` | `c_ranks_by_direction_and_rarity` (exact C scores and ranks carried over from the prior B–D fixture), `withholding_keeps_base_file_rankable_and_excludes_remaining_paths`, `cochange_and_fused_ranks_use_only_eligible_history` | PASS |
| Trial records hold withheld paths and only rank/hit@10 for C, E, F; no production ranking or packing change | `Trial`, `Outcome::of`, `HIT_RANK`; A/B/D, packing, token, and scoping code removed; `main.rs` passes `History::for_trial` paths; `report.rs` consumes `Trial::arms` | `trial_record_is_ranking_only` (rank 10 hit, 11 miss, unranked miss; exact JSON), `summaries_follow_each_arms_hits`; `git diff --stat` touches only the example | PASS |
| No-network tests prove time isolation and a complete base-snapshot candidate universe | `evaluate` takes base index + all indexed blobs; E counts only indexed, non-excluded paths | `cochange_and_fused_ranks_use_only_eligible_history` builds `History` and shows a PR merged after the base changes results only when leaked; absent `pkg/y.py` scores nothing; `run_reads_each_input_once_and_reports_costs` whole-run fake | PASS |

Commands (all exit 0 in this session): `mise run test:pipeline` (example target 23 passed), `mise run test:symbolic-context:journey` (6 passed), `mise run fmt:check`, `mise run lints`, `git diff --check HEAD~1 HEAD`, `mise run gate`.

Non-goals stayed excluded: no production crate, public API, grammar pin, or `mise.toml` change; no packing or model-exposure output; no other arms. `report.rs` keeps only a per-arm hit@10 share; PR-level means, bootstrap, and decision stay with TASK-003.

Limit: tests and implementation were written together, so per-scenario RED failures were not separately observed.
