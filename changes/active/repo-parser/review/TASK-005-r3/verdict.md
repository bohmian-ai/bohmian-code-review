# TASK-005 r3 verdict: FIX_REQUIRED

**Immutable subject:** base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`, candidate `9ce990fe933ff69de464b3ffc0a4c073d1c4754b` (`HEAD` at review start and before this verdict). Authority: approved `changes/active/repo-parser/spec.md` revision 12, original `tasks/TASK-005-pr-context-and-shared-read.md`, both prior verdicts and R1/R2 remediation tasks. The complete cumulative base-to-candidate diff was reviewed; candidate source was not edited.

| Task obligation | Source and verification | Result |
|---|---|---|
| S5–S7, REQ-009, INV-004: private pinned PR overlay and stale-base reconciliation | `PrContext::run`, shared `Ingest`; concurrent-PR and stale-base tests, full gate | PASS |
| S10, REQ-005/006/008/009, INV-002: eligible-file coverage, safe rejected-entry classification, scoped fallback | R2's shared classifier and cold/update/overlay tests pass, but a rejected root `.gitattributes` loses eligibility policy; multiple changed heads bypass the scoped index ceiling | **FAIL — FIND-TASK-005-4, FIND-TASK-005-6** |
| S18, REQ-009/010: pinned pagination, file-local rejection, exclusion of all changed paths | `PrContext::pin`, `rejected_changed`, changed-path filter; moving-PR and rejected-path tests, full gate | PASS; prior FIND-TASK-005-5 closed |
| S19, REQ-010/013, INV-006: estimated token allocation, explicit exhausted budget | `Budget` and packing; exhausted-budget test, full gate | PASS |
| S14–S15, REQ-011–015, INV-005, AC-009: identity-scoped read, 24-hour expiry, separate context store, interruption and cleanup safety | `Contexts::publish/read/collect_expired`; read, expiry and dropped-call tests pass, but cleanup can retire a record while the same publication's object create is pending past expiry | **FAIL — FIND-TASK-005-7** |
| REQ-016/018 and task non-goals: independently callable injected facades; no consumer/model/comment/dependency/symbolic-crate change | Public pipeline API, manifests and full cumulative diff; full gate | PASS |

**Review record.** Wave 1 reports: task A **FAIL** (TASK-A-1), task B **FAIL** (TASK-B-1/2), standards **FAIL** (STANDARDS-1), source trust **FAIL** (DOMAIN-TRUST-1), budget safety **FAIL** (BUDGET-1), durability **FAIL** (DOMAIN-DUR-1). All six required reports are present. [Citation check](citations.txt): 38 `OK`, no `REJECT` or contract errors. [Wave 2 validation](findings-validation.md) independently reproduced and merged the claims into three blocking root causes, with no advisory notes or unreproduced claims. It narrowed the scoped finding to *index admission*: every valid changed head still needs analysis.

| Validated finding | Status and gate | Consequence |
|---|---|---|
| FIND-TASK-005-4 | `CLASS_INCOMPLETE`; REQ-005/006/009, INV-002, source trust | A rejected root `.gitattributes` can produce falsely complete snapshot/update/overlay coverage; scoped classification and diagnostics also use the unverified policy. |
| FIND-TASK-005-6 | `CONFIRMED`; REQ-008/013 | Changed-head ingest can exceed `max_scoped_files` before the fallback cap is applied. |
| FIND-TASK-005-7 | `REVISED`; REQ-012–015, INV-005, durability | Context cleanup can delete an in-flight publication's record before its object write finishes, yielding an unreadable ID or unrecorded source object. |

Prior FIND-TASK-005-1 through FIND-TASK-005-3 and FIND-TASK-005-5 remain closed: existing-object bytes are compared, later publications use distinct IDs, neighbors are classified before spending fallback capacity, and rejected canonical changed paths are excluded from secondary selection. FIND-TASK-005-4 reopens because R2 did not distinguish a rejected eligibility-policy input from an ordinary unsupported path. The validated correction for FIND-7 uses a caller-supplied safe collection cutoff, as approved snapshot cleanup already does; the 24-hour read cutoff remains fixed.

**Verification and limits.** I ran `mise run gate` at the candidate: exit 0, including pipeline and symbolic journey tests, formatting, lints, review scripts and skill-sync checks. `git diff --check fa4fa2101950626a9fe60b0fdff0439b04c16f01 9ce990fe933ff69de464b3ffc0a4c073d1c4754b` also exited 0. Reviewers ran focused existing tests; the three blocking scenarios are source-path reproductions and have no new executable regression test in this immutable candidate. No live GitHub or external storage adapter was exercised. No required sub-reviewer was missing or held beyond the user limit. The candidate remained unchanged throughout review.

**Next task:** [TASK-005-R3-policy-scoped-bound-and-context-cleanup.md](TASK-005-R3-policy-scoped-bound-and-context-cleanup.md), routed to `$cr-implement`. Review the next immutable cumulative candidate against TASK-005 again.
