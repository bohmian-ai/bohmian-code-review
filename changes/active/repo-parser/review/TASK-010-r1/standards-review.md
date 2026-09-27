# Repository standards review — TASK-010, round 1

Subject: base `97fff9f`, candidate `da81a598c10441b3b2e1231ce674d89003017f26`. I reviewed the complete diff, including spec revision 14 at `63d6bd0` and `review/maintainer-decision-stateless-snapshot.md`. This report audits written repository rules only; task acceptance is assigned to the task reviewers.

## Authority coverage

| Changed surface | Applicable authority | Coverage |
|---|---|---|
| `github.rs` compare and directory provider calls; `lib.rs` export | `AGENTS.md:23,27-34`; `architecture/agent-rules.md:3,7,10-11`; `architecture/bohmian-code-review-design.md` (outer pipeline and source trust); `architecture/references/languages/rust-core.md`; `spec.md:19,64,75-77` | Provider IO remains in pipeline; checked identity, typed errors, existing request policy, bounded compare response and rustdoc inspected. |
| `update.rs` update facade, directory traversal, tests | `AGENTS.md:23,27-32,40`; `architecture/agent-rules.md:3,7-8,10-12`; design (outer pipeline, immutable snapshot); Rust core and testing-workflows references; `spec.md:32-33,63-64,75,86-87` | Traced from compare through changed-path mode lookup, ingest, fallback, completeness and publication; reviewed focused and lifecycle tests. |
| `snapshot.rs` snapshot shape, ingest and fake provider | `AGENTS.md:23,27-33,38-40`; `architecture/agent-rules.md:3,7-12`; design (source-free snapshots, publication); Rust core and testing-workflows references; `spec.md:32-33,63-64`; maintainer decision | Confirmed per-path persisted outcomes, version bump, invocation-only exclusion counts, complete-publication gate and test fixture routing. |
| `tests/pipeline_journey.rs` | `AGENTS.md:38-40`; `architecture/agent-rules.md:4-5,12`; testing-workflows reference | External test crosses the public pipeline facades and verifies compare plus non-recursive call accounting. |
| `spec.md`, task evidence, maintainer decision | `AGENTS.md:44`; `architecture/agent-rules.md:14`; spec-driven-development and implementation-execution references | Checked approved revision, material decision and evidence against the diff; no code authority inferred from the task evidence alone. |
| Manifests, lockfile, grammar and generated artifacts | `AGENTS.md:9,33-34,40`; `architecture/agent-rules.md:6,12`; Rust core and testing-workflows references | Inspected workspace and pipeline manifests, `mise.toml`, and changed-path list: no dependency, grammar, lockfile, generated artifact or gate edit. |

## Written-rule results

| Rule | Result | Evidence |
|---|---|---|
| Outer pipeline alone owns provider/storage IO; symbolic crates stay IO-free; no duplicated symbolic stage (`AGENTS.md:14-23`; `architecture/agent-rules.md:3`) | PASS | All production changes are in `bohmian-code-review-pipeline`; `update.rs:100-140` uses existing GitHub, ingest and cold-publication owners. |
| Validate effective paths, identities and changed blobs; never publish partial coverage (`architecture/agent-rules.md:7-9`; `AGENTS.md:23`) | PASS | `github.rs:1159-1207` validates compare identity/status and canonical paths; `update.rs:117-140` resolves changed modes; `snapshot.rs:750-787` gates publication on completeness. This is a standards assessment; domain reviewers own deeper provider edge cases. |
| Snapshot facts remain source-free and versioned, with no aggregate persisted state (`AGENTS.md:21,23`; `architecture/agent-rules.md:9`; `spec.md:32`; maintainer decision) | PASS | `snapshot.rs:253-264` persists per-path outcomes only; `snapshot.rs:29` bumps format version to 3; `snapshot.rs:304-310` derives coverage without persisted exclusions. |
| Concrete workflow owners; no speculative traits/dependencies; borrow where useful (`AGENTS.md:27-28`; `architecture/agent-rules.md:10`) | PASS | Existing `GitHubClient`, `Update`, `ColdBuild`, `Ingest` own operations. `Directories` in `update.rs:164-253` is invocation state; no new trait or dependency. |
| Typed errors, no unchecked production `unwrap`, top-level imports, useful rustdoc for new/materially changed items (`AGENTS.md:30-32`; `architecture/agent-rules.md:11`) | PASS | New compare and directory operations have `# Errors` (`github.rs:1117-1208`); update and directory methods do too (`update.rs:60-253`). New test helpers and tests carry intent docs. |
| Preserve grammar fixtures/goldens and avoid generated edits (`AGENTS.md:33`; `architecture/agent-rules.md:6`) | PASS | No grammar, fixture, golden, or generated path changed. |
| External test earns its place by crossing a public boundary; preserve public journey and negative/lifecycle checks (`AGENTS.md:38-40`; `architecture/agent-rules.md:4-5,12`) | PASS | `pipeline_journey.rs:148-207,462-485` exercises the public stage journey. Update owner tests cover failed blob, dropped future, moving head and archive triggers (`update.rs:464-811`). Existing tests were adapted rather than disabled. |
| Use `mise.toml` lanes, preserve gates and inspect broad diff (`AGENTS.md:40`; testing-workflows and implementation-execution references) | PASS | `mise.toml` names owner, journey, format, lint and gate lanes; none changed. Task evidence records those lanes and `git diff --check`; this reviewer independently ran `git diff --check 97fff9f da81a59` (pass). I did not rerun Cargo lanes; the orchestrator should use its verification record. |
| Review immutable cumulative candidate under approved authority (`AGENTS.md:44`; `architecture/agent-rules.md:14`; spec-driven-development reference) | PASS | Candidate remained `da81a598c10441b3b2e1231ce674d89003017f26`; diff inspected from `97fff9f`. Spec revision 14 and maintainer decision authorize the persisted-shape change. |

## Material written-rule findings

None.

Overall: **PASS** for repository standards. Verification limit: Cargo-backed lane results were taken from task evidence and are not independently rerun in this report.

```citations
[]
```
