# TASK-002 independent task review A, round 2

Subject: base `3fecce95f055268359d7d1d87df6308ea504329e`, cumulative candidate `4196bbd36a150e26668bed32b2d80c90d07152a0`, approved `SPEC-cochange-eval` revision 2, original `TASK-002-ranking.md`, and `TASK-002-R1-base-history-anchor.md`. I read the complete base-to-candidate changed surface: the task and R1 packet, and the pipeline example's `main.rs`, `report.rs`, and `trial.rs`. Result: **PASS**.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Up to five deterministic withheld modified base files per PR; all qualifying paths when at most five; other changes remain excluded | `trial.rs:473-555` uses the PR/path draw, selects modified indexed files, and leaves remaining changes in the exclusion set | `trial_selection_is_bounded_and_deterministic`, `withholding_keeps_base_file_rankable_and_excludes_remaining_paths` | PASS |
| C preserves directional rarity scoring, document-frequency ceiling 100, complete base candidate universe, path ties, zero-score unranked | `trial.rs:120-193`, `340-407` build corpus from snapshot facts and all indexed base blobs, score distinct names, and sort scores then paths | `c_ranks_by_direction_and_rarity` checks exact scores and ranks; `trial_record_is_ranking_only` checks unranked | PASS |
| E uses eligible earlier history, sums `n(d,f)/n(d)` over indexed remaining anchors and indexed non-excluded candidates, with absent-base paths contributing nothing | `main.rs:342-397` obtains `History::for_trial`; `trial.rs:409-440` filters both anchor and candidate paths by base declarations; `history.rs:147-153` excludes trial PR and future merges | `cochange_and_fused_ranks_use_only_eligible_history`; R1 `absent_base_changed_path_anchors_no_cochange` distinguishes absent and indexed anchors, including resulting trial ranks | PASS |
| F fuses one-based C and E ranks with fixed constant 60; ties by path; unranked sides contribute zero | `trial.rs:340-353`, `442-451` builds F from ranked C/E only | `cochange_and_fused_ranks_use_only_eligible_history` verifies exact fused scores and order; R1 regression verifies no inherited E credit | PASS |
| Every arm uses the same base snapshot and trial, and records only withheld path, one-based rank or `unranked`, and hit@10 | `trial.rs:59-105`, `486-555`; `main.rs:568-586` serializes paired trials; legacy packing outcomes removed from example | `trial_record_is_ranking_only` checks rank 10, 11, unranked, and exact JSON; `report.rs` checks arm hit summaries | PASS |
| No production/API/grammar/gate change, source retention, extra arms, packing/model-exposure outcome, or provider write | Complete diff changes only the example and review/task Markdown; `Trial` contains path and outcomes, while base/head blobs remain local to `Api::trials` | Diff inspection; task evidence records pipeline, symbolic journey, format, lint, and gate passes | PASS |

**R1 closure:** The prior `FIND-TASK-002-1` class arose because an absent-base remaining path could anchor earlier co-change history. The filter in `rank_e` at `trial.rs:420-423` now rejects that anchor before counting `touching` history. The same corpus membership is used for destination eligibility and withholding. The focused R1 test at `trial.rs:1052-1101` exercises absent-base and indexed anchors through `rank_e` and `evaluate`, covering E and F. I found no sibling history-anchor consumer outside `rank_e`.

**Verification limit:** I did not rerun broad Cargo lanes during this parallel review; the orchestrator owns shared verification to avoid build-lock contention. Credentialed live runs, report production, PR-level effects, and bootstrap decisions belong to later tasks. No proposed findings.

```citations
[]
```
