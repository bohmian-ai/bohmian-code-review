# TASK-003-r5 — independent task review B

Subject: base `c74871cbc78140820c9b0a5fda60e0952c4265e4`, candidate `3ada5c59931f236716ed027d773a2746ccf3c3af`; approved `SPEC-repo-parser@12`, original TASK-003, and remediation TASK-003-R4. Earlier r1–r3 findings concern the superseded synchronous design. I inspected the complete base-to-candidate diff and the changed cold-build, archive, eligibility, persistence, provider, and in-memory store paths.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-005; TASK-003 S1: verified, bounded cold ingest and source-free reload | `archive.rs:39-245` validates tar entries against listed blob hashes; `snapshot.rs:698-761` drains verified files during decompression; `snapshot.rs:475-496,560-570` reuses or builds serialized facts | `complete_cold_snapshot_is_verified_source_free_and_reloadable`, `one_chunk_archive_keeps_verified_bytes_bounded`, archive owner tests; `mise run test:pipeline` passed | PASS |
| REQ-005, INV-002/003; S2: incomplete attempts preserve pointer and exact coverage | `snapshot.rs:499-558,764-842` records tree truncation, archive bounds, and per-file fallback failures; only complete coverage reaches publication | `incomplete_attempts_report_coverage_and_keep_the_prior_pointer`, `blob_ceiling_below_file_bound_is_not_oversize`, `verified_oversize_blob_is_a_benign_skip`; pipeline lane passed | PASS |
| REQ-006/012/013; S3: drop-safe ordered publication, validated adoption, typed conflict | `snapshot.rs:592-634` performs record upsert, create or byte-identical adoption, then pointer CAS | `dropped_publication_keeps_pointer_and_retry_converges`, `existing_object_with_other_bytes_is_refused`, `lost_pointer_swap_returns_retry_state`; pipeline lane passed | PASS |
| REQ-006/015/016, INV-007, AC-005/009; S4: scoped collection and isolation | `persistence.rs:131-235` specifies atomic record claims; `testing.rs:243-344` implements claim/upsert exclusion; `snapshot.rs:279-334` deletes object before record and rejects foreign keys | `collection_claim_and_publication_never_race` passed in an exact focused run; `collect_removes_only_stale_unreferenced_records`, `collect_refuses_foreign_keys`, `identities_cannot_read_or_advance_each_others_snapshots` passed in pipeline lane | PASS |
| TASK-003 S5: `.gitattributes` reset semantics | `eligibility.rs:80-157` distinguishes explicit false from unspecified; last matching rule wins | `attribute_resets_read_as_unset`; pipeline lane passed | PASS |
| R4 FIND-8: bounded verified-entry handoff | `snapshot.rs:737-761` drains after each decoder write; `archive.rs:218-223` queues only entries verified by SHA | `one_chunk_archive_keeps_verified_bytes_bounded` exercises 128 files in one compressed chunk and asserts a bounded observed peak; pipeline lane passed | PASS |
| R4 FIND-9: collector/build interleavings | `persistence.rs:192-216` requires atomic claim; `testing.rs:267-324` rejects upsert on claimed rows and claims under the pointer and record locks | `collection_claim_and_publication_never_race` exact focused test passed; pipeline lane passed | PASS |
| R4 FIND-10: provider blob ceiling below file ceiling | `snapshot.rs:776-790` treats `ResponseTooLarge` as benign only when the provider bound proves file oversize | `blob_ceiling_below_file_bound_is_not_oversize`; pipeline lane passed | PASS |
| R4 FIND-11: exhausted compressed or expanded archive bound | `snapshot.rs:525-558,698-725` retains archive-bound failure across fallback and returns `ArchiveTooLarge` before publication | `incomplete_attempts_report_coverage_and_keep_the_prior_pointer` covers both bounds and complete fallback; pipeline lane passed | PASS |
| Non-goals: no storage backend, scoped PR index, incremental update, detached worker, or new production dependency | Diff confines production changes to the cold-build owner and supporting seams; `tar` moves to dev-dependencies; no `tokio::spawn` call in pipeline source | Complete diff inspection; `rg -n 'tokio::spawn' crates/bohmian-code-review-pipeline/src` returned no match | PASS |

**Proposed findings:** none. The explicit caller preconditions on `older_than` and nonoverlapping collections are part of the approved contract; the in-memory implementation obeys them. No acceptance failure or scope drift was reproduced.

**Verification limit:** I ran the exact collection race test and `mise run test:pipeline`; I relied on the recorded candidate evidence for the journey, format, lint, and full gate commands rather than rerunning them in this independent sample. The pipeline lane emitted only existing vendored grammar compiler warnings.

**Overall verdict: PASS.**

```citations
[]
```
