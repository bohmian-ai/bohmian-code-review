# Repository standards review — TASK-003 r3

Subject: base `60cc56e4213aa90f2a13d3ac6b3d1de536ff81b2`, candidate `d95346349988fed1d79dcb6e8b532b185e05b563` (current HEAD). I reviewed the complete candidate `snapshot.rs`, `eligibility.rs`, and `testing.rs`, plus the complete base-to-candidate diff, including `lib.rs`, `persistence.rs`, `storage.rs`, and the task record. No `.codegraph/` index exists. Earlier review reports and verdicts were excluded from this independent standards assessment.

## Authority coverage

| Changed surface | Applicable authority |
|---|---|
| Outer crate API and ownership (`lib.rs`, `snapshot.rs`, `persistence.rs`, `storage.rs`) | `AGENTS.md:21-34`; `architecture/agent-rules.md:3,9-11`; `architecture/bohmian-code-review-design.md:20-22,32-34`; `architecture/references/languages/rust-core.md` |
| Tree eligibility and verified ingest (`eligibility.rs`, `snapshot.rs`) | `AGENTS.md:23,29-33`; `architecture/agent-rules.md:7-11`; `architecture/bohmian-code-review-design.md:22,26,32,34`; `architecture/references/languages/rust-core.md` |
| Snapshot publication, read, collection, and injected stores (`snapshot.rs`, `persistence.rs`, `storage.rs`, `testing.rs`) | `AGENTS.md:23,27-32`; `architecture/agent-rules.md:3,9-11`; `architecture/bohmian-code-review-design.md:5,22,32,34`; `architecture/references/languages/rust-core.md` |
| Tests, task evidence, and verification | `AGENTS.md:36-46`; `architecture/agent-rules.md:4-5,12-15`; `architecture/bohmian-code-review-design.md:36-38`; `architecture/references/languages/spec-driven-development.md`; `architecture/references/languages/testing-workflows.md` |

The symbolic-context reference concerns unchanged extractor, index, resolver, and selector internals; the candidate calls the public facade without changing those stages. The implementation-execution reference governs implementation evidence rather than this read-only standards audit. Grammar, fixtures, and generated artifacts were not changed.

## Rule results

| Rule | Result | Source evidence |
|---|---|---|
| Pipeline is the sole provider/storage IO owner and consumes the public symbolic facade; no duplicate symbolic stage, service, or checkout (`AGENTS.md:21-23`; agent rules `:3,9`; design `:20-22`) | PASS | `snapshot.rs:14-31,398-415,443-540,646-860`; manifest depends on `bohmian-code-review` only among symbolic crates. No production filesystem or service dependency appeared in the diff. |
| Source-free, versioned facts and honest coverage (`AGENTS.md:21,23`; agent rules `:7,9`; design `:5,32-34`) | PASS | `snapshot.rs:110-218,480-555,730-860` retains path/blob/facts/coverage, validates extraction version and identity on read, and marks unfetched or parse-failed files incomplete. Source bytes are local to ingestion. |
| Cohesive concrete workflows, narrow approved injected traits, borrowed inputs, typed errors (`AGENTS.md:27-30`; agent rules `:10`; Rust core) | PASS | `ColdBuild`, `Snapshots`, and `Ingest` own their workflows; `SnapshotStore` is the spec-approved consumer seam; `ColdBuildError` uses `thiserror`; ingestion borrows source. Added dependencies: none. |
| Input and stored-key validation, publication integrity, bounded ingest (`AGENTS.md:23,29-30`; agent rules `:7`; design `:22`) | PASS | `snapshot.rs:178-208,252-310,443-608,617-839`; `storage.rs:144-153`. Archive entries are matched against validated relative paths and Git blob IDs; published pointers name only the caller's snapshot key. |
| Imports, rustdoc, and lint discipline (`AGENTS.md:30-34`; agent rules `:11-12`) | PASS | Changed imports are at module heads; changed items have intent-focused rustdoc, and fallible changed methods document errors. No `unwrap`, lint suppression, ignored test, rewritten golden, or generated-file edit in changed source. Formatting and Clippy are mechanically governed by `fmt:check` and `lints`. |
| Inline tests and credential-free verification (`AGENTS.md:38-40`; agent rules `:4-5`; testing workflows) | PASS | `snapshot.rs:861-2104` and `eligibility.rs:291-397` contain owner tests; `testing.rs` is compiled only under `#[cfg(test)]` in `lib.rs:21-22`. `mise.toml` provides `test:pipeline`, the symbolic public journey, formatting, lints, and gate. |
| Spec/task lifecycle and Git boundaries (`AGENTS.md:42-46`; agent rules `:13-15`; spec-driven development) | PASS | Task points to approved revision 11 and records focused and broader checks. The candidate diff contains no generated skill mirror, Git identity change, production plan reference, or publication. |

`git diff --check 60cc56e d95346349988fed1d79dcb6e8b532b185e05b563` passed. The orchestrator reported one `mise run gate` failure in the unchanged `execution.rs` test `check_reports_cancellation_then_deadline` (`Instant::now()` yielded `Ok(0ns)` at its deadline). Its exact focused rerun and the pipeline lane passed; a second full `mise run gate` passed (exit 0). The first result remains a timing-flake observation, with no repository-rule violation introduced or materially touched by this diff. I did not run a competing Cargo command.

## Findings and result

No material repository-rule findings. **PASS** for repository standards.

```citations
[]
```
