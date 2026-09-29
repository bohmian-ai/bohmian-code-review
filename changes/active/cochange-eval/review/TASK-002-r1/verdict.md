# TASK-002 review verdict, round 1

**Verdict: FIX_REQUIRED.** The immutable subject is base `3fecce95f055268359d7d1d87df6308ea504329e` through candidate `1b6d31eeeb7c6b8edd1989abb19f6f2cf161d4c8`, under approved `changes/active/cochange-eval/spec.md` revision 2 and `changes/active/cochange-eval/tasks/TASK-002-ranking.md`. The candidate remained at `HEAD` throughout review. The complete diff changes the task evidence and the pipeline example's `main.rs`, `report.rs`, and `trial.rs` only.

## Acceptance matrix

| TASK-002 obligation | Result | Evidence |
|---|---|---|
| At most five deterministic withheld trials, all when five or fewer; changed-path exclusion | PASS | `trial.rs` selection and withholding tests; pipeline lane passed. |
| C directional rarity, ceiling 100, complete base candidate universe, ties and unranked rule | PASS | `rank_c` and C regression tests. |
| E co-change formula and absent-base history paths contribute nothing | FAIL | `rank_e` accepts an added remaining path absent from the base as a history anchor; FIND-TASK-002-1. |
| F reciprocal-rank fusion, constant 60, ties and unranked rule | FAIL on affected trials | Formula is correct, but F consumes E ranks changed by FIND-TASK-002-1. |
| Time-eligible history and no future/trial-PR leakage | PASS | `History::for_trial` trace and history tests. |
| Ranking-only C/E/F trial records, one-based rank and hit@10 | PASS | Exact JSON and rank-boundary tests. |
| No production/API/grammar/gate change, source retention, packing outcome, or extra arms | PASS | Four-file diff; standards review; `git diff --check`. |

## Review and validation

Wave 1: task sample A **PASS**; task sample B **FAIL** (`TASK-B-1`); repository standards **PASS**; ranking domain **PASS**; history domain **FAIL** (`HIST-1`). The citation checker accepted all six citations and rejected none; see `citations.txt`. Wave 2 independently reproduced the source path and merged both claims into **FIND-TASK-002-1**, status **CONFIRMED**, classification **INCORRECT**, task-contract gate. There are no notes, rejected findings, or prior finding IDs to close. The decision-complete correction is in `TASK-002-R1-base-history-anchor.md`; the independent ledger is `findings-validation.md`.

The base snapshot can omit an added remaining changed path, yet `rank_e` uses that path to select earlier co-change PRs. Its base-membership check applies only to credited destination paths. This violates the approved History rule and can change E and F ranks and hit@10. The one scoring owner serves every trial; no independent sibling owner exists.

## Verification limits

I observed `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `git diff --check` pass. The reviewers ran focused example checks. The passing tests do not exercise an absent-base **anchor** with matching history. A credentialed live run, repeat run, and checked-in repository reports belong to later report work and were not reproduced here. The task evidence records a passing gate but no observed per-scenario RED phase; I did not independently rerun the gate. These limits do not prevent the reproduced task-contract finding.
