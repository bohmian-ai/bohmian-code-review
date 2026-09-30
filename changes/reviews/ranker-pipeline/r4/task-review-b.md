# Task review B — cumulative ranker pipeline

**Subject:** base `322a2b9e0984062625d54274798a9c32cb01142d`, candidate `cd93bdadfa5cb17ed54752168e4a445c2e660068` (HEAD verified). **Authority:** `changes/reviews/ranker-pipeline/intent.md`, `architecture/references/domain/pipeline-consumer-ergonomics.md`, `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, and R1–R3 remediation tasks. There is no original task file by maintainer direction. I reviewed the cumulative diff and source, independently of the other R4 reports.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Decision 1: one public IO-free `Ranker`, S/C/E/F with experiment semantics | `select/src/rank.rs:33-313`; facade re-export in `bohmian-code-review/src/lib.rs` | `rank.rs` rarity, co-change, fusion tests; public symbolic journey | PASS |
| Decision 2: F default and empty-history Rarity ordering | `select/src/rank.rs:27-29,292-318`; facade `rank_files` | `rank::tests::fusion_sums_reciprocal_ranks_and_default_matches_rarity_without_history` | PASS |
| Decision 3 and R1/R2 FIND-1: feature-gated reusable named-arm eval, provider reads via pipeline, truthful decision identity, fixed reports | `eval/src/{report,trial,live}.rs`, `eval/Cargo.toml`; binary supplies fixed arms | `report::consumer_tests::caller_sample_and_named_rankers_produce_named_results`; `live::tests::run_report_names_the_callers_decision_pair` passed locally; checked-in report regression test exists | PASS |
| Decision 4 and R3 FIND-4: source-free, distinct persisted references; malformed records fail before replacement or snapshot read-back | `core/src/facts.rs:106-146`; `index/src/lib.rs:205-227`; `pipeline/src/snapshot.rs:320-327` | Focused core, public facade journey, and snapshot tests passed locally; index round-trip test inspected | PASS |
| Decision 5: fractional scores and context format 2 | `select/src/lib.rs` uses `f64`; `pipeline/src/context.rs:29` | Rank/packing tests and public journey; candidate source | PASS |
| Decision 6: co-change-only candidates pack indexed declarations under budget | `select/src/lib.rs` candidate fallback and packing; `select/src/rank.rs:240-287` | Select packing tests and symbolic public journey | PASS |
| Decision 7 and R1/R3 FIND-2: bounded complete-or-fail cold and incremental history, stable pages, source-free persistence, time/own-PR filtering | `pipeline/src/history.rs:354-449`; readers in `snapshot.rs` and `update.rs`; PR filter in `pull_request.rs` | Focused `history::tests::update_with_ten_new_prs_at_the_page_bound_fails` and `update::tests::update_history_is_complete_or_unpublished` passed locally; inspected cold, moved-listing, join and publication tests | PASS |
| R3 FIND-5: both listed-PR operations require matching base repo; forked head accepted; rejected history does not publish | `pipeline/src/github.rs:874-913,1580-1629,1681-1708` | Focused `github::tests::repository_lookup_and_pull_request_listing_are_verified` passed locally; cold-build rejection test inspected | PASS |
| Decision 8: one storage/store, shared file ceiling, per-call client, cold build only without pointer; stage facades public | `pipeline/src/review.rs:21-174`; exports in `pipeline/src/lib.rs` | Pipeline journey and shared-ceiling unit test in source | PASS |
| Constraints and non-goals: IO ownership, source-free state, store-clock durability, no new provider/ranker/config/worker, public journeys and untouched goldens | Cumulative manifest/source diff; `architecture/bohmian-code-review-design.md`; snapshot and context formats | Cumulative `git diff --check` passed; R3 evidence records `mise run gate` passed. No credentialed live run was available. | PASS |

## Closure and findings

**FIND-ranker-pipeline-2:** Closed. The shared reader now errors at an unproved ten-page bound (`history.rs:400-405`) regardless of the number of qualifying PRs. Cold reads stop only at the proved depth, and updates stop at the prior time boundary or exhaustion; rereads compare every earlier page's full listing (`:422-429`). The update publication test keeps the old pointer on a cap failure.

**FIND-ranker-pipeline-4:** Closed. `IndexedFileFacts::validate` rejects empty, duplicate, and unordered names before `SymbolIndex::load` mutates the index. Snapshot decode also validates facts before reporting complete coverage.

**FIND-ranker-pipeline-5:** Closed. Both PR listing operations call `listed_pull_request`, which requires the requested base repository id and leaves the head repository free to be a fork. The fake-provider tests cover absent/null/foreign ids and no publication on rejected closed history.

**Proposed findings:** None. I found no new missing, incorrect, drifted, violating, or regressing behavior that meets the task-review blocking gate. **Overall: PASS.**

**Verification limits:** I ran the exact focused tests named above and the public facade malformed-facts journey against candidate HEAD; all selected one test and passed. I inspected but did not rerun the broad `mise run gate` or credentialed live report. The R3 task records a passing gate; no token was supplied for a live run. `changes/active/impact-map/` was untouched.

```citations
[]
```
