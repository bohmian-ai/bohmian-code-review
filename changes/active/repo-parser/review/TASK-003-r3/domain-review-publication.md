# Domain review: snapshot publication, isolation, cleanup, and cancellation

**Subject:** `60cc56e` → `d95346349988fed1d79dcb6e8b532b185e05b563`, including the complete candidate `snapshot.rs`, `eligibility.rs`, and `testing.rs` because TASK-003 began before the base. Approved spec revision 11; original TASK-003 and R1/R2 remediation tasks. **Result: PASS.**

## Boundary and coverage

Reviewed `ColdBuild::run`, archive ingestion and fallback, `ColdBuild::publish`, `Snapshots::{read,collect}`, eligibility classification, object key validation, the snapshot-store and object-storage traits, their in-memory implementations, and the provider's bounded archive/blob paths. Governing obligations: REQ-005/006/012–016, INV-002/003/005/007, TASK-003 Scenarios 1–3, and the R1/R2 FIND-1–6 ledger. The base-to-candidate diff and complete candidate owner files were inspected, including tests.

| Boundary | Evidence | Result |
|---|---|---|
| Verified, bounded ingest and honest incomplete result | `snapshot.rs:482–543,637–873`; pending paths become `NotFetched`, failed blobs stay unindexed, and only complete coverage reaches publication | PASS |
| Archive cancellation and deadline, including selected bodies and tar iterator drain (FIND-2, FIND-6) | `snapshot.rs:607–630,675–738`; `Interruptible` checks before each compressed and expanded read. `cancellation_mid_archive_entry_stops_reading_the_body` and `expired_deadline_stops_archive_ingest` exercise both read sites without sleeps | PASS |
| Object-before-pointer publication and retry tracking (FIND-1) | `snapshot.rs:571–605`; record precedes create, read-back must equal built bytes, then conditional pointer move. Failure-injection test covers create, read, and publish errors | PASS |
| Identity-scoped read and collection (FIND-4) | `snapshot.rs:225–309`, `storage.rs:40–155`, `testing.rs:180–311`; read checks the exact caller-derived key before object access; collection refuses foreign namespace keys | PASS |
| Reasoned excluded and skipped coverage (FIND-3) | `eligibility.rs:38–84`, `snapshot.rs:637–671,837–870`; rejected entries are counted as `InvalidEntry`; `rejected_tree_entries_are_counted_as_invalid_exclusions` passes | PASS |
| Changed Rust signature ownership (FIND-5) | `snapshot.rs:238–309,710`; IO lives on `Snapshots`, and `stream_entries` accepts an imported bare `Read` bound | PASS |

## Verification and limits

Ran `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib snapshot::tests -- --nocapture`: **12 passed, 0 failed**. This validates the focused owner suite; it does not simulate an actual blocked network read. Once a transport read is already in progress, cancellation waits for that read's transport timeout. The candidate explicitly retains that limit, and the R2 task does not require synchronous preemption. Concurrent collection can race a build adopting an object, as marked at `snapshot.rs:283–286`; the R1 remediation task expressly excludes store-side leases and assigns scheduling to the consumer. Neither is a new blocking finding in this review.

## Proposed findings

None.

```citations
[]
```
