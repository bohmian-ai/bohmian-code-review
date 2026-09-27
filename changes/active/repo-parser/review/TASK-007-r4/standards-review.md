# TASK-007-r4 repository standards review

**Subject:** base `2257992f36a2aa88f33f397af1e070d6ab35961b`; candidate `7f7b75d5d8645f5dd2530779fb6e146dc122efe1`; approved `SPEC-repo-parser` revision 14; original TASK-007 and R3 remediation. The candidate is the reviewed HEAD. No `.codegraph/` index exists.

## Authority coverage

| Changed surface | Applicable authority inspected | Coverage |
|---|---|---|
| `src/snapshot.rs`: fixed-commit cold build and test | `AGENTS.md` §§ Product/Rust/Tests; `architecture/agent-rules.md:3-11`; `architecture/bohmian-code-review-design.md:20-22,36-38`; Rust core and testing-workflows references; spec REQ-004–006, REQ-013, AC-007; stateless-snapshot decision | Public entrypoint, existing `build`/`run` owner, inline test, storage pointer seam and manifest inspected. |
| `src/pull_request.rs`: pinned PR sampling and test | Same Rust/architecture/testing rules; spec REQ-009–013, AC-007; symbolic-context reference for stage boundary | `run`, `run_at`, shared `publish`, `pin`, `list`, and inline test inspected. |
| `examples/live_pr.rs`: S20 and fixed workloads | `AGENTS.md:23,27-34,38-40`; `architecture/agent-rules.md:3-5,7,9-12`; design source-lifetime/IO boundary; testing-workflows reference; spec REQ-013–014, REQ-017–018, AC-007/011 | Config, bounded calls, report, failure exit, imports and existing test adapters inspected. |
| `examples/provider_contract.rs`: controlled live contract and offline check | Same Rust/test/source-trust authorities; spec REQ-004, REQ-013–014, AC-006 | Production client/transport wrapper, redaction, response recording, verdict and inline test inspected. |
| `mise.toml`, `Cargo.toml`, `Cargo.lock` | `AGENTS.md:9,33-34,40`; `architecture/agent-rules.md:5-6,12-13`; testing-workflows reference | Opt-in lane and full gate graph inspected; no manifest or lockfile dependency change. |
| Task/evidence/review records in cumulative diff | `AGENTS.md:42-46`; `architecture/agent-rules.md:13-15`; spec-driven-development and implementation-execution references; spec REQ-013, AC-006/007/011 | Task evidence, R3 scope, JSON report schemas, and stateless-snapshot decision inspected. Prior review conclusions were not used as a verdict. |

## Rule checks

| Written rule | Result | Source and verification evidence |
|---|---|---|
| Pipeline is sole Git/storage IO owner; symbolic crates remain IO-free and only public facade is consumed (`AGENTS.md:14-23`; `agent-rules.md:3`; design:20-22) | PASS | Changed Rust remains in `bohmian-code-review-pipeline`; pipeline manifest depends on the public facade. No symbolic crate changed. |
| No duplicated symbolic stage, server, worker, or source persistence (`AGENTS.md:23`; `agent-rules.md:3,9`; design:20-22) | PASS | `ColdBuild::run_at` delegates to existing `build` (`snapshot.rs:640-654`); `PrContext::run_at` delegates to shared `publish` (`pull_request.rs:173-183`); examples invoke facades. Evidence JSON stores metadata and path/range/count records, not source spans' text. |
| Concrete workflow owners; borrow where possible; no speculative dependency or trait (`AGENTS.md:27-28`; `agent-rules.md:10`; Rust core) | PASS | The only new trait implementation is the contract example's adapter for the existing `HttpTransport` boundary (`provider_contract.rs:68-101`); no dependency changed. |
| Validated trust input and no apparent success on missing input (`AGENTS.md:28,30`; `agent-rules.md:7`) | PASS | `live_pr.rs:80-94,106-112,197-202` validates configuration and fails empty context/read mismatch; `provider_contract.rs:133-152,247-252` validates identity/commit and exits on failed cases. PR run-at checks the second base/head read before publication (`pull_request.rs:385-418`). |
| Durable time from store clock; cancellation/partial progress documented (`AGENTS.md:29,32`; design:22; spec REQ-012–015) | PASS | New library entrypoints do not read host time. Example `Instant` only measures latency; each facade call is wrapped in `tokio::time::timeout` (`live_pr.rs:61-70,100-154`). `run_at` documents publication failure on movement (`pull_request.rs:159-172`). |
| Library errors, import style, checked panics (`AGENTS.md:30-31`; `agent-rules.md:11`; Rust core) | PASS | New library methods retain typed errors and bare imported types. `expect("log lock")` states a mutex invariant; input and IO paths use `?`. No new unchecked `unwrap()`. |
| Useful rustdoc for new or materially changed Rust items, including tests (`AGENTS.md:32`; `agent-rules.md:11`; Rust core) | PASS | Fixed-commit test and original cold-journey test each have their own accurate rustdoc (`snapshot.rs:1703-1708,1740-1746`); provider test module has module rustdoc (`provider_contract.rs:327-332`). New fallible methods/functions have `# Errors`. |
| Tests stay inline or cross public boundary; credentialed checks remain out of CI (`agent-rules.md:5`; testing-workflows:3-15; spec REQ-017) | PASS | New unit tests are inline in `snapshot.rs`, `pull_request.rs`, and the example. `live:provider-contract` is an explicitly invoked `mise` lane (`mise.toml:72-74`); `gate` does not depend on live lanes (`mise.toml:77-102`). |
| Preserve source trust and source-free evidence (`AGENTS.md:21-23`; `agent-rules.md:9`; design:5,22; spec REQ-014) | PASS | Provider recorder logs only URL/status/selected headers and reduces redirect locations to host (`provider_contract.rs:56-109`). Report strings inspected across all five JSON evidence files: no long source string, credential or source body field; `live_pr.rs:156-195` emits span metadata rather than text. |
| Gate, test integrity and diff hygiene (`AGENTS.md:38-40`; `agent-rules.md:12`; testing-workflows:15-20) | PASS | Orchestrator ran `mise run gate` at the candidate HEAD, exit 0, including formatting, lints, pipeline tests, public journey, and skills sync. Independent `git diff --check 2257992… 7f7b75d…` exited 0. No changed golden, suppression, skip, or test target removal. |
| Task and evidence follow approved lifecycle/authority (`AGENTS.md:42-46`; `agent-rules.md:14-15`; spec-driven-development reference) | PASS | Approved revision 14 and the maintainer stateless-snapshot decision were present; R3 touches documentation ownership only. This review did not edit the subject. |

## Findings

None. No changed surface violates a written repository rule.

**Overall: PASS.** This is the repository-standards result only; task acceptance and domain correctness belong to their independent reviewers. The credentialed runs are supported by captured reports rather than rerun in this review.

```citations
[]
```
