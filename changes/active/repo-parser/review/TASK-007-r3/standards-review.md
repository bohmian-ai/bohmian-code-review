# Repository standards review — TASK-007 r3

Subject: `2257992..b721917`; approved `changes/active/repo-parser/spec.md` revision 14 and `review/maintainer-decision-stateless-snapshot.md`. This report independently checks repository rules, not task acceptance. No `.codegraph/` index exists.

## Authority coverage

| Changed surface | Applicable authority | Inspected evidence |
|---|---|---|
| `src/snapshot.rs`, `src/pull_request.rs` | `AGENTS.md:23,27-34,38-40`; `architecture/agent-rules.md:3-12`; design `:19-22,30-38`; Rust core and testing workflow references; stateless-snapshot decision | Complete production diff, callers through `ColdBuild::run`/`PrContext::run`, added inline tests, pipeline manifest. |
| `examples/live_pr.rs`, `examples/provider_contract.rs` | `AGENTS.md:23,27-34,40`; `architecture/agent-rules.md:3,5,7,9-12`; design `:19-22,38`; Rust core and testing workflow references | Complete examples, `mise.toml` live lanes, captured contract report, source-free span schema. |
| `mise.toml` and workspace manifests | `AGENTS.md:33,38-40`; `architecture/agent-rules.md:5-6,12`; testing workflow reference | New `live:provider-contract` lane; `gate` dependencies; unchanged crate dependencies and `Cargo.lock`. |
| `changes/active/repo-parser/tasks/TASK-007-integrated-pipeline-and-real-pr.md`, remediation/review packet, evidence JSON | `AGENTS.md:36-46`; `architecture/agent-rules.md:14`; design `:38`; spec-driven and implementation-execution references | Revision-14 authority, original and R1/R2 tasks, evidence keys and recorded commands, complete changed-file list and diff statistics. |

## Rule results

| Written rule | Result | Evidence |
|---|---|---|
| Outer pipeline alone owns Git/storage IO; symbolic facade remains its only symbolic dependency (`AGENTS.md:14-23`; agent rules `:3,9`; design `:19-22`) | PASS | Only pipeline code changed; `Cargo.toml` still depends on `bohmian-code-review`; `run_at` paths reuse existing owners. |
| Source-free snapshots, provider identity and input validation (`AGENTS.md:23,28,30`; agent rules `:7,9`) | PASS | Fixed commit is parsed as `ObjectSha`; `ColdBuild::run_at` verifies repository and commit before existing `build`; evidence spans store path/blob/range/name/score/tokens, not text. |
| Concrete workflow owners, existing dependencies, no prohibited runtime SDK (`AGENTS.md:27,34`; agent rules `:10`) | PASS | `ColdBuild` and `PrContext` own the new methods; no manifest or lockfile dependency change. Example-only recording implements the existing transport trait. |
| New or materially modified Rust items have meaningful rustdoc (`AGENTS.md:32`; agent rules `:11`) | **FAIL** | The existing snapshot journey test loses its rustdoc when the new test is inserted ahead of it at `snapshot.rs:1703-1744`. |
| Credential-free local tests, opt-in live checks, intact gates (`AGENTS.md:38-40`; agent rules `:5,12`; testing workflow reference) | PASS with verification limit | `live:pr` and `live:provider-contract` are absent from `gate`; added tests use fake provider or captured JSON. `git diff --check 2257992 b721917` passed. I did not rerun the Cargo gate while the orchestrator owns that shared verification. |
| Benchmark claims name environment and reproducible measures (design `:38`) | PASS | TASK-007 records macOS/Apple silicon/release/network, exact commits and merged PRs, cold/warm measurements, RSS, snapshot bytes and API calls; evidence reports retain commit, tree, base and head. |
| No hand-edit of grammar/generated extraction artifacts; no weakening of tests (`AGENTS.md:33,40`; agent rules `:6,12`) | PASS | Neither grammar nor extraction fixture changed; new test assertions and contract-failure mutations were added. |

## Material finding

**STD-R3-1 — Rustdoc is detached from the existing snapshot journey test.** Classification: repository-rule violation. Defect class: insertion of a new test between an existing rustdoc block and its test item. Owner shape permits this ordinary source-order error; no public interface is involved. `AGENTS.md:32` and `architecture/agent-rules.md:11` require meaningful rustdoc for materially modified Rust items and tests. The original doc at `snapshot.rs:1703-1707` describes the complete cold snapshot journey, but the inserted fixed-commit test begins at `:1710`; the journey test now begins at `:1743` with no rustdoc. The only sibling is the newly introduced `examples/provider_contract.rs` test module at `:328`, which also lacks module rustdoc under the literal all-items rule; its individual test does have rustdoc. Consequence: the existing test's intent and invariants are incorrectly attributed to another test, and the repository's written documentation requirement fails. Correction: put the original journey doc immediately before its own test, give the fixed-commit test a separate accurate doc, and document the new test module if the all-items rule is applied to modules. Focused closure: inspect those item boundaries and run the owner test lane; no behavior change is needed.

Overall: **FAIL** on the written rustdoc rule. The full gate result remains an orchestrator verification limit, not a second finding.

```citations
[{"id":"STD-R3-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":1703,"snippet":"    /// Scenario 1: the cold build verifies archive entries, falls back to","symbols":["fixed_commit_build_ignores_head_advance_and_run_follows_it","complete_cold_snapshot_is_verified_source_free_and_reloadable"]},{"id":"STD-R3-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":1743,"snippet":"    #[tokio::test]","symbols":["complete_cold_snapshot_is_verified_source_free_and_reloadable"]},{"id":"STD-R3-1","path":"crates/bohmian-code-review-pipeline/examples/provider_contract.rs","line":328,"snippet":"mod tests {","symbols":["tests"]}]
```
