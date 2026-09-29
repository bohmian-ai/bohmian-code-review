---
id: TASK-001-R2
kind: remediation
status: proposed
spec: SPEC-cochange-eval
spec_revision: 2
parent_task: TASK-001
remediates: [FIND-TASK-001-3, FIND-TASK-001-5]
---

# Close TASK-001 provider-cost and bounded-ingest findings

Base `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec`; reviewed cumulative candidate `20fa48d5cfecf0d4970d325d7334e0aede671728`. Authority: approved `changes/active/cochange-eval/spec.md` revision 2. Original task: `changes/active/cochange-eval/tasks/TASK-001-history.md`. Prior remediation: `changes/active/cochange-eval/review/TASK-001-r1/TASK-001-R1-history-contract.md`. Reassess the complete cumulative candidate in the next review.

## Diagnosis and correction

1. **`FIND-TASK-001-3` — request accounting remains incomplete.** `Api::listing` at `crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs:196-213` calls `pull_request_files` once per page, but returns only a complete `Listing`. `History::build` at `history.rs:119-124` increments `ListingReads.cold` or `.incremental` once per PR; `record` at `main.rs:552-559` presents those logical listing counts as production provider requests. Both cached qualification listings (`main.rs:289-306,433-463`) and history-only listings (`main.rs:458-465`) share this loss of page counts. The cold total, incremental per-PR unit, and incremental aggregate are all affected. REQ-002 asks for production *provider requests*, while REQ-001's one listing per PR can require multiple page requests. The existing fake reads PR 15 on pages 1 and 2 (`main.rs:815-819`) within 11 cold logical listings and one merged-PR listing page, yet asserts cold cost `1 + 11` (`main.rs:826-835`): 12 reported versus 13 actual HTTP requests. A paginated incremental PR likewise costs more than the fixed `per_merged_pr: 1` and the current aggregate. The passing test proves listing reuse but locks in the cost error. **CLASS_INCOMPLETE** reopens R1's cost correction because the prior shape distinguished one PR from the run aggregate while discarding page counts.

   Correct the existing listing/accounting boundary so each needed PR's actual file-page request count survives through history selection into `Requests` and `record`. Keep the logical one-complete-listing-per-PR invariant separate from HTTP request cost. Report the actual cold total, the page requests needed for observed incremental PR updates, and any aggregate under an explicit aggregate label. Preserve cached listing reuse, full pagination and completeness failure, cold depth ten, review-time zero, and separate evaluation-only timestamp reads. This stays within the example's candidate-owned report shape and needs no new provider framework or production API.

2. **`FIND-TASK-001-5` — direct JSON bodies are unbounded.** The materially rewritten `Api::get` at `main.rs:166-187` streams every 200 response body chunk into `Vec<u8>` via `bytes.extend(chunk)` at line 185 before decoding. Repository metadata (`main.rs:407`), merged-PR listing pages (`main.rs:416-424`), and base-commit timestamps (`main.rs:260-279,322-324`) all use this owner. A large successful body can grow memory past the pipeline's existing JSON limit. This violates bounded ingest in `AGENTS.md:23` and `architecture/bohmian-code-review-design.md:22`. The base had an unbounded direct read, but the cumulative candidate materially rewrote this owner. The client already defines `RequestLimits::max_json_bytes` and uses it in its own bounded reader; the example's private direct reader bypasses it.

   Apply that existing limit at `Api::get`, rejecting a chunk that would exceed it before appending. Keep the direct path's single attempt, fail-on-non-200 behavior, and JSON decoding. The pipeline client's private reader is unavailable to the example; a small check at the shared direct owner covers all three callers. No additional transport or configuration layer is needed.

## Scope and closure proof

Expected write set: `crates/bohmian-code-review-pipeline/examples/context_relevance/{main,history}.rs` and their inline tests; touch `sample.rs` only if preserving page counts through the existing qualification cache actually requires it. Keep production crates and public APIs, persisted formats, grammar pins, checked-in historical reports, and `gate` unchanged. Do not add ranking arms, packing claims, a production co-change store, ancestry lookups, or per-trial provider reads.

- For `FIND-TASK-001-3`, extend the existing no-network provider run proof. A paginated cold history PR must make the report count 13 actual cold HTTP requests in the current fixture. Add a paginated incremental PR and prove its second page appears in the observed update cost and aggregate. Cover both cached and history-only listing paths while asserting one complete logical listing per PR, no per-trial history reads, correct evaluation-only timestamps, and complete failure at the page ceiling.
- For `FIND-TASK-001-5`, feed a valid 200 JSON response just above `RequestLimits::max_json_bytes` through the existing fake direct transport and prove failure before full buffering and report emission. A response at the limit must still decode. Because all direct reads share `Api::get`, this proves the rule for repository metadata, listing pages, and commit timestamps at their common owner.

Run exact focused new test selectors, `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Run `mise run gate` for the cumulative candidate before its next immutable review. Route directly to `$cr-implement`.

## Implementation evidence

Commit `c12e8b7`. Write set: `crates/bohmian-code-review-pipeline/examples/context_relevance/{main,history}.rs` only; `sample.rs`, production crates, public APIs, persisted formats, grammar pins, historical reports, and `gate` unchanged.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| `FIND-TASK-001-3`: actual file-page requests survive into `Requests` and `record`; logical one-listing-per-PR kept separate | `Api::listing` returns `(Listing, pages)`; qualification cache stores `(Changed, pages)`; `History::build` sums cold pages and records `ListingReads::incremental` pages by PR; report emits `cold_build`, `incremental_update.page_requests_by_merged_pr`, and `aggregate_page_requests_through_newest_base` | `tests::run_reads_each_input_once_and_reports_costs`: history-only paginated cold PR 15 gives `cold_build` 13; cached paginated incremental PR 40 appears as 2 pages, aggregate 50; each listing URL read once, 50 timestamp reads, no per-trial reads; `history::tests::history_is_cold_depth_plus_time_eligible_increments` asserts per-PR incremental pages | PASS |
| Complete failure at the page ceiling on both listing paths | Unchanged `history::changed` completeness check | `tests::run_fails_without_report_on_missing_incomplete_or_rate_limited_reads`: PR 14 (history-only) and PR 30 (cached) each read 30 pages then fail with `IncompleteListing`, no report | PASS |
| `FIND-TASK-001-5`: direct JSON bounded by `RequestLimits::max_json_bytes` before buffering | `Api::get` rejects a chunk that would pass the limit before appending; single attempt, non-200 failure, and decoding unchanged | `tests::direct_json_is_bounded_by_the_client_limit`: body at the limit decodes; limit + 1 fails the run with no report and no listing reads | PASS |

Commands, all exit 0 in this session: `cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features` (24 passed), `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`. Non-goals (ranking arms, packing claims, co-change store, ancestry lookups, per-trial reads) remain excluded. Limit: no live credentialed run.
