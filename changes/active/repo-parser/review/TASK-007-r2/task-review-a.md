# TASK-007 Wave 1 task review A

**Subject:** `2257992f36a2aa88f33f397af1e070d6ab35961b..f820d662a841f7e13773223b2f93cf90e82b6274`. Authority: approved `SPEC-repo-parser@14`, original `TASK-007`, `TASK-007-R1`, the stateless-snapshot maintainer decision, `AGENTS.md`, and applicable architecture references. Reviewed the complete cumulative diff, including code, task and remediation records, and the refreshed JSON evidence. The candidate was `f820d66` throughout this review.

## Acceptance matrix

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Four independent facades and the composed cold, update, PR, read journey; REQ-004/008–012/018, AC-002–004/010 | Public pipeline facades and `tests/pipeline_journey.rs`; cumulative range preserves the earlier implementation | Journey tests recorded 2/2 in TASK-007; prior review ran the gate; no changed journey code in this remediation | PASS |
| Complete, source-free snapshot facts and honest coverage under revision 14 and the stateless-snapshot decision; REQ-009/011, INV-001–007 | `snapshot.rs` still persists per-path outcomes; `run_at` delegates to the existing verified `build` path | Refreshed Wyrd, VS Code, and Rust reports each satisfy `eligible = indexed + skipped`, respectively 1,681 = 1,461 + 220; 14,691 = 12,762 + 1,929; 39,496 = 36,780 + 2,716. All report published snapshots and equal context read-back | PASS |
| S1–S19, negative cases, from-scratch/property oracles, and resource bounds; AC-008 | Existing owner tests and public journey; remediation changes neither those contracts nor the gate | TASK-007 records `mise run gate`; TASK-007-R1 records owner, journey, format, lint, and gate passes. Parent orchestrator is running the gate for this review | PASS subject to recorded gate result |
| Configurable real PR with nonempty selected context, read-back, and visible failure; J7, AC-011 | `examples/live_pr.rs` uses the production facades with injected in-memory stores; `mise.toml` keeps `live:pr` outside `gate` | Wyrd PR #1 report has 50 selected spans, full coverage, and `read_back_equal: true`; TASK-007 records visible missing-token, inaccessible-PR, wrong-ID, and empty-context failures for PR #91 | PASS |
| Prior `FIND-TASK-007-1`: reproducible fixed-commit 1k/10k/50k capacity reports; AC-007 | `BCR_COMMIT` selects `ColdBuild::run_at`; `GitHubClient::commit` verifies the requested SHA, and `ColdBuild::build` uses its tree and archive. Normal `run` still pins default head | Exact fixed-SHA commands, commit/tree, archive bytes, cold time, warm p50/p95, snapshot size, peak RSS, and calls are recorded for all three sizes. `snapshot::tests::fixed_commit_build_ignores_head_advance_and_run_follows_it` passed here: 1 selected, 75 filtered. `git diff --check` passed | PASS; prior class closed |
| Prior `FIND-TASK-007-2`: controlled real-provider compatibility; REQ-017, AC-006 | `examples/provider_contract.rs` uses production `GitHubClient` and `ReqwestTransport` for repository, commit/tree, blob, archive, and PR files; the separate ETag replay uses the same transport as a provider observation. `live:provider-contract` is opt-in | Captured Wyrd report: repository/commit/tree/blob 200, archive 302→200 with gzip magic, three PR-file pages and 38 patch omissions, API rate headers, conditional 304. Local fake client tests cover 403/429 and GHES; the report does not claim a live quota exhaustion | PASS; prior class closed |
| No consuming service, server, worker, clone, durable raw source, extra snapshot aggregate, or credentialed normal CI | Complete diff changes an example, `snapshot.rs`, `mise.toml`, and evidence; the added snapshot route reuses the existing owner | Diff inspection and `mise.toml` dependency graph show the opt-in lanes stay outside `gate`; evidence JSON contains identities, metrics, omission facts, and selected span metadata rather than source text | PASS |

## Findings and result

No proposed findings. The candidate-owned `run_at` method is needed to bind capacity runs to an immutable commit and reuses the existing cold-ingest path. The provider recorder is confined to the opt-in contract command; its raw conditional request is explicitly labeled a provider observation because the production client does not send conditional requests. I found no task drift or new source, persistence, or admission behavior in the cumulative diff.

**Overall: PASS**, conditional only on the orchestrator's independent gate result. This report checked the focused fixed-commit test, `git diff --check`, the complete changed code and documentation, and the recorded JSON arithmetic and provider statuses. I did not rerun credentialed GitHub calls, independently measure RSS, or run a second broad Cargo gate. A live 403/429 and GHES request remain local fake-test coverage, as the remediation explicitly permits.

```citations
[]
```
