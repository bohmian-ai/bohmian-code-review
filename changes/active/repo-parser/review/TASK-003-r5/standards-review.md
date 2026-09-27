# Repository standards review — TASK-003 r5

Subject: base `c74871cbc78140820c9b0a5fda60e0952c4265e4`, candidate `3ada5c59931f236716ed027d773a2746ccf3c3af`. I reviewed the complete cumulative diff, including the original task, R4 remediation, and pipeline code. This report addresses written repository rules, not task acceptance.

## Authority coverage

| Changed surface | Applicable authority |
|---|---|
| `archive.rs`, `github.rs`, pipeline `Cargo.toml` | `AGENTS.md:21-23,27-33`; `architecture/agent-rules.md:3,7-12`; `architecture/bohmian-code-review-design.md:5,22`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md` |
| `snapshot.rs`, `persistence.rs`, `testing.rs` | `AGENTS.md:21-23,27-33,35-39`; `architecture/agent-rules.md:3-5,7-12`; design:22,32; Rust core and testing-workflows references |
| `eligibility.rs`, `lib.rs` | `AGENTS.md:21-23,27-33,35-39`; `architecture/agent-rules.md:3,5,7,9-12`; design:22,32; Rust core and testing-workflows references |
| Original task and R4 review/evidence packet | `AGENTS.md:41-45`; `architecture/agent-rules.md:14-15`; `architecture/references/languages/spec-driven-development.md`; `architecture/references/languages/implementation-execution.md` |

The router is `architecture/references/README.md`. The symbolic-context reference, grammar rules, and generated-output rules have no changed owner here; the public symbolic crates, grammar pins, checked-in fixtures, and generated files are unchanged.

## Rule results

| Written rule | Result | Evidence |
|---|---|---|
| Pipeline alone owns Git/storage IO; it consumes the public symbolic facade, with no duplicated symbolic stage (`AGENTS.md:21-23`; agent rules:3; design:22) | PASS | All production changes are in the pipeline crate. `snapshot.rs:799-816` calls `SymbolicContext::upsert`; `snapshot.rs:566` exports facade facts. No symbolic crate changed. |
| Source remains transient and exported snapshots source-free; paths and blobs are validated (`AGENTS.md:21-23,28`; agent rules:5-9; design:5,22,32) | PASS | `archive.rs:178-194` validates archive paths as `RelativePath` and bounds buffered files; `archive.rs:218-222` hashes bytes before releasing them; `snapshot.rs:155-169` stores facts and identities rather than source. The task and domain reviews own behavioral edge cases at this boundary. |
| Stateful workflows use concrete owners; consumer traits are narrow injected boundaries (`AGENTS.md:27`; agent rules:10; design:22) | PASS | `TarStream`, `Ingest`, `ColdBuild`, and `Snapshots` own their state. `SnapshotStore` is the expressly approved consumer persistence trait (`persistence.rs:134-222`). |
| Rust errors, imports, borrowing, and rustdoc follow repository rules (`AGENTS.md:28-31`; agent rules:11; Rust core reference) | PASS | Changed fallible operations have `# Errors`, including `archive.rs:78-84,131-134,197-202,229-253` and `snapshot.rs:240-254,274-326`; cancellation and partial progress are documented in `snapshot.rs:10-16,274-294,444-455`. Imports are at module tops; no unchecked `unwrap()` appears in changed production code. |
| No prohibited dependency, generated edit, grammar-pin change, source persistence, or detached task (`AGENTS.md:32-33`; agent rules:6,9; design:22) | PASS | Existing `tar` moves to dev dependencies in the pipeline manifest; `Cargo.lock` is unchanged. `rg -n 'tokio::spawn' crates/bohmian-code-review-pipeline` has no matches. The diff contains no generated or grammar file. |
| Tests remain with owning modules; required gates are not weakened; public journey is preserved (`AGENTS.md:35-39`; agent rules:4-5,12; testing-workflows reference) | PASS | New tests are inline in `archive.rs`, `snapshot.rs`, and `eligibility.rs`. `mise.toml` lanes are unchanged. The task packet records exact focused selectors and passing `test:pipeline`, `test:symbolic-context:journey`, and `gate`. I independently ran `mise run fmt:check`, `mise run lints`, and `git diff --check <base> <candidate>`; all exited 0. |
| Approved spec/task lifecycle and immutable review subject (`AGENTS.md:41-45`; agent rules:14-15; spec-driven-development reference) | PASS | The candidate task maps to approved spec revision 12; the R4 packet retains prior findings and cumulative identities. This report changes no production source. |

## Material findings

None. Overall: **PASS** for repository standards. Verification limit: Cargo tests and the full gate are supported by candidate task evidence, not independently rerun in this parallel standards review. This result does not preempt the task or sensitive-domain audits.

```citations
[]
```
