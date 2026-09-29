# TASK-002 independent task review A

Subject: base `3fecce95f055268359d7d1d87df6308ea504329e`, candidate `1b6d31eeeb7c6b8edd1989abb19f6f2cf161d4c8`; approved `changes/active/cochange-eval/spec.md` revision 2 and original `tasks/TASK-002-ranking.md`. I inspected the complete four-file diff and the resulting example, its history producer, and relevant authorities. The appended implementation evidence in the task was excluded from the judgment.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Up to five deterministic trials per PR; all qualifying paths when five or fewer; withheld base file rankable and other changed paths excluded (INV-002, AC-003, scenario 1) | `trial.rs:450-479,483-551`: stable PR/path draw, bounded choice, remaining-path exclusions, one paired trial per choice | `trial_selection_is_bounded_and_deterministic` and `withholding_keeps_base_file_rankable_and_excludes_remaining_paths`; focused trial tests passed | PASS |
| C preserves directional rarity weighting, document-frequency ceiling 100, full indexed base universe, score ties, and zero-score unranked (scenario 2) | `trial.rs:120-193,340-407,483-505`: corpus covers exported base facts and base references; distinct-name maximum credit; descending score then path | `c_ranks_by_direction_and_rarity` and withholding test passed; source comparison with base C rule | PASS |
| E uses only time-eligible history; for each remaining changed path sums `n(d,f)/n(d)` for indexed, non-excluded candidates; absent base paths contribute nothing (INV-003, AC-003) | `main.rs:384-398` supplies `History::for_trial`; `history.rs:147-153` filters by base time and PR; `trial.rs:409-436` computes E over corpus paths | `cochange_and_fused_ranks_use_only_eligible_history` passed, including multiple changed paths, absent base path, and future PR | PASS |
| F uses `1/(60+rank_C) + 1/(60+rank_E)`, zero for unranked sides, with descending score and path ties (AC-003) | `trial.rs:340-353,438-447` fuses only ranked C/E candidates | Co-change/fusion test passed, including C-only, E-only, scores, and tie | PASS |
| Trial record exposes the withheld path, each arm's one-based rank or `unranked`, and hit@10 only; all arms use the same trial and snapshot (INV-001, INV-004, scenario 3) | `trial.rs:59-105,483-551`; `main.rs:568-579` serializes trials and rank-only metadata | `trial_record_is_ranking_only` passed with rank 10, rank 11, unranked, and exact JSON | PASS |
| No production ranking, packing, persisted format, public API, grammar, other arms, or model-exposure outcome changes (INV-001, AC-004, non-goals) | Diff changes only `examples/context_relevance/{main,report,trial}.rs` and task evidence; old experimental packing and A/B/D code are removed | `git diff --name-status 3fecce9 1b6d31e`; `mise run test:pipeline` passed | PASS |
| Source remains transient; result is source-free and command read-only (INV-004) | `trial.rs:120-173,83-99` keeps only extracted facts and ranking outcomes; `main.rs:506-520,582-600` accumulates JSON then prints after success | Source trace through example; no source-bearing result field or write path in changed code | PASS |
| Report means, paired effects, bootstrap interval, decision rule, and checked-in live reports | Original task assigns trial results to the report owner; TASK-002 does not claim these later report outcomes. | Not a TASK-002 completion criterion; verify with report task | PASS |

## Proposed findings

None. No missing, incorrect, drift, violation, or regression finding survived the task-scoped audit. The changed C implementation retains the old rule while removing packing-only machinery. The E/F code adds only the experiment's fixed scoring and the selector is bounded without adding a dependency.

## Verification limits

I ran `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::tests:: -- --nocapture` (6 passed) and `mise run test:pipeline` (exit 0). I did not perform a live credentialed run or a second live rerun; the approved task requires no-network proof here, while live report production belongs to the report task. I did not rerun the full gate independently.

Overall result: **PASS**.

```citations
[]
```
