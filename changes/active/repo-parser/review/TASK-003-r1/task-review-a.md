# TASK-003 task review r1, sample A

- Candidate: `1144845a8deadda54ee546548f43f96b4d5a284f`; base `60cc56e`.
- Subject: all of `snapshot.rs`, `eligibility.rs`, `testing.rs` in `crates/bohmian-code-review-pipeline/src/`, plus `git diff 60cc56e 1144845` (`lib.rs`, `persistence.rs`, `storage.rs`).
- Authority: spec revision 11 (`changes/active/repo-parser/spec.md`), plan §8 scenarios S1/S10/S11/S14, TASK-003 up to `## Implementation Evidence`.
- No `.codegraph/` index, so I used targeted `rg`/`sed` searches.

**Overall: FAIL** (TASK-A-1, TASK-A-2)

## Verification run (read-only)

| Command | Result |
|---|---|
| `mise run fmt:check` | exit 0 |
| `mise run lints` | exit 0 |
| `mise run test:symbolic-context:journey` | exit 0, 6 passed |
| `mise run test:pipeline` (`--all-targets --all-features` + doc) | exit 0, 27 passed |
| Scratch-copy reproduction (HEAD exported to `/tmp`, two tests added to `snapshot::tests`, `cargo test -p bohmian-code-review-pipeline --all-features scratch_`) | both reproductions **passed**, which confirms both defects (details under each finding). I deleted the scratch copy afterwards. The repo source was not modified. |

## Matrix

| Requirement / AC / constraint / non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| S1 cold snapshot: verified, source-free, object before pointer, reload without source | `ColdBuild::run`/`publish` (snapshot.rs 424–576); `read_snapshot` checks key, digest and decode | `complete_cold_snapshot_is_verified_source_free_and_reloadable`: source-text absence, reload with Blob count unchanged, reuse without a second archive | PASS |
| REQ-005: eligibility, streamed archive, per-entry blob verification, bounded fallback, deadline/cancel | `classify`, `stream_entries` (git_blob_sha check, size bound, symlink/traversal skipped), `fallback` (max_fallback_blobs, `execution.check`) | incomplete-attempts test (fallback bound, expansion bound); interruption test; tarball fixture includes a symlink and a `../` entry | PASS |
| S10: truncated tree gives an incomplete result, no reusable partial snapshot | `listing.truncated` returns early with `Incomplete(TruncatedTree)` and writes nothing | incomplete test, truncated case: no keys, prior pointer intact | PASS |
| S11: archive substitution/omission gives verified fallback | mismatched or omitted entries stay pending and are fetched through `client.blob` (hash-verified in github.rs) | complete test: substituted/omitted indexed through 2 fallbacks | PASS |
| S11 / Decision 2: LFS pointer and oversize are reasoned exclusions that keep the snapshot complete | `SkipReason::is_benign` | complete test: LFS skip with Published | PASS. The spec glossary defines LFS pointers as "skipped after content inspection" and eligible files as "at most 1 MiB". Plan S11 requires a "reasoned exclusion" and "no false complete pointer". REQ-005 makes only truncated tree, unindexed *eligible* file, or exhausted bound incomplete. The task's Scenario 2 wording is narrower, but the spec controls. An archive entry over the bound is refetched, not benignly skipped (`same_tree_serializes_identically_across_ingest_paths`). |
| Scenario 2: parse failure or exhausted bound never advances the pointer; exact omissions reported | `IncompleteReason::UnindexedFiles{count}`, `Coverage.skipped` with blob and reason | incomplete test: ParseFailed, NotFetched, count, prior pointer unchanged | PASS |
| Scenario 2: object-write crash case; prior pointer kept; whole-build retry | `publish` order: create, read-back, record, CAS pointer | `publication_is_crash_safe_and_retry_converges` (create failure, publish failure, retry) | PASS for pointer safety. The record gap is TASK-A-1. |
| REQ-006: key scoped by identity, extraction/format version and tree; duplicate reused only after validation | `snapshot_key`; the adopt path compares bytes | same-tree determinism test | **FAIL**: TASK-A-2. Snapshot content depends on `max_file_bytes`, which the key does not include. |
| REQ-006 / Decision 1: collection of expired unreferenced snapshots and namespace removal | `record_snapshot`, `unreferenced_snapshots`, `delete_snapshot`, `collect_snapshots`, `remove_repository` | collection test | Contract shape PASS: REQ-015 explicitly assigns "expired-snapshot cleanup" to the snapshot store, and the cross-identity listing matches the approved `ContextStore::expired_contexts` sibling. Behavior **FAIL**: TASK-A-1. An object written before its record is never collectable or returned. |
| Decision 3: adopt/collect race left with a `ponytail:` note | snapshot.rs 268–271; `run` rebuilds when the pointed-to object is `NotFound` (line 455) | collection test: lost object is rebuilt at the same key | PASS with observation. The window (collector lists an old record, a build adopts that same tree, then the object is deleted) briefly leaves a pointer to a missing object, contrary to INV-003's wording. The spec requires no concurrent-collection guarantee, and the next cold build self-heals. Closing it needs store-side coordination that the spec does not ask for. I do not raise it as a finding. The change review should know about it. |
| S14 / AC-005 / INV-007: cross-identity read/advance rejected; missing identity fails closed | `read_snapshot` checks the `ForeignKey` pre-read; memory store keyed by `SnapshotScope`; `is_snapshot_of` guards collection | `identities_cannot_read_or_advance_each_others_snapshots`; forged-record case in the collection test | PASS |
| AC-009: snapshot store separate from context store, in-memory | `MemorySnapshotStore` only implements `SnapshotStore` | tests | PASS |
| INV-005 / REQ-014: no archive, checkout or raw blob cache persisted | bytes are dropped per entry; snapshot JSON holds facts only | source-text absence assertion | PASS |
| Non-goal: no storage backend, no deadline maximum, no caller scheduling | `MAX_COLD_BUILD_DEADLINE` removed; `ColdBuildLimits` has no deadline | 24 h execution publishes | PASS |
| Stop condition: coverage distinguishes excluded from missing eligible | `Coverage.excluded` by reason vs `skipped` with `is_benign` | tests | PASS |
| Rustdoc/errors rules | new items documented; `# Errors` present | lints clean | PASS |

