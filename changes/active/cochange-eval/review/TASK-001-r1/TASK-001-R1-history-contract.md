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
