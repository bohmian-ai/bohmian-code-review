---
id: TASK-005
kind: implementation
status: proposed
spec: SPEC-repo-parser@12
depends_on: [TASK-001, TASK-002, TASK-003]
maps: [REQ-008, REQ-009, REQ-010, REQ-011, REQ-012, REQ-013, REQ-014, REQ-015, REQ-016, REQ-018, INV-002, INV-004, INV-005, INV-006, INV-007, AC-005, AC-009]
---

## Outcome and Value

One PR invocation overlays its pinned head on a private index, ranks related files from changed lines, stores selected source spans once, and returns a context ID that authorized consumers can read for 24 hours.

## Owners, Scope, Consumers, and Prohibited Changes

The outer library owns PR source/diff reconciliation, ranking orchestration, budgeted packing, immutable context publication, and read-time integrity/expiry checks. The six symbolic crates own extraction and scoring. Callers supply verified identity and implementations of the traits each facade uses, and they bound each call by dropping its future. The library takes no cancel token or deadline and never detaches a task; concurrent blob fetches use a `JoinSet` owned by the call. The traits used by each facade are: the PR-context facade takes the snapshot store, the context store, and object storage for snapshots and contexts; the read facade takes only the context store and context object storage. This task first consumes the context store, so it fixes that trait's method contract and proves it with an in-memory implementation. The library does not invoke a model or publish comments.

## Approach

1. Load a compatible snapshot into an invocation-private index and reconcile it with the PR head tree, or build a bounded scoped index when the cold build reports fallback.
2. Guard paginated PR files with before/after base/head reads and head blob checks; classify missing or malformed patches per file.
3. Rank and fetch only selected candidate blobs, and enforce the candidate budget.
4. Publish in order: upsert the context record with `expires_at`, then create the object if absent. Read one immutable context object by ID.

## Ordered Implementation Scenarios

### Scenario 1 — Pinned multi-file PR

**Behavior.** Concurrent PRs and a stale base each see only their own head facts; selected candidate spans and reasons equal a from-scratch head parse when coverage is complete. A failed cold build uses changed paths for bounded scoped indexing and reports scoped coverage.

**RED.** Add a fake-provider public PR-context journey for S5–S7 and a failed-cold-build scoped case from S10; the missing overlay/fallback/ranking/publication fails it.

**GREEN.** Compose the snapshot, private overlay, PR diff and existing symbolic facade; publish one validated object and tenant-scoped ID.

**REFACTOR.** Keep overlays invocation-local and reuse the symbolic selector's deterministic ranking.

### Scenario 2 — Drift, omissions, and budget

**Behavior.** A base/head change during pagination triggers retry; bad or absent patches affect only their files; exhausted budget and incomplete overlay are reported without claiming full coverage.

**RED.** Add fake-provider S18–S19 and failed-blob cases; assert no mixed-head publication, no stale facts, and explicit reasons.

**GREEN.** Add bounded retry, file-local omission, coverage and token-estimate behavior.

**REFACTOR.** Keep one coverage/budget result shape for full and scoped reviews.

### Scenario 3 — Shared read and expiry

**Behavior.** Before expiry, repeated reads by ID return the same selected spans without GitHub calls. These reads fail: at or after expiry, from another tenant or installation, for a corrupt object, and with a missing identity. A read ignores a record whose object is absent. Dropping the PR-context future after the record upsert or after the object create leaves no readable half context and no unrecorded object; a retry returns a readable context. Cleanup handles contexts past `expires_at` in the same order as snapshot collection: delete the object, where a missing object is success, then delete the record. A failed delete keeps its record for retry and is reported as lag (REQ-012, REQ-014, REQ-015).

**RED.** Add paused-clock reads for S14–S15 over the in-memory context store with two identities. Include a `tokio::time::timeout` drop at each publication step, a record whose object is absent, and a failing delete. These cases fail until the expiry, integrity, isolation, and cleanup checks exist.

**GREEN.** Read the exact published record and object under the supplied identity and check expiry and integrity. Never rebuild from the current branch. Expose expired-context cleanup in object-then-record order, reusing TASK-003's cleanup order.

**REFACTOR.** Keep source-bearing storage confined to the context object and drop transient source after use.

## Acceptance Criteria

S5–S7, S10, S14–S15 and S18–S19 pass with injected test capabilities, the context store exercised separately from the snapshot store (AC-009): pinned identity, private/scoped indexing, selected text, coverage, omissions, expiry and budget. The diff remains separate. The read facade enforces identity and expiry. A dropped PR-context call leaves no readable partial context, and a retry converges. A downstream model caller counts its complete request separately.

## Expected Write Set and Consumer Closure

Outer crate PR and read facades, focused fake-provider/storage tests and the public library journey. No consumer implementation is required.

