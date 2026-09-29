# History and provider boundary review

Subject: base `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec`, cumulative candidate `a2eeb67dfc4b739513c74dd085c0bd8ff549ca71`; approved `changes/active/cochange-eval/spec.md` revision 2 and `TASK-001-history.md` with R1–R3 remediation tasks.

## Reviewed boundary and coverage

| Boundary | Authority | Source and proof inspected | Result |
|---|---|---|---|
| Merged default-branch PR discovery, cutoff, timestamp and identity trust | REQ-001, INV-002; `AGENTS.md` source-trust and bounded-ingest rules | `sample.rs` `Discovery::new`, `offer`, `into_sample`, `is_timestamp`; `main.rs` `Api::select`, `Api::base_time`; sample and provider fake tests | PASS |
| Complete and bounded PR-file listings; mass-edit classification | REQ-001, INV-002; R1/R2; `architecture/agent-rules.md` effective-input validation | `main.rs` `Api::listing`, `qualify`, `select`; `sample.rs` `Listing::push`, `Screen::new`; `history.rs` `changed`; page-ceiling and rejected-entry tests | PASS |
| Cold depth, temporal eligibility, trial exclusion, and listing reuse | History definition, INV-003, AC-003; R1/R2 | `history.rs` `History::build`, `for_trial`; `main.rs` cached qualification listings and history-only read closure; focused history and whole-run fake tests | PASS |
| Request accounting and bounded direct JSON reads | REQ-002, INV-005; R1/R2; `architecture/bohmian-code-review-design.md` bounded ingest | `main.rs` `Requests`, `Api::get`, `listing`, `record`; pipeline `RequestLimits`; paginated cold/incremental and oversize-body tests | PASS |
| Failure atomicity | REQ-001, INV-002, INV-005; TASK-001 scenario 3 | `main.rs` `run` buffers all repository records before `print`; missing commit, incomplete listing, exhausted client rate limit, and oversize body tests | PASS |

The cumulative source diff was reviewed against the original task and R1–R3 corrections. The example's direct REST reader rejects non-200 responses and bodies above the pipeline JSON limit; the client owns retry handling for PR-file listings. The sample is ordered by validated merge time then PR number, cold history stops after ten complete non-mass PRs before the earliest sampled base, and trial history admits only merges strictly before that trial's base time. File-page requests survive both cached and history-only paths into separate cold, per-PR incremental, aggregate, and evaluation-only report counts.

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features` passed: 24 tests, 0 failures. The tests use a no-network provider fake. No live credentialed GitHub run or cross-run reproducibility measurement was available; those are outside this bounded domain review. No candidate source was edited.

## Proposed findings

None.

## Result

**PASS** for the history/provider boundary.

```citations
[]
```
