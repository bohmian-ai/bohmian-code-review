---
id: TASK-001-R2
kind: remediation
status: review
spec: SPEC-context-relevance-eval
spec_revision: 4
requirements: [REQ-002, REQ-003, INV-003, AC-005]
depends_on: [TASK-001, TASK-001-R1]
parent_task: TASK-001
remediates: [FIND-TASK-001-3, FIND-TASK-001-4, FIND-TASK-001-5]
---

# Close TASK-001 input integrity and workflow ownership findings

Approved spec: `changes/active/context-relevance-eval/spec.md` revision 4. Original task: `changes/active/context-relevance-eval/tasks/TASK-001-pinned-sample.md`. Prior remediation: `changes/active/context-relevance-eval/review/TASK-001-r1/TASK-001-R1-listing-completeness-and-rustdoc.md`. Reviewed base: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab`; cumulative candidate: `a1342a5a3c154b7678cf59764dab1ceb0d70cc76`. Implement against the approved behavior and later review the complete base-to-new-candidate range.

## Diagnosis and required correction

**FIND-TASK-001-3 — duplicate paths can masquerade as distinct changed files.** `Listing::push` at `crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs:235` appends all valid provider entries (`self.files.extend(page.files);`) without enforcing canonical-path uniqueness. This is the root-cause owner and the missing input-integrity check. Its sibling effects are `Screen::new` at `sample.rs:300-320`, which copies both entries, and `Screen::qualifies` at `:334-339`, which counts vector entries and considers different indices to be different files. `main.rs:96-108` builds this one listing; `:156-188` screens it; `:197-244` can add it to the sample; `:251-289` prints it. `GitHubClient::pull_request_files` validates entries separately and preserves duplicates. Two individually valid `Modified` entries for `src/a.py`, each with a changed patch, yield `source.len() == 2` and satisfy the other-file condition when the base snapshot declares `src/a.py`. There is only one distinct changed file. This violates REQ-002's 2–15 **files**, REQ-003's other-file condition, and INV-003's fail-on-inconsistent-provider-input rule. Existing tests do not exercise repeated paths.

**Outcome and recommendation:** At the existing `Listing` fold, fail the run when a canonical path repeats anywhere in the one logical PR listing, including within a page or across pages. Treat that entry as unusable through the existing `SampleError::RejectedFile` path, with its path and duplicate reason; do not deduplicate into a seemingly complete listing. Unique paths keep their provider order and present behavior. This one owner check closes both the count and other-file symptoms without changing screening, provider APIs, or trial semantics.

**FIND-TASK-001-4 — timestamp shape is mistaken for a usable instant.** Shared `is_timestamp` at `sample.rs:205-214` verifies the fixed twenty-byte layout and digits but not Gregorian date or 24-hour clock ranges. `Discovery::new` at `:140-149` accepts the cutoff, and `Discovery::offer` at `:160-188` accepts update and merge times through the same helper; `settled` at `:194-201` orders the accepted strings. The cutoff comes from `main.rs:253`; PR times come from decoded GitHub JSON at `main.rs:73-87,216-225`. `2026-02-30T25:61:61Z` passes the current helper even though it is not an instant. An impossible merge time can then enter the sample instead of producing the TASK-001 run failure, violating REQ-002; an impossible cutoff invalidates the fixed-cutoff condition. Current tests only reject malformed shape.

**Outcome and recommendation:** Validate real Gregorian date and UTC clock values once in the existing shared timestamp validator, retaining the exact fixed-width `YYYY-MM-DDTHH:MM:SSZ` representation so lexical comparison remains valid. Apply it to cutoff, update, and merge values through their existing calls; preserve existing error variants and ordering. Standard-library arithmetic is sufficient; do not add a date parser dependency or timezone modes. This closes every timestamp ingress with one correction.

**FIND-TASK-001-5 — stateful IO workflow has no concrete owner.** `main.rs:96-244` defines `listing`, `base_snapshot`, `qualify`, and `select` as free functions. Each performs provider/store IO or coordinates other IO steps, with state passed through argument bundles. `qualify` calls the first two at `:162,:171`; `select` calls `qualify` at `:229`; the command entry point calls `select` at `:264`. `AGENTS.md:27`, `architecture/agent-rules.md:10`, and `architecture/references/languages/rust-core.md:3` reserve free functions for stateless transformations and require cohesive concrete owners for stateful workflows. These four new functions violate that rule; `run` and `main` are command entry points and are outside this finding. Tests do not enforce the rule.

**Outcome and recommendation:** Extend the existing concrete command `Api` owner in `main.rs` to hold the GitHub client and configuration needed by the sampling workflow, and make the four IO operations inherent methods on that owner. Keep `run` and `main` as entry points. Preserve provider requests, one file listing, snapshot limits, screening, errors, and printed output. No new trait, wrapper, public API, or separate orchestration layer is needed. Source inspection, not a new behavioral test, proves this editorial rule; existing behavior checks must stay green.

## Constraints and preserved behavior

Write set: `crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs` and `main.rs`, plus task implementation evidence if recorded. Keep the R1 3,000-entry fail-closed rule and rustdoc closure intact. Keep 50-PR ordering and size rules, missing/malformed patch omissions, base-snapshot completeness failure, source-free output, read-only in-memory use, and existing production provider/storage APIs. Do not add a second listing, `PrContext`, provider replay, trial source blobs, production ranking/packing changes, persisted-format changes, dependencies, or a gate change.

## Acceptance and focused proof

1. **FIND-TASK-001-3:** An in-memory listing with the same canonical path twice fails before qualification, whether repetitions are in one page or separate pages. Equality is by path even if statuses or patches differ. Distinct paths still retain the current qualification behavior. The test must exercise the existing listing fold and sampler failure path, covering `Listing::push`, `Screen::new`, and `Screen::qualifies` consequences together.
2. **FIND-TASK-001-4:** A focused no-network check rejects impossible day, non-leap February 29, month 0/13, and invalid clock fields for cutoff, updated, and merged ingress; accepts a real leap day and normal GitHub UTC times; retains merge-time and PR-number ordering. The shared validator covers all three calls.
3. **FIND-TASK-001-5:** Source inspection shows `listing`, `base_snapshot`, `qualify`, and `select` belong to one cohesive concrete command owner, with their existing call chain intact. `run` and `main` remain entry points. The five current focused sampler tests still pass.
4. Record focused RED and GREEN outcomes using the exact implemented test selectors under `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features`; confirm each selector runs at least one test. Then run `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Preserve the R1 focused cap test. The credentialed live sample and reproducibility comparison remain TASK-003 obligations.

