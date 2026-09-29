# TASK-002 round 2 repository standards review

Subject: base `3fecce95f055268359d7d1d87df6308ea504329e`, candidate `4196bbd36a150e26668bed32b2d80c90d07152a0`. Reviewed the complete cumulative diff, the approved revision 2 specification, task and R1 packet as authority/context, the changed example and its tests, `mise.toml`, workspace manifests, and `Cargo.lock`. This is a repository-rule audit, not a task-acceptance verdict.

## Authority coverage

| Changed surface | Applicable written authority |
|---|---|
| `trial.rs`: corpus, changed-file preparation, C/E/F scoring, selection, serialization | `AGENTS.md` product boundaries, Rust/design, tests; `architecture/agent-rules.md` validation, concrete owners, rustdoc, tests; `architecture/bohmian-code-review-design.md` public facade and source-lifetime boundaries; `architecture/references/languages/rust-core.md`; `architecture/references/domain/symbolic-context.md`; `architecture/references/languages/testing-workflows.md` |
| `main.rs`: history handoff, blob access, report record | Same product/source boundaries and Rust rules; `architecture/bohmian-code-review-design.md` pipeline IO owner; `architecture/references/languages/testing-workflows.md` |
| `report.rs`: C/E/F aggregate and inline test | Rust item, error, and test-placement rules in `AGENTS.md`, `architecture/agent-rules.md`, Rust core and testing-workflows references |
| `TASK-002-ranking.md` and R1 review packet | `AGENTS.md` change lifecycle; `architecture/agent-rules.md` immutable cumulative review; `architecture/references/languages/spec-driven-development.md` task, review, and evidence contracts |

## Rule results

| Written rule | Result and evidence |
|---|---|
| Six symbolic crates stay IO-free; only the outer pipeline owns provider IO; example uses the public facade rather than changing a production stage (`AGENTS.md`, Product and ownership boundaries; design, Crate direction). | **PASS.** The base-to-candidate code diff is confined to `crates/bohmian-code-review-pipeline/examples/context_relevance/{main,report,trial}.rs`. `main.rs:341-395` uses the existing pipeline client and passes a `SymbolicContext` to `evaluate`; `trial.rs:201-209,311-315` uses the public facade. No production crate or dependency graph changed. |
| Source is transient and reports source-free; no clone, checkout, write, or network in symbolic stages (`AGENTS.md`, Product and ownership boundaries; design, Purpose). | **PASS.** `trial.rs:124-130` retains facts and paths, not source bytes; `trial.rs:527-556` returns only `Trial` records; `main.rs:523-579` serializes paths, ranks, counts and metadata. Blob reads remain in the outer example at `main.rs:369-395`. |
| Validate effective paths, ranges, and supplied input; no unchecked input/IO unwrap, typed errors (`AGENTS.md`, Rust and design rules; `architecture/agent-rules.md`; Rust core). | **PASS.** Paths arrive as `RelativePath`; missing base/head bytes and symbolic failures are typed in `trial.rs:37-57,153-161,305-315`. The example uses the facade to validate changed-line input. Test `parsed_patch_beyond_verified_head_fails_the_run` covers that boundary. Existing patch omissions are recorded by the sample/report path at `main.rs:539-548`. |
| Use concrete owners, borrowed inputs, top-level imports, and useful rustdoc with `# Errors` for fallible items (`AGENTS.md`, Rust and design rules; `architecture/agent-rules.md`; Rust core). | **PASS.** `Corpus` and `Change` own their workflows; `ranked`, `rank_c`, `rank_e`, `rank_f`, `draw`, and `chosen` are stateless. New and materially changed items, including tests, have intent-oriented docs; fallible `Corpus::new`, `Change::new`, `extract`, `evaluate`, and `Api::trials` document errors. Imports are at module tops. |
| Preserve public journey; inline owner tests, no credential/network requirement; run owner lane, journey, format/lints, full gate for broad changes (`AGENTS.md`, Tests and verification; `architecture/agent-rules.md`; testing workflows). | **PASS for placement and current mechanical checks; verification limit for full gate.** New checks are inline in `trial.rs` and `report.rs`, with the existing fake-provider command test in `main.rs`; the public symbolic journey target remains present in `mise.toml`. I ran `mise run fmt:check` and `mise run lints`, both exit 0, and `git diff --check` exits 0. The task and R1 evidence record passing `test:pipeline`, `test:symbolic-context:journey`, and `gate`; I did not rerun those lanes during this standards audit. |
| Keep grammar pins, generated artifacts, manifests, and gates intact; do not weaken tests (`AGENTS.md`, Rust/design and Tests; `architecture/agent-rules.md`). | **PASS.** No grammar, generated file, manifest, lockfile, mise lane, golden, or production test changed. The changed tests add coverage without disabling checks. |
| Review an immutable cumulative candidate against approved authority (`architecture/agent-rules.md`; spec-driven-development reference, Review and remediation). | **PASS.** The audited candidate is the named commit, including original implementation and R1 correction; the reviewed source was not edited. R1 artifacts and task evidence are present in the diff. |

## Findings

None. Overall repository-standards result: **PASS**. This result is limited to written repository rules and the checked diff; it makes no independent task-acceptance claim.

```citations
[]
```
