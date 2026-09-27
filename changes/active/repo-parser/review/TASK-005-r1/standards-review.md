# Repository standards review: TASK-005 r1

Subject: `fa4fa2101950626a9fe60b0fdff0439b04c16f01` → `e68bcdb65b44095989b5f42dfda5c7d87295582b`. This audits repository rules on the complete cumulative diff, not task acceptance. No `.codegraph/` index exists. The only changed production crate is `bohmian-code-review-pipeline`; no manifest, lockfile, grammar, generated artifact, or symbolic crate changed.

## Authority coverage

| Changed surface | Governing written authority | Inspected evidence |
|---|---|---|
| `pull_request.rs`, `github.rs` | `AGENTS.md:21-33,37-39`; `architecture/agent-rules.md:3-11`; `architecture/bohmian-code-review-design.md:22-34`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md` | `PrContext::run`, `pin`, `changed_head`, `pack`, helper bodies, GitHub PR types/page call, inline scenarios, pipeline manifest |
| `context.rs`, `persistence.rs` | `AGENTS.md:23-33,37-39`; `architecture/agent-rules.md:3,5,7,9-11`; `architecture/bohmian-code-review-design.md:22,32`; Rust and testing references | context object, read/publish/cleanup, context store contract, inline tests, object key and identity consumers |
| `snapshot.rs`, `testing.rs` | `AGENTS.md:23,25-33,37-39`; `architecture/agent-rules.md:3,5,8-12`; Rust and testing references | shared attributes and ingest methods, fake transport, in-memory stores, existing snapshot tests and caller paths |
| `lib.rs` public exports | `AGENTS.md:21-23,27,31-33`; `architecture/agent-rules.md:3-4`; `architecture/bohmian-code-review-design.md:20-22` | module/re-export surface, pipeline manifest and dependency direction |
| task evidence addition | `AGENTS.md:39,43-45`; `architecture/agent-rules.md:14-15`; `architecture/references/languages/spec-driven-development.md` and `implementation-execution.md` | evidence table, recorded commands, actual diff and independent rerun results |

## Rule results

| Applicable rule | Result | Exact authority and source or check evidence |
|---|---|---|
| Git/storage IO belongs only in the outer crate; symbolic work uses its public facade | PASS | `AGENTS.md:21-23`; `architecture/agent-rules.md:3,9`; `architecture/bohmian-code-review-design.md:20-22`. Diff is confined to pipeline and task evidence; `pull_request.rs:30-35,234-259,427-436` calls `SymbolicContext::rank_files` and `build_candidate_context`; pipeline manifest depends on `bohmian-code-review`. |
| Source-free index/snapshot, selected-context-only persisted text; partial coverage is explicit | PASS | `AGENTS.md:21-23`; `architecture/agent-rules.md:7-9`; `architecture/bohmian-code-review-design.md:5,22,32`. `pull_request.rs:169-232` uses transient `Ingest` and reports scope, snapshot, overlay and truncation; `context.rs:178-205` places selected text in `ReviewContext`; `context.rs:305-319` publishes that object. |
| Stateful workflows have concrete owners; traits are narrow injected boundaries; no new dependency or prohibited adapter | PASS | `AGENTS.md:27,33`; `architecture/agent-rules.md:10`; `architecture/bohmian-code-review-design.md:22`. `PrContext` and `Contexts` own workflows (`pull_request.rs:104-149`, `context.rs:267-292`); existing `ObjectStorage`/`SnapshotStore` and approved `ContextStore` are consumed. Manifest and lockfile are unchanged. |
| Validate trust-boundary paths, changes, identity and counted budgets; failures stay visible | PASS | `AGENTS.md:28-29`; `architecture/agent-rules.md:7-8`. `pull_request.rs:298-333` retries moving PR pagination; `pull_request.rs:364-405` checks eligible path/blob and parses patch; `pull_request.rs:234-279` excludes changed paths and records budget/omissions; `context.rs:335-375` checks exact identity/id/key, expiry, digest and object metadata. Domain reviewers separately assess behavioral completeness of these checks. |
| Rust errors, imports, rustdoc, no unchecked input/IO unwrap, no generated or golden edits | PASS | `AGENTS.md:29-33`; `architecture/agent-rules.md:6,11-12`; Rust reference. New fallible methods include `# Errors`, owner and test items carry intent docs, imports are module-top, errors use `thiserror`; no new `unwrap`, lint allowances, ignored tests, grammar or generated changes in diff. |
| Inline credential-free tests and required lanes | PASS | `AGENTS.md:37-39`; `architecture/agent-rules.md:4-5,12`; `architecture/references/languages/testing-workflows.md`. Tests sit in `context.rs`, `pull_request.rs`, and owning support modules. Independent rerun at candidate: `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check` all passed (orchestrator result). |
| Review lifecycle and no consumer, generated mirror, identity or publication changes | PASS | `AGENTS.md:43-45`; `architecture/agent-rules.md:13-15`; spec-driven reference. Only task evidence and pipeline source changed; no skills mirror, Git identity, push or publication in candidate diff. |

## Findings

None. This is a repository-rule result; task obligations and sensitive-domain behavior are reviewed independently.

Overall: **PASS**.

```citations
[]
```
