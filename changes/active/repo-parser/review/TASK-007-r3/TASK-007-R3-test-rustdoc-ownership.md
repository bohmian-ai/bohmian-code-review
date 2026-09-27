---
id: TASK-007-R3
kind: remediation
status: ready
spec: SPEC-repo-parser@14
parent_task: TASK-007
remediates: [FIND-TASK-007-3]
depends_on: [TASK-007, TASK-007-R1, TASK-007-R2]
---

# TASK-007-R3 — Restore test rustdoc ownership

## Authority and subject

Approved [revision 14 specification](../../spec.md), [original TASK-007](../../tasks/TASK-007-integrated-pipeline-and-real-pr.md), [R1](../TASK-007-r1/TASK-007-R1-fixed-commit-and-provider-proof.md), [R2](../TASK-007-r2/TASK-007-R2-stable-workload-and-contract-validation.md), and [stateless-snapshot decision](../maintainer-decision-stateless-snapshot.md). Reviewed base `2257992f36a2aa88f33f397af1e070d6ab35961b`; candidate `b721917420113ab2e3530966e9e456eecc89dcff`. Implement this bounded correction through `$cr-implement`.

## FIND-TASK-007-3 — Changed Rust test items lack accurate rustdoc

**Diagnosis.** `AGENTS.md:32` and `architecture/agent-rules.md:11` require useful rustdoc for new or materially changed Rust items, including tests. At the base, the Scenario 1 cold-journey rustdoc immediately described `complete_cold_snapshot_is_verified_source_free_and_reloadable`. At the candidate, `crates/bohmian-code-review-pipeline/src/snapshot.rs:1703-1711` attaches that same description to the newly inserted `fixed_commit_build_ignores_head_advance_and_run_follows_it`; the original journey test begins at `:1743-1744` without rustdoc. The new `crates/bohmian-code-review-pipeline/examples/provider_contract.rs` test module begins at `:327-328` without module rustdoc. These are siblings of one documentation ownership defect class. The fixed-commit and provider tests pass, but runtime success cannot show that rustdoc describes the correct item. A reader sees the wrong invariant on the fixed-commit test and none on the cold journey or provider test module.

**Correction outcome.** Each of those three Rust items has a useful description of its own intent and tested boundary. Restore the cold-journey description to its original test, describe the fixed-commit test's head-advance behavior on that test, and describe the captured-report validation purpose on the provider test module. Reuse the existing rustdoc and item structure. No new behavior, helper, dependency, fixture, or test harness is needed. Preserve test bodies, case validation, PR workload pinning, source trust, stateless snapshots, and the opt-in live lanes.

**Write set.** `crates/bohmian-code-review-pipeline/src/snapshot.rs` and `crates/bohmian-code-review-pipeline/examples/provider_contract.rs` only. No evidence report or specification change is required for this comment-only correction.

**Acceptance and proof.** Source inspection shows the Scenario 1 description directly attached to `complete_cold_snapshot_is_verified_source_free_and_reloadable`, a separate accurate description directly attached to `fixed_commit_build_ignores_head_advance_and_run_follows_it`, and useful module rustdoc on `provider_contract::tests`. Run `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. The final task review rechecks the cumulative base-to-candidate range. No behavioral test is added for comments.

## Implementation evidence

Commit `74bfd29` (comment-only; write set `snapshot.rs`, `provider_contract.rs`).

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Scenario 1 rustdoc directly attached to `complete_cold_snapshot_is_verified_source_free_and_reloadable` | `crates/bohmian-code-review-pipeline/src/snapshot.rs` (doc restored above the cold-journey test, text as at base) | Source inspection | PASS |
| Separate accurate rustdoc on `fixed_commit_build_ignores_head_advance_and_run_follows_it` | Same file; describes pinned publish, reuse after head advance, and `run` following the new head | Source inspection against the test body | PASS |
| Module rustdoc on `provider_contract::tests` | `crates/bohmian-code-review-pipeline/examples/provider_contract.rs` inner `//!` doc on the captured-report verdict checks | Source inspection | PASS |
| Verification commands | — | `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`: all exit 0 | PASS |

Non-goals held: no test body, behavior, helper, dependency, fixture, evidence report, or specification change.
