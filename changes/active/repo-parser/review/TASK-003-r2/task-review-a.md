# TASK-003 r2 — task review A

**Result: PASS.** Reviewed immutable base `60cc56e` and candidate `a10c874ff346d063714be75b46b6ec2c34b990b5` against approved spec revision 11, the original TASK-003, the r1 `FIND-*` ledger, and TASK-003-R1. The subject includes the complete candidate `snapshot.rs`, `eligibility.rs`, and `testing.rs`, plus the base-to-candidate diff. Candidate HEAD matched the requested commit during inspection.

| Requirement, criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| S1 / REQ-005: pinned, eligible, blob-verified cold build with bounded archive fallback | `snapshot.rs:438-550`, `Ingest` at `snapshot.rs:611-826`; `eligibility.rs:235-301` | `complete_cold_snapshot_is_verified_source_free_and_reloadable`; `incomplete_attempts_report_coverage_and_keep_the_prior_pointer`; `mise run test:pipeline` exit 0 | PASS |
| S1 / REQ-006 / INV-003: complete source-free immutable object, validated before pointer publication, reload without source fetches | `snapshot.rs:149-223`, `snapshot.rs:555-605`; exact-key read at `snapshot.rs:248-270` | complete cold snapshot, deterministic-bytes, publication retry, and isolation owner tests; pipeline lane exit 0 | PASS |
| S10–S11 / INV-002: incomplete and truncated attempts expose coverage, retry reason, and preserve prior pointer | `snapshot.rs:481-550`, `snapshot.rs:806-826`; `Coverage` at `snapshot.rs:108-139` | incomplete-attempt owner test; pipeline lane exit 0 | PASS |
| FIND-TASK-003-1 / R1: every written object has a tracking row, including failed create, read-back, and pointer publication | `ColdBuild::publish` records before create at `snapshot.rs:581-604`; record contract `persistence.rs:165-183` | `every_interrupted_publication_leaves_a_tracked_key`; pipeline lane exit 0 | PASS |
| FIND-TASK-003-2 / R1: archive cancellation and deadline stop publication, leaving exact unfinished coverage | archive checkpoint `snapshot.rs:696-706`; fallback retains interruption `snapshot.rs:740-778`; final checkpoint `snapshot.rs:512-521` | `interruption_during_a_complete_archive_never_publishes`; `interruption_is_explicit_and_deadlines_are_caller_owned`; pipeline lane exit 0 | PASS |
| FIND-TASK-003-3 / R1: rejected tree entries counted separately by reason without increasing eligibility | `listing.rejected.len()` passed at `snapshot.rs:494-500`; `InvalidEntry` at `eligibility.rs:32-52`; count at `snapshot.rs:810-820` | `rejected_tree_entries_are_counted_as_invalid_exclusions`; pipeline lane exit 0 | PASS |
| S14 / AC-005 / AC-009 / INV-007: identity-scoped reads and pointers, separate injected snapshot store, missing identity fails closed | exact key enforced before read at `snapshot.rs:248-270`; identity-scoped `testing.rs:176-310` | `identities_cannot_read_or_advance_each_others_snapshots`; pipeline lane exit 0 | PASS |
| REQ-006 / R1: expired unreferenced collection and repository namespace removal retain tracked keys | `Snapshots::collect` at `snapshot.rs:274-304`; snapshot-store contract `persistence.rs:185-220`; in-memory implementation `testing.rs:241-310` | `collection_removes_only_expired_unreferenced_snapshots`; failed-publication test; pipeline lane exit 0 | PASS |
| FIND-TASK-003-4 / R1: stateful snapshot read and cleanup belong to concrete owner | `Snapshots` at `snapshot.rs:229-305`; reuse at `snapshot.rs:459`; `lib.rs` re-export | Source and base-to-candidate diff; pipeline lane exit 0 | PASS |
| FIND-TASK-003-5 / R1: changed signatures use imported bare types | `stream_entries<R: Read>(&mut self, expanded: R, execution: &Execution)` at `snapshot.rs:696` | Source and base-to-candidate diff; pipeline lane exit 0 | PASS |
| Task bounds and non-goals: caller-owned deadline, no new storage backend, source cache, scoped PR implementation, or new dependency | `ColdBuild::run` uses caller `Execution`; source held per entry in `snapshot.rs:696-734`; manifest and diff | cancellation/deadline owner tests; diff inspection | PASS |

No proposed blocking findings. The prior r1 `FIND-*` set is closed on this candidate. The pre-existing collection/adoption race and configurable file-size ceiling remain the r1 verdict's nonblocking notes; neither is a new task-contract failure.

```citations
[]
```
