---
id: TASK-003
kind: implementation
status: review
spec: SPEC-repo-parser@11
depends_on: [TASK-001, TASK-002]
maps: [REQ-005, REQ-006, REQ-015, REQ-016, INV-002, INV-003, INV-005, INV-007, AC-005, AC-009]
---

## Outcome and Value

A first repository invocation builds a verified, source-free default-branch snapshot from a streamed archive. Only a complete snapshot is published; an incomplete or oversized build returns an explicit fallback result for the PR-context facade.

## Owners, Scope, Consumers, and Prohibited Changes

The outer library owns eligibility, bounded archive/blob ingest, snapshot serialization, publication order, and fallback signaling. The cold-build facade accepts only an object storage and a snapshot store, both caller-implemented, plus a caller-supplied deadline and cancellation; the library ships no storage backend and imposes no deadline maximum. This task first consumes the snapshot store, so it proves that trait's contract with an in-memory implementation. No source archive, raw blob cache, or checkout is persisted. Caller job scheduling is outside this crate.

## Approach

1. Apply supported-path, mode, size, root attributes, and LFS eligibility to the pinned tree.
2. Stream and verify eligible archive entries, falling back to bounded verified blob calls where needed.
3. Publish only validated complete snapshots before advancing the pointer; return incomplete coverage and a scoped-fallback reason otherwise.

## Ordered Implementation Scenarios

### Scenario 1 — Complete cold snapshot

**Behavior.** A fake GitHub repository produces a verified source-free snapshot; its pointer appears only after the object is complete, and a later load reproduces symbol facts without source fetches.

**RED.** Add an outer-crate cold-build journey over injected in-memory object storage and snapshot store; the absent snapshot/pointer flow fails.

**GREEN.** Build the eligible index, serialize facts and coverage, write/validate the object, then publish metadata and pointer.

**REFACTOR.** Reuse the public symbolic facade and provider identities; keep publication as one observable boundary.

### Scenario 2 — Incomplete build outcome

**Behavior.** Truncated tree, archive mismatch, LFS pointer, parse failure, or exhausted bound never advances a complete pointer; the build result carries exact omissions and a reason for the PR-context facade to use scoped fallback.

**RED.** Add negative fake-provider cases matching S10–S11 and an object-write crash case; the initial result wrongly publishes or hides coverage.

**GREEN.** Enforce bounds and incomplete-attempt results; preserve an existing pointer and permit whole-build retry.

**REFACTOR.** Keep eligibility and coverage accounting reusable by the later scoped PR path without treating incomplete input as full repository coverage.

### Scenario 3 — Snapshot-store isolation and cleanup

**Behavior.** With two tenant/installation identities, one identity can neither read nor advance the other's pointer; a missing identity fails closed; interruption after object write is retry-safe. Collection removes expired unreferenced snapshots and never a pointed-to one; repository namespace removal returns its object keys for deletion.

**RED.** Add cold-build and cleanup cases over the in-memory snapshot store for cross-identity access, interruption after write, and a referenced versus unreferenced expired snapshot; they fail before isolation and cleanup exist.

**GREEN.** Enforce identity and object-before-pointer through the snapshot store and expose snapshot cleanup; rerun Scenarios 1–2.

**REFACTOR.** Keep publication and cleanup checks at the one publication boundary.

## Acceptance Criteria

The cold-build portions of S1 and S11 pass: complete source-free snapshot, verified archive/fallback blobs, immutable object and safe pointer publication. Incomplete attempts and crashes leave the prior pointer intact with exact coverage and retry/fallback reasons. The snapshot-store portion of S14 and AC-009 passes against an in-memory implementation separate from any context store. The later library journey proves the composed first-PR, scoped-review and independent-clock outcomes.

## Expected Write Set and Consumer Closure

Outer crate ingest/snapshot behavior and focused tests, injected test capabilities, scoped lane and relevant lockfile. TASK-004 and TASK-005 consume immutable snapshots.

## Verification and Evidence

Run the outer crate's scoped lane, `mise run test:symbolic-context:journey`, `mise run fmt:check`, and `mise run lints`; record stored object/row inspection and paused-clock behavior.

## Material Stop Conditions

Stop if the chosen storage operation cannot make incomplete objects invisible or if coverage cannot distinguish excluded from missing eligible files.

## Authority Links

[Revision 11 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md).

## Implementation Evidence

