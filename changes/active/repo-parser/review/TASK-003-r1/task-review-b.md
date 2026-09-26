# TASK-003 task review r1, sample B

- Subject: TASK-003 against SPEC-repo-parser@11. Candidate `1144845` (base `60cc56e`). I reviewed the full candidate `snapshot.rs`, `eligibility.rs`, and `testing.rs`, plus the `lib.rs`, `persistence.rs`, and `storage.rs` diffs.
- Verification run: `cargo test -p bohmian-code-review-pipeline --all-features` passed all 27 tests. `mise run fmt:check` and `mise run lints` both passed.
- Overall: **FAIL** (TASK-B-1, TASK-B-2)

## Decisions validated

1. **New SnapshotStore methods (`record_snapshot`, `unreferenced_snapshots`, `delete_snapshot`): PASS.** REQ-015 gives the snapshot store "snapshot metadata/pointers, conditional publication and expired-snapshot cleanup". REQ-006 requires "collection of expired unreferenced snapshots". Recording before publishing the pointer is the least that makes S17 orphans collectable. The cross-identity listing follows the same pattern as the already-reviewed `ContextStore::expired_contexts`. Per REQ-015, privileged operator operations are the consumer's concern. Each abstraction is needed (Ponytail rung 1 holds).
2. **LFS pointers and oversized blobs as benign exclusions: PASS.** The spec's "Eligible file" definition says a file is "at most 1 MiB" and "LFS pointers are skipped after content inspection ... exclusions are reported by reason". S11 requires "reasoned exclusion, no false complete pointer". A complete pointer over a tree whose only skips are verified LFS or oversize content is therefore not false. The task's Scenario 2 list also includes "archive mismatch", which the spec resolves by verified fallback rather than incompleteness. So that list means "never a *false* complete pointer", not "never a pointer". Both benign reasons are decided only on hash-verified bytes: archive bytes are checked against `git_blob_sha` and `GitHubClient::blob` verifies its bytes. An oversized archive entry is refetched rather than turned benign (tested in `same_tree_serializes_identically_across_ingest_paths`).
3. **Race between collection and snapshot reuse left under a `ponytail:` comment: FAIL.** See TASK-B-1. The race is reachable through the spec's own S17 retry path and breaks an explicit invariant. It is not a tolerable performance ceiling.

## Matrix

| Requirement/AC/constraint/non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-005 list tree, apply eligibility, stream the pinned archive without writing to disk, verify each blob | `run` L460-491, `classify`, `stream_entries` L646-679 | `complete_cold_snapshot_is_verified_source_free_and_reloadable` | PASS |
| REQ-005 bounded verified blob fallback for archive substitution or omission | `fallback` L684-720 | Omitted and substituted fixtures; `max_fallback_blobs: 1` case | PASS |
| REQ-005 truncated tree, unindexed file, or exhausted bound gives an incomplete result | L463-513 | `incomplete_attempts_report_coverage_and_keep_the_prior_pointer` | PASS |
| REQ-005 honors caller deadline **and cancellation** | Deadline is capped through `timeout_global`. Cancellation is checked only in `fallback` (L694). | The test asserts `archive_entries == 4` after cancelling at the tarball request (L1362) | **FAIL (TASK-B-2)** |
| REQ-006 immutable key per identity, versions, and tree; duplicate reused only after validation | `snapshot_key`, `publish` L550-558 | `publication_is_crash_safe_and_retry_converges`, `same_tree_serializes_identically_across_ingest_paths` | PASS |
| REQ-006 / INV-003 pointer only after the object is validated; incomplete attempts leave the prior pointer | `publish` L550-575; early returns L463-513 | Scenario 2 test (prior pointer preserved, no keys written) | PASS |
| REQ-006 collection of expired unreferenced snapshots; namespace removal | `collect_snapshots` L278-294; `remove_repository` | `collection_removes_only_expired_unreferenced_snapshots` (sequential only) | **FAIL (TASK-B-1)** |
| Task Scenario 3 "never a pointed-to one"; S17 "no visible pointer to missing object" | Deletion does not re-check the reference at delete time (L285-290) | No interleaving test | **FAIL (TASK-B-1)** |
| REQ-015 snapshot-store contract carries identity; object-before-row; in-memory impl | `persistence.rs` trait; `MemorySnapshotStore` | Owner tests | PASS |
| REQ-016 injected storage, no backend shipped | `ObjectStorage` only; `MemoryStorage` is `cfg(test)` | Crate builds with no backend dependency | PASS |
| INV-002 exclusions distinct from missing eligible files | `Coverage { excluded, skipped, eligible, indexed }` | Scenario 1 and 2 coverage asserts | PASS |
| INV-005 no source archive or raw blob cache persisted | The snapshot holds facts, paths, and blob ids only | The Scenario 1 test inspects the stored object | PASS |
| INV-007 / S14 / AC-005 identity isolation, including a missing identity | `read_snapshot` key equality; `Snapshot::decode` identity check; `is_snapshot_of` guard | `identities_cannot_read_or_advance_each_others_snapshots`; forged-record case | PASS |
| AC-009 snapshot store exercised separately from the context store | `MemorySnapshotStore` is its own type | Owner tests | PASS |
| Non-goal: no deadline maximum imposed | `MAX_COLD_BUILD_DEADLINE` removed | Day-long deadline publishes (L1380-1382) | PASS |
| Gates: `fmt:check`, `lints`, crate lane | n/a | All green (run by this reviewer) | PASS |

