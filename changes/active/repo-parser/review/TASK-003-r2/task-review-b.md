# TASK-003 r2 — independent task implementation review B

**Result: PASS.** Subject: approved `changes/active/repo-parser/spec.md` revision 11; original TASK-003 and TASK-003-R1; base `60cc56e`, candidate `a10c874ff346d063714be75b46b6ec2c34b990b5`. Because TASK-003 began before the base, I reviewed the complete candidate `snapshot.rs`, `eligibility.rs`, and `testing.rs`, plus the base-to-candidate diff and its `lib.rs`, `persistence.rs`, and `storage.rs` seams.

| Requirement, criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Complete cold build pins default branch and verified tree, streams archive, verifies blob bytes, indexes through the public facade, and reloads source-free facts (REQ-005, Scenario 1) | `ColdBuild::run` (snapshot.rs:443–557), `Ingest::stream_entries` (696–734), `Snapshot::into_context` (216–222) | `complete_cold_snapshot_is_verified_source_free_and_reloadable`; focused snapshot lane: 10 passed | PASS |
| Immutable tree-keyed bytes and object-before-pointer publication; existing key adopted only after byte comparison (REQ-006, Scenario 1) | `Snapshot` (snapshot.rs:153–168), `ColdBuild::publish` (569–608) | `same_tree_serializes_identically_across_ingest_paths`, `publication_is_crash_safe_and_retry_converges` | PASS |
| Interrupted publication leaves a tracked key for collection or namespace removal (R1 FIND-TASK-003-1, Scenario 3) | `record_snapshot` precedes `storage.create` (snapshot.rs:581–587); `Snapshots::collect` deletes object before record (293–309); trait contract at persistence.rs:161–177 | `every_interrupted_publication_leaves_a_tracked_key` injects create, read, and pointer-publish failures | PASS |
| Cancellation and caller deadline stop archive ingest and prevent publication; no library deadline maximum (R1 FIND-TASK-003-2, REQ-005/013) | `Ingest::stream_entries` checks execution at each entry (snapshot.rs:696–703), `fallback` retains interruption (740–775), `run` checks before completeness (518–533) | `interruption_during_a_complete_archive_never_publishes`, `interruption_is_explicit_and_deadlines_are_caller_owned` | PASS |
| Exact incomplete outcome and coverage; prior pointer remains; bounded fallback (Scenario 2, INV-002) | `Coverage` and `IncompleteReason` (snapshot.rs:110–137, 319–329); `run` (479–540); `Ingest::fallback` (740–775) | `incomplete_attempts_report_coverage_and_keep_the_prior_pointer` | PASS |
| Tree-listing validation failures are reasoned exclusions, separate from eligible counts (R1 FIND-TASK-003-3) | `ExclusionReason::InvalidEntry` (eligibility.rs:46–47); rejected count enters `Ingest::new` (snapshot.rs:503–508) and `finish` (807–823) | `rejected_tree_entries_are_counted_as_invalid_exclusions` covers invalid path and mode | PASS |
| LFS, oversize, unsupported, symlink, submodule, generated/vendor/minified rules are reasoned; no false complete pointer for unindexed eligible files (REQ-005, Scenario 2) | `eligibility::classify` (eligibility.rs:230–289); `SkipReason::is_benign` (snapshot.rs:83–90), ingest (780–797) | `classification_reports_each_reason`, `incomplete_attempts_report_coverage_and_keep_the_prior_pointer` | PASS |
| Identity-scoped read and pointer, expired unreferenced collection, namespace removal (REQ-006/015, Scenario 3, AC-009) | `Snapshots::read/collect` (snapshot.rs:252–310); `MemorySnapshotStore` (testing.rs:173–314) | `identities_cannot_read_or_advance_each_others_snapshots`, `collection_removes_only_expired_unreferenced_snapshots` | PASS |
| Stateful storage workflows reside on a concrete owner; signatures use imported bare types (R1 FIND-TASK-003-4/5) | `Snapshots` with inherent `read/collect` (snapshot.rs:227–310); `stream_entries<R: Read>` (696) | Diff inspection; focused snapshot lane | PASS |
| No storage backend, checkout, persisted source/archive, library deadline maximum, or TASK-004/005 PR/update work | Pipeline manifest, `ColdBuild::run`, `Snapshot`, base-to-candidate diff | Source and manifest inspection | PASS |

**Ponytail check:** The candidate removes the fixed deadline and public stateless wrappers, reuses the existing `Execution`, `Coverage`, storage and persistence traits, and adds no dependency. `Snapshots` is justified by the repository's concrete workflow-owner rule. The retained `max_file_bytes` knob and collection/adopt race are r1 notes outside the required correction; neither is a new blocking finding.

**Verification:** I ran `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib snapshot -- --nocapture`: 10 passed, 0 failed. Prior candidate evidence records `mise run test:pipeline`, `test:symbolic-context:journey`, `fmt:check`, `lints`, and `gate` passing. No independent full gate rerun in this sample.

**Proposed findings:** None.

```citations
[]
```
