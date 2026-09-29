# Repository standards review

Subject: `322a2b9e0984062625d54274798a9c32cb01142d..100249c3031d645ce8db9148b4b6fa43e71481d1` (candidate is current `HEAD`). Review authority: `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, and the applicable reference slices routed by `architecture/references/README.md`. I inspected the complete changed-file list and the governing source, manifests, tests, and consumers. No `.codegraph/` index exists.

## Authority coverage

| Changed surface | Applicable authority | Result and evidence |
|---|---|---|
| Core/index facts and resolver | `AGENTS.md:14-23,27-34`; `architecture/agent-rules.md:3,7-11`; design **Crate direction**, **Public behavior**; Rust core and symbolic-context references | **PASS.** `IndexedFileFacts` carries declaration facts and distinct reference names; `SymbolIndex` retains no source, updates reference postings on replacement/removal, validates before load, and preserves exact ambiguous lookup. The resolver remains an independent symbolic stage. |
| Selector rankers and context packing | `AGENTS.md:18-21,27-32,38-40`; `architecture/agent-rules.md:3-4,7,10-11`; design **Public behavior**, **Verification contract**; Rust core, symbolic-context, and testing references | **PASS.** `select/src/rank.rs` owns `Ranker` and its implementations; `select/src/lib.rs` owns packing. The facade composes them, with no provider IO in the symbolic dependency graph. The changed public journey exercises rank and budget paths. |
| Public symbolic facade and journey | `AGENTS.md:19-21,38-40`; `architecture/agent-rules.md:3-5,7-9`; design **Public behavior**, **Verification contract**; symbolic-context and testing references | **PASS.** `bohmian-code-review/src/lib.rs` re-exports the rankers and composes index, resolution, and selection; `tests/symbolic_context_journey.rs` crosses those stages through the public API with in-memory source. |
| Provider, history, snapshot/update, PR context, storage and pipeline wrapper | `AGENTS.md:23,27-32,38-40`; `architecture/agent-rules.md:3,5,7-11`; design **Crate direction**, **Public behavior**, **Verification contract**; Rust core, testing, symbolic-context, and pipeline-consumer-ergonomics references | **PASS.** Provider and storage IO remain in `bohmian-code-review-pipeline`; snapshots persist source-free facts and history. `ReviewPipeline` uses one object storage and one metadata store, a per-call client, and delegates to the public stage facades. `tests/pipeline_journey.rs` exercises the public boundary with fake provider/stores. Existing store-clock ownership remains unchanged; no added host-clock persistence call appears in the diff. |
| Evaluation crate and live command | `AGENTS.md:23,27-34,40`; design **Crate direction**, **Verification contract**; Rust core and testing references | **PASS.** The crate is compiled behind `eval`, invokes the public `Ranker` through the facade, and uses pipeline provider/transport types; the live binary is outside `gate`. The crate does not enter pipeline or symbolic runtime dependencies. |
| Manifests, docs, and mise lanes | `AGENTS.md:9,23,33-34,40,44`; `architecture/agent-rules.md:6,12-14`; spec-driven-development and testing references; design **Crate direction**, **Verification contract** | **PASS.** `Cargo.toml` adds only the feature-gated eval crate; grammar pins/fixtures and generated artifacts are untouched. `mise.toml` adds `test:eval` to `test:rust` and moves the live command. README and design describe the new public entry point. |

## Rule checks and verification

| Rule | Result | Evidence |
|---|---|---|
| One-way symbolic crate ownership, pipeline as sole IO owner, eval outside runtime (`AGENTS.md:14-23`; `architecture/agent-rules.md:3,9`) | **PASS** | Workspace manifests and changed imports/dependencies; symbolic crates contain no Git, network, storage, or source-retention additions. |
| Validated trust boundaries, complete replacement, source-free/versioned facts (`AGENTS.md:21,28`; `architecture/agent-rules.md:7-9`) | **PASS** | `core/src/facts.rs`, `index/src/lib.rs`, `pipeline/src/{github,history,snapshot,pull_request}.rs`; behavior checks in owner tests and journeys. |
| Concrete workflow owners, typed errors, documented Rust items (`AGENTS.md:27,30-32`; `architecture/agent-rules.md:10-11`) | **PASS** | `ReviewPipeline`, `History`, `CoChangeHistory`, and ranker types own their work; fallible changed operations document `# Errors`. No added unchecked `unwrap`, lint suppression, or ignored test appeared in the diff. |
| Store-clock durable time, no source persistence (`AGENTS.md:21,29`; design **Crate direction**) | **PASS** | History stores provider merge/commit event times as data; no new `SystemTime::now` enters durable metadata. Snapshot and indexed fact records contain paths, names, spans, and history only. |
| Public journey, scoped lanes and full broad-change gate (`AGENTS.md:38-40`; `architecture/agent-rules.md:4-5,12`; testing reference) | **PASS** | Changed symbolic and pipeline journey tests; parent orchestrator reports `mise run gate` exit 0 at candidate `HEAD`, and base-to-candidate `git diff --check` exit 0. The `test:eval` lane is in the aggregate gate. |
| Grammar and generated artifact integrity (`AGENTS.md:33`; `architecture/agent-rules.md:6,13`) | **PASS** | No grammar pins, extraction fixtures, build scripts, or generated skill mirrors changed in the diff; gate includes mirror verification. |

**Overall: PASS.** No material repository-rule finding. Acceptance semantics and domain-specific behavior are left to the other independent reviewers. The live reproduction claim in `intent.md` was not independently rerun because it requires external GitHub access; this does not affect the repository-rule result.

```citations
[]
```
