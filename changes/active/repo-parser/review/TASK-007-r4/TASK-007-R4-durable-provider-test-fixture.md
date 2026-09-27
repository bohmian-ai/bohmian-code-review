---
id: TASK-007-R4
kind: remediation
status: ready
spec: SPEC-repo-parser@14
parent_task: TASK-007
remediates: [FIND-TASK-007-4]
depends_on: [TASK-007, TASK-007-R1, TASK-007-R2, TASK-007-R3]
---

# TASK-007-R4 — Keep the provider verdict test usable after completion

## Authority and subject

Implement this bounded correction through `$cr-implement` under the approved [revision 14 specification](../../spec.md), [original TASK-007](../../tasks/TASK-007-integrated-pipeline-and-real-pr.md), [R1](../TASK-007-r1/TASK-007-R1-fixed-commit-and-provider-proof.md), [R2](../TASK-007-r2/TASK-007-R2-stable-workload-and-contract-validation.md), [R3](../TASK-007-r3/TASK-007-R3-test-rustdoc-ownership.md), and [stateless-snapshot decision](../maintainer-decision-stateless-snapshot.md). Reviewed base: `2257992f36a2aa88f33f397af1e070d6ab35961b`; candidate: `7f7b75d5d8645f5dd2530779fb6e146dc122efe1`.

## FIND-TASK-007-4 — Transient evidence is a compile-time test fixture

**Diagnosis.** The existing provider verdict test defines `CAPTURED` with `include_str!` at `crates/bohmian-code-review-pipeline/examples/provider_contract.rs:336-339`, pointing to `changes/active/repo-parser/evidence/TASK-007/provider-contract-wyrd.json`. Its `captured()` producer at `:341-345` supplies `every_named_case_decides_the_verdict` at `:349-380`. This is the only pipeline-crate `include_str!`/`include_bytes!` path into active evidence. `architecture/references/languages/spec-driven-development.md:61-71` and `$cr-complete` require removal of the entire active packet after final approval. Once removed, Rust cannot compile that example test because `include_str!` needs the file at compile time. The current focused test passes only while the packet exists; it does not prove the completed repository can run the test. `mise.toml:54-59,76-85,100-102` routes the example test through `test:pipeline`, `test:rust`, and `gate`; the all-targets lint lane also compiles it. This violates the required completion lifecycle and `AGENTS.md:40` gate integrity. It leaves the provider regression proof unavailable after completion.

**Correction outcome and recommendation.** Keep the real provider report in the active packet as live review evidence until normal completion. Give the existing offline verdict test durable, checked-in case data owned by `bohmian-code-review-pipeline`, then point its existing compile-time fixture reference to that data. Preserve the passing named cases and the broken/missing-case mutations, so the test still proves that a partial provider report fails the command. Use the current `failures` validator and test; no second validator, runtime file read, dependency, or harness is needed. The candidate-owned test fixture path is the root-cause owner. A compact copy of the necessary captured case results and exchanges suffices; the whole live evidence report need not become a permanent crate asset.

**Write set.** `crates/bohmian-code-review-pipeline/examples/provider_contract.rs` and a durable fixture under `crates/bohmian-code-review-pipeline/`. Update the task's implementation evidence in this packet as required by the lifecycle. Do not change the opt-in provider command, production transport, live report, snapshot behavior, or external specification.

**Acceptance and proof.** The example test has no compile-time dependency on `changes/active/`; its passing case and every broken or missing named case retain the existing assertions. Prove this by compiling and running `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example provider_contract --all-features tests::every_named_case_decides_the_verdict -- --exact` with the active packet absent in an isolated copy, without altering the reviewed worktree. Then run `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check`. The next task review reassesses the entire cumulative base-to-new-candidate range and all four prior findings.

## Implementation evidence

Commit `fca3837`. The verdict test now uses `include_str!("provider_contract_cases.json\")`, a compact case fixture (5.3 KB, copied from the live report's cases with `etag`/`content-type` headers dropped and `without_patch` trimmed to one path) at `crates/bohmian-code-review-pipeline/examples/provider_contract_cases.json`. The test body, `failures`, all nine broken/missing-case mutations, the opt-in command, transport, and live report are unchanged.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| No compile-time dependency on `changes/active/` | `examples/provider_contract.rs` `tests::CAPTURED` | rsync copy excluding `changes/active`, `target`, `.git`; focused `cargo test ... --example provider_contract ... -- --exact`: 1 passed, exit 0 | PASS |
| Passing and every broken/missing named case keep existing assertions | `every_named_case_decides_the_verdict` unchanged | same focused test; `mise run test:pipeline` exit 0 | PASS |
| Lanes green | — | `mise run fmt:check`, `mise run lints`, `mise run gate`, `git diff --check` all exit 0 | PASS |

Non-goals held: no change to provider command, production transport, live report, snapshot behavior, or specification. Only the example file, the new fixture, and this task file changed.
