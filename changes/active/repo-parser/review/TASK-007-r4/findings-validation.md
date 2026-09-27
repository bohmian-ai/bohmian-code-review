# TASK-007 r4 findings validation

Subject: `2257992f36a2aa88f33f397af1e070d6ab35961b..7f7b75d5d8645f5dd2530779fb6e146dc122efe1`; approved `SPEC-repo-parser@14`, original TASK-007, R1–R3 remediation, and the stateless-snapshot maintainer decision. Claim input: `claim-ledger.md`; citation check: both cited lines `OK`. Recommendation: **FIX_REQUIRED**.

## Blocking ledger

### FIND-TASK-007-4 — Provider test fixture is owned by the temporary change packet

- **Source/status:** `TASK-A-1`; **CONFIRMED**, new ID. This is a fixture-lifetime defect introduced by R2, distinct from `FIND-TASK-007-2`'s missing provider-result validation.
- **Gate/classification:** Written repository lifecycle and test standard; **VIOLATION** of `architecture/references/languages/spec-driven-development.md:61-71` (completion removes the active packet) combined with `AGENTS.md:40` (an absent test target is a failing prerequisite; the gate must not be weakened). It also prevents the required post-completion Rust gate from compiling.
- **Root cause/defect class:** The offline provider verdict test binds a compile-time `include_str!` to `changes/active/repo-parser/evidence/TASK-007/provider-contract-wyrd.json` at `crates/bohmian-code-review-pipeline/examples/provider_contract.rs:336-344`. The example test's source and its required fixture have different lifetimes. The compiler must open that path before the test runs; after normal completion deletes `changes/active/repo-parser/`, the example test cannot compile.
- **Sibling sweep:** This is the sole `include_str!` or `include_bytes!` in the pipeline crate. `captured()` is called by the one example test at `provider_contract.rs:350-351,380`; the test is compiled by `mise.toml:54-59` (`test:pipeline`, `--all-targets`), then `:76-85` (`test:rust`) and `:100-102` (`gate`). The opt-in `live:provider-contract` release example does not compile this `#[cfg(test)]` fixture and remains separate.
- **Reproduction:** The exact focused command `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example provider_contract --all-features tests::every_named_case_decides_the_verdict -- --exact` currently selects and passes one test. Source trace: `gate` → `test:rust` → `test:pipeline` → Cargo `--all-targets` → example test → `include_str!` path in the active packet. The lifecycle authority requires deletion of that entire packet at completion, and `include_str!` fails compilation if its file is absent. The post-deletion state was not created in the reviewed worktree; the compile-time dependency and required deletion establish the failure without changing the immutable subject.
- **Observable consequence:** A correctly completed change leaves `mise run test:pipeline`, `test:rust`, `lints` (Clippy `--all-targets`), and `gate` unable to compile the example test. The provider verdict regression check cannot run.
- **Smallest safe correction:** Keep the captured real-provider report in the active packet as task evidence until completion, but give the example test a durable, checked-in fixture under `crates/bohmian-code-review-pipeline/` and point its existing `include_str!` there. Reuse the current captured case data and the existing `failures` test; no new validator, runtime file read, dependency, or test harness is needed. The candidate-owned test fixture path is the correction owner. Preserve the opt-in live command and its report behavior. A compact fixture with the same required case results and exchanges is sufficient; do not copy unrelated capacity reports into the crate.
- **Focused closure proof:** Build and run the exact example test above with the active packet absent in an isolated copy or after lifecycle completion; then run `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `mise run gate`. The test must still reject each broken named case and pass the valid captured case set. Review the fixture path to ensure it has no dependency on `changes/active/`.

## Prior finding closure and remediation drift

| Prior finding | Disposition at this candidate |
|---|---|
| `FIND-TASK-007-1` | **CLOSED.** `ColdBuild::run_at` pins the cold commit; `PrContext::run_at` and the one-read benchmark reject a moved PR. The capacity rows name the fixed commits and PR base/head pairs. The new claim does not touch those paths. |
| `FIND-TASK-007-2` | **CLOSED as a behavior class.** `provider_contract::run` calls `failures(&cases)` before returning success, and `failures` checks the named provider outcomes. The offline test exercises those checks. Its fixture lifetime is this new, separate finding. |
| `FIND-TASK-007-3` | **CLOSED.** The fixed-commit test, original cold-journey test, and provider example test module now each have accurate rustdoc on their own item. |

No redundant guard, duplicate provider validator, or stateless-snapshot drift was identified in R1–R3. The correction should preserve the existing test and move only its data dependency to a durable location.

## Claim disposition and limits

| Source claim | Result | Gate |
|---|---|
| `TASK-A-1` | **CONFIRMED** → `FIND-TASK-007-4` | Written lifecycle and test standard |

No rejected claims or notes. Validation used the full cumulative source diff, the approved task and remediation packets, the prior finding ledgers, lifecycle authority, the exact focused test, and caller/producer tracing around the fixture and provider verdict. I did not delete the active packet in the reviewed worktree, rerun credentialed GitHub calls, or rerun the large capacity measurements. Those live reports remain existing evidence; they do not bear on this compile-time defect.
