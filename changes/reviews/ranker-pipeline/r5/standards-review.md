# Repository standards review

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d` → `9efafd309207c6cf5dd96b306ce8d22325281e5d` (candidate equals `HEAD` at inspection). **Overall: PASS.** This audits the complete cumulative range for written repository rules, including the R4 typed-error correction. The user supplies `changes/reviews/ranker-pipeline/intent.md` and `architecture/references/domain/pipeline-consumer-ergonomics.md` as review authority in place of an original task. No `.codegraph/` index exists. The untracked `changes/active/impact-map/` was not touched.

## Authority coverage

| Changed surface | Applicable authorities |
|---|---|
| Core facts, index, resolver, selector/rankers, symbolic facade, public journey | `AGENTS.md:14-21,27-34,38-40`; `architecture/agent-rules.md:3-11`; `architecture/bohmian-code-review-design.md` crate direction, public behavior and verification contract; `architecture/references/languages/rust-core.md`; `architecture/references/domain/symbolic-context.md`; `architecture/references/languages/testing-workflows.md` |
| Pipeline provider, history, snapshot/update, context, wrapper, store traits and journey | `AGENTS.md:21-23,27-34,38-40`; `architecture/agent-rules.md:3-11`; design outer pipeline and public behavior; Rust core, testing workflows, symbolic context and `architecture/references/domain/pipeline-consumer-ergonomics.md` |
| Evaluation library, live binary, samples, trials and reports | `AGENTS.md:23,27-34,38-40`; `architecture/agent-rules.md:3,5,10-12`; design evaluation and verification contract; Rust core and testing workflows |
| Workspace manifests, lockfile, mise tasks, README, architecture/reference and review documents | `AGENTS.md:3-9,33-34,38-46`; `architecture/agent-rules.md:6,12-15`; `architecture/references/README.md`; `architecture/references/languages/spec-driven-development.md`; testing workflows |
| Grammar, fixtures and generated artifacts | `AGENTS.md:33-34`; `architecture/agent-rules.md:6,12-13`; testing workflows; the range changes none of these files |

## Written-rule results

| Written rule | Result | Source and verification evidence |
|---|---|---|
| Six symbolic owners, one-way dependency, pipeline-only provider/storage IO, eval through public rankers and pipeline types (`AGENTS.md:14-23`; `agent-rules.md:3,9`) | PASS | `select/src/rank.rs` owns rankers; `bohmian-code-review/src/lib.rs` composes stages; the pipeline manifest depends on the public facade; `eval/src/lib.rs:7-15` states and exports its ranker/provider boundary. No symbolic crate acquired provider or storage IO. |
| Trust-boundary path/range/token validation, source-free versioned facts, complete replacement and no false success for omissions (`AGENTS.md:21,28`; `agent-rules.md:7-9`; design public behavior) | PASS | `core/src/facts.rs` validates loaded records before index replacement; `index/src/lib.rs` replaces only validated facts; pipeline provider/history errors preserve incomplete listings and identity failures; both changed public journeys exercise their boundaries. R4 changes only eval error propagation. |
| Concrete state owners, real trait seams, one store/storage and per-call client (`AGENTS.md:27`; `agent-rules.md:10`; pipeline ergonomics reference) | PASS | `pipeline/src/review.rs:80-95,112-115` owns the wrapper and accepts `GitHubClient` per call. `eval/src/live.rs:140-151` keeps its state on `Api`. `Ranker` has multiple implementations. |
| Typed `thiserror` library errors, no unchecked input/IO unwrap, named-invariant expect, top-level imports and rustdoc (`AGENTS.md:30-32`; `rust-core.md:7`; `agent-rules.md:11`) | PASS | R4 changes the public eval workflow and its private siblings to `LiveError` (`eval/src/live.rs:47-90,161-165,371-379,390-403,410-418,541-546`), retaining typed provider/history/snapshot/sample/trial/serialization causes. `:355-369` checks sample size before the named-invariant `expect`. Imports are at `:25-45`; changed fallible items document `# Errors`. `eval/src/bin/context_relevance.rs` still exits through its displayed error. The repository `gate` passed on this candidate. |
| Store clock for durable time and transient source (`AGENTS.md:21,29`; design outer pipeline) | PASS | Persisted history contains merge time and paths, not source. `pipeline/src/context.rs` confines selected text to review context; no changed store trait passes host time. Pipeline `SystemTime::now` is used for retry timing, not durable metadata. |
| Real public journeys, owner lanes, feature-gated eval and full gate; no disabled check (`AGENTS.md:38-40`; `agent-rules.md:4-5,12`; testing workflows) | PASS | Both public journey files change; `mise.toml` includes `test:eval` in `test:rust` and `gate`. Parent orchestrator reports `mise run gate` passed on candidate; `git diff --check 322a2b9e... 9efafd3...` exited 0 independently. No fixture golden, `#[allow]`, or `#[ignore]` was introduced in the range. |
| Pinned grammar, generated artifact and prohibited-dependency boundaries (`AGENTS.md:33-34`; `agent-rules.md:6,13`) | PASS | No grammar pin, fixture, build script, generated mirror or prohibited production dependency changes. Eval is feature-gated and not a runtime dependency. |
| Immutable review and Git handling (`AGENTS.md:44-46`; `agent-rules.md:14-15`; spec-driven development authority) | PASS | The cumulative subject is committed and unchanged at inspection; production code contains no review packet references. This report makes no subject edit or publication. The user's substitute authority resolves the absent original task. |

**Material findings:** none. **Verification limit:** I did not rerun Cargo lanes during this parallel review; the orchestrator reported a passing candidate gate. The credentialed live command was not run without a token; fake-provider tests are part of the gate.

```citations
[]
```
