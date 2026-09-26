# TASK-003 r1 — domain review: durable publication, identity isolation, cleanup concurrency

Candidate: 1144845a8deadda54ee546548f43f96b4d5a284f (base 60cc56e). Spec: SPEC-repo-parser rev 11.

## Boundary

Object-before-pointer publication (create/adopt, byte-compare read-back, record, conditional pointer advance), crash/retry safety, immutability and determinism of snapshot bytes, tenant/installation isolation on read and advance, and snapshot collection plus repository-namespace removal. Out of scope: eligibility semantics, archive/blob verification, GitHub rate behaviour.

## Coverage

- Read in full: `crates/bohmian-code-review-pipeline/src/snapshot.rs` (the production half plus all 7 owner tests), `persistence.rs`, the snapshot-store part of `testing.rs`, and the storage.rs diff (`is_snapshot_of`, `ObjectStorage` create/read/delete contracts).
- Spec: REQ-005, REQ-006, REQ-012, REQ-014, REQ-015, INV-003, INV-004, INV-007, AC-005, AC-009; plan S14 and the snapshot-store line; TASK-003 up to `## Implementation Evidence`.
- Ran `cargo test -p bohmian-code-review-pipeline --all-features --lib snapshot`. All 7 passed.

## Verification limits

- I made no source edits. I reproduced the DOM-PUB-1 race by tracing the code, not by running a test. The interleaving is deterministic and could be written as a sequential test.
- The candidate has no PR-context or update facade (TASK-004/005), so `read_snapshot` is the only snapshot reader I could check. I did not verify how a later facade will handle a pointer whose object is missing.
- I did not read `eligibility.rs`. It falls outside this boundary.

## Answers to the directed questions

**(a) Trait surface.** This is consistent with the spec. REQ-015 names the snapshot store's responsibilities as "snapshot metadata/pointers, conditional publication and expired-snapshot cleanup". The plan's version is: "load and conditionally publish snapshot metadata/pointers, and clean up expired unreferenced snapshots". `record_snapshot`, `unreferenced_snapshots` and `delete_snapshot` are the candidate's own design for that named cleanup capability. They follow the pattern already approved for `ContextStore` (`expired_contexts`/`delete_context`). `unreferenced_snapshots` runs across identities, which sits uneasily with INV-007's "identity at every durable seam". It follows the approved `expired_contexts` precedent, REQ-015 assigns "separate privileged operator operations" to the consumer, and `collect_snapshots` rejects any returned key outside its identity's namespace (`is_snapshot_of`). I am not raising this as a finding.

**(b) Adopt-vs-collect race.** The spec does forbid this outcome. See DOM-PUB-1. The claim that "the next build rebuilds" holds only for the cold-build facade at the same tree (`run` treats `Storage(NotFound)` on the current pointer as a reason to rebuild, snapshot.rs:455; tested at the end of `collection_removes_only_expired_unreferenced_snapshots`). REQ-005 has the consumer call cold build only "with no compatible pointer", and after the race a compatible pointer does exist. Until something calls cold build for that scope, every reader gets `ColdBuildError::Storage(StorageError::NotFound)`. That error is visible, so there is no silent data loss and no wrong facts, but there is no fallback either. Collection never notices that the pointer is dangling.

**Isolation.** Passes. Pointers are keyed by `SnapshotScope` (the full identity plus versions). `read_snapshot` compares the pointer key with the key the library derives for the caller's own identity and tree before touching storage. Cold build for another tenant or installation writes its own key and cannot move the owner's pointer (tested). Identity is a validated non-optional type at every method, so a missing identity cannot reach the store. The memory store matches rows on full `Identity` equality.

**Determinism and immutability.** Passes. A snapshot depends only on identity, versions and tree: files are in canonical order, `skipped` is sorted, `excluded` is a `BTreeMap`, and ingest stats, commit and time are left out. The archive path and the all-fallback path produce byte-identical output (tested). An adopted object is compared byte for byte, and a different object is refused as `Corrupt`.

