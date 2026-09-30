# Wave 2 findings validation

Subject: `322a2b9e0984062625d54274798a9c32cb01142d..9efafd309207c6cf5dd96b306ce8d22325281e5d`. Authority: `changes/reviews/ranker-pipeline/intent.md`, `architecture/references/domain/pipeline-consumer-ergonomics.md`, `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, and R1–R4 remediation tasks. There is no original task file by maintainer direction. The citation check exited 0 and the claim ledger has no surviving Wave 1 claims. I assessed the candidate source and cumulative diff independently, without Wave 1 reasoning or an intended verdict.

## Validated ledger

**Empty.** No blocking claim survives validation. There are no new `FIND-*` IDs, reopened classes, or notes from the claim ledger. The R4 change introduces no redundant guard or error path that needs removal: it replaces the example's erased result at the existing live owner and uses the crate's existing `thiserror` dependency.

## Prior finding closure

| ID | Independent closure trace |
|---|---|
| `FIND-ranker-pipeline-1` | `report::Arms` supplies the validated baseline and decision identities; `live::run` uses `arms.adopt_key()`, and `live::record` uses `arms.rule()` and `arms.verdict_key()` (`eval/src/live.rs:414-438,485-488`). The custom-arm and checked-in fixed-report tests pass in `test:eval`. |
| `FIND-ranker-pipeline-2` | The shared `CoChangeHistory::read` returns `UnprovedHistory` at the discovery cap before appending prior records unless a cold read already reached its depth; an incremental read succeeds only after the prior boundary or listing exhaustion is proved (`pipeline/src/history.rs:369-433`). Both update and cold build use this reader; the cap and no-publication tests are present. |
| `FIND-ranker-pipeline-3` | Eval `Api` uses `GitHubClient` for repository discovery, PR/file listing, commits, and blobs (`eval/src/live.rs:203-229,277-283,320-377`); the pipeline retains provider request ownership. The R4 error conversion did not add an eval HTTP path. |
| `FIND-ranker-pipeline-4` | `IndexedFileFacts::validate` rejects empty, repeated, and unordered reference names (`core/src/facts.rs:123-145`). `SymbolIndex::load` calls it before replacement (`index/src/lib.rs:218-230`), and snapshot decode validates persisted indexed files before reporting complete coverage (`pipeline/src/snapshot.rs:284-329`). |
| `FIND-ranker-pipeline-5` | Both closed and all-PR listing operations call `listed_pull_request`, which rejects a missing or foreign base repository ID while permitting a forked head (`pipeline/src/github.rs:874-887,1610-1611,1703-1706`). The existing fake-provider listing and no-publication tests cover this boundary. |
| `FIND-ranker-pipeline-6` | Public `live`, `run`, and `print`, private `Api::base_snapshot`, `qualify`, `trials`, and `select`, and the `History::build` closure use `LiveError` (`eval/src/live.rs:52-98,161-165,203-210,256-264,315-320,371-379,397-402,414,546`). Its typed variants retain provider, history, snapshot, sample, trial, symbolic, identity, transport, arms, and serialization causes; token and repository configuration have explicit variants. The binary maps `ArmsError` into `LiveError` and retains its failure exit. `test:eval` passed 27 library and 1 binary tests, including typed provider/history/configuration failures, custom-arm report identity, and fixed-report reproduction. |

The cumulative candidate still exposes the four rankers and fused default through the symbolic facade, persists source-free reference facts and bounded history, uses fractional candidate scores and bounded declaration packing, and provides the single-store `ReviewPipeline` consumer path (`select/src/rank.rs`, `bohmian-code-review/src/lib.rs`, `pipeline/src/{snapshot,pull_request,review}.rs`). I found no reachable mismatch with decisions 1–8 or the written repository rules in those inspected paths. `git diff --check` for the immutable range exited 0.

## Verification limits and result

I reran `mise run test:eval` successfully. The checked-in R1–R4 implementation evidence reports the broader owner lanes and `gate` passing; I did not rerun the entire gate. No credentialed live GitHub run was available, so provider behavior rests on pipeline source tracing and fake-provider tests. This does not reopen a claim because the required local failure and report proofs passed.

**Result: validated empty blocking ledger.** The cumulative task is eligible for `PASS` from this independent Wave 2 validation.
