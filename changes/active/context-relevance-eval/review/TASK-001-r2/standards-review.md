# Repository standards review — TASK-001 round 2

Subject: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab` → `a1342a5a3c154b7678cf59764dab1ceb0d70cc76`. Reviewed the cumulative diff, including the TASK-001-R1 correction. The candidate was `HEAD` at inspection. No `.codegraph/` index exists.

## Authority coverage

| Changed surface | Applicable authority | Result |
|---|---|---|
| Opt-in evaluator `main.rs`, provider and snapshot calls | `AGENTS.md:21-34,38-40`; `architecture/agent-rules.md:3,5,7,9-11`; `architecture/bohmian-code-review-design.md` crate direction and source lifetime; `architecture/references/languages/rust-core.md:3-7`; `architecture/references/languages/implementation-execution.md:207-222` | **FAIL** on workflow ownership; other inspected rules pass |
| Sampler `sample.rs`, including R1 listing cap and inline tests | `AGENTS.md:25-34,38-40`; `architecture/agent-rules.md:5,7,10-12`; `architecture/references/languages/rust-core.md:3-7`; `architecture/references/languages/testing-workflows.md:3-19` | PASS |
| Pipeline `Cargo.toml`, `mise.toml`, `Cargo.lock` relationship | `AGENTS.md:9,33-34,38-40`; `architecture/agent-rules.md:5-6,12`; `architecture/references/languages/testing-workflows.md:15-19` | PASS |
| Task evidence and prior remediation packet | `AGENTS.md:42-46`; `architecture/agent-rules.md:13-14`; `architecture/references/languages/spec-driven-development.md` task and review contracts; `architecture/references/languages/implementation-execution.md:114-137,265-284` | PASS |

## Rule results

| Rule and authority | Source and verification evidence | Result |
|---|---|---|
| Concrete owner for stateful workflows; free functions only for stateless transformations — `AGENTS.md:27`, `architecture/agent-rules.md:10`, `architecture/references/languages/rust-core.md:3` | `main.rs:96,117,156,197,251` define IO and multi-step sampling workflows as free functions; `Api` owns only direct REST reads. | **FAIL** |
| Outer pipeline owns Git/storage IO; symbolic crates remain IO-free — `AGENTS.md:14-23`, `architecture/agent-rules.md:3,9` | All added provider/storage calls are in the pipeline example; no symbolic crate changed. `base_snapshot` uses `ColdBuild` and `Snapshots`. | PASS |
| Validated path boundary and file-local omissions — `AGENTS.md:28`, `architecture/agent-rules.md:7` | `sample.rs:284-320` consumes provider-validated `PullRequestFile.path: RelativePath`; missing/malformed patches are recorded as omissions. | PASS |
| Rustdoc, fallible-function errors, imports, unchecked unwrap — `AGENTS.md:30-32`, `architecture/agent-rules.md:11` | New functions and tests have intent docs; fallible functions state `# Errors`; no input `unwrap`; imports are at module tops. R1 added docs to test module and `PATCH`. | PASS |
| No grammar/generated/dependency drift — `AGENTS.md:33-34`, `architecture/agent-rules.md:6,12` | Diff changes no grammar, generated file, workspace dependency, or `Cargo.lock`; example reuses locked `reqwest`, `serde`, `thiserror`, and `tokio`. | PASS |
| Inline, credential-free focused test; opt-in live lane outside gate — `AGENTS.md:38-40`, `architecture/agent-rules.md:5`, `architecture/references/languages/testing-workflows.md:15-19` | Five inline tests pass via `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::`; `mise.toml` adds only `live:context-relevance`, with no `gate` edit. Candidate evidence records passing pipeline, format, lint, and gate lanes. | PASS |
| Approved task packet and immutable cumulative review — `AGENTS.md:44`, `architecture/agent-rules.md:14`, `architecture/references/languages/spec-driven-development.md` review contract | Task and R1 packet are tracked; this inspection uses the full base-to-candidate diff rather than only R1. | PASS |

## Material finding

**STD-R2-1 — IO workflows are free functions.** Classification: repository-rule violation. Root cause and defect class: the opt-in command has no cohesive sampler owner for the multi-step Git read, snapshot, qualification, and sample-selection workflow, leaving these stateful operations as free functions. The same rule violation occurs at `main.rs:96` (`listing`), `:117` (`base_snapshot`), `:156` (`qualify`), `:197` (`select`), and `:251` (`run`). `main` is the required entry point and is excluded; `is_timestamp` is a permitted pure helper. This violates `AGENTS.md:27`, `architecture/agent-rules.md:10`, and `architecture/references/languages/rust-core.md:3`. Consequence: the candidate introduces a workflow shape explicitly prohibited by the repository design rule, even though the focused behavior tests pass. Testable correction: place the sampling workflow on one cohesive concrete owner that holds its provider and configuration state, leave stateless timestamp validation free, and show the same five focused tests plus format/lints still pass. This is one class with all sibling sites, not five findings.

No other material repository-rule finding was found. The focused test ran successfully: 5 passed, 0 failed. The live lane needs credentials and was not run; its completion belongs to later task evidence.

Overall: **FAIL**.

```citations
[{"id":"STD-R2-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":96,"snippet":"async fn listing(","symbols":["listing"]},{"id":"STD-R2-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":117,"snippet":"async fn base_snapshot(","symbols":["base_snapshot"]},{"id":"STD-R2-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":156,"snippet":"async fn qualify(","symbols":["qualify"]},{"id":"STD-R2-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":197,"snippet":"async fn select(","symbols":["select"]},{"id":"STD-R2-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":251,"snippet":"async fn run() -> Result<(), Box<dyn Error>> {","symbols":["run"]}]
```
