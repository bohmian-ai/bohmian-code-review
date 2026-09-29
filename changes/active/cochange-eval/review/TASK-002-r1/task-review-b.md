# TASK-002 independent implementation review B

Subject: base `3fecce95f055268359d7d1d87df6308ea504329e`; candidate `1b6d31eeeb7c6b8edd1989abb19f6f2cf161d4c8`. Authority: approved `changes/active/cochange-eval/spec.md` revision 2, original `tasks/TASK-002-ranking.md`, repository rules, and the complete base-to-candidate diff. No implementation summary or other reviewer report informed this review.

## Acceptance matrix

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Up to five deterministic withheld trials; all when five or fewer; changed paths excluded and withheld base file remains rankable | `trial.rs` `draw`, `chosen`, `excluded`, `trial`, `evaluate` | `trial_selection_is_bounded_and_deterministic`; `withholding_keeps_base_file_rankable_and_excludes_remaining_paths` | PASS |
| C retains directional rarity weighting, document-frequency ceiling 100, candidate universe, and canonical-path ties | `trial.rs` `Corpus`, `rank_c`, `ranked` | `c_ranks_by_direction_and_rarity`; `withholding_keeps_base_file_rankable_and_excludes_remaining_paths` | PASS |
| E sums `n(d,f)/n(d)` over time-eligible history; paths absent from the trial base contribute nothing | `main.rs` `Api::trials` passes `History::for_trial`; `trial.rs` `rank_e` filters candidate paths but accepts an absent-base changed path as an anchor | Existing co-change test covers an added path with no history; source trace below exposes the missing case | FAIL |
| F uses `1/(60+rank_C) + 1/(60+rank_E)`, unranked sides contribute zero; all arms discard zero scores and sort path ties | `trial.rs` `rank_f`, `ranked` | `cochange_and_fused_ranks_use_only_eligible_history` | PASS |
| One-based withheld rank or `unranked` and hit@10 only in each paired C/E/F trial; no packing or model-exposure result | `trial.rs` `Outcome`, `Trial`, `Outcome::of`; old packing fields removed | `trial_record_is_ranking_only`; exact JSON assertion | PASS |
| History before base commit, trial PR excluded, no per-trial provider fetch | `history.rs` `for_trial`; `main.rs` `Api::trials` and `run` | `history_is_cold_depth_plus_time_eligible_increments`; `history_excludes_trial_pr`; `run_reads_each_input_once_and_reports_costs` | PASS |
| No production ranking, packing, extraction, snapshot, public API, grammar, or gate change; source transient and no extra experimental arms or outcomes | Diff changes only task Markdown and the pipeline example's `main.rs`, `report.rs`, `trial.rs`; trial result holds paths/ranks/bools | `git diff --name-only` and `git diff --check` for the immutable range; example test target passed | PASS |

The changed `report.rs` emits a temporary all-trial hit@10 aggregate. TASK-003 explicitly owns replacing all-trial aggregates with equal-PR effects and complete reports, so this is deferred to that dependent task rather than treated as a TASK-002 finding.

## Proposed finding

### TASK-B-1 — INCORRECT: absent-base changed paths participate as E anchors

- **Obligation:** Spec History: a history path absent from the trial's base snapshot contributes nothing; TASK-002 Scenario 2 and acceptance require E over the base-snapshot candidate universe with absent-base paths excluded.
- **Root cause and defect class:** `rank_e` selects history PRs touching every remaining changed head path without checking that the anchor path exists in the base corpus. The separate membership check applies only to credited candidate paths. `rank_e`'s raw `Change` input permits an added or renamed head path absent from base to act as an anchor.
- **Location and siblings:** `crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs:423` is the incorrect anchor decision; line 427 is the candidate-only check. There is one E ranking owner and no sibling implementation site; `trial` calls it for every withheld file, and `evaluate` includes added files among remaining changes.
- **Reachable consequence:** A sampled PR can add `pkg/u.py` while changing a withholdable base file. If an earlier merged history PR touched `pkg/u.py` and `pkg/x.py`, but `pkg/u.py` is absent from this trial's base snapshot, `rank_e` still credits `pkg/x.py` by 1. E and F can then rank the withheld file differently from the specified history rule. The existing test includes added `pkg/u.py` but gives it no history, so it cannot detect this case.
- **Testable correction:** At the `rank_e` owner, let a remaining changed path contribute only if it is in the trial base candidate universe before counting its history participation. A focused in-memory trial should pair an added, absent-base changed path with a historical co-change to an indexed candidate and prove that E and F receive no credit from that path; preserve the present tests for indexed changed paths and absent candidate paths.

## Verification and result

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features` passed 23 tests. `git diff --check 3fecce95f055268359d7d1d87df6308ea504329e 1b6d31eeeb7c6b8edd1989abb19f6f2cf161d4c8` passed. I did not run the credentialed live experiment; checked-in reports and PR-weighted inference belong to TASK-003. Per-scenario RED evidence was unavailable; green checks alone do not close TASK-B-1.

Overall result: **FAIL**.

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":423,"snippet":"            .filter(|paths| paths.contains(&change.file.path))","symbols":["rank_e"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":427,"snippet":"            if corpus.declarations.contains_key(path) && !excluded.contains(path) {","symbols":["rank_e"]}]
```
