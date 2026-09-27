# TASK-002 r8 verdict

## Immutable subject and scope

- Repository: `bohmian-code-review`.
- Base: `ab2fa3a8615431f7c32c719a0f321f2f252e68fe`.
- Candidate: `3979b427ed52e298f5fa88ef0c00bb324f0acc6b` (checked at review start and end).
- Authority: approved `changes/active/repo-parser/spec.md` revision 12; `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`, **Revision 12 delta**.
- Reviewed diff: `git diff ab2fa3a 3979b427ed52e298f5fa88ef0c00bb324f0acc6b`; the previous TASK-002 behavior and FIND-1 through FIND-12 passed in `review/TASK-002-r7`. `snapshot.rs` cold-build internals belong to TASK-003; this review checks only that they compile, stay bounded, and disclose buffering honestly.

## Verdict: FIX_REQUIRED

`FIND-TASK-002-13` blocks AC-R12-1. The async headerless-403 classifier copies an entire provider body chunk into a buffer meant to inspect at most 4096 bytes, then truncates. A large chunk thus exceeds the preserved body-inspection bound before it is classified. The correction is packaged in `TASK-002-R6-bounded-rate-limit-prefix.md`.

## Acceptance matrix

| Revision 12 obligation | Implementation and verification evidence | Result |
|---|---|---|
| AC-R12-1: async `reqwest`, no `ureq`, preserved host trust, redirects, identity, retry and bounded bodies | `github.rs` async transport/request policy; `cargo tree` shows `reqwest 0.13.5` and tokio, no `ureq`; source-trust domain review PASS; pipeline tests pass. `github.rs:819` exceeds the classifier's prefix copy bound. | **FAIL**: FIND-13 |
| AC-R12-2: three async traits with `Send` futures, generic stores, no `async_trait`, unchanged contracts | `storage.rs`, `persistence.rs`, `testing.rs` and generic `snapshot.rs` consumers; `snapshot::tests::cold_build_future_is_send`; pipeline tests and lints pass. | PASS |
| AC-R12-3: delete library cancellation/deadline types and tests | Deleted `execution.rs`; `lib.rs` exports removed; `rg -n 'ureq|Execution|CancelToken|Interrupted|tokio::spawn' crates/bohmian-code-review-pipeline Cargo.toml` found none. | PASS |
| AC-R12-4: dropping a retry wait sends no later request | `send_with_policy` awaits tokio sleep in caller future; exact `github::tests::dropped_call_sends_no_further_requests` ran 1 test, passed. | PASS |
| AC-R12-5: no detached work | No `tokio::spawn` in pipeline crate; async domain reviewer traced caller-owned work. | PASS |
| Snapshot compile/bound/buffering scope | Pipeline compiles and its 29 tests pass; compressed archive is accumulated only from `Archive::chunk`, which enforces `max_archive_bytes`; expansion uses `BoundedReader`; `snapshot.rs:653-655` marks buffering with `ponytail:`. | PASS within stated scope |
| No unrelated expansion or prohibited dependencies | Changed code stays in the pipeline crate plus workspace manifests/lockfile and task evidence; dependency tree contains no prohibited production SDK or backend. | PASS |

## Review waves and citations

| Reviewer | Report | Result | Claim |
|---|---|---|---|
| task-rev A | `task-review-a.md` | PASS | none |
| task-rev B | `task-review-b.md` | FAIL | `TASK-B-1` |
| repo-rev | `standards-review.md` | FAIL | `S-1` |
| domain-rev, source trust | `domain-review-source-trust.md` | PASS | none |
| domain-rev, async contracts | `domain-review-async-contract.md` | PASS | none |
| Ponytail validation | `findings-validation.md` | FIX_REQUIRED | merged both claims as `FIND-TASK-002-13` |

The first citation check rejected `TASK-B-1` and `S-1` because their `symbols` arrays used an unrecognized `HttpBody::chunk` spelling. Each reviewer corrected that metadata without changing the cited code, claim, or ID. The final `mise run review:verify-citations …/TASK-002-r8 3979b427…` exited 0 with both citations **OK** (`citations.txt`). No final citation was rejected. `claim-ledger.md` contains only citation-checked claims.

## Validated ledger and prior closure

`FIND-TASK-002-13` is **CONFIRMED**, classification INCORRECT, task-contract gate AC-R12-1 and REQ-013. Reproduction: `GitHubClient::repository` reaches the shared 403 policy; a `FixedBody` may yield a 4097-byte first chunk; `names_rate_limit` copies all 4097 bytes before truncating to 4096. The source path and sibling sweep are recorded in `findings-validation.md`. The remediation keeps the existing classifier and bounds its own prefix copy. No separate note survived Wave 2. FIND-TASK-002-1 through FIND-TASK-002-12 remain closed; this is a new async-port defect class, not a reopened prior finding.

## Verification and limits

- Candidate checkout remained `3979b427ed52e298f5fa88ef0c00bb324f0acc6b`; `git diff --check ab2fa3a 3979b427…` passed.
- Exact dropped-call test: 1 passed. `mise run test:pipeline`: 29 unit tests passed, doc step passed. `mise run fmt:check`, `mise run lints`, and `mise run gate`: exit 0. Gate included the symbolic journey and all workspace tests; its pipeline run passed 29 tests. `cargo tree -p bohmian-code-review-pipeline --depth 1 --all-features` recorded the new dependency set.
- Passing tests use short 403 bodies and do not prove the classifier's temporary prefix allocation bound. Wave 2 reproduced the defect by tracing reachable source, not by measuring allocator peak.
- No live GitHub/GHES call or controlled real-provider contract run. TASK-003 cold-build semantics were excluded as requested. No sub-reviewer gap; every required report and Wave 2 validation completed within the 20-minute reviewer cap.
