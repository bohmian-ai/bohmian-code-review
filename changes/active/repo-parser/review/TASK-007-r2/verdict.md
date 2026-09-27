# TASK-007 task review, round 2

**Verdict: FIX_REQUIRED.** Blocking findings: `FIND-TASK-007-1`, `FIND-TASK-007-2`. Remediation: [TASK-007-R2-stable-workload-and-contract-validation.md](TASK-007-R2-stable-workload-and-contract-validation.md).

## Immutable subject

- Base: `2257992f36a2aa88f33f397af1e070d6ab35961b`
- Candidate: `f820d662a841f7e13773223b2f93cf90e82b6274` (unchanged throughout review)
- Authority: approved `SPEC-repo-parser@14`, original `TASK-007`, `TASK-007-R1`, prior r1 verdict/findings, `review/maintainer-decision-stateless-snapshot.md`, `AGENTS.md`, and applicable architecture references.
- The complete cumulative range includes the fixed-commit cold path and test, controlled-provider example and lane, refreshed real-provider/capacity reports, task evidence, and prior review packet. The resulting pipeline and its unchanged facade consumers were inspected in scope.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Four independent facades; composed cold, update, PR, read journey; S1–S19 and from-scratch oracles; AC-002–004/010 | Existing pipeline owners and `pipeline_journey.rs`; unchanged production PR/update paths | `mise run gate` passed locally, including pipeline and public journeys | PASS |
| Revision 14 source-free, complete snapshots and stateless per-path outcomes | `ColdBuild::run_at` shares verified cold `build`; no new stored field | Three reports reconcile eligible = indexed + NoFacts (1,681; 14,691; 39,496) and publish complete cold snapshots | PASS |
| Configurable live S20, nonempty selected context and equal read-back; AC-011 | `live_pr.rs` uses the public facades with injected stores | Wyrd PR #1 report has 50 spans and equal read-back; PR #91 visibly fails empty context | PASS |
| Reproducible fixed-commit 1k/10k/50k capacity evidence; AC-007; prior `FIND-TASK-007-1` | `BCR_COMMIT` pins cold build, while first and warm `PrContext::run` calls independently read current PR | Fixed cold SHA test passes and reports carry required metrics; warm samples lack one enforced PR base/head workload | **FAIL — `FIND-TASK-007-1` CLASS_INCOMPLETE** |
| Controlled real-provider contract; REQ-017/AC-006; prior `FIND-TASK-007-2` | `provider_contract.rs` uses production client/transport plus a labeled raw conditional observation | Captured Wyrd report has passing tree, blob, archive, patch-omission, rate-header and 304 observations; command can still exit 0 on failed named outcomes | **FAIL — `FIND-TASK-007-2` CLASS_INCOMPLETE** |
| Non-goals: no consumer service, clone, server, worker, durable source or credentialed normal CI | Complete diff and `mise.toml` | No prohibited surface added; `live:pr` and `live:provider-contract` remain opt-in | PASS |

## Review disposition

Wave 1: task A **PASS**, task B **FAIL** (`TASK-B-1`), standards **PASS**, capacity domain **FAIL** (`CAP-1`), provider domain **FAIL** (`PROV-1`). Citation verification exited 0 with all 12 entries `OK`; no rejected citations. Wave 2 independently traced all claims, merged `TASK-B-1` with `CAP-1`, and reproduced both blocking paths in [findings-validation.md](findings-validation.md). No notes or rejected claims. Both prior finding IDs reopen as `CLASS_INCOMPLETE`; cold pinning and the captured passing provider observations are credited, but neither earlier correction closes its whole proof class.

## Verification limits

`mise run gate` and `git diff --check 2257992 f820d66` exited 0. Task A's exact fixed-commit focused test passed (1 selected). The five checked-in JSON reports parsed. This review did not repeat credentialed GitHub calls, independently measure peak RSS, or provoke live 403/429/GHES cases; existing fake tests cover those branches. Source traces establish the two false-pass paths. All required reviewers completed within the requested time limit.