## Findings

### TASK-B-1: INCORRECT. Collection can delete an object that a pointer names

- **Root cause:** `collect_snapshots` (snapshot.rs L285-290) decides "unreferenced" at listing time. It then deletes the object and the record without any store-side condition that the key is still unreferenced. Meanwhile `ColdBuild::publish` (L550-560) adopts an existing object on `AlreadyExists`, refreshes its record, and publishes a pointer to it. The first owner producing the wrong outcome is the `SnapshotStore` cleanup contract. `delete_snapshot` (persistence.rs L196) is unconditional and nothing claims or tombstones a key under collection, so the owner's shape cannot prevent the interleaving.
- **Defect class:** check-then-act (TOCTOU) between the collection listing and the concurrent adopt-and-publish of the same immutable key.
- **Sibling locations:** `collect_snapshots` L285/L289; `ColdBuild::publish` adopt path L550 and record L560; the unconditional `SnapshotStore::delete_snapshot` persistence.rs L196 plus its `MemorySnapshotStore` implementation.
- **Violated obligation:**
  - Task Scenario 3: "Collection removes expired unreferenced snapshots and never a pointed-to one".
  - INV-003: "A pointer targets only a validated, complete snapshot".
  - S17: "Retry converges; no visible pointer to missing object".
- **Evidence (reachable path, the S17 retry):**
  1. A build writes and records the object for tree T, then its pointer publish fails. This is the tested `fail_next_publish` path.
  2. Later, after `before` has passed, a periodic `collect_snapshots` lists the orphan.
  3. The caller's retry build for the unchanged tree T gets `AlreadyExists`, reads the object back successfully, refreshes the record, and publishes the pointer.
  4. The collector runs `storage.delete(&key)` and then `delete_snapshot`.
  5. The pointer now names a missing object and its record is gone.

  A revert that restores an earlier root tree reaches the same state. The implementer's own `ponytail:` comment at L268 acknowledges the window. The collection test is sequential and never interleaves.
- **Observable consequence:** a published pointer resolves to `StorageError::NotFound`. Any reader (the TASK-004/005 warm path) fails until another cold build happens to run and rebuild. The collector also returns success for having deleted a pointed-to snapshot.
- **Testable correction:** make collection claim a key atomically in the store before deleting the object. The claim holds only if the key is still unreferenced and `recorded_at < before`, and it leaves a tombstone. While the tombstone exists, `record_snapshot` and `publish_snapshot_pointer` for that key fail or conflict, so a build retries after deletion and recreates the object. Add an interleaving test: a `MemorySnapshotStore` hook runs `ColdBuild::run` for the same tree between `unreferenced_snapshots` and deletion. Then assert that either the collector skipped the key, or the build returned a retryable conflict and a subsequent `read_snapshot` of the published pointer succeeds.

