# TASK-010 independent task review A

Subject: base `97fff9f`, candidate `da81a59` (`da81a598c10441b3b2e1231ce674d89003017f26`), approved `changes/active/repo-parser/spec.md` revision 14 at `63d6bd0`, original `TASK-010-compare-based-update.md`, and `review/maintainer-decision-stateless-snapshot.md`. Reviewed the complete base-to-candidate diff, not the task's implementation summary. The source candidate was unchanged during this review.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-007: compare the pointer commit with pinned head; use changed files without a full-tree relisting | `update.rs:82-118`; `github.rs:1153-1207` | `compare_sorts_changed_paths_and_verifies_its_ends`; `change_delete_and_rename_match_a_fresh_parse` asserts one compare and no recursive listing | PASS |
| REQ-007: get changed-path mode from head parent directory; remove deleted, renamed-old, or ineligible facts; retain unchanged outcomes | `update.rs:117-139,164-253`; `snapshot.rs:955-995`; `eligibility.rs:242-260` | Update oracle compares serialized snapshot with independent cold build; symlink and deleted/renamed paths asserted absent | PASS |
| REQ-007: changed blobs are verified; failed replacement cannot publish stale facts | `snapshot.rs:1122-1150,742-786`; `github.rs:1237-1275` | `failed_changed_blob_keeps_the_pointer_until_a_retry`; `mise run test:pipeline` passed | PASS |
| REQ-007: root `.gitattributes`, non-descendant status, provider file ceiling, or changed-file ceiling cause archive rebuild | `update.rs:104-115`; `github.rs:1175-1178` | `rebuild_triggers_use_the_archive`; `changes_past_the_ceiling_rebuild_from_the_archive` | PASS |
| REQ-006, REQ-012, INV-003, AC-003: complete-only immutable publication, retry, moving head, version change, running-review isolation | `snapshot.rs:742-786,789-849`; `update.rs:82-161` | Existing update lifecycle tests and public `pipeline_journey`; `mise run test:pipeline` passed | PASS |
| Revision 14 and maintainer decision: snapshot stores only per-path outcomes, with no exclusion counts or source bytes | `snapshot.rs:247-264,304-311,771-777`; `snapshot.rs:904-909` | Snapshot-byte cold oracle; format bumped to 3 | PASS |
| No full-tree listing on incremental path; no carry for changed path; no incomplete publication; no extra rebuild trigger | `update.rs:109-139`; `snapshot.rs:964-995,750-786` | Call accounting and incomplete-pointer tests above | PASS |
| Required pipeline and journey evidence | `mise.toml:54-63`; `tests/pipeline_journey.rs:462-482` | Ran `mise run test:pipeline`: exit 0, library tests and doc tests passed | PASS |

The scenario wording says changed blobs only, while unchanged root `.gitattributes` is fetched to classify changed paths (`update.rs:119-124`, `snapshot.rs:851-868`). Revision 14 requires changed-path eligibility and the stateless snapshot decision bars persisting the parsed attributes; this control fetch is consistent with the approved contract. The existing test explicitly accounts for it. It is not a finding.

No proposed findings. Overall **PASS**. The fake provider and byte-for-byte cold oracle verify the implemented behavior; no live GitHub/GHES call was part of this task review.

```citations
[]
```
