# History and provider domain review

Subject: base `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec`, candidate `20fa48d5cfecf0d4970d325d7334e0aede671728`; approved `changes/active/cochange-eval/spec.md` revision 2; original `tasks/TASK-001-history.md`; remediation `TASK-001-R1-history-contract.md`. Reviewed the complete cumulative diff and the provider path from `Api::select` through discovery, qualification, base time, `History::build`, report construction, and the fake transport. The reviewed source is unchanged.

## Boundary result

| Boundary | Source and authority | Result |
|---|---|---|
| Source eligibility and timestamps | `sample.rs` `Discovery`, `eligible`, `Screen`; spec Sample, History, INV-002/003 | PASS: fixed cutoffs, default-branch filter, ordered sample, exact Zod `.ts`/`.tsx`, validated base committer time, strict trial time. |
| Complete history inputs | `Api::listing`, `history::changed`, `History::build`; REQ-001, INV-002/003, R1 finding 1 | PASS: both cached and history-only listings use up to 30 pages; truncation and rejected paths fail before the mass-edit drop; cold depth and incremental time filter are enforced. |
| Read sharing and complete failure | `Api::select`, `run`, `print`; REQ-001, INV-004/005, R1 finding 2 | PASS: one merged-PR listing sequence, one logical file listing per needed history PR, no history read per trial; errors prevent printing every report. Fake-provider test exercises the full path. |
| Provider request accounting | `History::build`, `record`; REQ-002, TASK-001 Scenario 3, R1 finding 3 | FAIL: file-listing pages are counted as one provider request per PR. See `DOMAIN-PROVIDER-1`. |

## Material proposed finding

### DOMAIN-PROVIDER-1 — logical listings are reported as HTTP request counts

- **Classification and gate:** INCORRECT; named task contract REQ-002 and TASK-001 Scenario 3. **Defect class:** file-listing page requests are not accounted before reporting provider request cost. The shape of `ListingReads` permits only one count per PR, losing the actual number of pages.
- **Root cause and siblings:** `History::build` increments `ListingReads.cold` or `.incremental` once per PR after its read callback; the callback invokes `Api::listing`, which can call `pull_request_files` on several pages. `record` then uses the logical cold count in `production.cold_build` and emits a constant one request per incremental merged PR. The cold and incremental cost fields are siblings of the same unit mismatch. The sample qualification cache changes which calls the evaluation makes, but it does not change the production cost definition.
- **Evidence and reproduction:** The candidate's fake PR 15 has two file-listing pages. The fake observes 50 qualification file pages, two pages for PR 15, and ten more cold-history file pages: 62 file-page HTTP requests in all. Production cold build at depth 10 needs one merged-PR listing page plus 12 cold-history file pages (11 logical listings, including the mass edit): **13 HTTP requests**. The generated fake report asserts `cold_build: 12` (`1 + 11`). The focused check `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features run_reads_each_input_once_and_reports_costs` passed, confirming the incorrect report is the checked behavior. An incremental mass edit requiring multiple pages likewise costs more than the reported `per_merged_pr: 1` and can understate the aggregate. No rate-limit retry is needed for this reproduction.
- **Consequence:** REQ-002 asks for provider requests production would need. The report understates cold-build requests even in its own complete-listing fixture and can understate incremental update requests for paginated PR listings.
- **Testable correction:** Account for actual page requests for every needed history listing at the existing listing/read owner and use those counts in the report's cost fields. Preserve complete-listing validation, the one logical listing per PR rule, cached sample reuse, and the separate evaluation-only timestamp count. A focused fake-provider check should compare the report's cold and incremental request counts with observed page requests for both a paginated cold mass edit and a paginated incremental PR.

## Prior finding closure and limits

R1 findings 1, 2, and 4 are closed on this domain path. R1 finding 3 corrected the incremental cost's *per-PR versus aggregate* label, but the page-request unit defect above remains in its request-accounting class. No live credentialed run was made; provider response semantics were checked through the pipeline client and fake transport. The direct REST reads use one attempt and fail on non-200; an exhausted rate limit therefore fails the run. This review does not assess TASK-002 ranking or report-outcome obligations.

**Overall: FAIL.**

```citations
[{"id":"DOMAIN-PROVIDER-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":203,"snippet":"        for page in 1..=max_pages {","symbols":["listing","pull_request_files"]},{"id":"DOMAIN-PROVIDER-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/history.rs","line":121,"snippet":"                history.reads.cold += 1;","symbols":["History::build","ListingReads"]},{"id":"DOMAIN-PROVIDER-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/history.rs","line":123,"snippet":"                history.reads.incremental += 1;","symbols":["History::build","ListingReads"]},{"id":"DOMAIN-PROVIDER-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":554,"snippet":"                \"cold_build\": requests.listing_pages + requests.history.cold,","symbols":["record","Requests"]},{"id":"DOMAIN-PROVIDER-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":556,"snippet":"                    \"per_merged_pr\": 1,","symbols":["record","Requests"]},{"id":"DOMAIN-PROVIDER-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":830,"snippet":"                    \"cold_build\": 1 + 11,","symbols":["run_reads_each_input_once_and_reports_costs"]}]
```
