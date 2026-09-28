---
id: TASK-003
kind: implementation
status: proposed
spec: SPEC-context-relevance-eval
spec_revision: 4
requirements: [REQ-001, REQ-002, REQ-006, REQ-007, INV-001, INV-003, INV-004, INV-005, AC-001, AC-002, AC-005]
depends_on: [TASK-002]
parent_task:
remediates: []
---

# Reproducible live reports

## Outcome and Value

Checked-in reports for `pydantic/pydantic-ai` and `tokio-rs/tokio` show the fixed cohort, all trial and aggregate results, and an exact rerun command. A second invocation reproduces the sample and per-trial results.

## Owners, Scope, Consumers, and Prohibited Changes

The opt-in evaluation command and tracked report artifacts own this outcome. Use read-only GitHub credentials and the approved evaluator. Do not make the live run part of `gate`, commit credentials or source text, alter production code, or change the approved sample and scoring rules to make a report pass.

## Approach

1. Choose and record one fixed cutoff and exact invocation for both repositories.
2. Run the opt-in evaluator with read-only credentials; diagnose incomplete coverage or provider failures instead of accepting partial reports.
3. Run it again with the same cutoff and compare sample and per-trial results, excluding wall-clock timings.
4. Check in the two source-free reports and record their verification evidence.

## Proof Strategy

This task produces live data artifacts from behavior implemented and tested in TASK-001 and TASK-002. It does not add executable behavior, so a manufactured RED is not applicable. The proof is two real API runs per repository, deterministic report comparison, report-schema inspection, and the repository checks below. If a defect requires a behavior change, add a focused RED/GREEN test in the owning prior task before rerunning the live reports.

## Acceptance Criteria

- Exactly the 50 qualifying PRs per repository are represented with cutoff, tie ordering, pinned commits, omissions, and every required trial and aggregate.
- A second run reproduces the cohort and per-trial results exactly apart from timings; no in-run PR-list replay or reconciliation is added to force agreement.
- Reports contain no source text or credentials; the opt-in lane remains outside normal CI and `gate`.

## Size Expectation

This task is two report artifacts and rerun evidence. It should add no evaluation framework or provider synchronization machinery; any code defect goes back to the narrow owner in TASK-001 or TASK-002.

## Expected Write Set and Consumer Closure

Likely: two checked-in JSON reports and any brief rerun instructions associated with the opt-in command. Changes to executable evaluator code belong with its owner and require the corresponding behavioral test. No production crate behavior change is expected.

## Verification and Evidence

Record the exact live command, cutoff, pinned PR commits, run outputs, and comparison result. Run `mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check` sequentially. Inspect tracked and untracked files for secrets, source text, and unintended production changes. An absent live target or incomplete sample is a failure, not a skip.

## Material Stop Conditions

Stop if the real API cannot supply the specified 50 qualifying PRs at the fixed cutoff, if repeated pinned inputs produce different per-trial results, or if the reports would disclose source or credentials. If credentials are unavailable, finish local checks and report the live evidence as blocked without fabricating reports.

## Authority Links

[Approved spec](../spec.md); [contributor rules](../../../../AGENTS.md); [design](../../../../architecture/bohmian-code-review-design.md); [testing workflows](../../../../architecture/references/languages/testing-workflows.md).
