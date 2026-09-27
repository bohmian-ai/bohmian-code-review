# TASK-009 independent task review B

Subject: base `597f5180808b78e1b104c10c1a375d5604b89a2e`, candidate `ef48e308838fd081561754059e78a3814f03cf8a` (`HEAD`), approved `SPEC-repo-parser@14`, original `TASK-009`. I reviewed the complete four-file diff, the changed GitHub tree client, cold-build and PR consumers, and relevant repository rules. The task's Implementation Evidence section was not used as authority.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-005; truncated root and nested recursive trees become one complete listing with full paths, modes and blob IDs | `github.rs:995-1079` fetches non-recursive entries at each truncated tree, prefixes descendants, preserves their mode and SHA, and recurses on directory entries; `snapshot.rs:680-705` consumes that listing | `truncated_listing_splits_into_the_complete_snapshot` compares coverage and serialized snapshot bytes with the untruncated oracle and checks five tree calls; focused `truncated` run passed 2/2 | PASS |
| INV-002; a non-recursive listing that is itself truncated must remain incomplete and unpublished | `github.rs:1031-1035` marks the joined listing truncated; `snapshot.rs:680-692` returns before archive ingest or publication | `unsplittable_truncated_listing_is_incomplete_and_unpublished` checks incomplete reason, no object, unchanged pointer; focused run passed | PASS |
| REQ-008; truncation no longer triggers scoped fallback for a PR with a usable snapshot | `pull_request.rs:163-205` always loads a compatible snapshot and creates a repository overlay; `pull_request.rs:240-255` keeps an unsplittable head tree's overlay incomplete | Existing PR and pipeline journey lanes passed; source trace covers this exact branch | PASS |
| REQ-013; use existing provider policy and caller-owned cancellation, without a new endpoint | `github.rs:1062-1079` routes both request forms through `get_json(Operation::Tree)`, which uses the existing request policy and bounded JSON read; recursive work is awaited in the facade future | `mise run test:pipeline` passed; no real large-repository call-count measurement in this task | PASS, with verification limit |
| Task prohibitions; no partial published snapshot, large-tree scoped result, clone, or new provider endpoint | Cold build exits on incomplete listing; split work uses `/git/trees/{sha}` only; the diff adds no clone or provider route | Full diff inspection and `git diff --check` passed | PASS |
| Preserve existing cold, update and PR behavior | Changed files only adapt tree listing and PR snapshot selection; update still uses the cold-build entry point on rebuild | `mise run test:pipeline` passed, including pipeline tests and doc tests | PASS |

## Findings

None. I found no root-cause defect in the candidate that violates the task or its mapped spec obligations. The split listing is assembled at the shared GitHub tree owner; downstream cold build still rejects an unsplittable partial listing before publication. The PR path marks its overlay incomplete when that listing is unsplittable.

## Verification limit

I ran `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib truncated` (2 passed), `mise run test:pipeline` (exit 0), and `git diff --check` (exit 0). I did not run the live `rust-lang/rust` capacity rerun; TASK-007 explicitly assigns that rerun after TASK-008, TASK-009, and TASK-010. These local checks do not establish the real subtree call count or live GitHub behavior. I did not rerun the journey, formatting, or lint lanes in this independent sample.

Overall: **PASS**.

```citations
[]
```
