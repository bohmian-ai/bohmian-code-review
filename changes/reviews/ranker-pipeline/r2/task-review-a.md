# Wave 1 task review A

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d..efc381f776e397ace4f43f9f4ab45fc4187436a5`. Authority: `changes/reviews/ranker-pipeline/intent.md` decisions 1–8, `architecture/references/domain/pipeline-consumer-ergonomics.md`, repository rules, and the R1 remediation task. There is no original task file by maintainer direction. I inspected the cumulative diff and candidate source. **Result: PASS.**

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| 1. Public IO-free `Ranker`, S/C/E/F semantics | `select/src/rank.rs` owns one trait and four rankers; facade re-exports them; eval calls `SymbolicContext::rank_with` | Ranker owner tests and public symbolic journey; `test:eval` passed | PASS |
| 2. F default, C order with empty history | `select/src/rank.rs:25-27,285-320`; PR facade uses `DEFAULT_RANKER` | Fusion and journey tests in `test:pipeline`; prior R1 rank-domain audit | PASS |
| 3. Feature-gated reusable eval over caller sample and named rankers, thin live binary, report compatibility | `eval/src/report.rs:167-292` validates caller arms and accumulates caller PRs; `trial.rs:297-318` runs each named ranker; live binary supplies S/C/E/F; `GitHubClient` owns live reads | `test:eval` passed 26 tests including `caller_sample_and_named_rankers_produce_named_results`, checked-in report reconstruction, and provider ownership tests | PASS |
| 4. Distinct persisted reference/candidate names, snapshot v4 | Index facts export/load retains names; `snapshot.rs:38` sets version 4 | Index and pipeline lanes plus prior R1 evidence | PASS |
| 5. Fractional score, context v2 | `SelectedSpan.score` is `f64`; `context.rs:29` sets version 2 | Selector and pipeline tests | PASS |
| 6. Co-change-only declarations pack in location order under budgets | `select/src/lib.rs:437-462` falls back to declaration spans when ranking reasons are empty | Selector owner tests and public journey | PASS |
| 7. Bounded complete-or-fail history, both passes, persisted and time eligible | `history.rs:353-436` fails an unproved page bound, rechecks all traversed pages before the last, joins prior history only after a proved boundary; snapshot and update share it; PR facade filters by base commit time and PR number | `test:pipeline` passed, including unproved bound, moved lower entry, cold/update no-publication cases | PASS |
| 8. One consumer `ReviewPipeline` and public stage facades | `review.rs:21-173` provides one storage/store, shared file ceiling, per-call client, context/refresh/read; stage facades remain exported | Pipeline journey and owner tests in `test:pipeline` | PASS |
| Constraints and non-goals: symbolic IO boundary, source-free facts/history, store clock, no background worker, no unrelated change | Provider reads are pipeline-owned; snapshot history holds number/time/paths; diff adds no worker or production provider SDK | Source and cumulative diff inspection; `test:eval` provider boundary test | PASS |

R1 closure: FIND-ranker-pipeline-1 is closed by caller-provided `Arms` and `Evaluation::add`; FIND-ranker-pipeline-2 by the shared history reader's bound failure and ordered page identity comparison; FIND-ranker-pipeline-3 by routing discovery, listing, and base commit time through `GitHubClient`. I traced their production callers and sibling sinks. No surviving material finding. `mise run test:eval` and `mise run test:pipeline` both exited 0 here. The authenticated live GitHub experiment was unavailable without a token; checked-in reports are reconstructed in a local test. The untracked `changes/active/impact-map/` is outside the subject.

```citations
[]
```
