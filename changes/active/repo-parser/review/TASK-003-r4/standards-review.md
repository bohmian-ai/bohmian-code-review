# Repository standards review — TASK-003 revision 12

Subject: base `c74871cbc78140820c9b0a5fda60e0952c4265e4`, candidate `0a1dea22b206e6174d4421d5ba7f7590ded93673`. Reviewed the complete nine-file diff. This report checks written repository rules, not task acceptance or whether the custom tar parser is the simplest implementation.

## Authority coverage

| Changed surface | Applicable authority |
|---|---|
| `archive.rs`, `github.rs`, `Cargo.toml`, `Cargo.lock` | `AGENTS.md:23,27-33`; `architecture/agent-rules.md:3,9-12`; `architecture/bohmian-code-review-design.md:5,22`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md` |
| `snapshot.rs`, `persistence.rs`, `testing.rs` | `AGENTS.md:21-23,27-31,35-39`; `architecture/agent-rules.md:3-5,7,9-12`; `architecture/bohmian-code-review-design.md:5,22,32`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md` |
| `eligibility.rs`, `lib.rs` | `AGENTS.md:21-23,27-31,35-39`; `architecture/agent-rules.md:3,5,7,10-12`; `architecture/bohmian-code-review-design.md:22,32`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md` |
| Task packet evidence | `AGENTS.md:41-45`; `architecture/agent-rules.md:14-15`; `architecture/references/languages/spec-driven-development.md`, especially task and verification contracts; `architecture/references/languages/implementation-execution.md` |

The relevant reference router is `architecture/references/README.md`. The symbolic-context domain reference is not applicable because no symbolic crate or algorithm changed. No grammar or generated artifact changed.

## Rule results

| Written rule | Result | Source and verification evidence |
|---|---|---|
| Pipeline is sole Git/storage IO owner; symbolic crates stay IO-free and pipeline consumes only public facade (`AGENTS.md:21-23`; design:22) | PASS | All production changes are in `bohmian-code-review-pipeline`; `snapshot.rs:741` calls `SymbolicContext::upsert`; no symbolic crate changed. |
| Source bytes transient, facts source-free, paths validated, archive/blob identity verified, incomplete coverage explicit (`AGENTS.md:21-23`; agent rules:7-9; design:5,22,32) | PASS | `archive.rs:175-191` uses `RelativePath`, `archive.rs:215-219` hashes wanted bytes before release; `snapshot.rs:674-702` drains verified entries, `snapshot.rs:528-545` publishes only complete coverage and exported facts. Consumer-facing completeness behavior warrants the task/domain audit, but no separate written-rule breach was established here. |
| Concrete owners and inherent methods; only intentional consumer storage traits (`AGENTS.md:27`; agent rules:10; design:22) | PASS | `TarStream`, `Ingest`, `ColdBuild`, and `Snapshots` own state; `SnapshotStore` is the architecture-approved injected consumer boundary. |
| Borrowing, typed errors, top-level imports, useful rustdoc including errors and cancellation (`AGENTS.md:28-31`; agent rules:11) | PASS | Changed methods document errors; `snapshot.rs:433-450` documents dropped futures and partial progress; `archive.rs:75-80,226-249` documents fallible writes. No unchecked `unwrap`, lint allowance, or ignored test appears in changed Rust files. |
| No prohibited runtime stack, changed grammar pins, generated artifacts, or source persistence (`AGENTS.md:32-33`; agent rules:6,9) | PASS | `Cargo.toml` moves existing `tar` to dev dependencies; `Cargo.lock` and grammar pins are unchanged. Snapshot serialization at `snapshot.rs:538-545` uses source-free `context.export()`. |
| Tests live with owners, preserve public journey and gates, use repository tasks (`AGENTS.md:37-39`; agent rules:4-5,12; testing-workflows:5,15-17) | PASS | New tests are inline in `archive.rs`, `snapshot.rs`, and `eligibility.rs`; task evidence records exact focused selectors and `test:pipeline`, journey, fmt, lints, and gate. Independently ran `mise run fmt:check` and `git diff --check` successfully. Cargo-backed lanes were not rerun in this parallel review. |
| Approved change lifecycle and local Git identity (`AGENTS.md:43-45`; agent rules:14-15; spec-driven-development task contract) | PASS | Candidate task is mapped to revision 12 and contains implementation evidence. Review writes a separate report and does not modify candidate source. |

## Material findings

None. The installed `tar` dependency question is a task/Ponytail simplicity judgment, not by itself a violation of a written repository standard.

Overall: **PASS** for repository standards. Verification limit: this report relies on the task packet's recorded Cargo gate for those mechanical checks; it independently ran only format and diff whitespace checks.

```citations
[]
```
