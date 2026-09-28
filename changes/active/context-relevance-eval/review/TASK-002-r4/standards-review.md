# Repository standards review — TASK-002 R4

Subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `e2914e6d65a203519fefb47872040e1d2e454e20`. I audited the complete cumulative diff, including the original task and R1–R3 packets. The candidate equaled `HEAD`; no `.codegraph/` index exists. This report assesses repository standards, not task acceptance.

## Authority coverage

| Changed surface | Governing authority | Inspected evidence |
|---|---|---|
| `examples/context_relevance/main.rs`: verified blob reads, transient cache, report output | `AGENTS.md:12-34`; `architecture/agent-rules.md:3,7,9-11`; `architecture/bohmian-code-review-design.md` purpose and crate direction; Rust core and symbolic context references | `main.rs:1-410`, pipeline `Cargo.toml`, workspace manifest, `Cargo.lock`, provider/facade consumers |
| `examples/context_relevance/trial.rs`: corpus, ranking, scoring, packing, omissions, inline tests | Same ownership, validation, source-lifetime, rustdoc, and test rules; `architecture/references/domain/symbolic-context.md:3-7`; `architecture/references/languages/testing-workflows.md:3-20` | `trial.rs:1-1694`, `Change::new` through `evaluate`, inline tests, R3 correction and callers |
| `examples/context_relevance/report.rs`: source-free aggregates and inline test | `AGENTS.md:27-32,36-40`; `architecture/agent-rules.md:5,10-12`; Rust core and testing references | `report.rs:1-185`, JSON assembly in `main.rs:378-405` |
| `mise.toml`: live task description | `AGENTS.md:36-40`; testing workflows `:15-20` | `mise.toml:1-105`; unchanged test and gate task definitions |
| Task evidence and R1–R3 review/remediation packets | `AGENTS.md:42-46`; `architecture/agent-rules.md:14-15`; `architecture/references/languages/spec-driven-development.md` task, test, and review sections; implementation execution reference | Approved spec revision 4, original task, R1–R3 packets and evidence; prior reports treated as history, not proof of candidate behavior |

The reference router also selects implementation execution for the task record. Grammar, generated artifacts, symbolic crate implementations, production dependencies, and persisted contracts are untouched.

## Applicable rule results

| Written rule | Result | Candidate evidence |
|---|---|---|
| Pipeline owns Git IO; symbolic crates remain IO-free and stage owners unchanged (`AGENTS.md:14-23`; `architecture/agent-rules.md:3,9`) | PASS | Only the opt-in pipeline example accesses `GitHubClient::blob` at `main.rs:259-267`; `trial.rs:233-348,825-863,961-1050` uses public facade facts and ranking. No production or symbolic crate changed. Experimental B–D scoring remains within the approved evaluator. |
| Validate paths, changed ranges and source presence; do not turn an omitted file into success (`AGENTS.md:28,30`; `architecture/agent-rules.md:7`) | PASS | `RelativePath` is used at source boundaries. `Corpus::new` and `Change::new` return `MissingSource` for missing required blobs (`trial.rs:247-255,583-590`). After a patch parses, facade rank/extraction errors now propagate as `TrialError::Symbolic` (`trial.rs:590-596`); only absent/malformed patches are recorded omissions (`:552-565`). |
| Keep source transient and reports source-free (`AGENTS.md:21-23`; `architecture/agent-rules.md:9`; design purpose) | PASS | `main.rs:227-276` caches verified bytes only during the run; `Trial`, `Outcome`, `VariantSummary`, and `Miss` serialize paths, names, scores, and counts, with no source blob field (`trial.rs:87-149`; `report.rs:17-41`). |
| Concrete workflow owners, borrowed inputs, top-level imports, typed errors, rustdoc and no unchecked input unwrap (`AGENTS.md:27-32`; `architecture/agent-rules.md:10-11`; Rust core reference) | PASS | `Api`, `Corpus`, `Change`, and `Query` own state. Imports are at module tops; new or modified items and fallible methods have intent and error docs. No new trait, dependency, unchecked input `unwrap()`, lint suppression, or ignored test appeared in the diff. |
| Inline, credential-free tests and preserved public journeys; exact named selector; format, lint and gate integrity (`AGENTS.md:36-40`; `architecture/agent-rules.md:4-5,12`; testing workflows `:9-20`) | PASS | Inline evaluator tests at `trial.rs:1053-1694` and `report.rs:117-185`; the R3 named regression's exact selector ran one passing test in this review. R3 evidence records the sequential pipeline, pipeline journey, symbolic journey, formatting, lint, diff, and full gate checks passing. `mise.toml` changes only the live task description. |
| Immutable cumulative review and approved packet (`AGENTS.md:44`; `architecture/agent-rules.md:14`; spec-driven reference) | PASS | Base/candidate are unambiguous; approved spec, original task, and R1–R3 records are present. `git diff --check` on the cumulative range exited 0. |

## Verification and limits

`mise exec -- cargo test --locked -q -p bohmian-code-review-pipeline --example context_relevance --all-features trial::tests::parsed_patch_beyond_verified_head_fails_the_run -- --exact` exited 0: one test passed, 15 filtered out. `git diff --check ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41 e2914e6d65a203519fefb47872040e1d2e454e20` exited 0. Broader lane results are recorded in the R3 task packet and were not rerun by this reviewer. Credentialed live evaluation is outside this standards review.

## Overall result

**PASS.** No material repository-rule finding was established.

```citations
[]
```
