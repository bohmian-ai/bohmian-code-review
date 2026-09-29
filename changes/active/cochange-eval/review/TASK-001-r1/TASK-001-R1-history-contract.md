---
id: TASK-001-R1
kind: remediation
status: proposed
spec: SPEC-cochange-eval
spec_revision: 2
parent_task: TASK-001
remediates: [FIND-TASK-001-1, FIND-TASK-001-2, FIND-TASK-001-3, FIND-TASK-001-4]
---

# Close TASK-001 history and provider-contract findings

Base `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec`; reviewed candidate `f880e9e5b2621166f5240f179f2393f836e259ea`. Approved authority: `changes/active/cochange-eval/spec.md` revision 2. Original task: `changes/active/cochange-eval/tasks/TASK-001-history.md`. The cumulative candidate, rather than only this remediation diff, must pass the next review.

## Diagnosis and correction

1. **`FIND-TASK-001-1` — incomplete mass-edit listings succeed.** `history::changed` at `history.rs:32-43` returns `None` when the visible count exceeds 30 before examining `Listing::truncated` or `rejected`. `Api::select` at `main.rs:443-449` supplies both cached qualification listings and history-only listings; the latter use only one page. `Listing::push` at `sample.rs:273-287` marks a page with `has_next` as truncated. The candidate test at `history.rs:292-298` expressly accepts a rejected, truncated mass edit. Thus an incomplete history-only listing can be silently dropped and a report emitted, violating REQ-001, INV-002, and Scenario 3. Validate a needed listing before deciding whether it is a mass edit, and use the existing bounded listing reader to obtain all pages needed for completeness. Keep the >30-file drop for complete listings. Proof must cover a history-only next-page listing, an unresolved page ceiling, rejected entries above the mass threshold, and a complete 31-file mass edit. Both cached and history-only inputs must follow the same rule.

2. **`FIND-TASK-001-2` — the required provider proof is absent.** The tests at `history.rs:202-321` count calls to a synthetic `History::build` closure, while actual listing reuse, base-commit reads, request totals, and output buffering live in `Api::get`, `listing`, `base_time`, `select`, and `run` at `main.rs:145-495`. The only `main.rs` test checks fixed constants. A duplicate real provider read or partial report could pass every present test, so AC-003 and TASK-001's fake-provider acceptance remain unproved. Make the existing example's provider read path controllable by a no-network fake, reusing pipeline test support where practical and keeping the seam local to this command. Exercise the selection and output path, counting merged-PR listing pages, each needed PR-file listing, each sampled base-commit read, and no history provider reads per trial. Inject missing/incomplete responses and exhausted rate-limit recovery and prove failure emits no report. Preserve the command's real GitHub path and request policy.

3. **`FIND-TASK-001-3` — incremental cost uses cumulative units.** `History::build` counts every post-cold history listing (`history.rs:118-123`); `record` emits that total as `production.incremental_update` (`main.rs:526-540`). The existing test observes four such reads, although REQ-002 specifies one file listing **per merged PR** for an incremental production update. Report that per-PR unit explicitly; if the aggregate run count is retained, label it separately as an aggregate. Preserve cold-build listing-page plus cold-history cost, review-time zero, and separate evaluation-only timestamp reads. A report test with multiple incremental PRs must distinguish the single-update unit from the run total.

4. **`FIND-TASK-001-4` — Zod source eligibility is too broad.** `eligible` at `sample.rs:243-249` accepts the configured `Language::Typescript`; `main.rs:75-78` configures it for Zod, while the public router maps `.mts` and `.cts` to that language (`routing.rs:49-53`). Those paths can affect the 2–15 file qualification, sample, and trials despite the spec's `.ts`/`.tsx` set. At the shared sample eligibility owner, enforce Zod's exact suffix set while retaining the public path query and extractor. Preserve Python and Rust eligibility. A focused qualification test must accept `.ts` and `.tsx`, reject `.mts` and `.cts`, and show rejected paths do not change qualification.

## Scope and proof

Expected write set: `crates/bohmian-code-review-pipeline/examples/context_relevance/{main,sample,history}.rs` and their local tests; include only other existing test-support files the actual provider seam needs. Keep production crates, public APIs, persisted formats, grammar pins, checked-in historical reports, and `gate` unchanged. Do not add ranking arms, packing claims, production co-change storage, ancestry lookups, per-trial provider reads, or a generic provider framework.

The four focused proofs above are acceptance criteria for their respective finding and every sibling path. Run exact focused new test selectors, `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Run `mise run gate` for the cumulative candidate before the next immutable review. Route this task directly to `$cr-implement`.

## Implementation Evidence

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| `FIND-TASK-001-1`: completeness validated before the mass-edit drop, same rule for cached and history-only listings | `history.rs` `changed` checks `truncated`, then `rejected`, then >30 files; `main.rs` `Api::select` reads history-only listings through `MAX_FILE_PAGES` like qualification | `history::tests::history_fails_on_short_or_incomplete_input` (rejected entry above threshold fails, unresolved page ceiling fails, complete 31-file listing dropped, 30 kept); `tests::run_reads_each_input_once_and_reports_costs` (history-only PR 15 read over pages 1–2 and dropped); `tests::run_fails_without_report_on_missing_incomplete_or_rate_limited_reads` (history-only PR 14 at the 30-page ceiling fails with `IncompleteListing { number: 14 }`) | PASS |
| `FIND-TASK-001-2`: real provider path proved over a no-network fake | `main.rs` `Api.direct: Box<dyn HttpTransport>` (live: `ReqwestTransport`), `live`, `run(api, repos) -> Vec<Value>`, `print`; tests reuse pipeline `testing::RoutedTransport` and `GitHubClient::with_transport` | `run_reads_each_input_once_and_reports_costs`: direct reads = 1 repo + 1 merged-PR page + 50 base commits in sample order; PR-file reads = PRs 65..16 page 1, PR 15 pages 1–2, PRs 14..5 page 1, nothing per trial (100 trials run); failure test: missing base commit (404), incomplete listing, and 429 beyond `max_retry_wait` (`GitHubError::RateLimited`) each return `Err` and no reports | PASS |
| `FIND-TASK-001-3`: incremental cost reported per merged PR, aggregate labelled separately | `main.rs` `record`: `incremental_update: { per_merged_pr: 1, aggregate_through_newest_base }` | Report assertion: `cold_build` 12 (1 page + 11 cold listings), `per_merged_pr` 1, aggregate 49, `review` 0, evaluation-only timestamps 50 | PASS |
| `FIND-TASK-001-4`: Zod source limited to `.ts`/`.tsx` | `sample.rs` `eligible` takes an optional suffix set with the public path query; `main.rs` Zod `suffixes: Some(&[".ts", ".tsx"])`, Python/Rust `None` | `sample::tests::typescript_eligibility_uses_public_query`: `.ts`/`.tsx` kept, `.mts`/`.cts` dropped; `.ts`+`.mts` is not plausible; 15 `.ts`+`.cts` stays plausible | PASS |

Commands (all exit 0 in this session): `cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features run_` (2 passed), `mise run test:pipeline` (example target 23 passed), `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`. Mutation checks: reading history-only listings one page, or reporting `per_merged_pr` 2, fails the new tests.

Scope: only `examples/context_relevance/{main,history,sample}.rs` and this file changed. No production crate, public API, persisted format, grammar pin, historical report, `mise.toml`, or gate change; no ranking arms, packing, co-change store, ancestry lookups, or per-trial reads. Direct REST reads keep their single-attempt, fail-on-non-200 policy, now with the client's default 60 s request timeout. No live credentialed run.
