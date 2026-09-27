# TASK-007-r4 independent task review B

Subject: base `2257992f36a2aa88f33f397af1e070d6ab35961b`; candidate `7f7b75d5d8645f5dd2530779fb6e146dc122efe1`; approved `SPEC-repo-parser@14`; original TASK-007 and R1–R3 remediation, with the stateless-snapshot decision. I inspected the cumulative diff and the changed source, opt-in commands, recorded reports, and applicable repository rules. This is an acceptance review, not a re-review of other agents' reports.

| Requirement, criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Four independently callable facades compose through cold build, update, PR context, and read-back; cancellation leaves no false pointer and retry converges (REQ-012, REQ-018, AC-002, AC-010) | `tests/pipeline_journey.rs`; unchanged production facade separation; `src/snapshot.rs` and `src/pull_request.rs` changes retain distinct calls | `mise run test:pipeline` exited 0 in this review; recorded `mise run gate` and journey lane | PASS |
| S1–S19 and the complete-coverage, negative, and from-scratch oracles stay in the repository (REQ-017, AC-003/004) | Existing pipeline owner tests, including `tests/pipeline_journey.rs` | `mise run test:pipeline` exited 0; recorded gate | PASS |
| Fixed-commit capacity runs retain source and snapshot identity, while ordinary cold build follows the default head (AC-007, R1, revision 14) | `src/snapshot.rs:621-655`; `examples/live_pr.rs:103-115`; `snapshot::tests::fixed_commit_build_ignores_head_advance_and_run_follows_it` | Test in `mise run test:pipeline`; fixed commit/tree and metrics in three capacity reports | PASS |
| Every measured PR sample uses the same base/head; a moved PR fails instead of producing mixed workload statistics (AC-007, R2) | `src/pull_request.rs:150-182,364-419`; `examples/live_pr.rs:119-143`; `samples_at_one_read_pr_refuse_a_moved_head` | Test in `mise run test:pipeline`; recorded merged-PR runs for Wyrd, VS Code, and Rust | PASS |
| S20 invokes production facades against a real existing PR, yields nonempty spans, and reads the same context by ID; missing credentials, inaccessible PR, and empty context fail (AC-011) | `examples/live_pr.rs`; `evidence/TASK-007/s20-wyrd-pr1.json` and negative PR #91 report | Recorded read-only Wyrd #1 run, `read_back_equal: true`, 50 selected spans, and negative commands in TASK-007 | PASS |
| Controlled provider contract records and enforces real response cases, while local tests own 403/429 and GHES branches (AC-006, R1/R2) | `examples/provider_contract.rs:39-307`; `mise.toml:72-74`; `evidence/TASK-007/provider-contract-wyrd.json` | Captured repository/commit/tree/blob/archive/PR-files/304/rate-header report; `every_named_case_decides_the_verdict` in `mise run test:pipeline` | PASS |
| Capacity evidence follows TASK-008–010, covers 1k/10k/50k classes, and reports complete snapshots or blocking eligible files (AC-007) | TASK-007 capacity table; `s20-wyrd-pr1.json`, `bench-vscode.json`, `bench-rust.json` | Each report identifies fixed cold commit/tree, merged PR base/head, full coverage, calls, timing, and snapshot size; commands and environment recorded | PASS |
| R3 rustdoc attaches the correct intent to both snapshot tests and the provider test module | `src/snapshot.rs:1703-1746`; `examples/provider_contract.rs:327-332` | Source inspection; `mise run test:pipeline` exited 0 | PASS |
| No consumer service, hosted run, persistent derived snapshot state, credentialed normal CI lane, clone, worker, or all-in-one production method (INV-001, REQ-017, TASK-007 non-goals) | Cumulative production diff touches only the pipeline owners and opt-in examples; `mise.toml:72-74` does not enter `gate`; maintained per-path snapshot outcomes | Cumulative diff inspection; stateless-snapshot decision | PASS |

I found no material `MISSING`, `INCORRECT`, `DRIFT`, `VIOLATION`, or `REGRESSION` claim in this candidate. The provider and capacity live calls were not rerun during this offline review; the checked-in reports and exact recorded commands are the live evidence. `mise run test:pipeline` and `git diff 2257992 7f7b75d --check` exited 0 here.

Overall result: **PASS**.

```citations
[]
```
