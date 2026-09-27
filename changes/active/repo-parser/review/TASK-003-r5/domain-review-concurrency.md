# TASK-003 r5 — concurrency and durability domain review

Subject: base `c74871cbc78140820c9b0a5fda60e0952c4265e4`, candidate `3ada5c59931f236716ed027d773a2746ccf3c3af`. Reviewed approved spec revision 12, original TASK-003, R4 remediation, cumulative diff, repository rules, and the publication/collection source and tests. Earlier r1–r3 covered the superseded synchronous design.

## Boundary and evidence

| Boundary | Authority and source coverage | Result |
|---|---|---|
| Durable publication | Spec REQ-006/012/013; TASK-003 Scenario 3; `ColdBuild::run` and `publish`, `SnapshotStore`, `ObjectStorage`, and in-memory implementations | PASS: record upsert precedes create-only object write; byte equality is checked before adoption; the tree CAS follows a complete validated snapshot. Dropping the future between steps leaves a record for any created object and an unchanged pointer. |
| Collection racing publication | Spec REQ-006; TASK-003 Scenario 4; R4 FIND-9; `Snapshots::collect`, `SnapshotStore::unreferenced`/`upsert_record`, `MemorySnapshotStore`, and race tests | PASS under the approved caller cutoff: selection and claim share the record/pointer critical section, an existing claim rejects a builder's upsert, and a refreshed record is too recent to claim. Object deletion precedes record deletion; a dropped or failed collection can reclaim the row on retry. |
| Namespace and read-back | Spec REQ-006/015/016 and AC-009; `Snapshots::read`/`remove_repository`, key validation, pointer/store methods, in-memory isolation tests | PASS for this boundary: read-back derives the key from the caller identity and tree, verifies digest and decoded identity, and collection rejects a returned key outside the identity's snapshot namespace. Repository removal documents its no-active-build precondition. |

The focused command `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib snapshot::tests::collection_claim_and_publication_never_race -- --exact` passed (1 test). I traced the collector-first and builder-first interleavings through the public facades and the in-memory store. The test covers the R4 claim repair; it does not exercise a consumer's durable SQL/object-store adapter, which is outside this library's implementation and is a verification limit. Correctness also depends on the spec's caller rule that `older_than` exceed the build timeout and on nonoverlapping `collect` calls for one identity. The library provides no deadline by design, so this domain review cannot verify a consumer's timeout/cutoff policy.

## Material findings

None. The R4 collection claim closes the prior list-then-delete race on the reviewed boundary. The candidate's claim mechanism and the existing three-step publication are the smallest relevant owners; I found no new in-scope durability defect requiring correction.

Overall: **PASS** for the concurrency and durability domain.

```citations
[]
```
