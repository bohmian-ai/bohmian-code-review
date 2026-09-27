# TASK-005 independent task review B

Subject: base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`; candidate `866f777a1a41a03384c14c12276a5877dcac94a2`; approved `changes/active/repo-parser/spec.md` revision 13. I reviewed the cumulative outer-crate diff and the original task plus R1–R4 remediation. The R3/R4 maintainer decisions control conflicts; FIND-TASK-005-7 is withdrawn and is not a review claim.

| Requirement, criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Private head-correct overlay, stale-base reconciliation, changed paths excluded | `pull_request.rs` `PrContext::run`, `changed_head`, `rank`; `snapshot.rs` `Ingest::reuse`/`fallback` | `concurrent_prs_see_only_their_own_heads`, `stale_base_overlays_drift_or_reports_incomplete`; pipeline lane exit 0 | PASS |
| Bounded scoped fallback and honest coverage | `pull_request.rs` `near`, shared `index_cap`, `ContextCoverage`; `snapshot.rs` `Ingest::finish` | `failed_cold_build_falls_back_to_scoped_context`, `changed_heads_share_the_scoped_index_ceiling`; pipeline lane exit 0 | PASS |
| Pinned paginated PR files and file-local omissions | `pull_request.rs` `pin`, `changed_head`; `github.rs` PR-file validation | moving-PR, malformed-patch, rejected-file tests; pipeline lane exit 0 | PASS |
| Candidate-only ranked context and explicit budget | `pull_request.rs` `rank`, `pack`, `Budget`; public symbolic selector reused | pinned multi-file and exhausted-budget tests; pipeline lane exit 0 | PASS |
| Verified PR file ceiling and compatible snapshot scope | `github.rs` `GitHubClient::blob` limits returned bytes; `snapshot.rs` `snapshot_scope`; `pull_request.rs` `fetch` | `file_ceiling_omits_oversize_heads_and_snapshots`; pipeline lane exit 0 | PASS |
| Root policy in both regular Git modes | `snapshot.rs` `root_attributes` accepts `File | Executable` for all cold/update/PR callers | `executable_root_attributes_exclude_generated_files`; pipeline lane exit 0 | PASS |
| Broken tree entries fail; invalid paths are exclusions; rejected PR files stay distinct | `github.rs` `validate_tree_entry`, `TreeListing::invalid_paths`, PR-file rejection; `snapshot.rs` coverage | broken-tree and noncanonical-path tests; pipeline lane exit 0 | PASS |
| Store-owned durable clock and tenant-scoped context read/expiry | `persistence.rs` `ContextStore`, `SnapshotStore`; `context.rs` `Contexts`; in-memory store clock | read/expiry, cleanup, dropped-publication tests; pipeline lane exit 0 | PASS |
| Source lifetime, injected storage, and no model/comment/checkout work | Source text confined to `ReviewContext` object; pipeline depends on symbolic public facade | cumulative diff and pipeline lane | PASS |

## Root-cause and sibling sweep

I traced `PrContext::run` through `pin` → head tree/root policy → `Ingest` reuse/changed heads/fallback → symbolic ranking/packing → `Contexts::publish`, and separately traced `Contexts::read` and `collect_expired`. I checked the corresponding cold-build and update callers of the shared `root_attributes`, `snapshot_scope`, `Ingest`, and bounded blob fetch. The current byte ceiling is imposed by the provider-return shape, and incompatible snapshot ceilings have distinct keys. The common source-trust and coverage producers therefore cover their sibling paths. No material root-cause finding survived.

## Verification limits

`mise run test:pipeline` exited 0 during this review. The orchestrator reported `mise run gate` and `git diff --check` both exited 0 at the pinned candidate. I did not run a live credentialed GitHub PR; it is outside this task's normal lane. I did not treat withdrawn FIND-TASK-005-7 as an open defect.

Overall result: **PASS**. Proposed findings: none.

```citations
[]
```
