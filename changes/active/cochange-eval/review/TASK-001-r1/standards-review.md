# Repository standards review — TASK-001, round 1

Subject: `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec` → `f880e9e5b2621166f5240f179f2393f836e259ea`.

## Authority coverage

| Changed surface | Governing authority | Coverage |
|---|---|---|
| `examples/context_relevance/main.rs`: fixed repositories, provider reads, snapshots, trials, reporting | `AGENTS.md:12-34,36-40`; `architecture/agent-rules.md:3-11`; `architecture/bohmian-code-review-design.md:20-34,36-38`; `architecture/references/languages/rust-core.md:3-7`; `architecture/references/languages/testing-workflows.md` | Inspected the full base-to-candidate diff, adjacent example modules, pipeline manifest, and owner lane. |
| `examples/context_relevance/history.rs`: history, async reader, inline tests | Same Rust, ownership, path, source-lifetime, and testing authorities; `architecture/references/languages/implementation-execution.md` | Inspected full file and its call from `Api::select`. |
| `examples/context_relevance/sample.rs`: language eligibility, screening, inline tests | `AGENTS.md:25-40`; `architecture/agent-rules.md:5,7,10-12`; `architecture/bohmian-code-review-design.md:24-34`; Rust and testing references | Inspected changed functions and nearby discovery, listing, and qualification tests. |
| `mise.toml`: opt-in command description | `AGENTS.md:36-40`; `architecture/references/languages/testing-workflows.md`; `architecture/bohmian-code-review-design.md:36-38` | Checked task definitions and gate dependencies. |
| `changes/active/cochange-eval/tasks/TASK-001-history.md`: implementation evidence | `AGENTS.md:42-46`; `architecture/agent-rules.md:14`; `architecture/references/languages/spec-driven-development.md` | Checked task evidence against the immutable diff and local checks. |

## Rule results

| Written rule | Result and evidence |
|---|---|
| Pipeline owns provider IO; symbolic crates remain IO-free (`AGENTS.md:12-23`; design `:20-22,32`) | PASS. Only the pipeline example gains provider reads (`main.rs:154-174,240-266`); no symbolic crate or dependency changes. |
| Use public facade and validated paths, preserve verified snapshots/blobs and source lifetime (`AGENTS.md:21-23,28`; agent rules `:7,9`; design `:5,26,32-34`) | PASS. Eligibility calls `SymbolicContext::path_language` (`sample.rs:243-249`); listing paths use `RelativePath`; base snapshots and blobs use existing pipeline owners (`main.rs:207-238,354-376`). Reports contain paths and facts, not source bytes (`main.rs:498-558`). |
| Cohesive stateful owners, stateless free helpers, no speculative dependency (`AGENTS.md:27`; agent rules `:10`; Rust core `:3`) | PASS. `Api`, `Discovery`, `Screen`, and `History` own workflows; `eligible`, `changed`, and `record` are deterministic transformations. Manifest and lockfile are unchanged. |
| Errors and docs on materially changed Rust items (`AGENTS.md:30-32`; agent rules `:11`; Rust core `:7`) | PASS. Changed fallible methods document `# Errors`; input and IO failures use `?` or explicit errors; no new unchecked `unwrap`, lint suppression, or undocumented panic path was found. |
| Inline, credential-free owner tests and named mise lanes (`AGENTS.md:38-40`; agent rules `:5,12`; testing workflows) | PASS. New tests are inline in `history.rs`, `sample.rs`, and `main.rs`. `mise run test:pipeline` passed locally, including 21 context-relevance tests; `mise run fmt:check`, `mise run lints`, and `git diff --check` passed. Existing public symbolic journey and gate definitions are unchanged. |
| Grammar pins, generated artifacts, production APIs, persistence, and dependency graph (`AGENTS.md:12-34`; agent rules `:3,6,13`) | PASS. The diff touches only the opt-in example, its task evidence, and a mise description. No grammar, generated, production API, or persistence file changed. |
| Active packet and immutable review discipline (`AGENTS.md:42-46`; agent rules `:14`; spec-driven development reference) | PASS. Task evidence is in the active packet; this report reviews the stated full immutable base-to-candidate range. |

## Findings

None under the repository-standards scope. Task acceptance and provider behavior belong to the independent task and domain reviews.

## Overall

**PASS.** The owner lane, format check, lints, and diff whitespace check passed locally. Live GitHub behavior was not exercised because the example requires credentials and the repository's fast lanes are credential-free.

```citations
[]
```