**Memory store fidelity.** It matches the contract in every respect but one. The pointer CAS compares tree only. The reference check matches any pointer of the same identity across scopes, which is correct because the key encodes the versions. `remove_repository` returns only recorded keys, which is the gap in DOM-PUB-2.

## Findings

### DOM-PUB-1 — Collection can delete an object that a concurrent adopting build has just pointed to

- **Defect class:** list-then-act collection with no claim against concurrent publication.
- **First owner:** the `SnapshotStore` cleanup contract (`persistence.rs:184` `unreferenced_snapshots` and `persistence.rs:196` `delete_snapshot`), used by `collect_snapshots` (`snapshot.rs:285-290`). The shape allows the race: the listing is a snapshot in time, and `delete_snapshot` deletes the row unconditionally. The candidate acknowledges the race in the `ponytail:` note at `snapshot.rs:268`.
- **Violated obligations:**
  - TASK-003 Scenario 3: "Collection removes expired unreferenced snapshots and never a pointed-to one".
  - INV-003: "A pointer targets only a validated, complete snapshot".
  - REQ-012: cleanup is "independently callable and safe to retry … converge to a valid result or a visible terminal failure".
- **Evidence (trace):**
  1. A retry after an interrupted publication, or a return to an earlier tree, leaves key K recorded, unpointed and old.
  2. `collect_snapshots` lists (identity, K).
  3. A concurrent `ColdBuild::run` calls `create(K)`, gets `AlreadyExists` and adopts K (snapshot.rs:550-551). It reads K back and the bytes match. It calls `record_snapshot`, which refreshes the time, then `publish_snapshot_pointer` succeeds.
  4. The collector then calls `storage.delete(K)` (snapshot.rs:289) and `store.delete_snapshot` (snapshot.rs:290).

  The end state is a pointer to K with no object and no record. The trigger in step 1 is the spec's own named retry path (REQ-012, "interruption between object write and metadata publication"), and that path is exactly what collection targets.
- **Observable consequence:** every `read_snapshot` for the scope returns `Storage(NotFound)` until the consumer happens to call cold build again for the same tree. A consumer that follows REQ-005 ("no compatible pointer") will not call it. The TASK-004 update and TASK-005 PR-context facades inherit a pointer to a missing object. This does not leak data across identities and does not produce wrong facts.
- **Sibling locations:** `remove_repository` running concurrently with a build for the same identity can leave a fresh object, record and pointer after the removal returns. The next removal call picks them up, so this converges, and I am not raising it separately. `ContextStore` expiry is not a sibling: context ids are never adopted (`AlreadyPublished`), and `expires_at` is fixed.
- **Testable correction:** close the window in the store contract with a claim, not with a comment. Mark rows for collection atomically, only if they are still unreferenced and `recorded_at < before`. Make `record_snapshot` on a claimed row fail with a retryable conflict. Record before create/adopt (see DOM-PUB-2). The collector then deletes the object and then the row. Test with a deterministic sequential interleaving in the owner tests: orphan K recorded at `UNIX_EPOCH`, claim it, then retry the cold build. The build must not publish (it gets a retryable error). Finish collection, run the build again, and it publishes with the object present. At no step may the pointer name a missing key. If the owner decides to accept the window instead, the spec or task must say so explicitly. The current `ponytail:` note contradicts Scenario 3.

### DOM-PUB-2 — Objects are created before they are recorded, so an interruption in between leaves a snapshot that collection and namespace removal cannot find

- **Defect class:** object before bookkeeping row; the object is untracked during the crash window.
- **First owner:** `ColdBuild::publish` (`snapshot.rs:550-560`): `storage.create` → read-back → `record_snapshot`. The trait doc at `persistence.rs:164-166` claims "an object orphaned by an interrupted build is still collectable". That is true only for an interruption after line 560.
- **Violated obligations:**
  - REQ-006: "The library exposes collection of expired unreferenced snapshots and removal of a repository namespace".
  - TASK-003 Scenario 3: "repository namespace removal returns its object keys for deletion".
  - REQ-012: retry-safe "after interruption between object write and metadata publication".
