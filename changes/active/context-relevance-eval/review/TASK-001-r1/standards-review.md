# Repository standards review: TASK-001

Subject: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab` → `c626450649edfdbea105011ab5293267df49ed74`. Reviewed the complete five-file diff. No `.codegraph/` index exists. Applicable authorities: `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, and the spec-driven development, Rust core, testing workflows, and symbolic-context references selected by `architecture/references/README.md`. The approved spec and task were read for boundary applicability, without judging task acceptance here.

## Authority coverage

| Changed surface | Applicable authority |
|---|---|
| `examples/context_relevance/main.rs` GitHub reads, snapshot build, sample output | `AGENTS.md` product/ownership and Rust rules; `architecture/agent-rules.md:3,7,9-11`; design outer-pipeline boundary; Rust core; symbolic-context source boundary |
| `examples/context_relevance/sample.rs` qualification and inline tests | `AGENTS.md` Rust and verification rules; `architecture/agent-rules.md:5,7,10-12`; Rust core; testing workflows |
| Pipeline `Cargo.toml` example registration and `mise.toml` opt-in lane | `AGENTS.md` dependency, verification, and gate rules; `architecture/agent-rules.md:5,12`; testing workflows |
| TASK-001 evidence/status edit | `AGENTS.md` lifecycle and verification rules; `architecture/agent-rules.md:14`; spec-driven development |

## Rule results

| Rule | Result | Evidence |
|---|---|---|
| Pipeline alone performs provider and storage IO; symbolic crates remain unchanged (`AGENTS.md` product boundary; `architecture/agent-rules.md:3,9`) | PASS | The only new executable code is the pipeline example. It calls `GitHubClient`, `ColdBuild`, `Snapshots`, and the public `SymbolicContext`; no symbolic crate changed. |
| Source-free output and no source retention in the symbolic layer (`AGENTS.md` product boundary; `architecture/agent-rules.md:9`) | PASS | The example's output at `main.rs:267-289` contains metadata, paths, and omission reasons. Its snapshot is the existing source-free `Snapshot`; no source blob is fetched by the example. |
| Canonical paths and bounded, checked inputs (`architecture/agent-rules.md:7`; Rust core) | PASS | `GitHubClient::pull_request_files` validates listed paths; `Screen::new` uses `RelativePath` and `parse_hunks`. `main.rs:165-169` uses the public path-language query. The existing cold build classifies incomplete coverage. |
| Concrete workflow owners; no speculative trait or dependency (`AGENTS.md` Rust rules; `architecture/agent-rules.md:10`; Rust core) | PASS | `Api`, `Discovery`, and `Screen` own their respective state. `Cargo.toml` adds only an example target and uses existing dependencies; `Cargo.lock` is unchanged. |
| Meaningful rustdoc on every new Rust item (`AGENTS.md` Rust rules; `architecture/agent-rules.md:11`; Rust core) | FAIL | The newly added test module and its `PATCH` constant in `sample.rs:323,362` have no rustdoc. Other new structs, fields, functions, and tests have intent descriptions, with `# Errors` on fallible functions. |
| Inline owner tests and credential-free fast lane (`architecture/agent-rules.md:5`; testing workflows) | PASS | Tests live inside `sample.rs`; the example is `test = true` and the four focused tests run without network or credentials. The live mise task is outside `gate`. |
| No weakened checks, changed goldens, generated artifacts, or grammar pins (`AGENTS.md`; `architecture/agent-rules.md:6,12`) | PASS | The diff changes none of those surfaces. `git diff --check` exits 0. |
| Task evidence records a focused selector and verification (`spec-driven development`; testing workflows) | PASS | TASK-001 records the focused command, RED/GREEN, and owner, formatting, lint, and gate outcomes. The exact focused command independently ran four tests successfully in this review. |

## Finding

**STANDARDS-1 — VIOLATION: new test items omit required rustdoc.** The root cause is that the new `tests` module and its shared `PATCH` fixture were introduced without item documentation. Siblings: `sample.rs:323` and `sample.rs:362`; no other new Rust item was found with the same omission. `AGENTS.md` Rust and design rules and `architecture/agent-rules.md:11` require useful rustdoc for every new item, including private items and tests. The missing descriptions leave the test boundary and fixture's intended patch semantics implicit. Add concise rustdoc explaining the module's tested contract and the fixture's single-line head-side change. No code or test behavior needs alteration.

## Verification limits and result

The focused command `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::` passed 4/4 tests. `git diff --check` passed. I did not rerun the full gate; the task packet records it passing, and no missing verification result is treated as a finding. No live credentialed run was performed, consistent with the opt-in lane.

Overall: **FAIL** (STANDARDS-1).

```citations
[{"id":"STANDARDS-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":323,"snippet":"mod tests {","symbols":["tests"]},{"id":"STANDARDS-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":362,"snippet":"    const PATCH: &str = \"@@ -1 +1 @@\\n-a\\n+b\";","symbols":["PATCH"]}]
```
