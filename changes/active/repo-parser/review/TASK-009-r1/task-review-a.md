# TASK-009 independent task review A

Subject: `597f5180808b78e1b104c10c1a375d5604b89a2e..ef48e308838fd081561754059e78a3814f03cf8a`; approved `SPEC-repo-parser@14`. Result: **PASS** for TASK-009's implemented behavior, with the later TASK-007 live capacity rerun still required. No reproduced source defect in the changed tree-listing path.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-005: split a truncated recursive root and nested subtree into a complete listing with full paths and original mode/blob identities | `github.rs:995-1060` recursively lists by tree SHA; `TreeListing::push` prefixes and validates; `snapshot.rs:2577-2642` compares published coverage and object bytes against an untruncated oracle | `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib truncated`: 2 passed. Fake oracle exercises root and nested truncation. The real large-tree run is explicitly scheduled under TASK-007 after TASK-008–010 pass. | PASS (live limit recorded) |
| INV-002/AC-004: an unsplittable nonrecursive listing remains incomplete and unpublished | `github.rs:1031-1035` sets `truncated`; `snapshot.rs:680-693` returns before ingest and publication | `snapshot.rs:2648-2681` proves no object and prior pointer retained; focused test passed | PASS |
| REQ-008: a successfully split tree does not trigger scoped fallback; failed full build may still yield scoped context | `github.rs:995-1060` clears truncation after successful split; `pull_request.rs:163-204` uses the listing and chooses snapshot or scoped ingest | Focused cold-build test passes; existing PR fallback test covers unsplittable listing | PASS |
| REQ-013 and TASK-009 material stop condition: subtree call count for a real large repository fits provider budget | `github.rs:1017-1052` issues sequential tree calls through the existing provider policy; fake split takes five calls | TASK-007:24,54,74 explicitly defer and supersede capacity evidence until TASK-008–010 pass. The prior `rust-lang/rust` row at :93 is pre-change. No live credentials in this review environment. | PASS for task; deferred integrated verification |
| No new endpoint, clone, or partial published snapshot | Diff uses only `Operation::Tree` and existing archive/blob path; `ColdBuild::start` rejects unsplittable listings | Diff inspection; focused tests above | PASS |
| Existing cold, update, and PR-context behavior | Changed fake routes and PR snapshot selection traced through `ColdBuild::start`, `Ingest::reuse`, and coverage construction | Candidate task claims pipeline and journey lanes passed; I independently reran only the focused `truncated` tests. These broader lane results are not independently reproduced in this sample. | PASS with verification limit |

## Findings

None. The live run is a later integrated acceptance gate owned by TASK-007, not a prerequisite for approving TASK-009. A source correction is not prescribed without an observed failure.

## Verification limits

TASK-007:24,54,74 explicitly schedules its capacity rerun after TASK-008, TASK-009 and TASK-010 pass; the old TASK-007 row predates this candidate and is superseded. The focused fake tests pass, but cannot establish GitHub's actual listing semantics or the real call count for the named large repository. TASK-007 must record that result before integrated completion. The review environment has no `GITHUB_TOKEN` or `BCR_REPO_ID`.

```citations
[]
```
