# TASK-010 task review, round 1

**Verdict: PASS.** No remediation task or finding ID.

## Immutable subject

- Base: `97fff9feaffa2ec46fef512f5f476133830f1ac3`
- Candidate: `da81a598c10441b3b2e1231ce674d89003017f26`
- Authority: approved `SPEC-repo-parser@14` at `63d6bd003cb811de0f6a3ce22bf26f041afd992d`, `tasks/TASK-010-compare-based-update.md`, and `review/maintainer-decision-stateless-snapshot.md`
- Scope: complete base-to-candidate diff, including GitHub compare, snapshot ingest and update, the public pipeline journey, and the revision 14 snapshot definition

The candidate remained at `da81a59` throughout the review. This is the first TASK-010 review; there are no prior `FIND-*` IDs to close.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Compare the pointer commit with the pinned head and update without a recursive tree listing | `GitHubClient::compare`; `Update::run`; non-recursive `Directories` lookup at the head | Compare and update call-accounting tests; public pipeline journey | PASS |
| Preserve unchanged outcomes; replace changed blobs; remove deleted, renamed-old, and now-ineligible paths; match a cold head build | `Ingest::carry` excludes compared removals; changed-path mode comes from verified head directory entries; blob ingest verifies identity | `change_delete_and_rename_match_a_fresh_parse` compares snapshot bytes with a cold build | PASS |
| Rebuild from archive for root `.gitattributes`, non-descendant head, compare ceiling, or changed-blob bound | `Update::run` trigger branch and bounded compare response | `rebuild_triggers_use_the_archive`; `changes_past_the_ceiling_rebuild_from_the_archive` | PASS |
| Keep incomplete attempts off the pointer; preserve retry, interruption, version, and running-review guarantees | `ColdBuild::complete` gates publication; record/object/pointer CAS order remains; immutable snapshot read | Failed-blob, dropped-update, moving-head and version-change tests; journey | PASS |
| Persist only source-free per-path outcomes with no derived exclusions, per revision 14 and maintainer decision | Snapshot format version 3 and per-path outcome payload; build-local exclusion reporting | Byte-for-byte cold oracle and snapshot reuse coverage tests | PASS |
| Preserve task non-goals and repository rules | Incremental path has no recursive listing or base listing; no extra rebuild trigger, symbolic-crate IO, new dependency, or persisted source | Full diff and authority audits; `git diff --check` | PASS |

## Review and validation

Wave 1: `task-review-a.md` **PASS**, `task-review-b.md` **PASS**, `standards-review.md` **PASS**, `domain-review-source-trust.md` **PASS**, and `domain-review-lifecycle.md` **PASS**. Every report proposed zero findings and supplied one empty citations block. `mise run review:verify-citations changes/active/repo-parser/review/TASK-010-r1 da81a59` exited 0; `citations.txt` contains its output. Rejected citations: none.

Wave 2 independently checked the empty claim ledger and the complete diff. `findings-validation.md` records an empty validated blocking ledger, no notes, and **PASS**. No remediation or spec revision is required.

The task scenario's “blob fetches only for changed paths” wording has one approved nuance: update reads unchanged root `.gitattributes` when needed to classify changed paths. Revision 14 and the maintainer decision forbid storing derived snapshot state; the reviewers found this control fetch consistent with the approved behavior.

## Verification and limits

The orchestrator reran `mise run test:pipeline`, `mise run test:pipeline:journey` (2/2), `mise run fmt:check`, `mise run lints`, and `mise run gate`; all exited 0. `git diff --check 97fff9f da81a59` passed. Reviewers also ran focused update tests (8/8). Verification used fake provider responses, with no live GitHub or GHES update call. All required reviewers completed; there is no sub-reviewer gap.
