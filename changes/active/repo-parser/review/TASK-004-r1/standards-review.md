# TASK-004 repository standards review

Subject: `a77c64d8194ff3b8e0bc9f51f68b2cc5ae0095ec..f5a6f4bbeae1877083f79f4198316205c75a63e8`. I inspected the complete four-file diff, the pipeline crate's surrounding snapshot, persistence, storage and facade contracts, nearest tests, workspace manifests, `Cargo.lock`, and `mise.toml`. No `.codegraph/` index exists. This report checks written repository rules; task acceptance is reviewed separately.

## Authority coverage

| Changed surface | Governing authority | Coverage |
|---|---|---|
| `update.rs`: `Update`, `Freshness`, and inline tests | `AGENTS.md:21-33,35-39`; `architecture/agent-rules.md:3,5,7-12`; `architecture/bohmian-code-review-design.md` crate direction and source lifetime; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md` | Complete: facade ownership, injected Git/storage IO, versioned scope, bounded fetch, cancellation, rustdoc, test placement. |
| `snapshot.rs`: limits, ingest reuse, cold-build decomposition, publication path, and shared test fixtures | Same Rust, pipeline, source-trust, durability, test and verification rules above; `architecture/agent-rules.md:8-10`; `architecture/references/languages/implementation-execution.md` | Complete: facts and skip reuse, source-free snapshot, verified blob/archive paths, incomplete coverage, record/object/pointer order, fixture changes. |
| `lib.rs` exports | `AGENTS.md:23,27-31`; `architecture/agent-rules.md:3,10-11`; design crate direction | Complete: public outer-pipeline surface and dependency direction. |
| TASK-004 implementation evidence | `AGENTS.md:39,43`; `architecture/agent-rules.md:12-14`; `architecture/references/languages/spec-driven-development.md` task and evidence contract; testing-workflows reference | Complete: candidate identity, scenario map, focused and broader commands, non-goals. |

## Rule results

| Written rule | Result | Source evidence |
|---|---|---|
| Outer pipeline alone owns provider/storage IO; symbolic stages remain behind the public facade (`AGENTS.md:21-23`, `architecture/agent-rules.md:3,9`) | PASS | `update.rs:12-18,35-50` composes `ColdBuild`; `snapshot.rs:765,945-955` uses `SymbolicContext`. No symbolic crate, workspace dependency, or grammar file changed. |
| Concrete owners, inherent stateful workflow, existing injected boundary, no new dependencies (`AGENTS.md:27,33`, `architecture/agent-rules.md:10`) | PASS | `update.rs:35-124` owns update operations and reuses `ColdBuild`; `snapshot.rs:499-680` shares its build and publication path. The diff adds no manifest entry or new trait. |
| Canonical paths, blob identity, source lifetime, and honest incomplete coverage (`AGENTS.md:23,28`, `architecture/agent-rules.md:7-9`) | PASS | `snapshot.rs:765-788` classifies into `RelativePath` keys; `snapshot.rs:869-897` carries facts only at equal path/blob; `snapshot.rs:905-957` fetches and indexes pending bytes; `snapshot.rs:652-680` refuses publication on incomplete coverage. The update does not store source text. |
| Typed errors, documented fallibility and cancellation, imports at module top, no unchecked production unwrap (`AGENTS.md:29-31`, `architecture/agent-rules.md:11`) | PASS | `update.rs:12-18,53-123`; `snapshot.rs:506-680`. Fallible new methods have `# Errors`; `fetch` reports its failure status via ingest data and a boolean. Production additions contain no `unwrap()` or `expect()`. |
| Inline owner tests, credential-free fast lane, no weakened fixtures or gates (`AGENTS.md:32,37-39`, `architecture/agent-rules.md:5,6,12`) | PASS | `update.rs:126-551` adds inline routed-provider/in-memory tests; `snapshot.rs:987-1268` shares only inline fixtures. No extraction fixture, grammar pin, test target, gate, lint allowance, or skip changed. Orchestrator reports the six exact focused update tests passed, `mise run gate` exit 0, and base-to-candidate `git diff --check` exit 0. |
| Immutable review subject and task evidence in the active packet (`AGENTS.md:43`, `architecture/agent-rules.md:14`, spec-driven-development reference) | PASS | The four-file diff adds implementation evidence to `changes/active/repo-parser/tasks/TASK-004-incremental-snapshot-lifecycle.md`; production comments contain no task or agent references. `HEAD` equals the candidate commit. |

## Findings

None. No introduced or materially touched violation of a written repository rule was found.

## Verdict

**PASS** for repository standards. Verification limit: this reviewer relied on the orchestrator's successful `mise run gate` result rather than running a concurrent Cargo command.

```citations
[]
```
