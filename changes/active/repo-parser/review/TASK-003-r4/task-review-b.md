# TASK-003 Wave 1 task review B

Subject: base `c74871cbc78140820c9b0a5fda60e0952c4265e4`, candidate `0a1dea22b206e6174d4421d5ba7f7590ded93673`; approved `SPEC-repo-parser@12`; complete base-to-candidate diff inspected. Prior r1–r3 findings were considered only where their defect class survives the async revision.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Cold snapshot, pinned streamed archive, blob verification, bounded fallback, source-free reload (REQ-005, INV-003, INV-005; S1/S11) | `snapshot.rs:451-554,674-780`; `archive.rs:38-250`; `github.rs:990-1020` | `complete_cold_snapshot_is_verified_source_free_and_reloadable`, `same_tree_serializes_identically_across_ingest_paths`, archive owner tests; implementation evidence records passing pipeline and gate lanes | PASS |
| Incomplete tree, archive/blob bound, mismatch or parse failure leaves pointer unmoved with honest coverage (REQ-005, INV-002) | `snapshot.rs:487-535,708-779` | `incomplete_attempts_report_coverage_and_keep_the_prior_pointer`; `verified_oversize_blob_is_a_benign_skip` | PASS |
| Record → create-if-absent → CAS; abort at each await leaves valid pointer and recorded objects; retry converges (REQ-006, REQ-012, REQ-013) | `snapshot.rs:570-612`; storage and store contracts | `dropped_publication_keeps_pointer_and_retry_converges`, existing-object and lost-CAS tests | PASS |
| Cleanup deletes only stale unreferenced objects before records, preserving pointed-to and re-touched snapshots (REQ-006; Scenario 4) | `snapshot.rs:289-303`; `testing.rs:280-317` | `collect_removes_only_stale_unreferenced_records` re-touches before collection starts; no interleaving after `unreferenced` | **FAIL: TASK-B-1** |
| Tenant/installation isolation and separate in-memory snapshot store (REQ-015/016, INV-007, AC-005/009) | `snapshot.rs:239-270,295-320`; `persistence.rs:128-217`; `testing.rs:183-332` | `identities_cannot_read_or_advance_each_others_snapshots`, collection test | PASS subject to cleanup race above |
| `.gitattributes` false versus unspecified reset (Scenario 5) | `eligibility.rs:80-153,281-292` | `attribute_resets_read_as_unset` | PASS |
| No detached task, no added storage backend, scoped PR indexing, update, scheduling, deadline, or durable source archive | `snapshot.rs:451-780`; complete diff; `tar` remains a dev dependency | `rg 'tokio::spawn'` has no match; diff and manifests inspected | PASS |

## Proposed finding

### TASK-B-1 — INCORRECT: cleanup uses a stale candidate list across an object deletion

**Root cause and defect class.** `SnapshotStore::unreferenced` returns only keys, losing the observed `touched_at`/pointer state. `Snapshots::collect` then deletes the object and record after independent awaits without a conditional store operation. This trait shape permits a record re-touch or pointer publication after selection to be ignored. Siblings: `Snapshots::collect` is the only normal cleanup sink; `Snapshots::remove_repository` delegates to it but documents caller serialization with builds, so it adds no separate defect.

**Obligation.** TASK-003 Scenario 4 and acceptance criterion 4 require `collect(identity, older_than)` never to collect a re-touched or pointed-to snapshot; spec REQ-006 requires the same. The task also requires every written object to retain a record.

**Reachable interleaving.** A stale unreferenced record for key K is returned by `unreferenced`. A concurrent cold build of K calls `upsert_record(K, now)` and begins/finishes `create(K)`. The collector then deletes K's object and record using its stale list. The build can create K after deletion or publish a pointer to K after deletion; either yields an unrecorded object or a pointer whose object is missing. A pointer can likewise move to K after selection and before deletion. The existing test refreshes K before `unreferenced`, so it cannot detect this interleaving. The caller's `older_than` spacing cannot prevent a new touch after selection.

**Consequence.** A complete build can return `Published` while its pointer is unreadable, or leave an unrecorded object. A live snapshot can be deleted. This violates snapshot durability and the stated cleanup criterion.

**Testable correction.** Coordinate eligibility and deletion atomically at the snapshot store/object boundary, or use a candidate-owned record state that prevents a collector from deleting a key whose touch or pointer changed since selection. Preserve object-before-record order and dropped-future retry safety. Exercise a deterministic stall after `unreferenced`, then re-touch/publish the selected key and resume collection; assert the object, record, and pointer remain valid. Also cover build creation after selection and object deletion.

## Archive reader judgment

The installed `tar` crate consumes synchronous `Read`; the provider supplies async chunks, while the task forbids durable archive buffering and detached work. Bridging `tar::Archive` would require a blocking producer/consumer worker with cancellation and backpressure, or full archive buffering. The push `TarStream` is warranted under these constraints. Its custom parsing does not by itself constitute DRIFT. It verifies wanted file bytes against the listed Git blob SHA before indexing, and malformed/omitted entries fall back to bounded blob fetches. The archive tests cover chunk boundaries, PAX/GNU names, sizes and bounds. No archive-specific blocking defect was established.

## Prior-art classes

R1 record-before-object, rejected-entry coverage, concrete IO owner, and bare imports are closed in the candidate. R3 `!attr` reset is closed by tri-state parsing and its owner test. R1–R3 explicit cancellation/deadline findings no longer apply: revision 12 intentionally replaces that design with future drop and idempotent publication. The earlier cleanup/adoption race is rechecked here because Scenario 4 now explicitly requires a re-touched record to survive.

## Overall

**FAIL** — TASK-B-1. Available verification: candidate task evidence records scenario tests, `test:pipeline`, symbolic journey, format, lints and gate passing; this review did not rerun the full gate. The proposed race is established by the source path and a concrete allowed interleaving, not by an added test.

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":295,"snippet":"        for key in self.store.unreferenced(identity, older_than).await? {","symbols":["collect","unreferenced"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":299,"snippet":"            self.storage.delete(&key).await?;","symbols":["collect","delete"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":583,"snippet":"            .upsert_record(identity, &key, SystemTime::now())","symbols":["publish","upsert_record"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/testing.rs","line":280,"snippet":"    async fn unreferenced(","symbols":["unreferenced"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/testing.rs","line":308,"snippet":"    async fn delete_record(","symbols":["delete_record"]}]
```
