# Domain review: context durability and store clock

Subject: base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`, candidate `866f777a1a41a03384c14c12276a5877dcac94a2`. Reviewed the cumulative TASK-005 diff, approved spec revision 13, original task, R1–R4 remediation and findings, and both maintainer decisions. The R4 clock decision overrides the R4 remediation shape and withdraws FIND-TASK-005-7; I do not reopen that claim.

| Boundary and authority | Source and verification coverage | Result |
|---|---|---|
| Store-owned durable clock: spec REQ-015, revision 13, R4 maintainer decision | `SnapshotStore::{upsert_record,unreferenced}` and `ContextStore::{upsert_context,context,expired_contexts}` carry no host-time input. `MemorySnapshotStore` and `MemoryContextStore` stamp or judge time using their manually advanced store clock. `Snapshots::collect` takes a grace duration; `Contexts::{publish,read,collect_expired}` take no timestamp. The remaining production `SystemTime::now()` in `github.rs` computes a rate-limit timer, as expressly allowed by the decision. | PASS |
| Context publication, retry and shared read: task S14–S15, REQ-011–012, REQ-014–016, INV-005/007 | `PrContext::run` publishes only after building selected spans. `Contexts::publish` upserts a tenant-scoped record before object creation and returns only after the create succeeds. `Contexts::read` obtains an exact identity/id lookup, verifies the expected key, object digest, format, object identity and pinned PR pair; an absent object is unreadable. Fresh random-suffixed IDs give separate records and lifetimes for repeated equal content. `pull_request::tests::dropped_publication_leaves_no_readable_half_context` and `context::tests::{reads_are_identity_scoped_and_expire,absent_corrupt_and_foreign_objects_fail,identical_publications_expire_and_clean_up_independently}` exercise the seams. | PASS |
| Expiry and cleanup: REQ-014–015, INV-005, R4 maintainer decision | The store decides live/expired at lookup and lists records at or beyond expiry. `Contexts::collect_expired` deletes each exact identity-scoped object before its record, treats a missing object as success, and reports failed deletes as lag. The in-memory store retains failed rows for a retry. `context::tests::expired_cleanup_deletes_object_then_record_and_reports_lag` covers this. FIND-TASK-005-7's prior collection-cutoff claim is expressly withdrawn. | PASS |
| Snapshot durability interaction: REQ-012–013/015 | Snapshot records are stamped by the store before object creation. Its `unreferenced` selection claims rows against concurrent upsert/pointer publication; `Snapshots::collect` deletes object then record; `remove_repository` uses zero grace. PR publication reads an immutable snapshot into a private overlay and does not move a snapshot pointer. | PASS |

## Findings and limits

No material proposed finding in this domain. I ran `mise run test:pipeline` on the candidate: exit 0. This domain review did not exercise a production SQL or object-storage adapter, or the external live PR test; those adapters are outside this library task. The candidate is immutable for this review. Overall: **PASS** for the reviewed durability boundary.

```citations
[]
```
