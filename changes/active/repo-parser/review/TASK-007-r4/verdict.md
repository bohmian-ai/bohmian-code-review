# TASK-007 task review, round 4

**Verdict: FIX_REQUIRED.** Blocking finding: `FIND-TASK-007-4`. Remediation: [TASK-007-R4-durable-provider-test-fixture.md](TASK-007-R4-durable-provider-test-fixture.md).

## Immutable subject

Base `2257992f36a2aa88f33f397af1e070d6ab35961b`; candidate `7f7b75d5d8645f5dd2530779fb6e146dc122efe1`, unchanged during review. Authority: approved `SPEC-repo-parser@14`, original TASK-007, R1–R3 remediation and verdicts, `review/maintainer-decision-stateless-snapshot.md`, `AGENTS.md`, and applicable architecture references. The cumulative diff and recorded provider, S20, and capacity evidence were inspected.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Independently callable cold-build, update, PR-context, and read facades; composed journey, fallback, retry, S1–S19, and oracles | Public pipeline journey and stage owners | `mise run gate` passed at candidate; focused journey passed in Wave 1 | PASS |
| Real S20 PR run, nonempty spans, read-back, visible failures; AC-011 | `examples/live_pr.rs`, Wyrd #1 and #91 reports | Captured real run and negative report; offline pipeline tests passed | PASS |
| Fixed-commit 1k/10k/50k complete snapshots and stable PR base/head per sample; AC-007; prior `FIND-TASK-007-1` | `ColdBuild::run_at`, `PrContext::run_at`, pinned examples and three reports | Fixed-commit and moved-PR focused tests passed; reports reconcile eligible/indexed/NoFacts and record warm metrics | PASS |
| Controlled provider contract with all named cases enforced; AC-006; prior `FIND-TASK-007-2` | `provider_contract.rs::failures` and live report | Focused captured-case mutation test passed at candidate | PASS |
| R3 test rustdoc ownership; prior `FIND-TASK-007-3` | `snapshot.rs` and `provider_contract.rs` item docs | Direct source inspection | PASS |
| Test and gate remain usable when normal completion removes `changes/active/repo-parser/` | `provider_contract.rs:336-345` embeds active evidence; `mise.toml:54-59` compiles the example test | Required completion deletion plus compile-time `include_str!` dependency | **FAIL — FIND-TASK-007-4** |
| Source-free/stateless snapshots and task non-goals: no consumer, worker, clone, credentialed normal gate, or durable source archive | Cumulative diff, snapshot payload and `mise.toml` | Source/diff inspection and passing gate | PASS |

## Review disposition

Wave 1: task A **FAIL** (`TASK-A-1`); task B, standards, capacity and provider **PASS**. The final citation check exited 0 with both citations `OK`; no citations were rejected in that check. Wave 2 independently confirmed the claim as `FIND-TASK-007-4`, a new fixture-lifetime defect distinct from the provider-validator defect in `FIND-TASK-007-2`. No notes or other retained claims. Prior `FIND-TASK-007-1`, `FIND-TASK-007-2`, and `FIND-TASK-007-3` are **CLOSED**. The validated ledger is [findings-validation.md](findings-validation.md).

## Verification limits

`mise run gate` and cumulative `git diff --check` passed on the intact candidate; focused provider, pinning and journey tests also passed. The credentialed GitHub runs, large-repository timing and peak RSS were assessed from checked-in reports, not repeated. Review did not delete the active packet in the source worktree; the required deletion and compile-time fixture path establish the post-completion failure. These limits do not affect the reproduced lifecycle finding.
