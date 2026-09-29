---
id: TASK-003
kind: implementation
status: review
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

## Implementation Evidence

Live command, cutoff `2026-09-01T00:00:00Z`, read-only `GITHUB_TOKEN` in the environment:

```sh
BCR_EVAL_CUTOFF=2026-09-01T00:00:00Z mise run live:context-relevance
```

The first attempt exited 1 with `PR #7813 listed twice; the PR listing changed while it was read`: discovery paged `pydantic/pydantic-ai` newest-updated first and qualified PRs between pages, so every PR update during the minutes-long qualification shifted unread pages. Fixed in the TASK-001 sampling owner (`777096c`): `Api::select` reads the whole `state=all&sort=created&direction=asc` listing before qualifying, which activity can only append to, and `Discovery::into_sample` orders kept PRs by merge time then number, descending. The duplicate check still fails a shifted read. `sample::tests::discovery_orders_merged_prs` (renamed from `…_and_settles_…`) covers the ordering; the fix and the test API change landed together, so no separate RED was observed. No sample, scoring, or report rule changed.

Two subsequent full runs each exited 0. Their stdout was byte-identical (sha256 `7c0c3a9c0582d5d6546e8f755754bac0364f3ea2be5c52f828ddd4bee8ab359b`; reports carry no timings), so sample, pinned commits, omissions, per-trial results, and aggregates reproduce exactly. The stdout was split per repository into the checked-in reports (`f81b492`):

| Repository | Report | PRs | Trials | Omitted files | Merged range | Hit rate at 10 (all) |
|---|---|---|---|---|---|---|
| `pydantic/pydantic-ai` | `reports/pydantic-ai.json` | 50 | 171 | 12 | 2026-08-23T19:15:42Z – 2026-08-31T23:45:58Z | A 0.415, B 0.567, C 0.602, D 0.556 |
| `tokio-rs/tokio` | `reports/tokio.json` | 50 | 159 | 0 | 2026-05-21T13:41:55Z – 2026-08-20T06:57:25Z | A 0.239, B 0.258, C 0.333, D 0.321 |

Each PR's base, head, and base tree commits are in its report's `sample` entries.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Exactly 50 qualifying PRs per repository with cutoff, tie ordering, pinned commits, omissions, every trial and aggregate | `reports/{pydantic-ai,tokio}.json`: `cutoff`, `command`, 50 `sample` entries (number, merged_at, base, head, base_tree, source_files, omitted), `trials`, `summary.{all,non_test}` per variant, `misses` | Report inspection: 50 samples each, keys and per-variant aggregates present; ordering by `Discovery::into_sample` | PASS |
| Second run reproduces cohort and per-trial results; no in-run replay or reconciliation | Discovery reads one listing once; no replay or cross-check added | Two runs, `cmp` identical, same sha256 | PASS |
| No source text or credentials; opt-in lane outside CI and `gate` | `live:context-relevance` unchanged and absent from `gate` | Scan of every report string: only paths, commits, names, reason codes, and the fixed rule text; token absent from both reports | PASS |

Verification, sequential, exit 0 each: `mise run test:pipeline` (77 + 2 + 16 + 1 passed), `mise run test:pipeline:journey` (2 passed), `mise run test:symbolic-context:journey` (6 passed), `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`.

Boundaries: executable changes are limited to `examples/context_relevance/{main,sample}.rs`; no production crate, `PrContext` call, replay layer, or gate change. Material limits: each run takes about 20 minutes, dominated by per-PR base snapshots and per-blob fetches. A PR deleted from the listing during the read would shift pages silently rather than duplicate; GitHub rarely deletes PRs. FIND-TASK-002-5 remains the accepted known limit.
