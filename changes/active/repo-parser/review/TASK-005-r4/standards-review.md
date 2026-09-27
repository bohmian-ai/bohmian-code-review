# TASK-005 r4 repository standards review

Subject: `fa4fa2101950626a9fe60b0fdff0439b04c16f01` → `62346cb030354596cdf3dd7596de63dbd8d99fcb` (HEAD checked at candidate). Scope: the complete cumulative diff, including the original task, R1–R3 remediation, and the maintainer decision that replaces R3 FIND-4. `.codegraph/` is absent. This review checks written repository rules, not task acceptance.

## Authority coverage

| Changed surface | Applicable authority | Inspected evidence |
|---|---|---|
| PR context and read facades: `pull_request.rs`, `context.rs`, `lib.rs` | `AGENTS.md:21-31,35-39`; `architecture/agent-rules.md:3-11`; `architecture/bohmian-code-review-design.md:20-38`; `architecture/references/languages/rust-core.md:3-7`; `architecture/references/languages/testing-workflows.md:3-19`; `architecture/references/domain/symbolic-context.md:1-9` | `pull_request.rs:106-590`, `context.rs:25-440`; inline tests in both modules; `mise.toml` owner and journey lanes |
| Provider and tree/PR-file validation: `github.rs`, `eligibility.rs` | `AGENTS.md:23,27-33`; `architecture/agent-rules.md:7-11`; design `:22,26,32`; Rust core `:3-7`; maintainer decision `maintainer-decision-rejected-tree-entries.md:1-10` | `github.rs:947-991,1121-1159,1378-1407`; `eligibility.rs:15-69,242-309`; provider/eligibility inline tests |
| Snapshot/overlay/update and persistence contracts: `snapshot.rs`, `update.rs`, `persistence.rs` | `AGENTS.md:21-31,35-39`; agent rules `:3,7-11`; design `:22,32-38`; Rust core `:3-7`; testing workflows `:3-19` | `snapshot.rs:765-805,999-1017,2319-2360`; `update.rs:70-126,354-378`; `persistence.rs:68-113,230-301`; inline tests |
| In-memory capability tests: `testing.rs` plus all changed inline tests | `AGENTS.md:35-39`; agent rules `:4-5,12`; testing workflows `:3-19`; spec-driven development `:127-170` | `testing.rs:23-489`; `context.rs:442-792`; `pull_request.rs:592-1538`; changed inline tests in other modules |
| Packet/task/review evidence under `changes/active/repo-parser/` | `AGENTS.md:41-45`; agent rules `:13-15`; spec-driven development `:1-42,170-202` | Approved revision 12 `spec.md`, original `TASK-005`, R1–R3 records, R3 maintainer decision; current review output is separate from candidate |
| Manifests, grammar, generated output, and tooling boundary | `AGENTS.md:29,32-33,39`; agent rules `:6,12-13`; testing workflows `:15-19` | `git diff --name-only` shows no manifest, lockfile, grammar, generated artifact, `mise.toml`, skill mirror, or symbolic crate change; `crates/bohmian-code-review-pipeline/Cargo.toml:12-24` depends on public facade and existing workspace dependencies |

## Written-rule results

| Rule | Result | Source and verification evidence |
|---|---|---|
| Pipeline owns Git/storage IO and consumes only the public symbolic facade (`AGENTS.md:21-23`; agent rules `:3,9`; design `:20-22`) | PASS | All changed Rust files are in the pipeline crate; `Cargo.toml:12-20` has `bohmian-code-review` and no lower symbolic crate; `pull_request.rs:497-520` calls facade ranking, `:449-462` calls facade packing. |
| Source-free index and source confined to selected context object (`AGENTS.md:21,23`; agent rules `:9`; design `:5,22,34`) | PASS | `pull_request.rs:283-307` builds source-bearing `ReviewContext`; `context.rs:300-338` stores its bytes in the context object; `snapshot.rs:765-805` uses a transient `SymbolicContext` and snapshot facts. No new symbolic-crate source retention. |
| Canonical trust-boundary values, typed failures, and no omitted file or budget silently presented as success (`AGENTS.md:28-29`; agent rules `:7-8`; Rust core `:7`) | PASS | `github.rs:1378-1397` validates tree mode, SHA, path; `:1400-1407` independently validates PR files. `pull_request.rs:203-249` counts PR-file rejections, `:260-301` records the budget and omissions; `context.rs:354-394` checks identity, expiry, digest and object metadata. Behavioral acceptance remains for task/domain reviewers. |
| Maintainer's rejected-tree-entry decision (user-supplied authority overriding R3 FIND-4) | PASS | `github.rs:978-990,1385-1397` errors on bad mode/SHA and counts noncanonical paths; `snapshot.rs:1002-1015` counts those paths as unsupported exclusions. PR-file rejection remains separate in `pull_request.rs:203-213`. |
| Concrete workflow owners, borrowing, imports, library errors and documentation (`AGENTS.md:27-31`; agent rules `:10-11`; Rust core `:3-7`) | PASS | Stateful facades are `PrContext` (`pull_request.rs:106-117`) and `Contexts` (`context.rs:272-279`); provider/store traits are the approved injected boundary (`persistence.rs:230-242`). Imports are module-top; new fallible facade methods document `# Errors` (`context.rs:296-300,348-354,411-414`; `pull_request.rs:143-149,314-320,386-389,438-441`). Changed tests and helpers have intent rustdoc; no new unchecked production `unwrap()` or lint allowance. |
| Public journey and inline tests; credential-free lanes (`AGENTS.md:35-39`; agent rules `:4-5`; testing workflows `:3-19`) | PASS | Inline public pipeline journeys in `pull_request.rs:809-1538`, context contracts in `context.rs:442-792`, and the preexisting symbolic journey target in `mise.toml:50-53`. Candidate task evidence records `mise run test:pipeline`, `test:symbolic-context:journey`, `fmt:check`, `lints`, and `gate` all exit 0. These checks were not rerun by this reviewer; `git diff --check base candidate` independently exited 0. |
| No forbidden dependencies, generated edits, fixture weakening, or production lifecycle metadata (`AGENTS.md:32-33,39,43`; agent rules `:6,12-13`) | PASS | Complete changed-path list has only pipeline Rust and active packet Markdown. No manifest, golden, grammar, generated file, skill mirror, or production plan reference added. |

No material repository-rule findings. This result does not determine the task's acceptance verdict or independently reproduce its runtime claims.

Overall: **PASS**.

```citations
[]
```
