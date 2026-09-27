---
id: TASK-010
kind: implementation
status: proposed
spec: SPEC-repo-parser@14
depends_on: [TASK-004]
maps: [REQ-007, REQ-012, INV-003, AC-003]
---

## Outcome and Value

The update facade takes changed files, one by one, from the provider's compare API between the snapshot's commit and the current default-branch head. It does not re-list the tree. Unchanged facts carry over, changed blobs are fetched, and deleted or now-ineligible paths are removed. A root `.gitattributes` change, or more changed files than compare or the changed-blob bound returns, rebuilds from the archive (REQ-007).

## Owners, Scope, Consumers, and Prohibited Changes

The outer pipeline crate's update facade and GitHub client own the change. The pointer's recorded commit is the compare base. Publication, the incomplete-attempt rules and running-review immutability from TASK-004 are unchanged.

Prohibited: re-listing the full tree on the incremental path; carrying facts for a path whose blob changed; publishing an incomplete update; new rebuild triggers beyond those REQ-007 names.

## Approach

1. Add a compare call to the GitHub client under the existing provider policy and identity validation.
2. Build the update's change set from the compare file entries: added, modified, removed, and renamed (removal of the old path plus addition of the new one).
3. Apply eligibility to each changed path, fetch its verified blob, and carry the prior snapshot's facts for everything else.
4. Rebuild from the archive for a root `.gitattributes` change, or for more changed files than compare or `max_changed_blobs` returns.

## Ordered Implementation Scenarios

### Scenario 1 — Update from compare without a tree listing

**Behavior.** After a push that adds, modifies, removes and renames eligible files, update publishes a snapshot whose facts equal a from-scratch build of the head tree. The fake provider records no recursive tree listing and blob fetches only for changed paths (REQ-007, AC-003).

**RED.** Change the existing update test in `crates/bohmian-code-review-pipeline/src/update.rs` to serve compare and to assert that no recursive listing is requested, run with `cargo test --locked -p bohmian-code-review-pipeline --all-features --lib update`. It fails because update lists the tree.

**GREEN.** Build the change set from compare. Rerun `mise run test:pipeline`.

**REFACTOR.** Share eligibility and verified-blob ingest with cold build rather than duplicating it.

### Scenario 2 — Archive rebuild triggers

**Behavior.** A compare that includes root `.gitattributes`, or that returns more files than it or `max_changed_blobs` covers, rebuilds from the archive and publishes only when complete (REQ-007).

**RED.** Fake-provider tests for each trigger, using the same `update` filter. They fail when update takes the incremental path.

**GREEN.** Route those cases to the archive rebuild.

**REFACTOR.** None required.

### Scenario 3 — Existing lifecycle guarantees stay

**Behavior.** Force-push handling, a failed changed blob leaving the prior pointer unchanged, abort-and-retry convergence, and a running review keeping its loaded snapshot all hold as they do today (REQ-012, INV-003, AC-003).

**RED.** No new RED. The existing TASK-004 update tests are the regression proof and are adapted only to serve compare.

**GREEN.** Rerun `mise run test:pipeline` and `mise run test:pipeline:journey`.

**REFACTOR.** Remove tree-diff code that nothing uses anymore.

## Acceptance Criteria

Update uses compare and never lists the tree on the incremental path. Its facts equal a from-scratch head build. `.gitattributes` and the too-many-files cases rebuild from the archive. Every existing TASK-004 lifecycle guarantee and the pipeline journey stay green.

## Expected Write Set and Consumer Closure

`crates/bohmian-code-review-pipeline/src/update.rs`, `crates/bohmian-code-review-pipeline/src/github.rs`, shared ingest in `snapshot.rs` if needed, and the pipeline journey's fake-provider support in `tests/pipeline_journey.rs`.

## Verification and Evidence

`mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run fmt:check`, `mise run lints`.

## Material Stop Conditions

Stop and return to spec authority in either of these cases. First, when the snapshot commit is not an ancestor of the head (force-push or diverged history), GitHub compare lists files from the merge base, not the snapshot commit, so it cannot yield the exact snapshot-to-head change set. Second, compare file entries lack the Git mode, so they cannot establish mode-based eligibility (symlink, submodule, and executable-bit changes) without another listing.

## Authority Links

[Revision 14 spec](../spec.md) (REQ-007), [TASK-004](TASK-004-incremental-snapshot-lifecycle.md), [pipeline plan](../symbolic-context-service-plan.md).

## Implementation Evidence

Environment: macOS 15 (Darwin 24.6.0), Apple silicon, 2026-09-27. Fake-provider tests only; no network. Commits `fdf1b14`, `461b5d6`, `f94e355`, and `47565fb` (maintainer decision [stateless snapshot](../review/maintainer-decision-stateless-snapshot.md): `Snapshot.excluded` removed, `SNAPSHOT_FORMAT_VERSION` 3, base-commit listings and `Comparison::invalid_removed` deleted).

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Update uses compare and never lists the tree on the incremental path | `GitHubClient::compare` (identity-checked base and, when `ahead`, head; renames split into old-path removal and new-path addition) and `GitHubClient::directory` (one non-recursive listing); `Update::run` classifies each changed path from its parent directory's listing at the head (`Directories`) and lists nothing at the base | `change_delete_and_rename_match_a_fresh_parse` asserts 1 compare, 3 non-recursive tree calls (head root, `src`, `src/api`), and no `recursive` URL; `compare_sorts_changed_paths_and_verifies_its_ends`; journey asserts 1 compare and 2 non-recursive listings | PASS |
| Its facts equal a from-scratch head build | `Ingest::carry` carries prior outcomes outside the removed set; the snapshot persists only per-path outcomes, so no derived state needs reconstruction | Scenario 1 push (change, delete, rename, file→symlink, deleted vendored file) writes the same snapshot bytes as a cold build of the head; the update's reported exclusions are only those it observed (the changed symlink) | PASS |
| `.gitattributes` and the too-many-files cases rebuild from the archive | `Update::run` calls `ColdBuild::build` for status other than `ahead`, a compare at `COMPARE_MAX_FILES` (300), more files than `max_changed_blobs`, or a root `.gitattributes` change | `rebuild_triggers_use_the_archive` (attributes, diverged, ceiling: 1 archive, 0 reused, bytes equal to cold build); `changes_past_the_ceiling_rebuild_from_the_archive` | PASS |
| TASK-004 lifecycle guarantees and the pipeline journey stay green | TASK-004 update tests adapted only through the fake provider, which serves compare from every earlier push and per-directory listings | `failed_changed_blob_keeps_the_pointer_until_a_retry`, `dropped_update_keeps_the_pointer_and_retry_converges`, `moving_head_is_reported_stale_and_updates_converge`, `version_change_rebuilds_under_a_new_key`; `mise run test:pipeline` (75 + 2 passed), `mise run test:pipeline:journey` (2 passed) | PASS |

A compare path absent from a complete listing of its parent fails with `MalformedResponse`. A truncated directory listing reports `IncompleteReason::TruncatedTree`, the same as cold build. A non-canonical added path is counted as one `Unsupported` exclusion in the update's reported coverage; a non-canonical removed path was never eligible and is dropped. Exclusion counts are reported by the build call that observed them and are not persisted, so a reused snapshot reports none. Non-goals stayed excluded: no full-tree listing on the incremental path, no facts carried for a changed blob, no incomplete publication, and no rebuild triggers beyond REQ-007. Only the expected write set changed.

Commands: `mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run fmt:check`, `mise run lints`, `mise run gate`, `git diff --check`.
