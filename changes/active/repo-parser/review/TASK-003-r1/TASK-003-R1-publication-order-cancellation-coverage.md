---
id: TASK-003-R1
kind: remediation
status: ready
spec: SPEC-repo-parser@11
remediates: TASK-003
review: changes/active/repo-parser/review/TASK-003-r1/verdict.md
findings: [FIND-TASK-003-1, FIND-TASK-003-2, FIND-TASK-003-3, FIND-TASK-003-4, FIND-TASK-003-5]
---

# TASK-003-R1 — Publication order, archive cancellation, rejected-entry coverage, owner shape

## Identities

- Approved spec: `changes/active/repo-parser/spec.md` revision 11.
- Original task: `changes/active/repo-parser/tasks/TASK-003-cold-snapshot-and-scoped-context.md`.
- Reviewed candidate: `1144845a8deadda54ee546548f43f96b4d5a284f` (base `60cc56e`; TASK-003 work began at `741c049`).
- Validation ledger: `changes/active/repo-parser/review/TASK-003-r1/findings-validation.md`.

All paths below are under `crates/bohmian-code-review-pipeline/src/`.

## Issue diagnosis

### FIND-TASK-003-1 — Object written before its tracking row

- **Root cause:** `ColdBuild::publish` (snapshot.rs:550–560) runs `storage.create` → read-back `storage.read` → `store.record_snapshot`. The object is durable before any row tracks it.
- **Defect class:** durable object written before its tracking row.
- **Siblings:** read-back error and `Corrupt` branches in the same window (snapshot.rs:554–557); `collect_snapshots` (snapshot.rs:285) and `SnapshotStore::remove_repository` (persistence.rs:206), which only see recorded keys; the trait rustdoc at persistence.rs:164, which states the (wrong) record-after-validate order.
- **Violated obligation:** TASK-003 Scenario 3 "interruption after object write is retry-safe … repository namespace removal returns its object keys for deletion"; REQ-006 collection and namespace removal.
- **Current behavior / reproduction:** with `record_snapshot` failing once, `ColdBuild::run` errors, `collect_snapshots(now + 1h)` returns `[]`, `remove_repository` returns `[]`, and the object remains in storage.
- **Consequence:** if the branch moves on before a same-tree retry, the tenant's object is stored forever and namespace removal reports nothing to delete.
- **Why current proof falls short:** `publication_is_crash_safe_and_retry_converges` injects its crash after the record is written.

### FIND-TASK-003-2 — Cancellation and deadline ignored during archive ingest

- **Root cause:** `consume_archive` (snapshot.rs:624) and `stream_entries` (snapshot.rs:646) never call `execution.check()`; the only checkpoint is inside `fallback` (snapshot.rs:693), which does nothing when the archive supplied every eligible file; `run` does not check again before `publish` (snapshot.rs:524).
- **Defect class:** missing cancellation checkpoint in the archive path.
- **Siblings:** `stream_entries` loop, `consume_archive`, `run` before `publish`, and test `interruption_is_explicit_and_deadlines_are_caller_owned` asserting `archive_entries == 4` after cancellation (snapshot.rs:1362).
- **Violated obligation:** TASK-003 Owners "caller-supplied deadline and cancellation"; REQ-005 "honors a caller-supplied deadline and cancellation"; REQ-013 "Cancellation and partial progress are reported honestly."
- **Current behavior / reproduction:** two eligible files, both in the archive, token cancelled on the tarball request → outcome `Published` and the pointer advances.
- **Consequence:** a cancelled or expired build still writes an object and moves the pointer; a deadline expiring during a large stream is ignored until it ends.
- **Why current proof falls short:** the owner test encodes the defect.

### FIND-TASK-003-3 — Rejected tree entries vanish from coverage

- **Root cause:** `ColdBuild::run` passes only `listing.entries` to `Ingest::new` (snapshot.rs:484); `TreeListing::rejected` (github.rs:456, filled at github.rs:938 by `validate_tree_entry`) is never read; `Coverage` (snapshot.rs:109–121) has no place for them.
- **Defect class:** rejected tree paths silently omitted from coverage accounting.
- **Siblings:** `Coverage::excluded` (snapshot.rs:120); `Ingest::finish` (snapshot.rs:751–765).
- **Violated obligation:** spec "Eligible file" "Eligibility and exclusions are reported by reason"; spec "Complete snapshot" "Excluded and unsupported paths remain counted separately"; INV-002; TASK-003 stop condition on distinguishing excluded from missing files.
- **Current behavior / reproduction:** tree with `src/user.ts` and `src/a\b.ts` (both 100644) → `Published`, `Coverage { eligible: 1, indexed: 1, skipped: [], excluded: {} }`; the second path is in no count.
- **Consequence:** the persisted snapshot claims complete coverage of a tree it partially dropped without a reason.

