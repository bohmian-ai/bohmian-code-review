# Repository standards review — TASK-001 R2

Subject: base `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec`, candidate `20fa48d5cfecf0d4970d325d7334e0aede671728`. Reviewed the complete cumulative diff. No `.codegraph/` index exists. This report assesses repository rules, not task acceptance.

## Authority coverage

| Changed surface | Applicable authority and inspected evidence |
|---|---|
| `examples/context_relevance/main.rs`: fixed repositories, direct API reads, selection, trials, reporting and fake transport | `AGENTS.md:21-34,36-40`; `architecture/agent-rules.md:3-12`; `architecture/bohmian-code-review-design.md:20-22,32,36-38`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md`. Inspected full changed owner, its call sites, `github.rs` request limits and reader, manifest, and inline tests. |
| `examples/context_relevance/history.rs`: history filtering and file-listing validation | Same Rust, pipeline ownership, source trust, and testing authorities; inspected full new file and its callers in `main.rs`. |
| `examples/context_relevance/sample.rs`: source eligibility and screening | Same Rust, path-validation, symbolic-facade, and testing authorities; inspected changed code, nearby screening code, and inline tests. |
| `mise.toml`, task and prior remediation evidence under `changes/active/cochange-eval/` | `AGENTS.md:36-46`; `architecture/references/languages/spec-driven-development.md`; `architecture/references/languages/testing-workflows.md`. Inspected task definition, named lanes, active task, and R1 remediation packet. Prior review reports are historical evidence, not authority. |

## Rule results

| Written rule | Result | Evidence |
|---|---|---|
| Pipeline owns provider IO; symbolic crates remain IO-free and dependency direction is preserved (`AGENTS.md:13-23`; design `:20-22`) | PASS | Rust changes are confined to the pipeline example; no crate manifest, symbolic crate, or lockfile changed. `main.rs:26-35` uses the public facade and pipeline client. |
| Provider ingest is bounded (`AGENTS.md:23`; design `:22`) | **FAIL** | `main.rs:183-187` accumulates every direct REST response chunk without the existing `RequestLimits::max_json_bytes` bound. See STANDARDS-1. |
| Canonical paths and effective source eligibility are checked at ingress (`AGENTS.md:28`; `architecture/agent-rules.md:7`) | PASS | `sample.rs:246-259` uses `RelativePath` plus the public `path_language` query; `main.rs:436-438` supplies each repository's languages and suffixes. `Listing::push` rejects repeated paths. |
| Concrete stateful owners, existing traits only for real implementations, and no added dependency (`AGENTS.md:27,34`; `architecture/agent-rules.md:10`) | PASS | State stays on `Api`, `Discovery`, `History`, and `Screen`. `HttpTransport` already has live and routed fake implementations; `Cargo.toml` and `Cargo.lock` are unchanged. |
| Imports, error handling, rustdoc, grammar provenance, and test integrity (`AGENTS.md:30-34`; `architecture/agent-rules.md:6,11-12`) | PASS | Imports are at module tops; new functions and tests have intent comments and fallible functions have `# Errors`; no unchecked `unwrap`, lint suppression, grammar edit, generated edit, or golden rewrite appears in the diff. |
| Inline owner tests, credential-free fast lane, and unchanged gate (`AGENTS.md:36-40`; `architecture/agent-rules.md:5`; testing reference) | PASS | Tests are inline in the example modules and use routed fake HTTP. `mise.toml` changes only the opt-in live task description. Implementation evidence records `test:pipeline`, `fmt:check`, `lints`, and `gate` passing; I independently ran `git diff --check` on the immutable range (pass). |
| Approved change packet and immutable-review discipline (`AGENTS.md:44-46`; spec-driven reference) | PASS | The cumulative candidate includes the active task and R1 remediation evidence. This review made no source changes. |

## Finding

**STANDARDS-1 — direct provider JSON is accumulated without a byte bound.** Classification: VIOLATION. Root cause and defect class: the direct `Api::get` response reader has no size check before `Vec::extend`, unlike the pipeline client's bounded JSON reader. Its sibling call paths are repository metadata (`select`), merged-PR listing pages (`select`), and sampled base-commit times (`base_time`); all reach the same owner. The rewritten reader at `main.rs:183-187` can grow with an arbitrarily large provider body before parsing or failing. This violates the explicit bounded-ingest ownership rule in `AGENTS.md:23` and `architecture/bohmian-code-review-design.md:22`. A provider response larger than the configured JSON limit can consume unbounded memory on the live command. Correct at `Api::get`, using the existing `RequestLimits::max_json_bytes` policy and rejecting excess bytes before appending; one focused fake-body check should cover the shared reader. The base version also had an unbounded direct reader, but this candidate materially rewrote that exact owner and retained the violation.

Overall: **FAIL**. Verification limit: I did not run Cargo lanes concurrently with other reviewers; their recorded results are available evidence, not independently rerun here.

```citations
[{"id":"STANDARDS-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":185,"snippet":"            bytes.extend(chunk);","symbols":["Api","get","bytes"]}]
```
