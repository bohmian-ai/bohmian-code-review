# TASK-007 round 5: independent task review A

**Subject:** `2257992f36a2aa88f33f397af1e070d6ab35961b..317b2da9a8cd45755418dca05111c96beb19eaa4`; approved `SPEC-repo-parser@14`, original TASK-007, R1–R4 remediation, prior verdicts, and the stateless-snapshot maintainer decision. I inspected the cumulative code and task diff, the checked-in live reports, the relevant facade paths and their callers. The large JSON reports were inspected by structure and recorded results rather than line-by-line source entries.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Separate cold, update, PR and read facades compose; S1–S19, scoped fallback after dropped cold build and retry; REQ-008/011/012/017/018, AC-002/003/004/010 | Existing `pipeline_journey.rs` and pipeline stage owners; `PrContext::run` remains separate | `pipeline_journey` ran 2/2 at this candidate; prior gate and owner-test evidence in task record | PASS |
| Source trust, source-free complete snapshots, stateless per-path outcomes, honest coverage and isolation; REQ-004, INV-001–005/007 | `ColdBuild::run_at` shares verified `build`; `GitHubClient::commit` verifies requested SHA; snapshot payload and stage owners remain unchanged | Prior owner tests and task evidence; Wyrd, VS Code and Rust reports reconcile eligible paths with indexed and recorded skips | PASS |
| Pin PR base/head, candidate budget and omission behavior; REQ-009/010, INV-006 | `PrContext::run_at` shares `list` and `publish` with `run`; `list` re-reads base/head before publication | Focused moved-head test is recorded green in R2; PR #91 report records empty context as failure; public journey passes | PASS |
| Fixed-commit 1k/10k/50k capacity runs after TASK-008/009/010; AC-007, prior FIND-TASK-007-1 | `ColdBuild::run_at`, `PrContext::run_at`, `BCR_COMMIT`, one PR read for all samples, recorded immutable commit/tree and merged PR base/head in three reports | Exact fixed-commit test ran 1/1 at this candidate; checked-in reports include complete snapshots, archive bytes, cold/warm timings, RSS, snapshot bytes and API calls | PASS |
| Controlled opt-in provider contract; REQ-017, AC-006, prior FIND-TASK-007-2 | `provider_contract.rs` uses production client/transport; `failures` rejects absent/failed named cases; `live:provider-contract` is outside gate | Exact captured-case test ran 1/1; live Wyrd report records seven passing cases, 302→200 archive, 304 observation and rate headers; fake transport covers 403/429 and GHES | PASS |
| Durable provider verdict proof after active packet completion; R4, prior FIND-TASK-007-4 | Example test includes `examples/provider_contract_cases.json` inside the crate; no pipeline-crate compile-time include from `changes/active/` | Exact provider test ran 1/1 with the active packet present; R4 records 1/1 in an isolated copy without `changes/active/` | PASS |
| Configurable real PR S20, nonempty selected spans and equal read-back, visible failures; AC-011 | `examples/live_pr.rs` calls public facades with injected in-memory stores, per-call timeout, and read-back; `live:pr` is opt-in | Recorded Wyrd PR #1 has 50 spans, full coverage, equal read-back; PR #91 empty-context and missing credentials/access are recorded failures | PASS |
| Required rustdoc and normal credential-free gate; prior FIND-TASK-007-3 | New/materially changed Rust items have owner-local rustdoc; live lanes are separate from `gate` | Source inspection; prior R4 gate, fmt and lint results; cumulative `git diff --check` passed here | PASS |
| Non-goals: no consumer service, clone, worker, new durable aggregate state, source archive or credentialed normal CI | Cumulative diff changes pipeline facades, opt-in examples, reports, one crate-owned fixture and task/review records | Cumulative name/status inspection and `mise.toml` | PASS |

**Findings:** None. The candidate-owned `run_at` APIs add only the control needed for fixed benchmark input and reuse existing verified stage owners. The R4 fixture is a small, durable case subset used by the existing validator test; I found no task-required reason to retain the larger live report inside the crate.

**Verification limits:** I did not repeat credentialed GitHub runs, large-repository benchmarks, peak-RSS measurement, or the full gate. Those results are checked-in prior evidence. The focused tests above ran at this candidate, and the R4 completion-copy check is recorded implementation evidence rather than independently rerun here. My first fixed-commit test invocation used an incomplete exact selector and selected zero tests; I reran it with the full module path and observed one passing test.

**Overall: PASS.** Prior FIND-TASK-007-1 through FIND-TASK-007-4 are closed for this task sample.

```citations
[]
```
