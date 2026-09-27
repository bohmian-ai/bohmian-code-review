# TASK-009 repository standards review

**Subject:** `597f5180808b78e1b104c10c1a375d5604b89a2e` → `ef48e308838fd081561754059e78a3814f03cf8a` (spec revision 14). The candidate is HEAD. I reviewed the complete diff in `github.rs`, `pull_request.rs`, `snapshot.rs`, and the TASK-009 evidence update. No `.codegraph/` index exists.

**Result: PASS.** No material violation of a written repository rule was found. This is a standards result; task acceptance and sensitive-domain behavior belong to their independent reviewers.

## Authority coverage

| Changed surface | Applicable authority | Evidence inspected |
|---|---|---|
| Git tree API, validation, and call accounting in `crates/bohmian-code-review-pipeline/src/github.rs` | `AGENTS.md:23,27-34,40`; `architecture/agent-rules.md:3,7,10-11`; `architecture/bohmian-code-review-design.md:5,22,32`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/implementation-execution.md` | Full file diff, `TreeListing`, `TreeEntry`, `GitHubClient::tree/list_into/tree_wire`, `validate_tree_entry`, `RequestLimits`, package/workspace manifests and lockfile |
| PR-context consumer in `crates/bohmian-code-review-pipeline/src/pull_request.rs` | `AGENTS.md:23,27-32,40`; `architecture/agent-rules.md:3,7,10-11`; `architecture/bohmian-code-review-design.md:22,32`; `architecture/references/domain/symbolic-context.md`; `architecture/references/languages/rust-core.md` | Full diff and surrounding `PrContext::run` through listing, attributes, snapshot choice and ingest |
| Coverage contract and cold-build consumer in `crates/bohmian-code-review-pipeline/src/snapshot.rs` | `AGENTS.md:23,27-32,38-40`; `architecture/agent-rules.md:3,5,7,10-12`; `architecture/bohmian-code-review-design.md:22,32`; `architecture/references/languages/testing-workflows.md`; `architecture/references/languages/rust-core.md` | Full diff and surrounding `Coverage`, `IncompleteReason`, `ColdBuild::start`, inline fake provider and tests |
| TASK-009 evidence update and test routing | `AGENTS.md:40,44`; `architecture/agent-rules.md:12-14`; `architecture/references/languages/spec-driven-development.md`; `architecture/references/languages/implementation-execution.md`; `architecture/references/languages/testing-workflows.md` | Task file diff, revision-14 spec, `mise.toml`, reported test commands, `git diff --check` |

## Rule results

| Written rule | Result | Exact source evidence |
|---|---|---|
| Pipeline owns Git IO and uses only the public symbolic facade (`AGENTS.md:21-23`; `architecture/agent-rules.md:3`; `architecture/bohmian-code-review-design.md:22`) | PASS | All production changes are in the existing pipeline crate. `github.rs:995-1079` performs Git tree requests; `pull_request.rs:158-209` and `snapshot.rs:676-704` consume its listing. No symbolic crate, dependency direction, or source retention changed. |
| Validate paths and IDs at provider ingress; incomplete coverage must remain explicit (`AGENTS.md:23,28`; `architecture/agent-rules.md:7`; `architecture/bohmian-code-review-design.md:32`) | PASS for written-rule compliance | `github.rs:487-509` sends entries through `validate_tree_entry`; `github.rs:1068-1079` checks each requested tree ID; `snapshot.rs:677-689` returns an incomplete result when a listing is unsplittable. The detailed source-trust and completeness claims are assigned to the domain/task reviewers. |
| Concrete workflow owner; no speculative traits, dependency, or SDK (`AGENTS.md:27,34`; `architecture/agent-rules.md:10`) | PASS | `list_into` and `tree_wire` are inherent methods of existing `GitHubClient` (`github.rs:1017-1079`). `Cargo.toml`, `Cargo.lock`, and workspace manifests did not change. |
| Rust errors, imports, rustdoc and no unchecked input `unwrap` (`AGENTS.md:30-32`; `architecture/agent-rules.md:11`) | PASS | New and materially changed items in `github.rs:471-509,977-1079` and `snapshot.rs:1264-1298,2573-2680` have intent documentation; fallible methods have `# Errors`. No new unchecked `unwrap`, import-placement issue, or panic path appears in the diff. Mechanically enforced lint and format rules passed their lanes. |
| Inline owner tests, no weakened test or golden (`AGENTS.md:32-33,38-40`; `architecture/agent-rules.md:5,12`; `architecture/references/languages/testing-workflows.md`) | PASS | New tests are inline in `snapshot.rs:2573-2680`; prior truncation assertion is replaced by a focused unsplittable-listing test, while the remaining scenario continues at `snapshot.rs:1971`. No grammar fixture, generated file, lint allowance, ignore attribute, or golden changed. |
| Review an immutable candidate against actual diff and approved authority (`AGENTS.md:44`; `architecture/agent-rules.md:14`; `architecture/references/languages/spec-driven-development.md`) | PASS | HEAD is `ef48e308838fd081561754059e78a3814f03cf8a`; the review uses the full specified base-to-candidate diff and spec revision 14. |
| Use repository task lanes (`AGENTS.md:40`; `architecture/references/languages/testing-workflows.md`) | PASS | Orchestrator ran `mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run fmt:check`, and `mise run lints` against candidate HEAD; all exited 0. I ran `git diff --check` on the pinned range; it exited 0. The live credentialed test was not run and is outside the normal gate. |

## Findings

None. The independent task and domain reviews must assess whether the joined listing satisfies the revision-14 behavior in every reachable case.

## Verification limits

I did not run Cargo-backed checks concurrently with the other reviewers; lane results above are orchestrator-observed. No controlled live GitHub listing was available in this standards pass.

```citations
[]
```
