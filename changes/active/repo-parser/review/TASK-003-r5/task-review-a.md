# TASK-003 r5 — task implementation review A

Subject: base `c74871cbc78140820c9b0a5fda60e0952c4265e4`, candidate `3ada5c59931f236716ed027d773a2746ccf3c3af` (HEAD matched). Authority: approved spec revision 12, original TASK-003, R4 remediation, `AGENTS.md`, architecture design and applicable references. The complete cumulative diff was inspected; r1–r3 concern the superseded synchronous design. `.codegraph/` is absent.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-005, S1/S11: verified streamed archive, bounded fallback, source-free deterministic snapshot and warm reload | `archive.rs` `TarStream`; `snapshot.rs` `Ingest`, `ColdBuild::run`, `Snapshot`, `Snapshots::read` | `complete_cold_snapshot_is_verified_source_free_and_reloadable`, `same_tree_serializes_identically_across_ingest_paths`, archive owner tests; focused snapshot lane passed (15/15) | PASS |
| FIND-TASK-003-8: many wanted files in one compressed chunk do not accumulate as a source set | `Ingest::decompress` drains `TarStream::verified` after each decoder write; `peak_verified_bytes` records the waiting batch | `one_chunk_archive_keeps_verified_bytes_bounded`; focused snapshot lane passed | PASS |
| REQ-005/013, Scenario 2 and FIND-TASK-003-11: incomplete attempts retain exact coverage and prior pointer, including compressed and expanded archive bounds | `ColdBuild::run` carries `archive_too_large` to `Incomplete(ArchiveTooLarge)` after coverage accounting, before publication | `incomplete_attempts_report_coverage_and_keep_the_prior_pointer` covers truncated tree, parse/fallback failure, both archive bounds and full fallback coverage; focused lane passed | PASS |
| REQ-005, INV-002/003 and FIND-TASK-003-10: provider blob ceiling does not mark an unverified eligible file benignly oversize | `Ingest::fallback` treats `ResponseTooLarge` as `Oversize` only when its `limit >= max_file_bytes`; smaller limits yield `FetchFailed` | `blob_ceiling_below_file_bound_is_not_oversize`, `verified_oversize_blob_is_a_benign_skip`; focused lane passed | PASS |
| REQ-006/012/013, Scenario 3: record → object → pointer, duplicate-byte validation, safe drop and retry, typed lost CAS | `ColdBuild::publish`; `SnapshotStore` and object-storage contracts | `dropped_publication_keeps_pointer_and_retry_converges`, `existing_object_with_other_bytes_is_refused`, `lost_pointer_swap_returns_retry_state`; focused lane passed | PASS |
| REQ-006/015/016, Scenario 4 and FIND-TASK-003-9: collect only stale unreferenced records; atomic claim coordinates with re-touch/publication; delete object before record; identity isolation | `SnapshotStore::unreferenced` claim contract, `MemorySnapshotStore` locked claim/upsert, `Snapshots::collect` and `remove_repository` | `collection_claim_and_publication_never_race`, `collect_removes_only_stale_unreferenced_records`, `identities_cannot_read_or_advance_each_others_snapshots`, `collect_refuses_foreign_keys`; focused lane passed | PASS |
| Scenario 5: `!attr` restores unspecified and `-attr` sets false | `eligibility.rs` tri-state attribute rule and resolver | `attribute_resets_read_as_unset`; candidate test source inspected | PASS |
| No detached spawn, persisted source/archive/blob, storage backend, scoped PR indexing, incremental update, scheduler, or library deadline | Cumulative production diff confines behavior to cold snapshot, eligibility and injected contracts | Diff inspection; `rg -n 'tokio::spawn' crates/bohmian-code-review-pipeline/src` found no call | PASS |

The focused command was `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib snapshot::tests:: -- --nocapture`: 15 passed, 0 failed. The candidate's task evidence records `test:pipeline`, public symbolic journey, `fmt:check`, `lints`, `gate`, and `git diff --check` as passing; I did not independently rerun those broader lanes. No blocker survived the source-path and test checks. No production file was edited.

**Overall: PASS.** Proposed findings: none.

```citations
[]
```