## Findings

### TASK-A-1 (INCORRECT): an object written before its record escapes collection and namespace removal

- **Root cause.** `ColdBuild::publish` (snapshot.rs:550–560) creates the immutable object first and only calls `record_snapshot` after the read-back succeeds. Any failure or crash between those points leaves an object at a snapshot key with no record. Collection and `remove_repository` enumerate records only, so they cannot see it.
- **Defect class.** Durable write ordered before its tracking record (orphan-able resource).
- **Does the owner's shape permit it?** Yes. The trait already has an idempotent `delete_snapshot`, and `storage.delete` is idempotent. Recording before `create` is possible with the current signatures, and a record without an object collects harmlessly.
- **Sibling locations** (same root):
  - snapshot.rs:554, the `Corrupt` read-back return, which also exits before recording.
  - persistence.rs:164–166, a doc claim that the resulting behavior contradicts.
- **Violated obligations.**
  - REQ-006: "The library exposes collection of expired unreferenced snapshots and removal of a repository namespace".
  - Task Scenario 3: "Collection removes expired unreferenced snapshots … repository namespace removal returns its object keys for deletion".
  - Task Scenario 3: "interruption after object write is retry-safe".
  - `record_snapshot` doc: "an object orphaned by an interrupted build is still collectable".
- **Evidence.** Scratch reproduction on the exported HEAD. A `SnapshotStore` wrapper fails `record_snapshot` once. `ColdBuild::run` then returns `Err(Persistence)` and storage holds 1 key. `collect_snapshots(.., now+1h, 10)` returns `[]`, `remove_repository(identity)` returns `[]`, and the object is still stored. The test `scratch_record_failure_leaks_untracked_object` passed.
- **Observable consequence.** A persistence blip or process kill between `create` and `record_snapshot`, followed by the default branch advancing before a retry, leaves that tenant's snapshot object in storage forever. It also survives repository namespace removal, which is the tenant/repository offboarding path.
- **Testable correction.** Call `record_snapshot` before `storage.create` in `publish`. Keep the read-back and pointer order. Add a record-failure switch to `MemorySnapshotStore`, then assert:
  1. A record failure leaves no object.
  2. A failure after the record leaves an object whose key both `collect_snapshots` (after expiry) and `remove_repository` return.

