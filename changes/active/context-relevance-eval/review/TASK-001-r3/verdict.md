# TASK-001 task review — round 3

**Verdict: FIX_REQUIRED.** Immutable base `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab`; cumulative candidate `c88bea58b62df3854b28a082fc7dd969ed8cfd6a`. Approved authority: `changes/active/context-relevance-eval/spec.md` revision 4, original `tasks/TASK-001-pinned-sample.md`, and R1/R2 remediation tasks. The complete base-to-candidate diff was reviewed; HEAD remained pinned. Review artifacts are the only working-tree additions.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001, REQ-007: named repositories and opt-in credentialed read-only command outside gate | `main.rs:37-42,256-296`; `mise.toml` live lane | Source and task graph inspection | PASS |
| REQ-002: 50 merged PRs before cutoff, deterministic order, valid identity and timestamps, or clear failure | `sample.rs:126-232`; `main.rs:196-248` | Seven sampler tests pass; `FIND-TASK-001-6` source-path reproduction | **FAIL** |
| REQ-002–003: one complete file listing, 2–15 eligible distinct paths, modified base declaration and another file with changed lines; patch omissions local | `sample.rs:235-373`; `main.rs:102-197` | Listing, qualification, omission, cap and duplicate-path tests; `TASK-A-1` rejected by match-arm inspection | PASS |
| INV-003: inconsistent provider input fails the run | `Discovery::offer` at `sample.rs:160-188` | `FIND-TASK-001-6` accepts a contradictory merge/update pair | **FAIL** |
| INV-001, AC-005 and non-goals: no production, persisted-format, gate, second-listing, `PrContext`, replay, or trial-blob change | Diff adds private example, manifest registration, live lane, and task/review evidence | Complete cumulative diff inspection | PASS |
| INV-004–005: source-free output and read-only in-memory sampling | `main.rs:120-157,251-296` | Source inspection | PASS |
| R1/R2 remediation: file ceiling, rustdoc, path uniqueness, Gregorian timestamps, concrete IO owner | `sample.rs:208-273,376-419`; `main.rs:60-249` | Seven focused tests, lints and source inspection | PASS; `FIND-TASK-001-1` through `-5` closed |

## Independent review and findings

Wave 1: task review A **FAIL** (`TASK-A-1`), task review B **FAIL** (`TASK-B-R3-1`), standards review **PASS**, source-trust domain review **PASS**, and sampling domain review **PASS**. `mise run review:verify-citations` exited 0 and accepted all eight citations; none was rejected by the citation checker.

Wave 2 independently **REJECTED** `TASK-A-1`: `Some(Ok(ranges))` matches an empty vector, so a valid deletion-only patch does not enter the omission arm. It **CONFIRMED** `TASK-B-R3-1` as new **FIND-TASK-001-6** under the task-contract gate: `Discovery::offer` accepts `merged_at > updated_at`, violating INV-003 and the settlement premise for REQ-002. The root cause, callers, producer, reproduction, correction boundary, and closure proof are in `findings-validation.md`. There are no retained notes or class-incomplete prior findings. The self-contained remediation task is `TASK-001-R3-timestamp-chronology.md`; it needs no spec revision.

## Verification limits

Locally observed exit 0: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::` (seven tests), `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `git diff --check e665bc17ce91e855e4a744f3231b58c1cd9cd0ab c88bea58b62df3854b28a082fc7dd969ed8cfd6a`, and the citation checker. Candidate implementation evidence records `mise run gate` passing; this review did not rerun it. Credentialed live sampling and same-cutoff comparison belong to TASK-003. The confirmed gap was reproduced by a traced reachable source path, without editing the immutable candidate to add a failing test.
