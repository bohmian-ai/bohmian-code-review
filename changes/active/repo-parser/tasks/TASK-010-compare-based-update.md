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
