# TASK-002 independent task review B

Subject: base `3fecce95f055268359d7d1d87df6308ea504329e`, candidate `4196bbd36a150e26668bed32b2d80c90d07152a0`; approved `SPEC-cochange-eval` revision 2, original TASK-002, and TASK-002-R1. I inspected the cumulative diff and its callers in the example, the governing architecture and task rules, and the R1 correction. The candidate commit remained the working HEAD. `git diff --check <base> <candidate>` exited 0. Shared Cargo verification is being run by the orchestrator; I did not run a competing broad lane.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| At most five deterministic trials per PR; all eligible files when five or fewer | `trial.rs:28-31,463-484,541-555` chooses by fixed PR/path FNV hash, then evaluates in listing order | `trial_selection_is_bounded_and_deterministic` covers five, seven, repeatability, and full `evaluate` | PASS |
| Same base-snapshot candidate universe and remaining changed-path exclusion in C, E, F | `Corpus::new` builds from every exported base fact (`trial.rs:120-174`); `excluded` removes remaining head and previous paths (`453-461`); all three ranks consume that corpus (`486-510`) | `withholding_keeps_base_file_rankable_and_excludes_remaining_paths`; `cochange_and_fused_ranks_use_only_eligible_history` | PASS |
| C retains directional rarity weighting, document-frequency ceiling 100, distinct-name maximum, path ties, and zero-score omission | `trial.rs:23-27,340-407`; compared cumulative diff with prior C scoring; removed span and packing data without changing C credit | `c_ranks_by_direction_and_rarity` asserts exact Python scores and Rust tie | PASS |
| E uses eligible history PR counts, sums `n(d,f)/n(d)` over eligible remaining base files, and excludes absent-base history paths | `main.rs:384-395` supplies `History::for_trial`; `trial.rs:409-440` filters anchor and candidate through `corpus.declarations`, then uses touching PR count as denominator | `cochange_and_fused_ranks_use_only_eligible_history`; R1 `absent_base_changed_path_anchors_no_cochange` checks both missing and present anchors | PASS |
| F uses `1/(60+rank_C)+1/(60+rank_E)` with an unranked side worth zero; all arms tie by canonical path | `trial.rs:32-35,340-353,442-451`; each arm passes through `ranked` | `cochange_and_fused_ranks_use_only_eligible_history` checks C-only, E-only, combined, ties, and unranked | PASS |
| Time isolation: no future or trial PR history in trial ranks | `main.rs:384-395` calls `History::for_trial`; `history.rs:147-153` filters by merge time and PR number | History producer tests plus `cochange_and_fused_ranks_use_only_eligible_history` compare eligible and deliberately leaked future history | PASS |
| Trial output contains withheld path and only C/E/F rank or `unranked` and hit@10; no packing or model-exposure outcome | `trial.rs:59-118`; `main.rs:523-579`; cumulative diff removes A/B/D and packing fields | `trial_record_is_ranking_only` asserts exact JSON and rank 10/11 boundaries; `summaries_follow_each_arms_hits` checks consumer | PASS |
| No production ranking, packing, persisted format, public API, grammar, or gate changes; source remains transient | Changed code is confined to the existing `context_relevance` example and change packet; `Corpus` stores facts and reference postings but no source bytes | Cumulative `git diff --name-only` and `git diff --check` | PASS |
| R1 FIND-TASK-002-1: an added remaining path absent from the trial base cannot anchor E or influence F, while an indexed anchor still contributes | `trial.rs:420-423` filters remaining paths by base declarations before history counts; `trial.rs:430-436` retains indexed candidates and normal numerator/denominator | `absent_base_changed_path_anchors_no_cochange` compares E 2/3 and 1/3 and paired E/F ranks | PASS |

I found no missing task behavior, incorrect scoring, scope drift, explicit constraint violation, or introduced regression in the cumulative candidate. The R1 root-cause class is closed at `rank_e`; `evaluate` still passes the complete remaining-change set to C, exclusion, and trial selection. The checked-in task evidence reports green owner, public journey, formatting, lint, and gate lanes; the orchestrator's current independent lane results remain a verification limit until attached to the final verdict.

Proposed findings: none.

Overall result: **PASS**.

```citations
[]
```
