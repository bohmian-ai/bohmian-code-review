# TASK-003 r2 — publication, isolation, and cleanup domain review

**Result: PASS.** No material findings in this domain.

## Reviewed boundary

Immutable base `60cc56e4213aa90f2a13d3ac6b3d1de536ff81b2`; candidate `a10c874ff346d063714be75b46b6ec2c34b990b5`. I reviewed the complete candidate `snapshot.rs`, `eligibility.rs`, and `testing.rs`, plus the base-to-candidate diff, for the cold snapshot's object and metadata publication, conditional pointer update, exact-key read, tenant/installation isolation, interruption recovery, expiry collection, and namespace removal. No subject file was edited.

## Authority and source coverage

The approved `changes/active/repo-parser/spec.md` revision 11 governs REQ-006, REQ-014, REQ-015, INV-003, and AC-009; the original TASK-003 Scenarios 1 and 3 and `TASK-003-R1-publication-order-cancellation-coverage.md` govern this task's failure windows. `AGENTS.md`, `architecture/agent-rules.md`, and `architecture/references/languages/spec-driven-development.md` govern storage ownership and review. I traced `ColdBuild::run` and `publish` (`snapshot.rs:443–607`), `Snapshots::read` and `collect` (`snapshot.rs:225–309`), `Snapshot::decode` (`snapshot.rs:174–209`), the `SnapshotStore` contract (`persistence.rs:134–213`), and the in-memory store/storage implementations (`testing.rs:89–312`). I checked the focused tests at `snapshot.rs:1440–1524,1581–1855` and the base-to-candidate diff.

## Assessment

| Obligation | Evidence | Result |
|---|---|---|
| `FIND-TASK-003-1`: every written object is tracked across create, read-back, and pointer-publication failures | `publish` records the key before `create`, byte-compares the stored object, then conditionally publishes the pointer (`snapshot.rs:582–605`). `every_interrupted_publication_leaves_a_tracked_key` covers all three failures, collection, and namespace removal. | Closed |
| Pointer names only validated complete facts for its identity and tree | `Coverage::is_complete` gates `publish` (`snapshot.rs:534–552`); `Snapshots::read` checks the exact derived key and digest before `Snapshot::decode` checks format, extraction version, identity, tree, and completeness (`snapshot.rs:255–269,174–209`). | PASS |
| Existing key adoption and pointer retry do not overwrite another object or pointer | `ObjectStorage::create` is create-only; `publish` requires byte equality and uses the observed pointer tree as the conditional expected value (`snapshot.rs:583–605`). | PASS |
| Cleanup preserves pointed objects and retries partial deletion | `unreferenced_snapshots` excludes keys held by a pointer; `Snapshots::collect` checks the key's identity, deletes the object before the record, and retains the record if object deletion fails (`snapshot.rs:293–309`, `testing.rs:258–299`). | PASS |
| Tenant and installation isolation | Snapshot keys derive from the validated identity; read rejects a foreign key before storage access (`snapshot.rs:255–264`); in-memory pointer and record maps are scoped by identity (`testing.rs:183–312`); `identities_cannot_read_or_advance_each_others_snapshots` exercises both dimensions. | PASS |
| `FIND-TASK-003-2`: cancelled or expired archive work cannot publish | `Ingest::stream_entries` checks execution per entry and `run` checks after fallback before the completeness decision (`snapshot.rs:518–533,696–702`); `interruption_during_a_complete_archive_never_publishes` exercises both cancellation and deadline with a prior pointer. | Closed for this boundary |

The prior r1 collection/adoption race remains a **note**: concurrent collection may delete an unreferenced object between listing and pointer publication (`snapshot.rs:283–287`). The approved spec leaves scheduling and cross-installation claims with the consumer, and the r1 verdict already classified this as outside the bounded task. It is not a new finding.

## Verification limits

I ran `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib snapshot::tests -- --quiet`: 10 passed, 0 failed. The exact `every_interrupted_publication_leaves_a_tracked_key` selector also passed. These are in-memory implementations; no concrete storage or database adapter exists in this task. The review does not prove atomicity of a future consumer's `SnapshotStore` implementation or concurrent collection safety, which remain consumer obligations.

```citations
[]
```
