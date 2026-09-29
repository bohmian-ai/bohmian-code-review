# TASK-001 Wave 2 findings validation

Subject: `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec..f880e9e5b2621166f5240f179f2393f836e259ea`; approved `changes/active/cochange-eval/spec.md` revision 2; original `tasks/TASK-001-history.md`. The citation check passed. This validation used the complete candidate diff and source, not the Wave 1 reports. There is no prior `FIND-*` ledger.

## Blocking ledger

### FIND-TASK-001-1 — incomplete history listings are accepted

- **Sources/status/gate:** `TASK-A-1`, `DOMAIN-HISTORY-1`; **CONFIRMED**; task contract (REQ-001, INV-002, Scenario 3) and provider completeness boundary.
- **Classification/root cause/defect class:** INCORRECT; `history::changed` classifies mass edits before testing whether the listing is complete or contains rejected entries. The `Changed = Option<Vec<RelativePath>>` result permits incomplete and rejected listings to become the same `None` as a valid mass edit.
- **Owner and siblings:** `history.rs:32-43` is the producing decision. `main.rs:443-449` supplies it both cached qualification listings and history-only listings. The latter are read with `max_pages = 1` at `main.rs:447`; `Api::listing` at `main.rs:182-198` sets `Listing::truncated` through `Listing::push` at `sample.rs:273-287`. `Api::qualify` at `main.rs:275-317` instead screens its full listing after classification and rejects a truncated/rejected listing at `sample.rs:332-342`; thus the reachable false success is the history-only path. `History::build` at `history.rs:98-137` trusts the closure result.
- **Reproduction:** The candidate's own `history.rs:292-298` constructs 30 accepted entries, one rejected entry, and `truncated = true`, then asserts `changed(7, &mass) == Ok(None)`. A history-only PR with 100 files and a next page is similarly marked truncated by the one-page read, then accepted as `None` because its first page exceeds 30 files. `Api::select` reaches this for any needed history PR whose listing was not cached during qualification.
- **Consequence:** A run can succeed and emit a report even though a required provider file listing is incomplete or rejected. Mass-edit exclusion may be inferable from the first page, but the approved contract explicitly requires incomplete provider responses to fail.
- **Correction:** Keep completeness and rejected-entry validation at the existing listing/classification boundary before mass-edit classification. Fetch the complete needed history listing using the existing bounded `Api::listing` path, including mass edits; do not silently turn an incomplete listing into `None`. Preserve the existing >30 mass-edit drop and the one-read-per-history-PR property. **Closure proof:** a focused fake-provider path through `Api::select` must make a history-only PR with `has_next` after page one either finish its pages or fail; it must fail on an incomplete ceiling or rejected entry even when the count exceeds 30, and still drop a complete 31-file listing.

### FIND-TASK-001-2 — fake-provider orchestration proof is missing

- **Sources/status/gate:** `TASK-A-2`, `TASK-B-2`, `DOMAIN-HISTORY-2`; **CONFIRMED**; task contract (AC-003 and TASK-001 acceptance criterion 3).
- **Classification/root cause/defect class:** MISSING; no provider seam in the candidate example makes `Api::select` and `run` executable with a fake provider. The added history tests stop at `History::build`'s synthetic closure; no test exercises the actual request orchestration or complete failure behavior.
- **Owner and siblings:** `Api` owns a concrete `reqwest::Client` and concrete `GitHubClient` at `main.rs:145-152`. Its direct `get` at `main.rs:160-173`, provider listing at `main.rs:182-198`, selection at `main.rs:388-458`, and command loop/report output at `main.rs:467-495` are the path requiring proof. Existing `history.rs:202-321` tests only fake the `History::build` reader; `main.rs:574-589` checks only constants. No other candidate test exercises `select`/`run` with a fake provider.
- **Reproduction:** From `run`, each fixed repository calls `api.select`, then trials and report construction. `select` calls live `Api::get`, `GitHubClient::pull_request_files`, `ColdBuild`, and base-commit reads. The test module creates no `Api` or fake transport. The `mise run test:pipeline` lane can therefore be green while request counts, reuse, and failed-response/no-partial-report behavior on this real path remain unproved.
- **Consequence:** AC-003's required no-network fake-provider proof is absent; the reported qualification/history counts and all-repositories-before-output behavior are not verified at their actual owner.
- **Correction:** Add the smallest no-network fake-provider seam that lets the existing example's selection and run orchestration execute with controlled responses. Reuse the pipeline's existing provider/testing behavior where possible; avoid an unrelated generic provider framework. **Closure proof:** one focused orchestration test must count listing, file, and base-commit requests across the 50-PR sample and history, show no per-trial history reads, and make incomplete/missing/rate-limit responses fail with no report output. Keep the pure `History::build` checks for time eligibility. This proof must cover the production-facing request path, not another synthetic `History::build` callback.

