# Ranking domain review

**Subject:** base `3fecce95f055268359d7d1d87df6308ea504329e`, candidate `4196bbd36a150e26668bed32b2d80c90d07152a0`. Reviewed the cumulative diff, approved revision 2 spec, TASK-002, and TASK-002-R1 independently. Boundary: deterministic withholding, C/E/F scoring and ordering, base candidate eligibility, time-isolated history supplied to trials, and paired ranking-only records.

| Boundary | Authority and source coverage | Result |
|---|---|---|
| Withholding and candidate universe | Spec lines 40–50; TASK-002 scenarios 1 and 3; `trial.rs` `Corpus::new`, `excluded`, `draw`, `chosen`, `trial`, `evaluate`; `main.rs` `Api::trials`; selection and exclusion tests | PASS. FNV draw is keyed by PR and path; at most five eligible modified base paths are selected; each trial excludes its other changed paths. |
| C directional rarity | Spec lines 19–24; TASK-002 scenario 2; `trial.rs` `Change::new`, `rank_c`, `ranked`; base implementation and `c_ranks_by_direction_and_rarity` | PASS. Distinct-name max credit, df ceiling 100, descending score/path ties, and zero-score omission remain consistent with C. |
| E and R1 base membership | Spec lines 21, 52–59; TASK-002-R1; `history.rs` `History::for_trial`; `main.rs` `Api::trials`; `trial.rs` `rank_e`; `cochange_and_fused_ranks_use_only_eligible_history` and `absent_base_changed_path_anchors_no_cochange` | PASS. `rank_e` now checks base membership before counting an anchor; it also checks candidate membership. An added path with matching history cannot lend E or F credit. Time filtering happens before `evaluate`. |
| F, ties, and paired record | Spec lines 22–28, 70–75; TASK-002 scenarios 2–3; `trial.rs` `rank_f`, `ranked`, `Outcome::of`, `Trial`, `trial`; `trial_record_is_ranking_only` | PASS. F sums reciprocal one-based ranks with constant 60. All arms share the same withheld file and remaining changes; only rank or `unranked` and hit@10 are serialized. |

**Verification limits:** This review traced source and inline tests and observed `git diff --check` pass for the immutable range. The orchestrator is running shared Cargo verification; I did not duplicate it. The `rank_e` regression fixture checks both an absent-base anchor with matching history and an indexed anchor. I found no material domain finding.

**Overall result: PASS.**

```citations
[]
```
