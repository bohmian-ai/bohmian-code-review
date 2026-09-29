# Domain review: history and provider boundary

**Subject:** base `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec`, cumulative candidate `ad498ab2c58d0334c2ded38fcc01e835bfcdac48`. **Result: PASS.**

## Authority and source coverage

| Boundary | Authority | Candidate source inspected | Result |
|---|---|---|---|
| One merged-PR discovery, deterministic sample, one base timestamp read per sampled PR | `spec.md` REQ-001, INV-002; `TASK-001-history.md` Scenarios 1–2 | `sample.rs` `Discovery`, `Screen`, `eligible`; `main.rs` `Api::select`, `Api::qualify`, `Api::base_time`; full base-to-candidate example diff | PASS |
| Cold depth ten, mass-edit exclusion, strict trial-time eligibility, no per-trial provider reads | `spec.md` History, INV-003, AC-003 | `history.rs` `changed`, `History::build`, `History::for_trial`; `main.rs` history and trial call paths | PASS |
| Complete listings, bounded provider responses, complete failure | `spec.md` REQ-001, INV-002, INV-005; `AGENTS.md` bounded ingest; `architecture/bohmian-code-review-design.md` outer pipeline boundary | `main.rs` `Api::get`, `Api::listing`, `Api::select`, `run`, `print`; `sample.rs` `Listing::push`; pipeline `github.rs` transport, limits, PR-file reader | PASS |
| Production request cost, separate evaluation-only timestamp reads | `spec.md` REQ-002; `TASK-001-R2-provider-cost-and-bound.md` findings 3 and 5 | `main.rs` `Api::listing`, qualification cache, `record`; `history.rs` `ListingReads`, `History::build`; fake provider tests | PASS |

The full base-to-candidate diff was inspected for the example's `main.rs`, `sample.rs`, and `history.rs`; the provider client and applicable design, spec-driven, and testing authorities were inspected around their relevant methods. History consumes one complete logical listing per needed PR. Its page count survives both the qualification cache and history-only read path into cold and per-PR incremental request figures. The shared direct reader rejects an oversized response chunk before appending it. This closes the two R2 findings at their owners.

## Verification and limits

- Exact no-network tests passed: `tests::run_reads_each_input_once_and_reports_costs`, `tests::direct_json_is_bounded_by_the_client_limit`, and `history::tests::history_is_cold_depth_plus_time_eligible_increments` in `bohmian-code-review-pipeline`'s `context_relevance` example. Each selector ran one test.
- The fake proves cold cost 13 page requests, incremental aggregate 50 with PR 40 taking two pages, 50 separate timestamp reads, and no trial-time history read. Oversize direct JSON fails with no report.
- No live credentialed provider run was performed. The fake establishes local logic and request accounting, while real GitHub availability and live data completeness remain unverified in this domain review.

## Material findings

None.

```citations
[]
```