- **Evidence (trace):** any of the following between line 550 and line 560 leaves the object at K with no record:
  - a process crash or cancellation;
  - `storage.read` returning `Backend` (line 554);
  - `record_snapshot` returning `Backend` (line 560);
  - the `Corrupt` read-back branch (line 555-558), which returns before recording.

  Recovery then depends on a later cold build of the same identity and tree adopting K. If the default branch moves on first, nothing will ever reference or record K again. `unreferenced_snapshots` iterates only over records, and `remove_repository` returns only recorded keys (`testing.rs` `remove_repository`), so neither collection nor namespace removal ever finds K. The existing crash test uses `fail_next_publish`, which fails after recording, so it does not cover this window.
- **Observable consequence:** objects that can never be collected pile up. After tenant or repository offboarding through `remove_repository`, source-free facts (paths, blob ids, symbol names, identity) stay in storage under the removed namespace. That makes the namespace-removal promise incomplete.
- **Sibling locations:** same method: the `Corrupt` branch at snapshot.rs:554-558 and read-back `Backend` at 554. No other publisher of snapshot objects exists.
- **Testable correction:** call `record_snapshot(identity, &key, now)` before `storage.create`. Readers see only pointers, so REQ-015 object-before-row visibility still holds for published metadata. Collection already tolerates a missing object because `ObjectStorage::delete` is idempotent. Test: add a one-shot read or record failure switch that fires after `create` succeeds, run the build and expect an error, then assert that `remove_repository(identity)` returns K and that `collect_snapshots(later)` deletes it.

### Suggestion (not a finding)

- `collect_snapshots` returns at the first `ForeignKey` (snapshot.rs:286-287). A single bad row from a faulty store therefore blocks the whole sweep on every call. Failing closed is defensible for a faulty store. Skipping and reporting the row would let the rest of the sweep continue. This is the owner's call.

## Overall

**FAIL.** Isolation, determinism, the adopt byte-compare, and pointer CAS publication are sound. DOM-PUB-1 and DOM-PUB-2 are reachable, testable gaps in the collection and crash-retry obligations that Scenario 3, REQ-006 and INV-003 name explicitly. The `ponytail:` note turns a stated obligation into an accepted defect, and neither the spec nor the task permits that.

```citations
[
  {"id":"DOM-PUB-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":268,"snippet":"/// ponytail: a build adopting an object between the store's listing and the","symbols":["collect_snapshots"]},
  {"id":"DOM-PUB-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":285,"snippet":"    for (identity, key) in store.unreferenced_snapshots(before, limit)? {","symbols":["collect_snapshots","unreferenced_snapshots"]},
  {"id":"DOM-PUB-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":289,"snippet":"        storage.delete(&key)?;","symbols":["collect_snapshots","delete"]},
  {"id":"DOM-PUB-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":551,"snippet":"            Ok(()) | Err(StorageError::AlreadyExists { .. }) => {}","symbols":["publish"]},
  {"id":"DOM-PUB-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":455,"snippet":"                Err(ColdBuildError::Storage(StorageError::NotFound { .. })) => {}","symbols":["run"]},
  {"id":"DOM-PUB-1","path":"crates/bohmian-code-review-pipeline/src/persistence.rs","line":196,"snippet":"    fn delete_snapshot(&self, identity: &Identity, key: &ObjectKey)","symbols":["delete_snapshot"]},
  {"id":"DOM-PUB-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":550,"snippet":"        match self.storage.create(&key, &bytes) {","symbols":["publish","create"]},
  {"id":"DOM-PUB-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":560,"snippet":"        self.store.record_snapshot(identity, &key, now)?;","symbols":["publish","record_snapshot"]},
  {"id":"DOM-PUB-2","path":"crates/bohmian-code-review-pipeline/src/persistence.rs","line":164,"snippet":"    /// The library records an object after it validates and before any","symbols":["record_snapshot"]},
  {"id":"DOM-PUB-2","path":"crates/bohmian-code-review-pipeline/src/persistence.rs","line":206,"snippet":"    fn remove_repository(&self, identity: &Identity) -> Result<Vec<ObjectKey>, PersistenceError>;","symbols":["remove_repository"]}
]
```
