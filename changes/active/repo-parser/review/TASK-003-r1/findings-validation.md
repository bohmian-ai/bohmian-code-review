# TASK-003-r1 Wave 2 findings validation (ponytail-rev)

> Saved verbatim by the orchestrator from the `ponytail-rev` hand-back; the harness blocked the subagent's own write to this path.

Candidate `1144845a8deadda54ee546548f43f96b4d5a284f`, base `60cc56e`, spec revision 11. Paths are under `crates/bohmian-code-review-pipeline/src/` unless stated otherwise.

I reproduced the findings with scratch tests in a throwaway worktree (`git worktree add --detach /tmp/t003-pr 1144845`). The tests were appended to the `snapshot.rs` test module there and run with `cargo test --all-features -p bohmian-code-review-pipeline --lib scratch -- --nocapture`. The worktree has been removed. Tracked source was not edited and nothing was committed.

## Merge table

| Source IDs | Merged claim |
|---|---|
| TASK-B-1, DOM-PUB-1 | M1: collection racing a concurrent adopt can delete an object a pointer now names |
| TASK-A-1, DOM-PUB-2 | M2: the object is created before it is recorded, so an interrupted publish leaves an object that neither collection nor namespace removal ever returns |
| TASK-B-2, DOM-TRUST-2 | M3: cancellation and deadline are not checked during archive streaming or before publish |
| DOM-TRUST-1 | M4: tree entries rejected by `validate_tree_entry` vanish from coverage |
| TASK-A-2 | M5: `max_file_bytes` shapes snapshot bytes but is not part of the key |
| REPO-1 | M6: `collect_snapshots` and `read_snapshot` are public IO workflows written as free functions |
| REPO-2 | M7: the `stream_entries` signature names the path-qualified `tar::Archive<R>` |
| REPO-3 | M8: `ponytail:` markers in production rustdoc |

## Retained blocking findings

### FIND-TASK-003-1 (M2: TASK-A-1, DOM-PUB-2)

**Status:** CONFIRMED. **Gate:** (2) task contract. **Classification:** INCORRECT; production path, crash recovery and cleanup.

**Root cause:** `ColdBuild::publish` (snapshot.rs:550–560) runs `storage.create`, then the read-back, then `store.record_snapshot`. The object is durable before its tracking row exists.

**Defect class:** a durable object written before its tracking row.

**Siblings:**
- The read-back error branch and the `Corrupt` branch (snapshot.rs:554–557), which sit in the same create-to-record window.
- `collect_snapshots` (snapshot.rs:285).
- `SnapshotStore::remove_repository` (persistence.rs:206).
- The trait doc at persistence.rs:164, which is false: "records an object after it validates … so an object orphaned by an interrupted build is still collectable".

**Violated obligation:**
- TASK-003 Scenario 3: "interruption after object write is retry-safe. Collection removes expired unreferenced snapshots…; repository namespace removal returns its object keys for deletion."
- REQ-006: "The library exposes collection of expired unreferenced snapshots and removal of a repository namespace."

**Reproduction:** a scratch `FailRecord` store fails `record_snapshot` once. Then `ColdBuild::run`, `collect_snapshots(before = now + 1h)` and `remove_repository`. Observed output:

`SCRATCH run_err=Some(Persistence(Backend { detail: "crash" })) collected=[] removed=[] stored=[ObjectKey("tenants/acme/installations/42/repositories/1296269/snapshots/…/1/7777…")]`

**Observable consequence:** after a persistence failure or crash following the object write, if the tree moves on so no same-tree retry occurs, the tenant's snapshot object stays in storage permanently. Repository-namespace removal reports nothing left to delete while that tenant's data is still stored.

**Correction (owner: `ColdBuild::publish`):**
- Move `self.store.record_snapshot(identity, &key, now)?` above `self.storage.create(...)`.
- Relies on the existing idempotent `ObjectStorage::delete` contract (storage.rs:231: "deleting an absent key succeeds so retries converge"): a record whose object was never written collects and removes cleanly.
- Rewrite persistence.rs:164 to say the row is recorded before the object is written.
- No new trait method.

**Closure proof:** one test injecting a failure at `storage.create`, at the read-back `storage.read`, and at `publish_snapshot_pointer`. For each, assert `remove_repository` returns the key and `collect_snapshots` past expiry leaves `storage.keys()` empty.

### FIND-TASK-003-2 (M3: TASK-B-2, DOM-TRUST-2)

**Status:** CONFIRMED. **Gate:** (2) task contract. **Classification:** MISSING; production path, reachable from `ColdBuild::run` on ordinary input.

