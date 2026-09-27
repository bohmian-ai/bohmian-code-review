# TASK-005 review verdict

**Verdict: FIX_REQUIRED.** Base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`; candidate `e68bcdb65b44095989b5f42dfda5c7d87295582b`. Authority: approved [revision 12 spec](../../spec.md) and [original task](../../tasks/TASK-005-pr-context-and-shared-read.md). The candidate remained at `e68bcdb65b44095989b5f42dfda5c7d87295582b` during review. The cumulative eight-file diff was reviewed; reviewed source was not changed.

## Acceptance matrix

| Obligation | Implementation and verification evidence | Result |
|---|---|---|
| REQ-009, INV-004, S5–S7: pinned private PR overlay, stale-base handling | `PrContext::run`, `Ingest::reuse/fallback`; concurrent-PR and stale-base tests pass | PASS |
| REQ-008, S10: bounded scoped index of eligible nearby files | `near` limits raw entries before `Ingest::new` classifies them; existing scoped test has no intervening exclusions | FAIL: FIND-TASK-005-3 |
| REQ-010, INV-002, S18: stable pagination, file-local failures, honest coverage | pin/retry and patch tests pass; rejected PR and tree entries can still accompany `is_full()` | FAIL: FIND-TASK-005-4 |
| REQ-010, INV-006, S19: estimated diff/prompt reserve and explicit exhausted budget | `Budget` and exhausted-budget test; estimate is labeled | PASS |
| REQ-011–013, S14–S15: validated immutable publication and readable retry | dropped-publication test passes; `AlreadyExists` can return success for unreadable bytes | FAIL: FIND-TASK-005-1 |
| REQ-012–015, INV-005, AC-005/009: 24-hour identity-scoped read and cleanup | identity, expiry, missing-object and serial cleanup tests pass; identical later publication extends an old ID and races its cleanup | FAIL: FIND-TASK-005-2 |
| REQ-016/018: independent facades and separate injected stores | public pipeline APIs and in-memory adapters; pipeline lane passes | PASS |
| Non-goals: no model/comment publication, consumer implementation, detached task, cancel token, deadline, or new dependency | complete diff and manifests | PASS |

## Review record

Wave 1: task samples A and B **FAIL**; standards **PASS**; source-trust, context durability, and budget domain reviews **FAIL**. All required reports are in this directory. The final `mise run review:verify-citations` output is [citations.txt](citations.txt): all citations `OK`, no final rejected citation. The independent Wave 2 [validation ledger](findings-validation.md) merged the expiry and cleanup claims by root cause. It retained four reproduced, blocking findings, no notes, and no rejected claims:

| Finding | Root cause | Gate |
|---|---|---|
| FIND-TASK-005-1 | `AlreadyExists` is accepted without validating the existing context object | Source integrity; REQ-011–013 |
| FIND-TASK-005-2 | Distinct publications share a digest-only ID, expiry record, and cleanup target | 24-hour cutoff and durability; REQ-012–015 |
| FIND-TASK-005-3 | Scoped cap is spent before shared eligibility classification | Wrong candidates; REQ-008 |
| FIND-TASK-005-4 | Rejected provider entries do not lower the full-coverage claim | Source trust and coverage; REQ-009–010, INV-002 |

No prior `FIND-*` ledger exists, so prior-finding closure is not applicable. The self-contained correction is [TASK-005-R1-context-publication-and-coverage.md](TASK-005-R1-context-publication-and-coverage.md); route it to `$cr-implement`, then review the cumulative candidate again.

## Verification and limits

`mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check fa4fa2101950626a9fe60b0fdff0439b04c16f01 e68bcdb65b44095989b5f42dfda5c7d87295582b` passed. The gate included 56 pipeline tests and six public symbolic journey tests. The four gaps are independently reproduced source paths and interleavings; no new executable regression test was added to the immutable candidate. No live GitHub or external storage adapter was used in this review.
