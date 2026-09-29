# TASK-001 review verdict: FIX_REQUIRED

## Immutable subject

- Base: `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec`
- Candidate: `20fa48d5cfecf0d4970d325d7334e0aede671728`
- Authority: approved `changes/active/cochange-eval/spec.md` revision 2, `tasks/TASK-001-history.md`, and `review/TASK-001-r1/TASK-001-R1-history-contract.md`
- Scope: complete base-to-candidate diff; source HEAD remained the candidate throughout review.

## Acceptance matrix

| Obligation | Implementation and verification evidence | Result |
|---|---|---|
| Three fixed, ordered 50-PR samples and exact Python, Rust, `.ts`/`.tsx` eligibility | `main.rs` repository constants and `Api::select`; `sample.rs` `Discovery` and `eligible`; sampler and fake-provider tests | PASS |
| Base commit times, cold depth ten, strict trial history, mass-edit exclusion | `Api::base_time`, `History::build`, `History::for_trial`; history tests and complete paginated mass-edit fixture | PASS |
| One merged-PR listing sequence, complete bounded file listings, no history read per trial, and no partial report on required-read failure | `Api::select`, `history::changed`, `run`; fake-provider request and failure tests | PASS |
| Production provider-request costs and separate evaluation-only timestamp reads | `History::ListingReads` and `record`; fake PR 15 takes two file pages but the report counts one | FAIL: `FIND-TASK-001-3` |
| Bounded outer-pipeline ingest | `Api::get` accumulates direct JSON bodies without `RequestLimits::max_json_bytes` | FAIL: `FIND-TASK-001-5` |
| Opt-in read-only command; no production API, persistence, gate, grammar, ranking-arm, or historical-report change | Complete diff, `mise.toml` task, no changed production crate | PASS |

## Review results

Wave 1: task reviews A and B **FAIL**; standards review **FAIL**; history/provider domain review **FAIL**. After reviewers corrected citation symbol spellings, the final citation check passed for every finding. There are no rejected final citations or notes. Wave 2 independently retained two distinct blocking root causes:

| ID | Status | Gate | Root cause |
|---|---|---|---|
| `FIND-TASK-001-3` | CLASS_INCOMPLETE | REQ-002, TASK-001 Scenario 3 | Actual HTTP file-page counts are lost when a complete PR listing is reduced to one logical read. |
| `FIND-TASK-001-5` | CONFIRMED | `AGENTS.md:23`, design `:22` | Direct REST JSON bodies are appended without the existing bounded-ingest limit. |

The R1 fixes for `FIND-TASK-001-1`, `-2`, and `-4` close their diagnosed behaviors. `FIND-TASK-001-3` remains open because the earlier unit correction still undercounts paginated listings. `findings-validation.md` records source traces, sibling paths, reproduction, and correction boundaries. The remediation is `TASK-001-R2-provider-cost-and-bound.md`.

## Verification limits

Independent local checks passed: `mise run test:pipeline` (23 context-relevance example tests), `mise run fmt:check`, `mise run lints`, and `git diff --check a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec 20fa48d5cfecf0d4970d325d7334e0aede671728`. The candidate task records a passing `mise run gate`; this review did not rerun it. No live credentialed run was made. The existing fake test passes while asserting the undercount; the direct-body finding is reproduced by source trace and the written bounded-ingest rule.

**Verdict: FIX_REQUIRED.**
