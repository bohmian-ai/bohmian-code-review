# TASK-007 Wave 1 task review A

Subject: base `2257992f36a2aa88f33f397af1e070d6ab35961b`, candidate `7f7b75d5d8645f5dd2530779fb6e146dc122efe1`; approved `SPEC-repo-parser` revision 14, original TASK-007, R1–R3, and the stateless-snapshot maintainer decision. I reviewed the cumulative production diff, opt-in commands, task evidence, applicable architecture, and lifecycle. No other Wave 1 report informed this review.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004/008/009/010/011/012/018, INV-001–006, AC-002/003/004/010: separate cold/update/PR/read facades compose, retain source trust, expose partial coverage and budget outcomes | `tests/pipeline_journey.rs`; `src/pull_request.rs:150-355`; existing owner tests | `pipeline_journey` with `testing`: 2 passed; candidate task records S1–S19 in `mise run gate` | PASS |
| REQ-013/017, AC-011: configurable real PR command uses public facades, bounded calls, nonempty spans, read-back, read-only GitHub and in-memory persistence | `examples/live_pr.rs:80-203`; `mise.toml:65-69` | Captured Wyrd PR #1: 50 spans, full coverage, equal read-back; PR #91 empty case fails visibly | PASS |
| AC-006, R1/R2 FIND-TASK-007-2: separate controlled-provider contract, conditional and rate observation, named cases decide success | `examples/provider_contract.rs:128-308`; opt-in `live:provider-contract` | Captured Wyrd case statuses; focused `tests::every_named_case_decides_the_verdict`: 1 passed; local fake transport covers GHES and 403/429 | PASS while active packet exists; see TASK-A-1 |
| AC-007, R1/R2 FIND-TASK-007-1: fixed-commit and one-PR-pair capacity workload; 1k/10k/50k measurements | `src/snapshot.rs:628-655`; `src/pull_request.rs:159-183,385-418`; `examples/live_pr.rs:106-141`; three JSON reports and TASK-007 table | Fixed-commit and moved-PR focused tests: 1 passed each; reports contain commit/tree, PR base/head, coverage, warm p50/p95, calls, time, snapshot bytes, peak RSS in task record | PASS |
| R3 FIND-TASK-007-3: rustdoc belongs to each changed test item | `src/snapshot.rs:1700-1752`; `examples/provider_contract.rs:327-350` | Source inspection; test bodies unchanged in R3 | PASS |
| AC-008 and repository lifecycle: code and normal gate remain usable after final completion retires the active packet | `examples/provider_contract.rs:337-339` | `cr-complete` requires deletion of `changes/active/repo-parser/`; the example is compiled by `mise run test:pipeline` and `gate` | FAIL: TASK-A-1 |
| Non-goals: no consuming service, credentialed normal CI, clone, durable archive, production conditional-request support, or aggregate snapshot state | Changed file list and opt-in `mise.toml` lanes | Source and diff inspection | PASS |

## Proposed finding

### TASK-A-1 — Transient evidence is a compile-time test dependency

**Classification:** REGRESSION / VIOLATION. **Defect class:** a normal test target embeds a file owned by the transient active change packet. The owner shape is the `include_str!` fixture path in the provider-contract example test, not the provider runtime or evidence report.

**Root cause and siblings:** `examples/provider_contract.rs:337-339` compiles the captured report from `changes/active/repo-parser/evidence/TASK-007/provider-contract-wyrd.json`. A repository search for `include_str!`/`include_bytes!` references to active evidence found this sole site. The same failure reaches `test:pipeline`, `test:rust`, and `gate` through the example test target; these are effects of one root cause, not separate findings.

**Violated obligation:** `architecture/references/languages/spec-driven-development.md:61-71` and `.agents/skills/cr-complete/SKILL.md` require the active packet to be removed after final approval. `AGENTS.md` requires the normal gate to remain valid; AC-006 requires the local test to stay credential-free and AC-008 requires successful completion of the lifecycle.

**Evidence and consequence:** `include_str!` resolves its path at compile time. After the required completion deletes `changes/active/repo-parser/`, compiling `provider_contract` as an example test fails with a missing file, so `mise run test:pipeline` and `mise run gate` cannot pass on the completed repository. The current green run only proves the transient packet is still present.

**Testable correction:** Make the provider verdict test self-contained with a compact local passing case value, or place any required fixture under a durable crate-owned test path. Keep the captured real-provider report as review evidence while active. Prove the example test compiles and its broken-case mutations fail without any compile-time reference into `changes/active/`, then run `mise run test:pipeline` and `mise run gate`.

## Result

**FAIL** — one bounded lifecycle regression. Live provider and capacity observations are captured results; this review did not repeat credentialed network runs. The focused offline provider, fixed-commit, moved-PR, and public journey tests passed. The R3 rustdoc defect is closed.

```citations
[{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/examples/provider_contract.rs","line":337,"snippet":"    const CAPTURED: &str = include_str!(","symbols":["CAPTURED","captured","every_named_case_decides_the_verdict"]},{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/examples/provider_contract.rs","line":338,"snippet":"        \"../../../changes/active/repo-parser/evidence/TASK-007/provider-contract-wyrd.json\"","symbols":["captured"]}]
```