### FIND-TASK-003-4 — Storage workflows as public free functions

- **Root cause:** `read_snapshot` (snapshot.rs:239) and `collect_snapshots` (snapshot.rs:278) are public free functions performing storage and persistence IO.
- **Siblings:** the reuse call in `ColdBuild::run` (snapshot.rs:447); re-exports in `lib.rs`.
- **Violated rule:** AGENTS.md:27 "Keep stateful capabilities and workflows on cohesive concrete structs. Use free functions only for stateless transformations"; architecture/agent-rules.md:10; architecture/references/languages/rust-core.md:3.

### FIND-TASK-003-5 — Path-qualified type in a signature

- **Root cause:** `fn stream_entries<R: Read>(&mut self, tar: &mut tar::Archive<R>)` (snapshot.rs:646).
- **Violated rule:** AGENTS.md:31 "Keep imports at module top and use imported bare types in signatures."

## Intended outcome

Every snapshot object the library writes is tracked before it exists; a cancelled or expired cold build never publishes; every tree entry the listing returned is accounted for by reason in `Coverage`; snapshot read and collection live on a concrete owner; no path-qualified type remains in the changed signatures.

## Decision-complete recommendation

1. **FIND-1 (owner `ColdBuild::publish`):** call `store.record_snapshot(identity, &key, now)` before `storage.create`. This reuses the existing idempotent `ObjectStorage::delete` contract (storage.rs:231, deleting an absent key succeeds), so a record whose object was never written collects and removes cleanly. Update the rustdoc at persistence.rs:164 to state record-before-write. No new trait method. Rejected alternative: reconciling storage listings against rows — needs a new listing capability and is larger.
2. **FIND-2 (owner `Ingest`, reuse `Execution::check`, `Interrupted`, `IncompleteReason::Interrupted`):** give the archive path the execution handle; check before each archive entry and stop reading on interruption, keeping that interruption in `Ingest` so `fallback` starts from it and the existing loop marks remaining pending files `NotFetched`; in `run`, take one final `execution.check()` before the completeness decision so an interrupted build never reaches `publish`. Correct the snapshot.rs:1362 assertion to the now-true value.
3. **FIND-3 (owner `Ingest`/`Coverage`, reuse the `Coverage::excluded` reason map):** add one `ExclusionReason` variant for tree entries rejected by validation (e.g. `InvalidEntry`), count `TreeListing::rejected` under it in `finish`, and keep it a reasoned exclusion (`is_complete` unchanged, like `Unsupported`). A new enum variant keeps old snapshots decodable; no format-version bump.
4. **FIND-4:** introduce one concrete owner holding `&dyn ObjectStorage` and `&dyn SnapshotStore` with inherent read and collect methods carrying the current behavior (exact-key check, identity-namespace check, object-before-record deletion); `ColdBuild` uses it on the reuse path; delete the two free functions and update `lib.rs` re-exports. Neither function is spec-named, so this is candidate-owned surface.
5. **FIND-5:** have `stream_entries` take the reader and construct the tar archive in its body, so no tar type appears in the signature (or import it under a non-clashing bare alias at module top).

## Constraints and preserved behavior

- Keep: exact-key refusal (`ForeignKey`), byte-compare adoption, tree-conditional pointer publication, identity isolation, deterministic snapshot bytes, LFS/oversize reasoned exclusions, no library deadline maximum, the sequential-contract collection guarantee.
- No new dependency, no storage backend, no source/archive/blob persistence.
- Every new or materially modified item (including tests) gets rustdoc per AGENTS.md; fallible items keep `# Errors`.
- Do not weaken or delete existing tests other than correcting the assertion that encodes FIND-2.

## Non-goals

