# TASK-004 independent task review B

Subject: base `a77c64d8194ff3b8e0bc9f51f68b2cc5ae0095ec`; candidate `f5a6f4bbeae1877083f79f4198316205c75a63e8`; approved `SPEC-repo-parser@12`; original `TASK-004`. I inspected the complete base-to-candidate diff and the implementation paths, not the task's implementation-evidence summary. The candidate remained at the specified commit during this review.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-007 / S3: changed, deleted, renamed, and now-ineligible files equal a fresh parse while preserving the old object | `Update::run` in `update.rs` pins a head; `ColdBuild::start` classifies it; `Ingest::reuse` in `snapshot.rs` carries facts only for matching path/blob pairs; `ColdBuild::complete` creates a new object | `change_delete_and_rename_match_a_fresh_parse` compares serialized objects, old bytes, lookup facts, coverage, and provider calls | PASS |
| REQ-007 / S4, S16: moving or force-pushed head is visible and repeated updates converge | `Update::freshness` compares pointer and pinned tree; `ColdBuild::publish` uses pointer CAS and returns `PointerConflict` | `moving_head_is_reported_stale_and_updates_converge` moves the head during publication, races writers, and retries; exact focused update suite passed | PASS |
| REQ-002 / S8: version change cannot reuse incompatible facts | `snapshot_scope` includes current extraction and format versions; `Update::run` cold-builds when the current scope has no pointer | `version_change_rebuilds_under_a_new_key` checks a new key and preserved old-version pointer | PASS |
| REQ-007 / S9: review-time stale-pointer query is typed and does not wait for an update | `Freshness::{Current,Stale}` and `Update::freshness` read the pointer and branch head without reading a tree or building a snapshot | `moving_head_is_reported_stale_and_updates_converge` checks typed states and zero tree calls | PASS |
| REQ-006, INV-002, INV-003: a failed changed blob removes its stale facts from the attempt, exposes incomplete coverage, and leaves the pointer intact | Changed paths remain pending after `Ingest::reuse`; `Ingest::fallback` records `FetchFailed`; `ColdBuild::complete` refuses incomplete coverage | `failed_changed_blob_keeps_the_pointer_until_a_retry` checks the skip, unchanged pointer/object/record state, and successful retry | PASS |
| REQ-006, REQ-012, REQ-013 / S17: dropping the future leaves a complete pointed-to snapshot and records every written object; retry converges | Shared `ColdBuild::publish` performs record upsert, create-if-absent, then CAS; the update owns no detached work or transaction spanning IO | Paused-clock `dropped_update_keeps_the_pointer_and_retry_converges` drops after each publication seam and checks records, objects, pointer, and retry | PASS |
| REQ-013: changed-blob calls have a ceiling and large changes rebuild from a bounded archive | `ColdBuildLimits::max_changed_blobs`; `Update::run` uses `ColdBuild::fetch` past the ceiling | `changes_past_the_ceiling_use_the_archive` checks archive use and call counts; `ColdBuild::fetch` applies existing archive/fallback limits | PASS |
| INV-004: immutable snapshots remain identity-scoped and deterministic | `Snapshots::read` validates key/digest/identity/tree/version; the update reuses only the loaded snapshot and publishes through the existing tree key | From-scratch object-byte oracle and old-byte assertions in the change/delete/rename journey | PASS |
| Task non-goals and scope: no queue, worker, deadline, cancel token, consumer code, new dependency, or duplicate symbolic stage | Diff changes only task documentation and pipeline `lib.rs`, `snapshot.rs`, `update.rs`; update delegates to existing symbolic facade and cold publication | Complete diff inspection; focused update suite passed | PASS |

Verification: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib update::tests:: -- --nocapture` exited 0: six tests passed, none failed. The orchestrator also reported `mise run gate` and `git diff --check <base> <candidate>` exited 0 on this candidate. The focused suite covers the changed update behavior; live provider behavior and later PR-overlay integration belong to other tasks or final change review.

Proposed findings: none. The added facade and changed-blob ceiling implement named task and specification outcomes; I found no candidate-introduced abstraction, dependency, or compatibility path to remove without losing them. The prior snapshot read, path/blob match, bounded fetch, complete-only publication, and pointer CAS form a reachable end-to-end update path.

Overall result: **PASS**.

```citations
[]
```
