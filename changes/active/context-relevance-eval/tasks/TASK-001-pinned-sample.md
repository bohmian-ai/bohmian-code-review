---
id: TASK-001
kind: implementation
status: proposed
spec: SPEC-context-relevance-eval
spec_revision: 4
requirements: [REQ-001, REQ-002, REQ-003, REQ-007, INV-001, INV-003, INV-004, INV-005, AC-005]
depends_on: []
parent_task:
remediates: []
---

# Select the PR sample

## Outcome and Value

The opt-in evaluator selects the 50 qualifying merged PRs per repository and passes their PR file listings, base/head commits, and base snapshot declaration facts to TASK-002. Sample selection is setup for the relevance trials, not a production review-context operation.

## Owners, Scope, Consumers, and Prohibited Changes

The opt-in evaluation command in the outer pipeline crate owns discovery and qualification. Use one complete GitHub PR file listing per PR (with pagination as needed), the public path-language query restricted to Python and Rust, and the existing fixed-commit base snapshot. Treat the listing as input. Fail the run for unusable PR identity or merge time, a rejected file entry, incomplete listing, or incomplete base snapshot. A missing or malformed patch is a file omission, not a run failure, and supplies no changed lines. Do not run `PrContext`, fetch a second listing to reconcile the first, add a transport replay layer, or fetch trial source blobs in this task. Do not change production APIs, behavior, persisted formats, or the gate.

## Approach

1. Discover merged PRs before the cutoff, ordered by merge timestamp and PR number.
2. For each PR, read its file listing once, count changed Python/Rust paths, and parse available patches for the other-file changed-lines condition.
3. Use the base snapshot to check whether a modified path has declarations; retain a PR when the remaining REQ-002 conditions hold.
4. Pass qualifying metadata, the listing, patch omissions, and the base snapshot to TASK-002; fail on the specific run-level conditions above.

## Ordered Implementation Scenarios

### Scenario 1 — Small, deterministic qualification

**Behavior.** Given PR metadata, one listing per PR, and base snapshot facts, the sampler orders ties by PR number, rejects nonqualifying PRs, and keeps 50 qualifying PRs with at least one modified base declaration and one other changed-line file (REQ-002–REQ-003).

**RED.** Add one focused no-network test of ordering and the filter using small supplied records; run its exact package, target, and selector and observe the missing qualification behavior.

**GREEN.** Implement only discovery, path checks, snapshot lookup, and filtering needed by the test; rerun it.

**REFACTOR.** Remove duplicate traversal or state while keeping the single-listing path visible and the test green.

### Scenario 2 — Omission versus failed run

**Behavior.** A missing or malformed patch is recorded and contributes no changed lines; unusable PR identity, rejected file entry, incomplete pagination, or incomplete base snapshot fails the run. Neither case triggers a second PR listing or `PrContext` (REQ-002, INV-001, AC-005).

**RED.** Extend the focused no-network sampler test with these contrasting outcomes and run its exact command; observe the missing failure or omission distinction.

**GREEN.** Add only the required classification and error path; rerun both scenarios.

**REFACTOR.** Keep one pass over the listing and avoid a second provider or reconciliation abstraction.

## Acceptance Criteria

- Qualification uses one logical PR file listing, path eligibility, and base snapshot declarations. It never runs `PrContext` or replays provider responses.
- The sample contains the required 50 PRs in deterministic order, or the run fails with a clear reason. Only the specified run-level failures stop sampling; patch omissions remain file-local. No extra reconciliation is used.
- No source blobs are retained or prepared for relevance trials in this task. No new public production contract is introduced.

## Size Expectation

This is a narrow sampler with focused tests of qualification and the failure/omission boundary, not a second PR ingest pipeline. If qualification starts requiring a second provider abstraction, replay wrapper, full PR-context call, or test support larger than the sampler itself, stop and simplify before review. A fake HTTP stack is not needed to prove the sampler's filtering rule.

## Expected Write Set and Consumer Closure

Likely: the opt-in evaluation command, its focused sampler test, and a `mise.toml` live task outside `gate`. TASK-002 consumes the selected PR metadata and file listing. Paths are guidance, not an implementation allowlist.

## Verification and Evidence

After naming the test, record its exact `mise exec -- cargo test --locked` command and RED/GREEN result. Run `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Inspect the diff for `PrContext`, replay, and production changes. Live sample completeness is proved in TASK-003; do not manufacture a fake-provider integration suite for this setup step.

## Material Stop Conditions

Stop if the specified sample cannot be selected from one PR listing plus the base snapshot, or if meeting the requirement needs a new public API or production workflow change.

## Authority Links

[Approved spec](../spec.md); [contributor rules](../../../../AGENTS.md); [design](../../../../architecture/bohmian-code-review-design.md).
