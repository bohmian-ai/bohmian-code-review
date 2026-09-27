# TASK-007 repository standards review

Subject: `2257992f36a2aa88f33f397af1e070d6ab35961b..f820d662a841f7e13773223b2f93cf90e82b6274`; approved `changes/active/repo-parser/spec.md` revision 14 and `review/maintainer-decision-stateless-snapshot.md`. I inspected the complete name/status and non-JSON diff, parsed every changed JSON report, and traced the changed Rust through its pipeline owners and consumers. `.codegraph/` is absent. I did not read other r2 reports.

## Authority coverage

| Changed surface | Governing authority | Coverage |
|---|---|---|
| `snapshot.rs`: fixed-commit cold build and inline test | `AGENTS.md:23-34,38-40`; `architecture/agent-rules.md:3,5,9-12`; `architecture/bohmian-code-review-design.md:19-22,38`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md`; revision 14 source and capacity rules; stateless-snapshot decision | Inspected new method and test with `run`, `pin`, `build`, existing fake provider, manifest, and callers. |
| `examples/live_pr.rs`: optional fixed commit | `AGENTS.md:23,27-34,40`; `architecture/agent-rules.md:3,5,7,9-12`; design `:19-22,38`; Rust and testing references | Traced `BCR_COMMIT` validation and cold call into `run_at`; checked live lane remains separate from gate. |
| `examples/provider_contract.rs`: controlled provider check | Same outer-crate, source-trust, Rust and testing rules; `architecture/references/languages/implementation-execution.md` evidence discipline | Inspected complete example, production transport use, report redaction, provider API methods, and the recorded case outcomes. |
| `mise.toml`: new opt-in lane | `AGENTS.md:40`; `architecture/agent-rules.md:5,12`; testing-workflows reference | Checked lane body, Cargo target/features, gate dependencies, and absence of credentialed lane from gate. |
| TASK-007 and R1 task/evidence/review artifacts | `AGENTS.md:40,44`; `architecture/agent-rules.md:12,14`; design `:38`; spec-driven-development and implementation-execution references | Inspected cumulative task change and R1 correction, prior review packet, all five current JSON report schemas and outcomes, deletion of superseded wyrd report. |
| Symbolic crates, grammar, generated artifacts | `AGENTS.md:14-21,33`; `architecture/agent-rules.md:3,6,9`; design crate direction; symbolic-context reference | No such production surface changed; pipeline manifest still depends on the public facade alone. |

## Rule results

| Written rule | Result | Source and verification evidence |
|---|---|---|
| Git and storage IO belong to the outer pipeline; symbolic stages stay below the public facade (`AGENTS.md:14-23`; `architecture/agent-rules.md:3`; design `:19-22`) | PASS | Only pipeline source changed; `ColdBuild::run_at` reuses `build`; pipeline manifest retains `bohmian-code-review` as its sole symbolic dependency. |
| Stateless, source-free snapshot; verified Git identity and blob input (`AGENTS.md:23`; `architecture/agent-rules.md:9`; maintainer decision) | PASS | `run_at` verifies repository and commit through the existing client, then calls the same `build` owner. It adds no persisted field or source storage. Report spans contain path, blob, range, names, score and token count, with no text/source/content/body key. |
| Concrete stateful owner; no speculative trait or dependency (`AGENTS.md:27`; `architecture/agent-rules.md:10`; rust-core reference) | PASS | `ColdBuild` owns the new workflow. `Recording` implements the existing `HttpTransport` test/contract boundary; no dependency or generic storage interface was added. |
| Validate inputs and handle IO errors (`AGENTS.md:28,30`; `architecture/agent-rules.md:7`; rust-core reference) | PASS | `BCR_COMMIT` passes through `ObjectSha::new`; provider and storage calls propagate typed errors. Contract command parses required settings and exits nonzero on a failed case. |
| Imports, rustdoc and panic conventions (`AGENTS.md:30-32`; `architecture/agent-rules.md:11`) | PASS | Imports are at module top; new fallible functions have `# Errors`; test `expect` calls name fixture invariants. One copied scenario sentence in the new test's rustdoc describes the adjacent cold-build test; this is editorial and does not obscure the fixed-commit intent stated immediately after it. |
| Keep fast lanes local and credential-free; use mise lanes and retain gates (`AGENTS.md:38-40`; `architecture/agent-rules.md:5,12`; testing-workflows reference) | PASS with verification limit | `live:provider-contract` and `live:pr` are opt-in and absent from `gate`; the inline fixed-commit test is local. `git diff --check 2257992 f820d66` passed. The task and R1 records report focused pipeline, format, lint and gate passes; I did not rerun Cargo while the orchestrator runs the gate once. |
| Benchmark claims need named environment and reproducible measurements (design `:38`) | PASS | TASK-007 records macOS/Apple silicon/release/network, exact fixed-commit commands for wyrd, VS Code and Rust, commits/trees, archive size, cold/warm timings, RSS, snapshot size and provider calls. `run_at` pins the requested commit after a fake default-head advance. |
| Preserve task authority and immutable cumulative review (`AGENTS.md:44`; `architecture/agent-rules.md:14`; spec-driven-development reference) | PASS | Task and R1 stay tied to approved revision 14; the five JSON reports record pinned evidence and the old wyrd report is explicitly superseded. No source or test assertion was changed to fit evidence. |
| Grammar pins, generated outputs, forbidden production dependencies and host-clock rule (`AGENTS.md:29,33-34`; `architecture/agent-rules.md:6`) | PASS / not touched | No grammar, generated artifact, manifest, durable-time path or prohibited dependency changed. |

## Findings and result

No material violation of a written repository rule was introduced or materially touched. Overall **PASS**, subject to the orchestrator's independent gate result. The live 403/429 and GHES cases remain separately identified as local fake-transport proof; the contract report does not claim to have provoked them on GitHub.

```citations
[]
```