**Root cause:** the archive path never calls `execution.check()`.
- `consume_archive` (snapshot.rs:624) and `stream_entries` (snapshot.rs:646) have no checkpoint.
- `BoundedReader` bounds bytes, not time (github.rs:576–598).
- The only checkpoint is inside `fallback` (snapshot.rs:693), which does not loop when the archive supplied every eligible entry.
- `run` does not check again before `publish` (snapshot.rs:524).

**Defect class:** missing cancellation checkpoint in the archive path.

**Siblings:** the `stream_entries` loop and `consume_archive`; `run` before `publish` (snapshot.rs:524); the owner test at snapshot.rs:1362, which asserts `archive_entries == 4` after cancellation and so encodes the defect.

**Violated obligation:**
- TASK-003 Owners: "plus a caller-supplied deadline and cancellation".
- REQ-005: "The call honors a caller-supplied deadline and cancellation".
- REQ-013: "Cancellation and partial progress are reported honestly."

**Reproduction:** two eligible files, both in the archive, token cancelled on the `/tarball/` request. Observed:

`SCRATCH cancelled=true outcome=Published(SnapshotPointer { tree: …7777, … })`

A cancelled build publishes and moves the pointer. A deadline passing during a multi-GiB stream is likewise ignored until the stream ends.

**Correction (owner: `Ingest`, reusing `Execution::check`, `Interrupted`, `IncompleteReason::Interrupted`):**
- Pass `execution` into `consume_archive`.
- In `stream_entries`, call `execution.check()` before each entry; on `Err`, store the interruption in `Ingest` and stop reading.
- `fallback` seeds its local `interrupted` from that state, so the existing loop marks remaining pending files `NotFetched`.
- In `run`, fold a final `execution.check().err()` into `interrupted` before the completeness check, so a cancelled build never reaches `publish`.

**Closure proof:** keep the scratch scenario (complete archive, cancel on tarball) asserting `Incomplete(Interrupted(Cancelled))` with `storage.keys()` empty; add an already-expired-deadline variant asserting the deadline `Interrupted`; update the assertion at snapshot.rs:1362 to `archive_entries == 0`.

### FIND-TASK-003-3 (M4: DOM-TRUST-1)

**Status:** CONFIRMED. **Gate:** (2) task contract. **Classification:** MISSING; production path, rare input (Git allows a backslash in a file name; trees can hold over-long paths or unknown modes).

**Root cause:** `ColdBuild::run` passes only `listing.entries` to `Ingest::new` (snapshot.rs:484); `TreeListing::rejected` (github.rs:456, filled at github.rs:938) is never read; `Coverage` (snapshot.rs:109–121) has no count for rejected entries.

**Defect class:** rejected tree paths silently omitted from coverage accounting.

**Siblings:** `Coverage::excluded` (snapshot.rs:120); `Ingest::finish` (snapshot.rs:751–765). The later scoped/PR path (TASK-004/005) that reuses this classification should consume the same count.

**Violated obligation:**
- Spec "Eligible file": "Eligibility and exclusions are reported by reason."
- Spec "Complete snapshot": "Excluded and unsupported paths remain counted separately."
- INV-002: "Missing facts, skipped files … remain distinguishable in persisted context."
- TASK-003 stop condition: "coverage cannot distinguish excluded from missing eligible files."

**Reproduction:** tree with `src/user.ts` and `src/a\b.ts`, both mode 100644 with a supported extension. Observed:

`SCRATCH outcome=Published(...) coverage=Coverage { truncated_tree: false, eligible: 1, indexed: 1, skipped: [], excluded: {} }`

The second file is missing from every count.

**Observable consequence:** the persisted snapshot claims complete coverage of a tree that listed a source file the library silently dropped; downstream, an absent candidate cannot be told apart from one never seen.

**Correction (owner: `Ingest`, reusing the `Coverage::excluded` reason map):**
- Add one `ExclusionReason::InvalidEntry` variant.
- `Ingest::new` takes the `TreeListing` (or rejected count); `finish` adds `listing.rejected.len()` under that reason.
- Reasoned exclusion (the facade cannot represent such a path, like `Unsupported`); `is_complete` unchanged.
- A new serde enum variant still decodes old snapshots; no format bump.

**Closure proof:** owner test with one backslash path and one unknown-mode entry asserting `coverage.excluded[&InvalidEntry] == 2`, `eligible` unchanged, outcome `Published`.

### FIND-TASK-003-4 (M6: REPO-1)

**Status:** CONFIRMED. **Gate:** (3) repository standard. **Classification:** VIOLATION.

**Root cause:** `collect_snapshots` (new, snapshot.rs:278) and `read_snapshot` (snapshot.rs:239, materially changed by the ForeignKey check) are public free functions performing multi-step storage and persistence IO.

**Defect class:** a stateful IO workflow outside a concrete owner struct.

**Siblings:** `read_snapshot` (snapshot.rs:239); the reuse path in `ColdBuild::run` that calls it (snapshot.rs:447); the re-exports in lib.rs.