- Store-side leases or claims closing the concurrent collection/adopt race (REQ-012 assigns scheduling and claims to consumers). The existing `ponytail:` marker stays.
- Removing `ColdBuildLimits::max_file_bytes` (recorded as a note, not required here).
- Scoped PR fallback indexing, incremental update (TASK-004/005).

## Acceptance criteria

| Finding / sibling | Criterion |
|---|---|
| FIND-1: create failure, read-back failure, pointer-publish failure | After each injected failure, `remove_repository` returns the snapshot key and `collect_snapshots` past expiry leaves object storage empty |
| FIND-1: persistence.rs:164 doc | Rustdoc states the record-before-write order |
| FIND-2: cancel during archive, archive complete | Outcome is `Incomplete` with `Interrupted` cancellation; no object written; prior pointer unchanged |
| FIND-2: expired deadline during archive | Outcome is `Incomplete` with the deadline interruption; no object written |
| FIND-2: snapshot.rs:1362 test | Assertion reflects that no archive entry is consumed after cancellation |
| FIND-3: rejected entries | A tree with one backslash path and one unknown-mode entry reports both under the new exclusion reason; `eligible` unchanged; outcome `Published` |
| FIND-4 | No `pub fn` performing storage IO remains in snapshot.rs; existing read/collect tests pass through the owner |
| FIND-5 | No path-qualified type in the changed signatures |

## Proof

- Focused: the owner tests above in `snapshot.rs`'s test module over `testing.rs` in-memory stores.
- Broader: `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`.

## Expected write set

`snapshot.rs`, `persistence.rs` (rustdoc), `lib.rs` (re-exports), `testing.rs` if a failure-injection hook is needed, and the TASK-003 task file's implementation evidence.

Route to `$cr-implement`.

## Implementation evidence

Candidate: `4bafce8` on `review-helper-setup`. Paths under `crates/bohmian-code-review-pipeline/src/`.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| FIND-1: create, read-back, pointer-publish failure each leave a tracked key | `ColdBuild::publish` calls `record_snapshot` before `storage.create`; `MemoryStorage::fail_next_read` (testing.rs) | `every_interrupted_publication_leaves_a_tracked_key` (collect past expiry empties storage; `remove_repository` returns the key, per failure) | PASS |
| FIND-1: persistence.rs rustdoc | `SnapshotStore::record_snapshot` doc states record-before-write | review of diff | PASS |
| FIND-2: cancel during a complete archive | `Ingest::stream_entries` checks `Execution` before each entry and keeps the interruption; `fallback` starts from it; `run` takes a final `execution.check()` before the completeness decision | `interruption_during_a_complete_archive_never_publishes` (Cancelled, no object, prior pointer kept) | PASS |
| FIND-2: expired deadline during archive | same | same test (DeadlineExceeded, no object, prior pointer kept) | PASS |
| FIND-2: owner-test assertion | `interruption_is_explicit_and_deadlines_are_caller_owned` now asserts `archive_entries == 0` and all six eligible files `NotFetched` | `mise run test:pipeline` | PASS |
| FIND-3: rejected entries counted | `ExclusionReason::InvalidEntry` (eligibility.rs); `Ingest::new` takes `listing.rejected.len()`, `finish` counts it | `rejected_tree_entries_are_counted_as_invalid_exclusions` (backslash path + mode `100600` → `InvalidEntry: 2`, eligible 1, `Published`) | PASS |
| FIND-4: storage workflows on a concrete owner | `Snapshots { storage, store }` with `read` and `collect`; free functions deleted; `lib.rs` re-exports `Snapshots` | existing read/collect tests route through `Snapshots` and pass | PASS |
| FIND-5: no path-qualified signature types | `stream_entries<R: Read>(&mut self, expanded: R, execution: &Execution)` builds the tar archive in its body | diff review, `mise run lints` | PASS |

Commands run this session, all exit 0: `cargo test --locked -p bohmian-code-review-pipeline --all-features --lib snapshot`, `mise run fmt:check`, `mise run lints`, `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run gate`, `git diff --check`.

Non-goals held: no store-side lease (the `ponytail:` marker moved with `Snapshots::collect`), `max_file_bytes` unchanged, no TASK-004/005 work, no new dependency, no format-version bump. Files changed: `eligibility.rs`, `lib.rs`, `persistence.rs`, `snapshot.rs`, `testing.rs`, and this record.