## Verification and Evidence

Run the outer crate's scoped lane, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `mise run gate`; record stored object, metadata, read result, provider calls and paused-clock expiry.

## Material Stop Conditions

Stop if provider data cannot be tied to one recorded base/head pair, or if context can be read by ID alone without tenant and installation checks.

## Authority Links

[Revision 12 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md).

## Implementation Evidence

Candidate: `PrContext` facade in `crates/bohmian-code-review-pipeline/src/pull_request.rs`; `ReviewContext`, `Contexts::{publish, read, collect_expired}` in `context.rs`. Reuse: the overlay is the snapshot `Ingest` (`new` over the head tree, `reuse` of the loaded snapshot, direct `ingest` of changed heads, bounded `fallback` for default-branch drift, `finish`), attributes via the extracted `snapshot::root_attributes`, snapshot load via `Snapshots::read`, ranking and packing via the facade's `rank_files`/`build_candidate_context`. `ContextStore::publish_context` became idempotent `upsert_context` (record before object); the context id is the object's SHA-256, so a retried publication reaches the same record and key. Reads take an explicit `now`, matching `collect(older_than)`.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| S5–S6: concurrent PRs editing the same path see only their own head; complete coverage equals a from-scratch head parse | `PrContext::run`, private `Ingest` per call | `pull_request::tests::concurrent_prs_see_only_their_own_heads`: `tokio::join!` of PR 1/2; spans equal oracle for each; PR 2 never matches PR 1's unmerged `pr1Only`; `coverage.is_full()`; 0 archive calls | PASS |
| S7: stale base overlays drift or reports incomplete | `fallback(max_overlay_changes)` | `stale_base_overlays_drift_or_reports_incomplete`: `src/user.ts` ranks on `renameUser`, never the removed `saveUser` or old blob; with bound 0 the drifted file is skipped, `overlay_complete=false`, no stale span | PASS |
| S10: failed cold build → bounded scoped index, honest scope, nothing published as a snapshot | `near`, `Scope::Scoped` | `failed_cold_build_falls_back_to_scoped_context`: truncated-tree cold build `Incomplete`; PR context `Scoped`, `snapshot_complete=false`, `eligible=6` (src/ only), `lib/far.ts` not selected, no snapshot object | PASS |
| S18: pagination, base/head drift retry, file-local omissions, changed paths never candidates | `PrContext::pin`, `changed_head`, changed-path filter | `moving_pr_retries_and_bad_patches_stay_file_local`: 400 files over 4 pages; head moved during attempt 1 → 4 PR reads, 8 page reads, context at new head; `MissingPatch`, `MalformedPatch`, 397 `NotEligible`; only `src/pr1.ts` selected. `perpetually_moving_pr_publishes_nothing`: `PullRequestMoved{3}`, no record | PASS |
| S19: exhausted budget reported, empty context, no false guarantee | `Budget` | `exhausted_budget_packs_nothing_and_says_so`: `exhausted`, allowance 0, no spans/omissions, 2 blob calls (attributes + changed head, no candidates) | PASS |
| S14–S15/AC-009: identity-scoped reads, 24 h expiry, separate keys, source only in the object | `Contexts::read` | `context::tests::reads_are_identity_scoped_and_expire` (repeat reads equal; `Expired` at and after `expires_at`; other tenant/installation `NotFound`; separate keys; record JSON has no source); journey read-back equal with unchanged provider call counts; snapshot and context stores are separate in-memory implementations | PASS |
| Absent object ignored; corrupt/foreign fail | `Contexts::read` | `absent_corrupt_and_foreign_objects_fail` | PASS |
| Dropped PR call leaves no readable half context; retry converges | `Contexts::publish` order | `dropped_publication_leaves_no_readable_half_context`: paused-clock `timeout` after upsert (record, no object, read `NotFound`) and after create; retry returns the same id and reads back | PASS |
| Cleanup object-then-record, missing object success, failed delete kept as lag | `Contexts::collect_expired` | `expired_cleanup_deletes_object_then_record_and_reports_lag` | PASS |

Commands (this session, all exit 0): `mise run test:pipeline` (56 lib tests), `mise run test:symbolic-context:journey` (6), `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`.

Limits: token counts are a byte/4 estimate, reported as such; candidate and changed-blob fetches are sequential in the call (no `JoinSet` needed); each changed head is extracted twice (validation, then union ranking). Closed or merged PRs are not refused. Non-goals held: no model call, comment publication, detached task, cancel token, deadline, consumer code, or new dependency. Changed files: `lib.rs`, `context.rs`, `pull_request.rs`, `persistence.rs`, `snapshot.rs`, `github.rs`, `testing.rs`, this task.
