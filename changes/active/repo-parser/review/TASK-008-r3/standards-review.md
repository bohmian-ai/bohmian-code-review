# TASK-008-r3 repository standards review

Subject: `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c..f56ec0e3091f1458e80c7addb3286577527f638f`. Authority: approved `changes/active/repo-parser/spec.md` revision 14 and `TASK-008-r1/maintainer-decision-error-regions.md` (FIND-008-1 invalid). I inspected the complete cumulative diff, not only R2. This report checks repository rules, not task acceptance or the Ponytail gate.

## Authority coverage

| Changed surface | Applicable written authority and inspected evidence |
|---|---|
| Active spec/task, two prior review packets, live-run evidence | `AGENTS.md` Change lifecycle and Git; `architecture/agent-rules.md` immutable task review; `architecture/references/languages/spec-driven-development.md` Authority, Task contract, Review and remediation. Read revision 14, the maintainer decision, original task, R2 task, review artifacts, and evidence shape. |
| Native language walkers, composite Svelte, extractor errors/version, parser pool | `AGENTS.md` Product and ownership boundaries, Rust and design rules; `architecture/agent-rules.md` crate ownership, grammar pins, rustdoc, error isolation; `architecture/bohmian-code-review-design.md` crate direction and source lifetime; `architecture/references/languages/rust-core.md`. Inspected all changed walker hunks, the extractor/error/pool diff, and remaining error/defer references. |
| Corpus tests and manifest | `AGENTS.md` grammar/golden and testing rules; `architecture/agent-rules.md` fixture and test integrity; `architecture/references/languages/testing-workflows.md`; `architecture/references/domain/symbolic-context.md`. Inspected the manifest and corpus diff, fixture expectations, and unchanged grammar pins/goldens. |
| Public facade and journey | `AGENTS.md` six-crate boundary and public journey; `architecture/agent-rules.md` public journey and source-free facts; `architecture/bohmian-code-review-design.md` facade and exported-fact contract; testing and symbolic-context references. Inspected facade, journey, and index consumer/projection. |
| Snapshot ingest/reuse, coverage tests, live PR example | `AGENTS.md` outer-pipeline IO ownership, durable time, Rust and verification rules; `architecture/agent-rules.md` source lifetime and concrete owners; `architecture/bohmian-code-review-design.md` pipeline boundary; Rust/testing references. Inspected changed snapshot paths and tests, example, workspace manifests, `Cargo.lock`, and `mise.toml`. |

## Rule results

| Written rule | Result and evidence |
|---|---|
| `AGENTS.md` and `architecture/bohmian-code-review-design.md`: symbolic crates remain IO-free; outer pipeline alone owns Git/storage IO and consumes the public facade | PASS — walker and facade changes add no IO or dependency; snapshot still calls `SymbolicContext`; manifests and lockfile are unchanged. |
| `AGENTS.md` and `architecture/agent-rules.md`: preserve exact grammar pins, vendored grammar input, checked-in goldens, and generated artifacts | PASS — no grammar, generated C, or golden file changed. The manifest's malformed rows switch `defer` to `recover` under approved revision 14; clean `match` rows remain unchanged. |
| `AGENTS.md`, agent rules, and Rust-core reference: concrete workflow owners, borrowed input, typed errors, validated paths, top-level imports, meaningful rustdoc, and named panic invariants | PASS — extraction remains with walkers/`Extractor`; no new trait or unchecked input/IO unwrap appeared. `Ingest::settle` documents its `expect` invariant, and new/modified items have intent and applicable error/panic docs. |
| `architecture/agent-rules.md` and design: malformed file handling is file-local; source-free facts carry an extraction version and reject incompatible records | PASS — file-local recovery remains in extractor/parser pool; the extraction version changes, and the public journey checks rejection of prior-version records. |
| `AGENTS.md` and `architecture/references/languages/testing-workflows.md`: public journey plus owner checks; no weakened gate, suppressed lint, or rewritten golden | PASS — the external facade journey and owner tests are updated; `mise.toml` targets still exist; diff contains no gate/lint suppression or golden rewrite. The task's recorded verification lists `test:extract`, `test:symbolic-context:journey`, `test:pipeline`, `fmt:check`, `lints`, `gate`, and `git diff --check`; I independently ran `git diff --check` on this range (pass). Cargo lanes were not rerun in this standards pass. |
| `architecture/references/languages/spec-driven-development.md` Authority and `AGENTS.md` rustdoc rule: approved spec outranks task wording and changed documentation must describe true invariants | PASS — R2 updates task outcome/scenario/stop condition and recovery rustdoc to permit declarations recovered inside error regions, matching approved REQ-002 and the maintainer decision. |
| `AGENTS.md` no source retention or publication without authority | PASS — exported facts remain declaration-only and source-free; the added live evidence contains path/blob identities and metrics, and the diff does not publish or push. |

Material repository-rule findings: none. Verification limit: this standards pass relies on the task's recorded Cargo lane results; it did not rerun those lanes. Overall result: **PASS**.

```citations
[]
```