### TASK-B-2: MISSING. Cancellation is ignored for the whole archive stream

- **Root cause:** `Ingest::consume_archive` (L624) and `stream_entries` (L646-679) take no `Execution` and never call `execution.check()` between tar entries. The deadline is enforced indirectly by the transport's `timeout_global = min(remaining, request_timeout)`, but the `CancelToken` is not visible to the transport. The first cancellation check is in `fallback` (L694), and only if something is still pending. `run` does not check again before `publish` (L524). The owner's shape (no `Execution` parameter) prevents honoring cancellation.
- **Defect class:** a long-running loop that does not observe the caller's cancellation token.
- **Sibling locations:**
  - `consume_archive` L624 and the `stream_entries` loop L647, both without `Execution`.
  - `run` L489 passes no execution.
  - `run` L506-524 publishes without a final execution check.
- **Violated obligation:** REQ-005: "The call honors a caller-supplied deadline and cancellation".
- **Evidence:** the existing test `interruption_is_explicit_and_deadlines_are_caller_owned` cancels on the tarball request, yet asserts `result.ingest.archive_entries == 4` (L1362). The archive was streamed, verified, and parsed completely after cancellation. It reports `Interrupted` only because the fixture omits two files and forces the fallback check. With an archive that supplies every eligible file, `pending` is empty, `fallback` never checks, and `run` writes the object and moves the pointer after cancellation. On the large-repository path the spec benchmarks (10k-50k files), streaming requires a caller-raised `request_timeout`. Cancellation (for example, pod shutdown) is then ignored for the full stream and parse duration.
- **Observable consequence:**
  - A cancelled cold build keeps downloading and parsing for up to `request_timeout`.
  - It then writes storage and advances the pointer, returning `Published` instead of `Incomplete(Interrupted(Cancelled))`.
  - A graceful shutdown is not graceful.
- **Testable correction:**
  - Thread `&Execution` into `consume_archive`/`stream_entries`, check it per entry, and stop with the interruption recorded.
  - Check `execution` once more before `publish`, returning `Incomplete(Interrupted(..))` with nothing written.
  - Test: archive containing every eligible file, cancel on the tarball request. Assert `Incomplete(Interrupted(Cancelled))`, `storage.keys().is_empty()`, and `archive_entries == 0`.

```citations
[
  {"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":268,"snippet":"/// ponytail: a build adopting an object between the store's listing and the","symbols":["collect_snapshots"]},
  {"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":285,"snippet":"    for (identity, key) in store.unreferenced_snapshots(before, limit)? {","symbols":["collect_snapshots","unreferenced_snapshots"]},
  {"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":289,"snippet":"        storage.delete(&key)?;","symbols":["collect_snapshots","delete"]},
  {"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":550,"snippet":"        match self.storage.create(&key, &bytes) {","symbols":["publish","create"]},
  {"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":560,"snippet":"        self.store.record_snapshot(identity, &key, now)?;","symbols":["publish","record_snapshot"]},
  {"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/persistence.rs","line":196,"snippet":"    fn delete_snapshot(&self, identity: &Identity, key: &ObjectKey)","symbols":["delete_snapshot"]},
  {"id":"TASK-B-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":489,"snippet":"            Ok(archive) => ingest.consume_archive(archive, self.limits.max_expanded_archive_bytes),","symbols":["run","consume_archive"]},
  {"id":"TASK-B-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":524,"snippet":"        let pointer = self.publish(&scope, current.as_ref(), &head.commit, &snapshot)?;","symbols":["run","publish"]},
  {"id":"TASK-B-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":624,"snippet":"    fn consume_archive(&mut self, archive: Archive, max_expanded_bytes: u64) {","symbols":["consume_archive"]},
  {"id":"TASK-B-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":647,"snippet":"        for entry in tar.entries()? {","symbols":["stream_entries","entries"]},
  {"id":"TASK-B-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":694,"snippet":"                interrupted = execution.check().err();","symbols":["fallback","check"]},
  {"id":"TASK-B-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":1362,"snippet":"        assert_eq!(result.ingest.archive_entries, 4);","symbols":["interruption_is_explicit_and_deadlines_are_caller_owned"]}
]
```
