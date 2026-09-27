---
id: TASK-004
kind: implementation
status: proposed
spec: SPEC-repo-parser@12
depends_on: [TASK-003]
maps: [REQ-007, REQ-012, REQ-013, INV-002, INV-003, INV-004]
---

## Outcome and Value

The update facade advances a repository pointer from one complete immutable snapshot to another after default-branch changes, while a review already using the old object remains valid.

## Owners, Scope, Consumers, and Prohibited Changes

The outer library compares tree path/blob identities, replaces changed facts, removes deleted facts, and publishes validated results. Callers decide when to invoke it and bound it by dropping its future; it takes no cancel token or deadline, and accepts only an object storage and the snapshot store. Changed-blob fetches that run concurrently use a `JoinSet` owned by the call, never a detached task. No queue or worker is part of this crate. No stale facts may survive a failed replacement; incomplete updates cannot move the pointer.

## Approach

1. Compare the current pinned branch tree with the loaded snapshot, including eligibility and version changes.
2. Reuse unchanged facts, fetch and verify changed blobs, and remove deleted/ineligible facts.
3. Publish only a complete, validated snapshot through TASK-003's publication path: record upsert with `touched_at`, then create-if-absent object, then pointer compare-and-swap. Otherwise report a whole-build retry reason.

## Ordered Implementation Scenarios

### Scenario 1 — Change, delete, and rename

**Behavior.** A push changing, deleting, and renaming files yields facts equivalent to a fresh parse of the new tree; old snapshot bytes remain unchanged.

**RED.** Add a fake-provider update journey with a from-scratch oracle; the missing update facade or stale facts fail it.

**GREEN.** Implement tree comparison and immutable publication; rerun the oracle and cold-build journey.

**REFACTOR.** Share the eligibility and verified blob path with cold ingest.

### Scenario 2 — Moving head and incomplete update

**Behavior.** A force-push or changed branch head is detected from tree identity. Failed changed blobs leave the old pointer intact, and a whole-build retry can publish a complete result. If the update future is dropped at any `.await`, the pointer stays on a complete snapshot and every written object has a record (REQ-013). Repeated calls converge on the current tree.

**RED.** Extend the update journey with paused or racing branch responses, a changed-blob failure, and a paused-clock `tokio::time::timeout` that drops the update mid-publication. Assert pointer identity, records, and explicit coverage, then retry.

**GREEN.** Use the pointer compare-and-swap, and return a retry state when another invocation has already advanced the pointer. Do not hold a snapshot-store transaction across provider or storage IO.

**REFACTOR.** Keep version and pointer checks at the publication boundary rather than duplicating them in callers.

## Acceptance Criteria

S3–S4, S8–S9, S16–S17 pass at the facade boundary: changed/deleted/renamed facts match a fresh parse, old snapshots are immutable, incomplete attempts do not move the pointer, and a moving branch head requests another update. A dropped update leaves the pointer on a complete snapshot, and a retry converges. A review-time stale-pointer check returns a typed result without blocking the current review.

## Expected Write Set and Consumer Closure

Outer crate update facade, focused fake-provider tests and caller-facing result contract. No consumer code is required.

## Verification and Evidence

Run the outer crate's scoped lane, `mise run fmt:check`, `mise run lints`, and `mise run gate`; record old/new snapshot bytes, pointer state, coverage and oracle equality.

## Material Stop Conditions

Stop if update publication can overwrite an immutable snapshot or expose a pointer before its object is validated.

## Authority Links

[Revision 12 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md).

## Implementation Evidence

Candidate: `Update` facade in `crates/bohmian-code-review-pipeline/src/update.rs`; `ColdBuild::run` split into `pin`/`build`/`start`/`fetch`/`complete` in `snapshot.rs` so the update reuses the one `publish` path (record upsert with `touched_at`, create-if-absent object, pointer CAS) and `Snapshots::collect` unchanged. `Ingest::reuse` carries facts and benign skips only for paths whose blob is unchanged; `ColdBuildLimits::max_changed_blobs` (default 2,000) switches changed-blob fetches to the archive; `IngestStats::reused_files` reports reuse. `Update::freshness` returns typed `Freshness::{Current, Stale}` from the pointer and branch head without reading or building.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| S3: changed/deleted/renamed/now-ineligible facts match a fresh parse; only changed blobs fetched; old snapshot immutable; pointer moves | `Update::run`, `Ingest::reuse` | `update::tests::change_delete_and_rename_match_a_fresh_parse`: 0 archive calls, 3 blob calls (attributes + 2 changed), `reused_files=2`; old object bytes equal before/after; update object bytes equal a from-scratch `ColdBuild` of the new tree; `saveUser`/`formatUser`/`substitutedFn` absent; repeat is `Reused` with no fetch | PASS |
| S4/S16: moving or force-pushed head detected from tree identity; repeated calls converge; lost swap is a retry state | CAS in `ColdBuild::publish`, `Update::freshness` | `moving_head_is_reported_stale_and_updates_converge`: head moved mid-publication → pointer on pinned tree, `Freshness::Stale` at new head; racing update loses → `PointerConflict`; retry `Reused`; `Freshness::Current` | PASS |
| S8: version change uses a new key, no old facts reused | current-version `snapshot_scope` | `version_change_rebuilds_under_a_new_key`: archive cold build, `reused_files=0`, new key ≠ old, old-scope pointer untouched | PASS |
| S9: review-time stale-pointer check is typed and non-blocking | `Update::freshness` | same moving-head test: stale result with 0 tree calls | PASS |
| Failed changed blob leaves pointer; whole retry publishes; coverage explicit | `ColdBuild::complete` | `failed_changed_blob_keeps_the_pointer_until_a_retry`: `Incomplete(UnindexedFiles{1})`, `FetchFailed` skip for `src/user.ts`, pointer/objects/records unchanged; retry publishes | PASS |
| S17/REQ-013: dropped update leaves pointer on a complete snapshot, every object recorded, retry converges | reused `publish` | `dropped_update_keeps_the_pointer_and_retry_converges`: paused-clock timeout after upsert, after create, before swap | PASS |
| Changed-blob ceiling bounded | `max_changed_blobs` | `changes_past_the_ceiling_use_the_archive` | PASS |

Commands (this session, all exit 0): `mise run test:pipeline` (46 lib tests), `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`.

Non-goals held: no queue, worker, detached task, cancel token, or deadline; changed-blob fetches are sequential in the call, so no `JoinSet` is needed; no snapshot-store transaction spans IO; no consumer code or new dependency. Changed files: `lib.rs`, `snapshot.rs`, `update.rs`, this task.
