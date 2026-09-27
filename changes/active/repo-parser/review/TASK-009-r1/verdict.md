# TASK-009 r1 verdict: PASS

## Immutable subject

- Base: `597f5180808b78e1b104c10c1a375d5604b89a2e`.
- Candidate: `ef48e308838fd081561754059e78a3814f03cf8a` (HEAD throughout review).
- Authority: approved `changes/active/repo-parser/spec.md` revision 14; original `changes/active/repo-parser/tasks/TASK-009-complete-large-tree-listing.md`.
- Reviewed the complete base-to-candidate diff in `github.rs`, `snapshot.rs`, `pull_request.rs`, and the task evidence. The candidate source was not edited.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-005: split truncated root and nested recursive listings into complete, repository-relative paths with original modes and blob IDs | `GitHubClient::tree/list_into/tree_wire` in `github.rs:995-1079` assembles subtree entries; `ColdBuild::start` in `snapshot.rs:675-704` consumes them | `truncated_listing_splits_into_the_complete_snapshot` compares coverage and serialized snapshot bytes with an untruncated oracle; focused tests passed 2/2 | PASS |
| INV-002, AC-004: an unsplittable non-recursive listing remains incomplete and unpublished | `list_into` marks the listing truncated in `github.rs:1031-1035`; `ColdBuild::start` returns before ingest/publication | `unsplittable_truncated_listing_is_incomplete_and_unpublished` checks no object and unchanged prior pointer; focused tests passed | PASS |
| REQ-008: truncation itself does not force scoped PR fallback; overlay coverage remains honest | `PrContext::run` selects snapshot or scoped ingest by snapshot availability and carries `listing.truncated` into coverage (`pull_request.rs:163-255`) | Pipeline and journey lanes passed; independent source trace of snapshot reuse and coverage | PASS |
| Provider and task constraints: existing Git tree endpoint/policy; no clone, partial full snapshot, or new endpoint | `tree_wire` uses `Operation::Tree` with recursive or non-recursive request (`github.rs:1062-1079`); cold build rejects incomplete listing | Complete diff inspection; `git diff --check` passed | PASS |
| Existing cold, update, and PR-context behavior | Changed surfaces remain in the outer pipeline crate; update uses its existing rebuild entry point | `mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run fmt:check`, and `mise run lints` each exited 0 | PASS |

## Independent review and citation result

Wave 1: task review A **PASS**; task review B **PASS**; repository standards **PASS**; provider-bounds domain **PASS**; tree-trust domain **FAIL** with proposed `TREE-1`. The latter proposed an aggregate resource-bound finding. Citation verification exited 0 but marked three `TREE-1` citations `REJECT` for symbols absent from the candidate, so the finding was dropped under the review contract. The full output is in `citations.txt`. Wave 2 independently validated the empty surviving claim ledger and recommended **PASS**; see `findings-validation.md`. There are no validated blocking findings, notes, remediation task, or prior `FIND-*` IDs to close.

## Verification limit

The live `rust-lang/rust` capacity row was not rerun here. TASK-007 explicitly schedules that rerun after TASK-008, TASK-009, and TASK-010 pass (`TASK-007-integrated-pipeline-and-real-pr.md:24,54,74`). The earlier row at line 93 predates this candidate. The local fake-provider tests prove split and unsplittable behavior, but do not measure real GitHub subtree call count, latency, or rate-limit behavior. The later integrated gate must record the live outcome; this is not an observed defect in this candidate.

**Verdict: PASS.** TASK-009's reviewed behavior satisfies spec revision 14 and its task contract on the available source and local verification evidence.