Starting point `741c049` (unverified draft) was kept where correct and fixed at the root: `read_snapshot` rebuilds `ObjectKey::snapshot` for the caller identity and pointer tree and refuses any other key (`ColdBuildError::ForeignKey`) before storage is read; per-attempt counters moved from the persisted `Coverage` to `ColdBuildResult::ingest` (`IngestStats`); the facade error is imported (`SymbolicError`); `glob_matches` is a segment reachability pass plus an iterative single-segment matcher; the archive closure is `Ingest::stream_entries`. Also removed: the draft's `MAX_COLD_BUILD_DEADLINE`/`DeadlineTooLong` (the task forbids a library deadline maximum) and the draft's benign `Oversize` skip for an oversized archive entry (now left pending for verified blob fallback, so an archive cannot turn an eligible file into a benign skip). Publication now compares the read-back object byte for byte with the built snapshot before adopting it. Eligibility internals (`classify`, `RootAttributes`, `EligibleTree`, `Exclusion`, `MAX_ELIGIBLE_FILE_BYTES`) are crate-private; only `ExclusionReason` is public because `Coverage` carries it.

Local decision: snapshot cleanup needs persisted snapshot rows, not only pointers, so `SnapshotStore` gained `record_snapshot`, `unreferenced_snapshots`, and `delete_snapshot`, and `remove_repository` returns every recorded key. The build records an object after it validates and before the pointer moves, so an object orphaned by an interrupted publication is collectable. `collect_snapshots` deletes object before record and refuses a returned key outside its identity's snapshot namespace. LFS pointers and verified oversize blobs are benign skips that keep their path/blob identity in the snapshot; they do not block completeness (spec "reasoned exclusion", S11).

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| S1 cold build: complete source-free snapshot, verified archive and fallback blobs, reload without source fetches, separate review clock | `snapshot.rs` `ColdBuild::run`, `Ingest::{stream_entries, fallback, ingest}`, `read_snapshot`, `Snapshot::into_context` | `complete_cold_snapshot_is_verified_source_free_and_reloadable` (stored object inspected for absent source fragments; blob call counts unchanged on reload; reuse makes no archive/tree call) | PASS |
| Immutable object: same tree serializes to identical bytes | `Coverage` holds only tree facts; `IngestStats` on `ColdBuildResult` | `same_tree_serializes_identically_across_ingest_paths` (archive-fed vs all-fallback bytes equal; no ingest counters or timestamps in the object) | PASS |
| S10–S11 incomplete attempts: truncated tree, archive mismatch past fallback bound, LFS, parse failure, exhausted bounds, cancellation keep the prior pointer and report exact coverage and reason | `IncompleteReason`, `Coverage::{is_complete, unindexed}`, `SkipReason` | `incomplete_attempts_report_coverage_and_keep_the_prior_pointer`, `interruption_is_explicit_and_deadlines_are_caller_owned` | PASS |
| Safe publication: object created/adopted, byte-compared, recorded, then conditional pointer; crash before/after object write retries by whole build | `ColdBuild::publish` | `publication_is_crash_safe_and_retry_converges`, `collection_removes_only_expired_unreferenced_snapshots` (missing pointed-to object is rebuilt) | PASS |
| S14 / AC-005 / AC-009 snapshot-store isolation: another tenant or installation sees no pointer, cannot read via a foreign pointer, cannot advance the owner's pointer | `read_snapshot` exact-key check, `SnapshotStore` scoping, `testing.rs` `MemorySnapshotStore` | `identities_cannot_read_or_advance_each_others_snapshots` | PASS |
| REQ-006 cleanup: expired unreferenced snapshots collected, pointed-to never; namespace removal returns its keys only | `collect_snapshots`, `ObjectKey::is_snapshot_of`, `SnapshotStore::{record_snapshot, unreferenced_snapshots, delete_snapshot, remove_repository}` | `collection_removes_only_expired_unreferenced_snapshots` (cutoff before/after record time, forged foreign record refused) | PASS |
| Caller-owned deadline, no library maximum | `ColdBuild::run` uses only `Execution` | `interruption_is_explicit_and_deadlines_are_caller_owned` (24 h deadline publishes) | PASS |
| Eligibility by reason; linear glob matching | `eligibility.rs` `classify`, `glob_matches`, `segment_matches` | `classification_reports_each_reason`, `attributes_resolve_like_git`, `glob_segments_and_many_stars_match_without_backtracking` (41-star pattern over 4000-byte near-miss) | PASS |

Paused-clock note: collection is driven by a caller-supplied cutoff; tests use `UNIX_EPOCH` (nothing expired) and now + 1 h (orphan expired) rather than sleeping. Deadline and cancellation use `Execution`, tripped by a transport hook.

Verification (this session, all exit 0): `mise run test:pipeline` (27 unit tests, doc tests), `mise run test:symbolic-context:journey` (6), `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`.

Non-goals held: no storage backend, no scoped PR fallback indexing (TASK-005 consumes `Coverage`/`IncompleteReason`), no incremental update, no source/archive/blob persistence, no new dependency. Changed files: `crates/bohmian-code-review-pipeline/src/{eligibility,lib,persistence,snapshot,storage,testing}.rs` and this task file.

Known limit: a build adopting an existing object between `unreferenced_snapshots` and deletion can publish a pointer to a just-deleted object; the next cold build sees `NotFound` and rebuilds (marked `ponytail:` in `collect_snapshots`).