### TASK-A-2 (INCORRECT): snapshot bytes depend on a caller knob that is not part of the immutable key

- **Root cause.** `ColdBuildLimits::max_file_bytes` (snapshot.rs:44) is a public caller-supplied value. It feeds eligibility (`Ingest::new` → `classify`, eligibility.rs:283) and the benign `Oversize` skip, so it changes `Snapshot.files`, `facts` and `coverage`. `snapshot_key` (snapshot.rs:298) keys only on identity, extraction version, format version and tree. The spec defines eligibility as "initially at most 1 MiB" and never asks for this knob.
- **Defect class.** Content-addressed or immutable object whose content depends on an input missing from its key.
- **Does the owner's shape permit it?** Yes. The facade exposes the knob publicly, and neither `publish` nor the reuse path checks it.
- **Sibling locations:**
  - `Ingest::stream_entries` size gate (line 667).
  - `Ingest::fallback` Oversize branch (line 702).
  - The reuse path at line 446, which returns a snapshot built under a different bound as `Reused`.
- **Violated obligations.**
  - REQ-006: "Snapshots are scoped by caller tenant, installation, repository, extraction version, format version and tree identity. A duplicate key is reused only after identity and content validation."
  - Glossary "Eligible file: … initially at most 1 MiB".
  - The module's own claim that "the same identity, versions, and tree always serialize to the same bytes" (snapshot.rs:149–151).
- **Evidence.** Scratch reproduction. The first build uses default limits and fails after the object is written (`fail_next_publish`). A retry of the same tree with `max_file_bytes: 100` returns `Err(ColdBuildError::Corrupt { .. })` ("stored object differs from the built snapshot"). The test `scratch_limit_changes_bytes_at_same_key` passed.
- **Observable consequence.** Two workers or deployments configured with different limits, for example during a config rollout, produce different bytes for the same immutable key. One of them fails with `Corrupt` for that tree until collection runs. Otherwise a pointer silently serves a snapshot whose eligibility differs from what the caller configured.
- **Testable correction.** Remove `max_file_bytes` from `ColdBuildLimits` and use `MAX_ELIGIBLE_FILE_BYTES` directly. This is Ponytail rung 1: the spec fixes the bound and no task needs the knob. Add a test that builds the same tree twice, with different values of the remaining limits, and gets identical bytes.

```citations
[
  {"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":550,"snippet":"        match self.storage.create(&key, &bytes) {","symbols":["publish","create"]},
  {"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":554,"snippet":"        if self.storage.read(&key)? != bytes {","symbols":["publish"]},
  {"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":560,"snippet":"        self.store.record_snapshot(identity, &key, now)?;","symbols":["publish","record_snapshot"]},
  {"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/persistence.rs","line":164,"snippet":"    /// The library records an object after it validates and before any","symbols":["record_snapshot"]},
  {"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":44,"snippet":"    pub max_file_bytes: u64,","symbols":["ColdBuildLimits"]},
  {"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":298,"snippet":"fn snapshot_key(identity: &Identity, tree: &ObjectSha) -> ObjectKey {","symbols":["snapshot_key"]},
  {"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":484,"snippet":"        let mut ingest = Ingest::new(&listing.entries, &attributes, self.limits.max_file_bytes);","symbols":["run","Ingest"]},
  {"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":667,"snippet":"            if entry.header().size()? > self.max_file_bytes {","symbols":["stream_entries"]},
  {"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":702,"snippet":"                Ok(bytes) if bytes.len() as u64 > self.max_file_bytes => {","symbols":["fallback"]},
  {"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/eligibility.rs","line":283,"snippet":"    if entry.size.is_some_and(|size| size > max_file_bytes) {","symbols":["exclusion","classify"]}
]
```
