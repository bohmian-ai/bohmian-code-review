# Domain review: snapshot lifecycle and durability

**Subject:** base `97fff9f`, candidate `da81a598c10441b3b2e1231ce674d89003017f26` (TASK-010; approved spec revision 14 at `63d6bd0`).

**Reviewed boundary:** `Update::run` from pointer read and head pinning through compare, changed-path classification, carry, blob ingest, completeness check, and pointer publication. I also traced the cold rebuild route, `Snapshots::read`, `SnapshotStore`'s pointer and record contract, and the update lifecycle tests. The authority is `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, spec REQ-006/007/012/013, INV-003/004, AC-003, TASK-010, and the maintainer's stateless-snapshot decision.

| Lifecycle obligation | Source and proof | Result |
|---|---|---|
| Prior facts change only by compare path; no base listing or derived state is needed | `update.rs:100-140`, `snapshot.rs:247-264,955-995`; changed paths are classified at the head, removed paths are not carried, and persisted snapshot data is per-path outcomes only. | PASS |
| Diverged history, root attributes, and excessive compare changes rebuild | `update.rs:104-115`, `snapshot.rs:646-677`; `rebuild_triggers_use_the_archive` and `changes_past_the_ceiling_rebuild_from_the_archive` pass. | PASS |
| Missing or truncated head listings cannot produce a complete publication | `update.rs:117-127,231-252`; a missing changed path errors, and any truncated listing returns an incomplete result before ingest or publication. | PASS |
| Failed changed blobs and other incomplete attempts leave the old pointer | `snapshot.rs:1122-1151,742-787`; completeness is checked before `publish`; `failed_changed_blob_keeps_the_pointer_until_a_retry` passes. | PASS |
| Cancellation and racing publishers preserve a valid pointer | `snapshot.rs:802-840`, `persistence.rs:150-225`; record upsert precedes object creation and pointer CAS; `dropped_update_keeps_the_pointer_and_retry_converges` and `moving_head_is_reported_stale_and_updates_converge` pass. | PASS |
| An active review retains its immutable loaded snapshot | `Snapshots::read` validates exact key and digest (`snapshot.rs:344-375`); publication creates a new tree-keyed object and swaps the pointer rather than altering the old object (`snapshot.rs:802-840`). | PASS |

**Verification:** I ran `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib update`: 8 passed, 0 failed. I also ran `git diff --check 97fff9f da81a59`: clean. The tests use an injected fake provider and in-memory persistence; this review did not exercise live GitHub, a real storage adapter, or a consuming service's clock/transaction implementation. Those are verification limits, not observed defects in the reviewed library path.

**Findings:** None. The lifecycle owner and its callers were swept for completeness and publication sinks: the only pointer write is `ColdBuild::publish`, reached after `ColdBuild::complete` rejects incomplete coverage; the cold rebuild path uses the same gate. No sibling missing-check site was found.

**Overall:** PASS for the snapshot lifecycle and durability domain.

```citations
[]
```
