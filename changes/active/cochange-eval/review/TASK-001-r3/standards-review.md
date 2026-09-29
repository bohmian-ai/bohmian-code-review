# Repository standards review — TASK-001 r3

Subject: base `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec`, candidate `ad498ab2c58d0334c2ded38fcc01e835bfcdac48`. Scope: the complete cumulative diff. No CodeGraph index exists. This review does not decide task acceptance.

## Authority coverage

| Changed surface | Applicable authority |
|---|---|
| `examples/context_relevance/{main,sample,history}.rs`: provider reads, in-memory sample/history, example report, inline tests | `AGENTS.md:12-40`; `architecture/agent-rules.md:3-12`; `architecture/bohmian-code-review-design.md:20-38`; `architecture/references/languages/{rust-core,testing-workflows,implementation-execution}.md` |
| `mise.toml`: opt-in command description and verification lanes | `AGENTS.md:36-40`; `architecture/agent-rules.md:5,12`; `architecture/references/languages/testing-workflows.md` |
| `changes/active/cochange-eval/{tasks/TASK-001-history.md,review/TASK-001-r1/*,review/TASK-001-r2/*}`: task and remediation evidence | `AGENTS.md:42-46`; `architecture/agent-rules.md:14-15`; `architecture/references/languages/spec-driven-development.md` |

The workspace manifest and lockfile are unchanged; the example remains the existing `bohmian-code-review-pipeline` Cargo target. No symbolic crate, production pipeline source, grammar, generated artifact, fixture, or public journey changed.

## Rule checks

| Written rule | Evidence | Result |
|---|---|---|
| Pipeline owns Git provider IO; symbolic crates remain IO-free (`AGENTS.md:12-23`, design `:20-22`) | Diff confines runtime changes to the pipeline example. `Api::get` and `Api::listing` in `main.rs:160-223` own provider calls; `sample.rs:243-260` uses the public path query. | PASS |
| Bounded ingest and no unchecked input/IO unwrap (`AGENTS.md:23,30`; design `:22`) | `main.rs:186-194` rejects oversized direct JSON before appending; `main.rs:204-223` caps file-listing pages; `history.rs:34-49` rejects incomplete and invalid listings. No introduced `unwrap()` appears in the diff. | PASS |
| Concrete workflow owners and borrowed inputs (`AGENTS.md:27-28`; agent rules `:10`) | `Api` owns provider workflow in `main.rs:149-483`; `History` owns temporal history in `history.rs:78-154`; `eligible` and `changed` are deterministic transformations. | PASS |
| Imports at module top and bare imported types in signatures (`AGENTS.md:31`; agent rules `:11`) | Imports are at the module top, but new `run` and `record` signatures in `main.rs:511,529` spell `serde_json::Value` without importing `Value`. | FAIL: STD-R3-1 |
| Rustdoc on new/materially changed items, including fallible errors (`AGENTS.md:32`; agent rules `:11`) | New owners and methods in `main.rs:52-612`, `history.rs:17-154`, and changed `sample.rs:213-260,298-397` have intent comments; fallible methods include `# Errors`. | PASS |
| Inline, credential-free tests and unchanged gate (`AGENTS.md:38-40`; agent rules `:5,12`) | New tests are inline in the example modules; `mise.toml:54-59` includes the example in `test:pipeline`; only the opt-in command description changed at `mise.toml:76-78`. No weakened test, skip, lint allowance, or golden change appears in the diff. | PASS |
| Approved change packet and immutable cumulative review (`AGENTS.md:44`; agent rules `:14`; spec-driven development) | Candidate carries the original task and R1/R2 remediation evidence; this review uses the specified base and candidate. | PASS |

Available implementation evidence records passing `test:pipeline`, `fmt:check`, `lints`, `git diff --check`, and `gate` for commit `c12e8b7` in `TASK-001-R2-provider-cost-and-bound.md:34-44`. I independently ran `git diff --check` over the base-to-candidate range; it exited zero. The mechanical lanes cover formatting and lint, but do not enforce the explicit bare-type rule. No live credentialed run is available; that limits experiment verification, not this written-rule audit.

## Finding

**STD-R3-1 — VIOLATION: qualified type in new signatures.** The new `run` and `record` signatures use `serde_json::Value` while `main.rs:43` imports only `serde_json::json`. This violates `AGENTS.md:31` and `architecture/agent-rules.md:11`. Defect class: the two newly introduced function signatures bypass the module's imported bare-type convention; both sibling locations are `main.rs:511,529`. Consequence: the candidate fails a written Rust style rule despite passing mechanical checks. Correction: import `Value` at module top and use `Value` in both signatures; retain the current behavior and tests. The owner shape is otherwise sound.

Overall result: **FAIL**.

```citations
[{"id":"STD-R3-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":511,"snippet":"async fn run(api: &Api, repos: &[Repository]) -> Result<Vec<serde_json::Value>, Box<dyn Error>> {","symbols":["run"]},{"id":"STD-R3-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":529,"snippet":"fn record(repo: &Repository, selected: &Selected, trials: &[Trial]) -> serde_json::Value {","symbols":["record"]}]
```
