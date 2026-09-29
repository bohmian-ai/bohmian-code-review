# History and provider domain review

Subject: base `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec`, candidate `f880e9e5b2621166f5240f179f2393f836e259ea`.

## Reviewed boundary

I traced merged-PR discovery, sample qualification, base-commit timestamp reads, history file-listing reads and reuse, mass-edit classification, time filtering, provider failure propagation, request accounting, and report emission. This covers `examples/context_relevance/{main,sample,history}.rs` and the pipeline client's PR-file pagination and retry policy. I reviewed the full base-to-candidate diff and its callers; no production crate is changed.

## Authority and source coverage

| Boundary | Authority | Source and result |
|---|---|---|
| Fixed samples, complete provider reads, bounded shared history | `spec.md` REQ-001, INV-002; `TASK-001-history.md` scenarios 1–3 | `main.rs` `Api::select`, `qualify`, `listing`; `sample.rs` `Discovery`, `Listing`, `Screen`; `history.rs` `History::build`, `changed`. Incomplete mass-edit listings fail the contract as below. |
| Cold depth and time eligibility | `spec.md` History, INV-003 | `main.rs` `base_time`, min/max sampled base times; `history.rs` `History::build` and `for_trial`. Strict timestamp comparisons and explicit trial-PR exclusion pass in source. |
| Rate limits and no partial report | `spec.md` INV-005, REQ-001 | Pipeline `GitHubClient::pull_request_files` applies its retry policy; direct REST `Api::get` propagates non-success; `run` buffers all reports before printing. Pass in source for run failure. |
| Cost and fake-provider proof | `spec.md` REQ-002, AC-003; `TASK-001-history.md` scenario 3 | `main.rs` `Requests` and `record`; `history.rs` counting closure tests. Provider-level call-count and failure tests are absent as below. |

## Verification limits

The task packet records successful `mise run test:pipeline`, `fmt:check`, `lints`, `gate`, and `git diff --check`. I inspected their named test source but did not repeat the full gate. The tests use in-memory `Listing` values and a counting closure into `History::build`; they do not drive `Api::select` through a fake provider. No live credentialed run is available here. Direct REST calls use `reqwest` rather than the production client's retry policy, but non-success fails the run, which satisfies the specified unmanageable-rate-limit outcome.

## Material proposed findings

### DOMAIN-HISTORY-1 — Incomplete mass-edit listings are accepted

- **Classification:** INCORRECT; task contract violation.
- **Defect class and root cause:** History classification returns the mass-edit outcome before checking whether the provider listing is complete or contains rejected entries. `changed` is the owner of this decision; its early return permits the defect. Sibling paths are qualification's call to `changed` and history's one-page call to `changed`, both using the same owner.
- **Obligation:** REQ-001 and INV-002 require a missing or incomplete provider response to fail the run; TASK-001 scenario 3 requires complete failure on incomplete data.
- **Evidence:** `Api::select` asks for only page 1 of previously unread history PRs and passes that `Listing` to `changed`. `Listing::push` sets `truncated` when that first page advertises a next page. A first page with 31–100 files and `has_next = true` is then returned as `Ok(None)` before `truncated` is checked. The same early return discards a rejected entry when accepted plus rejected entries exceed 30; the existing test explicitly asserts this behavior.
- **Observable consequence:** A history PR with an incomplete or rejected provider listing is silently dropped as a mass edit, and the run can emit reports despite required provider data being incomplete. The count and identity of history PRs are then accepted without the promised complete-read boundary.
- **Testable correction:** Check and propagate rejected or incomplete listings before accepting mass-edit classification; read subsequent pages when necessary to prove a complete listing. Add a fake-provider case with 31 entries, `has_next = true`, and a case with a rejected entry above the mass-edit threshold; both must fail the run without a report.

### DOMAIN-HISTORY-2 — Provider-level call and failure contract lacks its required fake-provider proof

- **Classification:** MISSING verification of an explicit acceptance criterion.
- **Defect class and root cause:** The tests stop at `History::build`'s closure and never exercise `Api::select` or `run` with a fake provider. The concrete `Api` shape is what prevents direct injection of the REST and PR-file readers into those tests.
- **Obligation:** TASK-001 scenario 3 and acceptance criteria require fake-provider tests proving no per-trial reads, accurate cost categories, and complete failure on incomplete data or exhausted rate-limit recovery; spec AC-003 requires no-network history proof.
- **Evidence:** `main.rs`'s only command test checks fixed repository metadata. `history.rs`'s counting fake records calls to its closure, not actual provider file-listing, commit, or merged-PR listing calls. The request totals are calculated in `Api::select` and `record`, outside that fake's reach.
- **Observable consequence:** A later change can add duplicate file, timestamp, or per-trial requests or emit partial reports without any focused fake-provider test failing; the named task acceptance remains unproved.
- **Testable correction:** Add one no-network fake-provider command-path test that counts listing pages, PR-file and base-commit reads, verifies cold/incremental/review/evaluation counts, and injects incomplete and exhausted-rate-limit responses to assert no report emission. Keep test support local to the example.

## Overall

**FAIL.** History time comparison, cold-depth selection, cached reuse, and buffered output follow the written rule in source, but the incomplete-listing acceptance and explicit fake-provider evidence gap block this task.

```citations
[{"id":"DOMAIN-HISTORY-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/history.rs","line":33,"snippet":"    if listing.files.len() + listing.rejected.len() > MASS_EDIT_FILES {","symbols":["changed"]},{"id":"DOMAIN-HISTORY-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":447,"snippet":"            let listing = self.listing(repository, number, 1).await?;","symbols":["select","listing"]},{"id":"DOMAIN-HISTORY-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":285,"snippet":"        self.truncated =","symbols":["push"]},{"id":"DOMAIN-HISTORY-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":575,"snippet":"    fn repositories_are_the_fixed_samples() {","symbols":["repositories_are_the_fixed_samples"]},{"id":"DOMAIN-HISTORY-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/history.rs","line":202,"snippet":"        let history = History::build(&prs, cold_point, newest_base, async |number| {","symbols":["build"]}]
```