### FIND-TASK-001-3 — incremental production cost uses cumulative units

- **Sources/status/gate:** `TASK-A-3`; **CONFIRMED**; task contract (REQ-002 and TASK-001 Scenario 3).
- **Classification/root cause/defect class:** INCORRECT; `record` labels the total number of incremental history listings in one evaluation as the request cost of one production incremental update.
- **Owner and siblings:** `History::build` increments `ListingReads.incremental` for every eligible PR (`history.rs:118-123`), and `main.rs:451` copies that aggregate into `Requests`. `record` at `main.rs:526-540` emits it as singular `production.incremental_update`; this is the only report producer for that field. `run` calls `record` once per repository at `main.rs:490`.
- **Reproduction:** The existing history test at `history.rs:238-246` proves `ListingReads { cold: 11, incremental: 4 }` for four incremental reads. The report would emit `incremental_update: 4`, while REQ-002 fixes production incremental cost at one file listing **per merged PR**.
- **Consequence:** The report overstates a single update when multiple PRs merge between cold build and newest trial base, and fails to express the approved per-PR cost.
- **Correction:** In `record`, report one file-listing request per merged PR as the production incremental-update unit. Retain the aggregate evaluation read count only under a clearly separate total if needed for request accounting; preserve cold-build pages plus cold history listings and review-time zero. **Closure proof:** a focused report check with multiple incremental PRs asserts the production per-update unit remains one and any aggregate is labeled as an aggregate.

### FIND-TASK-001-4 — Zod sample includes unapproved extensions

- **Sources/status/gate:** `TASK-B-1`; **CONFIRMED**; task contract (spec Sample and TASK-001 Scenario 1).
- **Classification/root cause/defect class:** INCORRECT; `eligible` treats a language enum as an exact filename-extension set. The public router maps `.mts` and `.cts` to `Language::Typescript` (`crates/bohmian-code-review-extract/src/routing.rs:49-53`), so the Zod language set at `main.rs:75-78` includes those extensions as well as `.ts`/`.tsx`.
- **Owner and siblings:** `sample.rs:243-249` produces the eligibility decision. `main.rs:421-423` passes it to `Api::qualify`; `Screen::new` at `sample.rs:332-366` includes the accepted paths, and `Screen::qualifies` at `sample.rs:378-385` uses them to admit sampled PRs. Trial inputs at `main.rs:342-375` consume the same source list. The test at `sample.rs:546-571` does not include `.mts` or `.cts`.
- **Reproduction:** `path_language("src/a.mts")` and `path_language("src/a.cts")` route to `Typescript` by the frozen extension map, then `languages.contains(&language)` returns true for Zod. Such paths count toward the 2–15 source-file threshold and may become withheld or remaining changed files.
- **Consequence:** The selected Zod sample and trials can differ from the approved `.ts`/`.tsx` sample, changing the experiment's input.
- **Correction:** At the existing `eligible` owner, require the spec's exact `.ts` or `.tsx` extension for the TypeScript repository while retaining the public path-eligibility query and extractor. Preserve Python and Rust eligibility behavior. **Closure proof:** a focused screen/eligibility test accepts `.ts` and `.tsx`, rejects `.mts` and `.cts`, and shows those rejected paths do not affect PR qualification.

## Notes and rejected claims

None. The four retained findings have separate root causes; duplicate Wave 1 IDs are merged above. No prior finding IDs are reopened. All corrections stay within candidate-owned example code and the approved contract; no spec revision is needed.

## Validation limit

The reproductions above are traced source paths and existing focused assertions. I did not run a new live provider command or add a test, because this is a read-only validation of the immutable candidate. Existing green lane evidence does not establish the missing orchestration proof described in FIND-TASK-001-2.