**Violated obligation (text verified):**
- AGENTS.md:27: "Keep stateful capabilities and workflows on cohesive concrete structs. Use free functions only for stateless transformations".
- architecture/references/languages/rust-core.md:3: "Use a free function only when its result is stateless and deterministic from arguments alone."
- architecture/agent-rules.md:10: "Use concrete owners and inherent methods for stateful workflows."

**Evidence:** both take `&dyn ObjectStorage` / `&dyn SnapshotStore`; `collect_snapshots` deletes objects and rows.

**Correction (the concrete-owner pattern `ColdBuild` already uses):** one owner holding the two store references with inherent `read(identity, pointer)` and `collect(before, limit)`; `ColdBuild` uses it on the reuse path; delete both free functions and update re-exports. The spec names neither function, so this is candidate-owned surface.

**Closure proof:** existing read and collect tests pass through the new methods; `rg '^pub fn' crates/bohmian-code-review-pipeline/src/snapshot.rs` returns nothing.

### FIND-TASK-003-5 (M7: REPO-2)

**Status:** CONFIRMED. **Gate:** (3) repository standard. **Classification:** VIOLATION.

**Root cause:** `fn stream_entries<R: Read>(&mut self, tar: &mut tar::Archive<R>)` (snapshot.rs:646) uses a path-qualified type (presumably to avoid clashing with `crate::github::Archive`). **Siblings:** none.

**Violated obligation:** AGENTS.md:31: "Keep imports at module top and use imported bare types in signatures."

**Correction:** take the reader instead (`fn stream_entries<R: Read>(&mut self, reader: R) -> io::Result<()>`) and build the tar archive in the body; `consume_archive` passes the expanded reader directly. The tar type leaves the signature.

**Closure proof:** signature grep; existing archive tests pass.

## Notes (non-blocking)

- **TASK-B-1 / DOM-PUB-1 (M1):** the race is real in source, but closing it needs a lease/claim semantic the spec assigns to consumers (see Rejected). The rustdoc at snapshot.rs:268–271 promises self-healing; it should instead state the caller obligation (do not run collection concurrently with builds of the same identity). `ColdBuild::run` rebuilds on `NotFound` (snapshot.rs:455), but a PR-context reader following the pointer gets `NotFound` until a cold build is re-run.
- **TASK-A-2 (M5):** recommend deleting `ColdBuildLimits::max_file_bytes` in favour of `MAX_ELIGIBLE_FILE_BYTES`. REQ-006 fixes the key to "tenant, installation, repository, extraction version, format version and tree identity"; the spec's configurable ceilings (line 95) list no file-size entry; the eligible-file definition says "initially at most 1 MiB". Changing the value makes a later same-tree build fail visibly with `Corrupt`. Not blocking: default path correct, failure visible.

## Rejected

- **TASK-B-1, DOM-PUB-1:** out of scope. The correction needs a new concurrency-semantics decision (store-side lease). REQ-012: "Job queues, scheduling and cross-installation claims are consumer responsibilities"; REQ-006: "callers choose when to invoke it". TASK-003's "never a pointed-to one" holds under the sequential store contract the task requires.
- **REPO-3:** the cited rule does not cover it. `ponytail:` marks a known simplification ceiling, not a plan/task/agent reference, and is established in base production code (`git grep ponytail 60cc56e -- crates`: `eligibility.rs:151` unchanged by this diff, `bohmian-code-review-select/src/diff.rs:388`, `bohmian-code-review-select/src/lib.rs:477`).

## Implementer-flagged decisions

1. **`SnapshotStore` gained `record_snapshot`, `unreferenced_snapshots`, `delete_snapshot`: VALID.** REQ-015: "a snapshot store for snapshot metadata/pointers, conditional publication and expired-snapshot cleanup". REQ-006: "The library exposes collection of expired unreferenced snapshots". Method shapes are candidate-owned; the record ordering is wrong (FIND-TASK-003-1).
2. **LFS pointers and verified oversize blobs are benign skips keeping a snapshot complete: VALID.** Spec "Eligible file": "initially at most 1 MiB … LFS pointers are skipped after content inspection … Eligibility and exclusions are reported by reason." Plan S11: "Verified fallback, reasoned exclusion, no false complete pointer." An oversize skip is reached only after `GitHubClient::blob` verifies the Git id (github.rs:969–975).
3. **Reuse/cleanup race left with a `ponytail:` comment: VALID** under REQ-012 scoping, but the wording must state the caller obligation (M1 note).

## Recommended outcome

**FIX_REQUIRED.** FIND-TASK-003-1 to -3 are behaviour defects, each fixable at one owner with existing mechanisms. FIND-TASK-003-4 and -5 are mechanical reshapes to meet written repository rules. None needs a spec revision.
