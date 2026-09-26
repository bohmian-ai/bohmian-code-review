# TASK-003 r3 — independent task implementation review A

**Subject:** base `60cc56e4213aa90f2a13d3ac6b3d1de536ff81b2`, candidate `d95346349988fed1d79dcb6e8b532b185e05b563`, spec revision 11. TASK-003 began in `741c049`, before this base. I inspected the complete candidate `snapshot.rs`, `eligibility.rs`, and `testing.rs`, plus the complete base-to-candidate diff, rather than treating only the r2 edit as the subject. `.codegraph/` is absent.

| Requirement, criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| S1 / REQ-005: pinned default tree; eligible files from verified archive or bounded blob fallback; source-free snapshot reload | `ColdBuild::run`, `Ingest::{consume_archive,stream_entries,fallback,ingest}`, `Snapshot::into_context` | `complete_cold_snapshot_is_verified_source_free_and_reloadable`; `mise run test:pipeline` | PASS |
| S10–S11 / REQ-005: incomplete, truncated, mismatched, failed, or bounded attempts retain the prior pointer and report coverage and retry reason | `Coverage`, `IncompleteReason`, `Ingest::finish`, `ColdBuild::run` | `incomplete_attempts_report_coverage_and_keep_the_prior_pointer`; `rejected_tree_entries_are_counted_as_invalid_exclusions` | PASS |
| R1 FIND-1 / REQ-006: every written object is tracked, object validates before pointer publication, retry and collection work | `ColdBuild::publish` records before create, compares read-back bytes, then conditionally publishes; `Snapshots::collect` | `every_interrupted_publication_leaves_a_tracked_key`; `publication_is_crash_safe_and_retry_converges` | PASS |
| R1 FIND-2 and R2 FIND-2 / REQ-005, REQ-013: cancellation and deadline stop archive reads, including selected entry and unselected tar drain; pending paths remain `NotFetched` | `Interruptible::read` checks `Execution`; `Ingest::consume_archive` wraps compressed and expanded reads; `fallback` preserves interruption; `run` checks again after fallback | Exact `cancellation_mid_archive_entry_stops_reading_the_body` and `expired_deadline_stops_archive_ingest` selectors each selected and passed one test; `interruption_during_a_complete_archive_never_publishes` | PASS |
| R1 FIND-3 / eligibility and exact coverage: rejected tree entries count separately from eligible paths; root attributes and supported modes apply | `ExclusionReason::InvalidEntry`, `Ingest::new/finish`; `eligibility::classify` | `rejected_tree_entries_are_counted_as_invalid_exclusions`; `classification_reports_each_reason`; `attributes_resolve_like_git` | PASS |
| S14 / AC-009: identity-scoped read, pointer, cleanup and namespace removal | `Snapshots::read/collect`; `SnapshotStore` scoped methods; `MemorySnapshotStore` | `identities_cannot_read_or_advance_each_others_snapshots`; `collection_removes_only_expired_unreferenced_snapshots` | PASS |
| R1 FIND-4/5: stateful IO on a concrete owner; imported bare types in changed signatures | `Snapshots` methods; `Ingest::stream_entries<R: Read>` | Source and diff inspection | PASS |
| R2 FIND-6: deadline proof uses no sleep or real-time polling | `expired_deadline_stops_archive_ingest` constructs an already-expired `Execution`; no `sleep` in `snapshot.rs` | Exact selector passed one test in 0.00 s; source inspection | PASS |
| Non-goals: no checkout, durable source/archive/blob cache, backend, scoped PR indexing, incremental update, new dependency, or library-wide deadline maximum | Source, manifests, base-to-candidate diff | `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `git diff --check 60cc56e d953463` exited 0 | PASS |

**Prior FIND ledger:** R1 FIND-TASK-003-1, -3, -4, and -5 remain closed. R1/R2 FIND-TASK-003-2 is closed by checks at both archive read boundaries and focused cancellation/deadline tests. R2 FIND-TASK-003-6 is closed by the deterministic owner test. The previously noted collection/adoption race remains outside this task's consumer scheduling contract; I found no new blocking task defect.

**Verification limit:** The tests use an in-memory routed transport and stores, so no live GitHub stream or durable backend was exercised here. A transport read already blocked when cancellation arrives waits for its transport timeout; the implementation stops subsequent reads. Full `mise run gate` was not rerun by this reviewer; the scoped pipeline and symbolic journey lanes passed.

**Overall result: PASS.** Proposed findings: none.

```citations
[]
```