Route this remediation directly to `$cr-implement`.

## Implementation evidence

`Listing::push` in `sample.rs` tracks listed paths and turns any repeated canonical path, within or across pages, into a `RejectedPath` ("path listed twice") that `Screen::new` reports as `SampleError::RejectedFile`; `files` holds distinct paths in provider order. `is_timestamp` keeps the fixed-width shape and now also requires a real Gregorian date and a 00:00:00–23:59:59 UTC clock, serving cutoff, update, and merge ingress. `Api` in `main.rs` now owns the GitHub client and cutoff; `listing`, `base_snapshot`, `qualify`, and `select` are its inherent methods with the same call chain; `run` and `main` remain entry points. The R1 cap fixture now names page-unique paths so it still exercises only the file ceiling.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| 1. FIND-TASK-001-3: repeated path fails before qualification, within one page or across pages, regardless of status/patch; distinct paths still qualify | `Listing::push`, `Listing::paths` in `sample.rs` | `sample::tests::listing_rejects_repeated_paths` | PASS |
| 2. FIND-TASK-001-4: impossible day, non-leap Feb 29 (2026, 1900), month 0/13, hour 24, minute/second 60 rejected for cutoff, update, merge; real leap days accepted; merge-time/PR-number order retained | `is_timestamp` in `sample.rs` | `sample::tests::discovery_rejects_impossible_timestamps` | PASS |
| 3. FIND-TASK-001-5: four IO workflows owned by concrete `Api`; `run`/`main` stay entry points; prior five sampler tests pass | `Api` impl in `main.rs` | Source inspection; `sample::` 7 passed | PASS |
| 4. Focused RED/GREEN, pipeline, format, lint, diff checks; R1 cap test preserved | — | Commands below | PASS |

Focused: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features <selector>`. RED (`sample::`): `listing_rejects_repeated_paths` failed (duplicate within one page accepted) and `discovery_rejects_impossible_timestamps` failed (`2026-02-30T00:00:00Z` accepted as cutoff); 5 passed. GREEN: `sample::tests::listing_rejects_repeated_paths` 1 passed, `sample::tests::discovery_rejects_impossible_timestamps` 1 passed, `sample::tests::listing_fails_at_provider_limits` 1 passed, `sample::` 7 passed. `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `git diff --check`, and `mise run gate` exited 0.

Non-goals held: no deduplication into a complete listing, second listing, `PrContext`, replay, trial blobs, dependency, production API, persisted-format, or gate change; write set is `main.rs`, `sample.rs`, and this file. Live sample and reproducibility remain TASK-003.
