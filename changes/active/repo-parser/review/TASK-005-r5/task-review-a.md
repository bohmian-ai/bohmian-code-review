# TASK-005 Wave 1 task review A

Subject: `fa4fa2101950626a9fe60b0fdff0439b04c16f01..866f777a1a41a03384c14c12276a5877dcac94a2`; approved `changes/active/repo-parser/spec.md` revision 13. I reviewed the cumulative diff, original TASK-005, R1–R4 remediation, and both maintainer decisions. The decisions control where remediation text differs; FIND-TASK-005-7 is withdrawn.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| S5–S7: private head-correct overlay, stale-base reconciliation, from-scratch equivalence | `pull_request.rs:151–267`, `snapshot.rs:789–951` | `concurrent_prs_see_only_their_own_heads`, `stale_base_overlays_drift_or_reports_incomplete` | PASS |
| S10: bounded scoped fallback with honest coverage | `pull_request.rs:172–256,591–600`, `snapshot.rs:958–985` | `failed_cold_build_falls_back_to_scoped_context`, `scoped_bound_skips_excluded_neighbors`, `changed_heads_share_the_scoped_index_ceiling` | PASS |
| S18: pinned pagination, file-local patch/source omission, changed-path exclusion | `pull_request.rs:259–267,326–439`, `github.rs:1124–1164` | `moving_pr_retries_and_bad_patches_stay_file_local`, `perpetually_moving_pr_publishes_nothing`, `rejected_changed_path_is_never_secondary` | PASS |
| S19: candidate-only bounded packing and explicit estimated budget | `pull_request.rs:268–310,451–494`, `context.rs:175–216` | `exhausted_budget_packs_nothing_and_says_so`; public symbolic journey | PASS |
| S14–S15 and AC-009: separate context store, identity-bound exact read, 24-hour store-clock expiry and integrity | `persistence.rs:250–323`, `context.rs:314–386`, `testing.rs:421–496` | `reads_are_identity_scoped_and_expire`, `absent_corrupt_and_foreign_objects_fail` | PASS |
| Dropped publication, retry, cleanup order and lag | `context.rs:299–335,389–420`, `testing.rs:428–532` | `dropped_publication_leaves_no_readable_half_context`, `expired_cleanup_deletes_object_then_record_and_reports_lag` | PASS |
| R1–R3 closure: nonaliasing context IDs, candidate integrity, nearby eligibility, rejected PR paths, rejected tree decision, scoped head bound | `context.rs:323–335`; `pull_request.rs:177–204,222–267`; `github.rs:975–991,1387–1399` | focused owner tests named above; `non_canonical_head_tree_path_keeps_full_coverage`, `broken_head_tree_entry_fails_the_run` | PASS |
| R4 FIND-8: PR source admission and snapshot reuse obey current file ceiling by shape | `pull_request.rs:371–383,422–430,497–507`; `github.rs:1004–1026`; `snapshot.rs:335–354,958–985` | `file_ceiling_omits_oversize_heads_and_snapshots`, `verified_oversize_blob_is_a_benign_skip`, `blob_ceiling_below_file_bound_is_not_oversize` | PASS |
| R4 FIND-9: executable root attributes apply at shared policy producer | `snapshot.rs:743–766`; shared calls in cold build, update and PR context | `executable_root_attributes_exclude_generated_files` | PASS |
| No model, comment, detached task, SQL backend, local checkout, or raw-source persistence | Outer-crate facade and trait diff; source-free `Snapshot` and source-bearing `ReviewContext` | source inspection; pipeline owner tests | PASS |

## Root-cause and sibling sweep

The R4 size class has two source forms. Verified bytes enter through the shared `GitHubClient::blob` bound, used by changed heads, selected candidates, and `Ingest::fallback`; archive bytes are bounded by `TarStream`. Source-free facts can enter through `Ingest::reuse`, but `SnapshotScope` and its object key include `max_file_bytes`, so a snapshot made under a different ceiling cannot be loaded for this PR. The `Oversize` versus provider `FetchFailed` distinction is exercised. Executable root policy is selected by the one `root_attributes` producer shared by cold, update, warm PR, and scoped PR flows. I found no remaining sibling needing a separate correction.

The R3 rejected-tree decision is implemented at `GitHubClient::tree`: malformed SHA or unknown mode fails the listing, while noncanonical paths are counted as unsupported. PR-file rejection remains separate. No finding is proposed.

## Verification and result

`mise run test:pipeline`, `mise run test:symbolic-context:journey`, and `git diff --check fa4fa2101950626a9fe60b0fdff0439b04c16f01 866f777a1a41a03384c14c12276a5877dcac94a2` exited 0 in this review. The orchestrator independently reports `mise run gate` exited 0 at candidate HEAD. The candidate stayed at `866f777a1a41a03384c14c12276a5877dcac94a2` during inspection. The executable policy test directly covers the shared producer through cold build; update and both PR paths are supported by the common call path rather than separate execution evidence.

**Overall: PASS.**

```citations
[]
```
