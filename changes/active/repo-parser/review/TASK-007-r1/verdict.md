# TASK-007 task review, round 1

**Verdict: FIX_REQUIRED.** Findings: `FIND-TASK-007-1`, `FIND-TASK-007-2`. Remediation: [TASK-007-R1-fixed-commit-and-provider-proof.md](TASK-007-R1-fixed-commit-and-provider-proof.md).

## Immutable subject

- Base: `2257992f36a2aa88f33f397af1e070d6ab35961b`
- Candidate: `6e9ab0f764f9d7961924dac7fb1e5c04be02a38c`
- Authority: approved `SPEC-repo-parser@14`, original `tasks/TASK-007-integrated-pipeline-and-real-pr.md`, `review/maintainer-decision-stateless-snapshot.md`, `AGENTS.md`, and applicable architecture references.
- The complete range refreshes four JSON evidence reports and the TASK-007 record, and removes a superseded Wyrd benchmark. Pipeline code landed before the base and was reviewed at the candidate. The candidate remained unchanged through both review waves. No prior TASK-007 finding IDs exist.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Four independently callable facades and composed cold, update, PR, read journey; REQ-017/018, AC-002/003/004/010 | `crates/bohmian-code-review-pipeline/{src,tests/pipeline_journey.rs}` | `mise run gate` passed locally; pipeline journey passed 2/2; owner tests and from-scratch oracle | PASS |
| Complete source-free snapshots and compare update under revision 14 and the stateless-snapshot decision | `snapshot.rs`, `update.rs`; three refreshed cold reports | Wyrd 1,681, VS Code 14,691, and Rust 39,496 eligible paths each equal indexed plus recorded `NoFacts` skips, with published cold pointers | PASS |
| Honest overlay, budgets, omissions, source and identity isolation; INV-001–007 | Pipeline source, journey and injected-adapter tests | PR #91 reports exhausted budget and no spans; VS Code reports incomplete overlay rather than full PR coverage; gate passed | PASS |
| S20 configurable real PR, nonempty selected context and equal read-back; AC-011 | `examples/live_pr.rs`, opt-in `mise.toml` lane | Wyrd PR #1 report has 50 spans, full coverage and equal read-back; task record reports visible missing-token, inaccessible-PR, wrong-ID and empty-context failures | PASS |
| Controlled real-provider contract; REQ-017, AC-006 | GitHub client and S20 source paths | Public live runs and local fake tests exist; no separate captured controlled contract cases establish the required conditional/rate behavior | FAIL — `FIND-TASK-007-2` |
| Reproducible fixed-commit 1k/10k/50k capacity measurements; AC-007 and TASK-007 approach 4–5 | `examples/live_pr.rs` calls `ColdBuild::run`, which pins the mutable default branch | Reports contain all requested metrics and warm reuse, but TASK-007 explicitly limits reruns to the period each commit remains default head | FAIL — `FIND-TASK-007-1` |
| Non-goals: no consumer service, server, worker, clone, persistent raw source, or S20 in normal CI | Outer crate, `mise.toml`, complete diff | No forbidden addition in the range; gate is credential-free | PASS |

## Review and validation

Wave 1 results: `task-review-a.md` **FAIL** (`TASK-A-1`, `TASK-A-2`); `task-review-b.md` **FAIL** (`TASK-B-1`); `standards-review.md` **FAIL** (`STD-1`); `domain-review-live.md` **PASS**; `domain-review-capacity.md` **PASS**. All five required reviewers completed. The domain reports accept the live observations while recording their replay limits; the task and standards reports apply the stricter written proof obligations.

`mise run review:verify-citations changes/active/repo-parser/review/TASK-007-r1 6e9ab0f` exited 0. `citations.txt` rejects `TASK-A-1` because three cited symbols are not verbatim in candidate files; that source finding was dropped. `STD-1`, `TASK-B-1`, and `TASK-A-2` survived. `claim-ledger.md` contains only those claims. Wave 2 independently reproduced them from the source path and report shape, merged the fixed-commit claims by root cause, and confirmed the two blocking findings in `findings-validation.md`. There are no notes and no prior findings to reopen.

## Verification limits

I ran `mise run gate` to a zero exit, including formatting, lints, review-script checks and Rust tests, and `git diff --check 2257992 6e9ab0f` passed. Reviewers separately ran the focused pipeline journey (2/2) and checked JSON arithmetic. No credentialed GitHub rerun, controlled-provider response capture, GHES call, live 403/429, or independent peak-RSS measurement was performed in this review. The recorded reports are read-only evidence of earlier runs. No sub-reviewer timed out or was omitted.
