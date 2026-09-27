# TASK-007 r3 task review A

Subject: `2257992f36a2aa88f33f397af1e070d6ab35961b..b721917420113ab2e3530966e9e456eecc89dcff`; `SPEC-repo-parser@14`, TASK-007, TASK-007-R1, TASK-007-R2, and the stateless-snapshot maintainer decision. I reviewed the cumulative code diff and the recorded evidence, not the implementation summary.

| Requirement, criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Four independent facades and composed cold → update → PR → read journey; no production all-in-one method (REQ-018, AC-002/003/010) | `tests/pipeline_journey.rs`; `src/snapshot.rs`, `src/update.rs`, `src/pull_request.rs`, `src/context.rs` | `mise run test:pipeline:journey`: 2 passed; owner scenario coverage recorded in TASK-007 | PASS |
| S1–S19, oracle, source trust, bounded failure and recovery contracts (AC-002–005, 009) | Existing owner tests and public journey; cumulative diff does not weaken those paths | Prior task reviews and TASK-007 `mise run gate` evidence; journey rerun above | PASS |
| S20 real PR with nonempty context, read-back, explicit failure on inaccessible/empty input, separate facade timeouts (AC-011) | `examples/live_pr.rs` uses `ColdBuild`, `PrContext`, and `Contexts`, prints identity, coverage, spans, calls and timings, then rejects empty spans or unequal read-back | `evidence/TASK-007/s20-wyrd-pr1.json`: 50 spans, full coverage, equal read-back; TASK-007 records credential and inaccessible-PR failures; live call not rerun in this review | PASS |
| Fixed cold commit and stable base/head for every capacity sample (AC-007; R1/R2 `FIND-TASK-007-1`) | `ColdBuild::run_at` verifies selected commit/tree; `live_pr.rs` reads one PR and uses `PrContext::run_at` for first and all warm calls; `PrContext::list` rereads and rejects movement before publication | Focused `fixed_commit_build_ignores_head_advance_and_run_follows_it` and `samples_at_one_read_pr_refuse_a_moved_head` both passed; three JSON reports and TASK-007 record fixed commit/tree and PR pair | PASS |
| Reproducible 1k/10k/50k capacity records, complete snapshots and warm reuse after TASK-008/009/010 (AC-007) | `s20-wyrd-pr1.json`, `bench-vscode.json`, `bench-rust.json`; TASK-007 size-class table and exact commands | Parsed reports: eligible 1,681/14,691/39,496; all `Published`, `full_coverage: true`, `read_back_equal: true`, and 3 warm samples; cold/warm timings, snapshot bytes and call counts present. TASK-007 records archive bytes and peak RSS. Live runs not rerun here | PASS |
| Controlled, opt-in, real-provider compatibility proof that fails each missing or failed case (AC-006; R1/R2 `FIND-TASK-007-2`) | `examples/provider_contract.rs` calls production client/transport, records exchanges, validates seven required cases, tree, archive, patch omission, conditional 304 and API rate headers; `mise.toml` keeps lane outside gate | Captured `provider-contract-wyrd.json` has passing statuses and values; `cargo test --locked -p bohmian-code-review-pipeline --example provider_contract every_named_case_decides_the_verdict` passed, including mutations of each named failure class. Live rerun not performed here | PASS |
| Stateless source-free snapshot and excluded scope (revision 14, maintainer decision, INV-001/005) | `run_at` uses the existing snapshot publication owner; `run_at` PR context uses existing overlay/publication; no new persisted aggregate, source archive, consumer, server, worker or normal-CI credentialed lane | Cumulative diff inspection; `git diff --check 2257992 b721917` passed | PASS |

## Findings

None. The R1 and R2 defect classes close at their owners: fixed commit selection, stable PR workload, and contract-result validation. The new public `run_at` methods serve the opt-in evidence path; ordinary `run` retains fresh default-head and PR pinning. I found no required behavior omitted, incorrect behavior, scope drift, written-rule violation, or regression in the reviewed range.

## Verification limits and result

The captured live GitHub runs and broad `gate` are evidence from the candidate record; I reran the credential-free journey and three focused checks above. The first attempted example test command used `--exact` on an unqualified name and filtered out its test; the corrected command ran one passing test. **PASS**.

```citations
[]
```
