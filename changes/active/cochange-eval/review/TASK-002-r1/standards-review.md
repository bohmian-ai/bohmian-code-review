# Repository standards review: TASK-002, round 1

Subject: `3fecce95f055268359d7d1d87df6308ea504329e..1b6d31eeeb7c6b8edd1989abb19f6f2cf161d4c8` (immutable). Scope is repository-rule compliance of the complete diff, not task acceptance.

## Authority coverage

| Changed surface | Applicable authority | Inspected source and consumers |
|---|---|---|
| `trial.rs` ranking and tests | `AGENTS.md` Rust, ownership, source lifetime, and verification rules; `architecture/agent-rules.md` concrete owners, source trust, rustdoc, test integrity; `architecture/bohmian-code-review-design.md` facade and pipeline boundaries; `architecture/references/languages/rust-core.md`, `testing-workflows.md`, `implementation-execution.md`; `architecture/references/domain/symbolic-context.md` | Complete candidate `trial.rs`, base-to-candidate diff, `sample.rs` screening, `main.rs` caller, pipeline manifest and mise lanes |
| `main.rs` trial integration and JSON report | Same Rust, source-trust, pipeline, and testing authorities; `architecture/references/languages/spec-driven-development.md` for task evidence | Complete changed regions, `Api::trials`, `run`, `record`, `History::for_trial`, `Screen::source`, `report::summarize` |
| `report.rs` aggregation and tests | `AGENTS.md` Rust and verification; `architecture/agent-rules.md` rustdoc and test integrity; `architecture/references/languages/rust-core.md`, `testing-workflows.md` | Complete candidate file, diff, `Trial::arms`, `record` consumer |
| `TASK-002-ranking.md` evidence | `AGENTS.md` change lifecycle; `architecture/agent-rules.md` immutable review; `architecture/references/languages/spec-driven-development.md`, `implementation-execution.md` | Task diff and evidence; `mise.toml`, pipeline manifest, workspace lockfile |

The reference router at `architecture/references/README.md` selects these Rust, testing, execution, spec-development, and symbolic-context references. Grammar and generated-artifact rules were checked for scope; neither surface changed. No `.codegraph/` index exists at the repository root.

## Applicable rule results

| Written rule | Result | Source evidence |
|---|---|---|
| `AGENTS.md` Product and ownership boundaries; `architecture/agent-rules.md:3` preserve symbolic owners and pipeline direction | PASS | Diff changes only the pipeline example and task Markdown. `trial.rs:14-18,133-173,201-209,311-315` uses the public symbolic facade; no production symbolic or pipeline library API changed. |
| `AGENTS.md` Rust and design rules; `architecture/agent-rules.md:8`; `rust-core.md` concrete owners, borrowing, no speculative traits | PASS | Existing `Api`, `Corpus`, `Change`, and `Trial` own state; `ranked`, `rank_c`, `rank_e`, `rank_f`, `chosen`, and `draw` are stateless functions in `trial.rs:340-550`. No dependency or trait was added. |
| `AGENTS.md` Rust and design rules; `architecture/agent-rules.md:9`; `rust-core.md` imports, checked errors, rustdoc | PASS | Imports are module-top in all changed Rust files. Changed items have intent docs (`trial.rs:23-35,59-123,133-146,257-267,509-522`; `main.rs:333-340,500-505`; `report.rs:1-24`). Fallible operations have `# Errors`; input and IO paths return errors, and no new unchecked `unwrap()` is used. |
| `AGENTS.md` source and coverage boundaries; `architecture/agent-rules.md:5-7`; design Public behavior | PASS | `main.rs:348-395` gets hash-verified blobs through the pipeline client, builds borrowed views, and feeds the facade. `trial.rs:147-173` requires every indexed base blob and `trial.rs:305-315` rejects a missing or invalid required head. `Screen::new` in `sample.rs:362-375` records missing or malformed patches as explicit omissions; `main.rs:529-546` retains that record. Trial records hold no source (`trial.rs:83-99`). |
| `AGENTS.md` Tests and verification; `architecture/agent-rules.md:2-3,10`; `testing-workflows.md` journey, inline tests, no weakened gates | PASS | New checks are inline in `trial.rs:554-1079` and `report.rs:34-74`, use in-memory source and fake provider history, and cover failure paths. Public journey and production tests were not deleted or changed. Task evidence records `test:pipeline`, symbolic journey, `fmt:check`, `lints`, and `gate` passing. The candidate diff passes `git diff --check`. |
| `AGENTS.md` Change lifecycle; `architecture/references/languages/spec-driven-development.md` task evidence and immutable review | PASS | `TASK-002-ranking.md` adds an evidence section without changing requirement or acceptance text. It records exact candidate, checks, and the missing observed RED phase. No plan or agent references entered production code. |
| `AGENTS.md` grammar, generated artifacts, forbidden dependencies, Git publication; `architecture/agent-rules.md:4,11,13` | PASS | Complete name-status diff has only the four listed example/task files; no grammar, generated file, manifest, lockfile, mirror, or publication change. |

## Material findings

None. The task evidence says scenario-specific RED failures were not separately observed. This limits process evidence but does not establish an introduced code or test-integrity violation; no assertion or gate was weakened.

Overall result: **PASS** for repository standards. This does not decide TASK-002 acceptance.

```citations
[]
```
