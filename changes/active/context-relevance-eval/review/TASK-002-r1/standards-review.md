# Repository standards review: TASK-002

Subject: `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41` → `e3d194de32f456f5136a214aaf7d8087483e7013`. Scope: complete five-file diff. Result: **PASS**.

## Authority coverage

| Changed surface | Applicable written authority | Inspection |
|---|---|---|
| `examples/context_relevance/main.rs` | `AGENTS.md` product boundary, Rust/design, tests; `architecture/agent-rules.md` ownership, source trust, rustdoc, tests; `architecture/bohmian-code-review-design.md` outer-pipeline and source lifetime; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md`; `architecture/references/domain/symbolic-context.md` | Full file and its `sample` module interface, pipeline crate manifest, new trial/report consumers |
| `examples/context_relevance/trial.rs` | Same Rust, symbolic, outer-pipeline, source and test rules; `architecture/references/languages/implementation-execution.md` verification integrity | Full file including inline tests; public facade calls and pipeline limits |
| `examples/context_relevance/report.rs` | `AGENTS.md` source-free output, Rust/docs/test rules; `architecture/agent-rules.md` rustdoc and test placement; Rust/testing references | Full file including inline test and serialization |
| `mise.toml` | `AGENTS.md` opt-in versus gate verification; `architecture/references/languages/testing-workflows.md` scoped lanes; `architecture/agent-rules.md` test integrity | Full task definitions and changed description |
| `tasks/TASK-002-counterfactual-trials.md` | `architecture/references/languages/spec-driven-development.md` task status, evidence and authority order; implementation-execution reference | Complete task and changed implementation evidence |

`architecture/references/README.md` routes these references. The diff does not touch grammar, manifests, lockfile, generated artifacts, or any production crate; their independent change rules have no changed surface to audit.

## Rule results

| Written rule | Evidence at candidate | Result |
|---|---|---|
| `AGENTS.md` product boundary and `architecture/agent-rules.md` dependency direction: only the outer pipeline may use Git provider IO, while symbolic stages stay behind the public facade | New code is entirely in the pipeline example. `main.rs:257-275` obtains verified blobs through `GitHubClient` and passes them to `evaluate`; `trial.rs:233-277,823-840,974-980` uses `SymbolicContext` and public packing. No production crate or dependency changed. | PASS |
| `AGENTS.md` and design source lifetime: do not persist source or include it in output | `main.rs:255-276` holds blob bytes in a local run cache; `trial.rs:114-149` and `report.rs:15-41` serialize paths, names, ranks, scores and counts. `main.rs:381-399` prints those records and metadata. No storage write or source field was added. | PASS |
| `AGENTS.md` Rust/design and `architecture/agent-rules.md`: cohesive concrete state owners, no speculative traits, top-level imports, borrowed inputs and typed errors | `main.rs:75-86,220-277` keeps provider state on `Api`; `trial.rs:210-223,502-521,672-678` has concrete corpus/change/query owners. Imports are at module tops. Fallible trial work uses `TrialError` (`trial.rs:36-56`); provider command propagates errors. No new trait or dependency. | PASS |
| `AGENTS.md`, `architecture/agent-rules.md`, and Rust-core reference: meaningful rustdoc on new or materially changed Rust items, `# Errors` on fallible functions, named invariants for `expect()` | New structures, methods and helpers have intent documentation; fallible `Api::trials` (`main.rs:220-227`), `Corpus::new` (`trial.rs:225-233`), `Change::new` (`trial.rs:523-534`), `rank_a` (`trial.rs:817-823`), `trial` (`trial.rs:954-959`) and `evaluate` (`trial.rs:1010-1022`) include error documentation. Production `expect()` at `main.rs:195` names the `RelativePath` invariant; remaining new `expect()` calls are named test-fixture invariants. No unchecked `unwrap()` or lint suppression appears in the changed code. | PASS |
| `AGENTS.md` and agent-rules testing: inline owner tests, exact focused selector, no weakened tests, scoped lanes plus formatting/lints and broad gate when warranted | New tests are inline in `trial.rs:1047-1430` and `report.rs:115-185`; the task records exact example selectors and sequential `test:pipeline`, pipeline journey, symbolic journey, `fmt:check`, `lints`, `git diff --check`, and `gate` with exit 0. `git diff --check` was independently clean. No changed gate/test script, ignored test, golden, or lint allowance. | PASS |
| `AGENTS.md` and spec-driven development: approved packet, bounded task, review status, evidence below spec authority | `spec.md` is approved revision 4; task front matter maps revision 4 requirements and changes status to `review`; the evidence appendix records focused and broad checks without editing spec authority. | PASS |
| `AGENTS.md` tooling and grammar pins: opt-in live lane outside normal CI, no new production dependencies, grammar or generated edits | `mise.toml:76-78` changes only the existing live-lane description. `gate` is unchanged; no manifest, lockfile, grammar fixture or generated file appears in the diff. | PASS |

## Findings and verification limits

No material violation of a written repository rule was found. The standards lane inspected source and diff and independently ran `git diff --check`; it did not rerun Cargo-backed checks, whose successful commands are recorded in the task packet. Task acceptance and prototype scoring semantics belong to the separate task and domain review lanes.

```citations
[]
```
