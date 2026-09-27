---
id: TASK-009
kind: implementation
status: proposed
spec: SPEC-repo-parser@14
depends_on: [TASK-003]
maps: [REQ-005, REQ-008, INV-002, AC-004]
---

## Outcome and Value

Cold build lists the complete default-branch tree even when GitHub truncates the recursive listing. It lists subtrees separately and splits further until no listing is truncated, so repository size alone never forces scoped fallback (REQ-005, REQ-008).

## Owners, Scope, Consumers, and Prohibited Changes

The outer pipeline crate's cold-build tree listing owns the change. Update and PR context consume the resulting listing and coverage. Scoped fallback stays for an archive that is too large or a failed full build. It no longer covers a truncated tree.

Prohibited: a scoped or partial result for a large repository; publishing a snapshot built from a listing that is still truncated; cloning or using any new provider endpoint beyond Git tree listings.

## Approach

1. When the recursive listing is truncated, list the tree's entries without recursion and list each subtree recursively, splitting any subtree that is still truncated.
2. Join the listings into one path/blob/mode listing with full repository-relative paths, identical to what an untruncated recursive listing would return.
3. Remove truncation as a scoped-fallback trigger, and keep honest incomplete coverage for any listing that cannot be split further.

## Ordered Implementation Scenarios

### Scenario 1 — Truncated root listing becomes a complete listing

**Behavior.** The fake provider truncates the root recursive listing and one nested subtree's recursive listing. Cold build still indexes every eligible file at its full path and publishes a complete snapshot. The facts equal a build over the same tree served untruncated (REQ-005).

**RED.** Replace the existing truncated-tree assertion in `crates/bohmian-code-review-pipeline/src/snapshot.rs`, which expects `truncated_tree` and nothing indexed, with a test run by `cargo test --locked -p bohmian-code-review-pipeline --all-features --lib truncated`. It fails because the build stops incomplete.

**GREEN.** Split truncated listings until none is truncated. Rerun `mise run test:pipeline`.

**REFACTOR.** Keep one listing result type for the rest of cold build to consume.

### Scenario 2 — Unsplittable listing stays honest

**Behavior.** If a non-recursive listing of a single tree is itself truncated, the attempt is incomplete with a coverage reason. It never publishes and never claims full coverage (INV-002).

**RED.** A fake-provider test that truncates a non-recursive listing, run with the same `truncated` filter. It fails if the build publishes or reports complete coverage.

**GREEN.** Report the incomplete reason.

**REFACTOR.** None required.

## Acceptance Criteria

A truncated recursive listing yields a complete, published snapshot equal to the untruncated oracle. A listing that cannot be split is reported incomplete and is not published. Truncation no longer causes scoped fallback. The existing cold, update and PR-context tests stay green.

## Expected Write Set and Consumer Closure

`crates/bohmian-code-review-pipeline/src/snapshot.rs`, `crates/bohmian-code-review-pipeline/src/github.rs` (a non-recursive listing if the client lacks one), the coverage fields in `snapshot.rs`/`context.rs` if the `truncated_tree` meaning changes, and their tests.

## Verification and Evidence

`mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run fmt:check`, `mise run lints`. Live proof is TASK-007's re-run capacity row for `rust-lang/rust`, which must no longer report a truncated tree.

## Material Stop Conditions

Stop if GitHub's listing semantics make a joined listing differ from the recursive one in path, mode, or blob identity. Stop if the number of subtree calls for a real large repository cannot fit within REQ-013's provider budget without a new bound.

## Authority Links

[Revision 14 spec](../spec.md) (REQ-005, REQ-008), [pipeline plan](../symbolic-context-service-plan.md).

## Implementation Evidence

Environment: macOS 15 (Darwin 24.6.0), Apple silicon, 2026-09-27. Fake-provider tests only; no network.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| A truncated recursive listing yields a complete, published snapshot equal to the untruncated oracle | `GitHubClient::tree` lists a truncated tree without recursion and each subtree recursively in its place (`list_into`, `tree_wire`, `TreeListing::push`), prefixing paths so entries match the recursive order; `truncated_listing_splits_into_the_complete_snapshot` truncates the root and `src` listings and asserts equal coverage and snapshot bytes against the untruncated oracle in 5 tree calls | `cargo test --locked -p bohmian-code-review-pipeline --all-features --lib truncated` (2 passed); `mise run test:pipeline` (73 + 2 passed) | PASS |
| A listing that cannot be split is reported incomplete and is not published | `TreeListing::truncated` now means a non-recursive listing was itself truncated; cold build keeps `IncompleteReason::TruncatedTree` and `Coverage::truncated_tree` for it; `unsplittable_truncated_listing_is_incomplete_and_unpublished` asserts no object, prior pointer kept | same `truncated` filter | PASS |
| Truncation no longer causes scoped fallback | `PrContext::run` no longer drops the snapshot for a truncated head listing; an unsplittable head listing keeps the snapshot and reports `files.truncated_tree`, so overlay coverage is not complete | `mise run test:pipeline` | PASS |
| Existing cold, update and PR-context tests stay green | Scenario 2's truncated segment moved to its own test; `truncate_tree` fake now truncates both listings; Scenario 10 unchanged | `mise run test:pipeline`, `mise run test:pipeline:journey` (2 passed) | PASS |

The joined listing uses only Git tree listings (`Operation::Tree`, same provider budget). The live `rust-lang/rust` capacity row belongs to TASK-007's re-run and was not run here. Non-goals stayed excluded: no scoped result for a large repository, no new endpoint or clone, no publication from a truncated listing.

Commands: `cargo test ... --lib truncated`, `mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run fmt:check`, `mise run lints`, `mise run gate`, `git diff --check`.
