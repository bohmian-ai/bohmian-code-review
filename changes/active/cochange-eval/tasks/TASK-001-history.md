---
id: TASK-001
kind: implementation
status: proposed
spec: SPEC-cochange-eval
spec_revision: 2
requirements: [REQ-001, REQ-002, REQ-004, INV-001, INV-002, INV-003, INV-004, INV-005, AC-003, AC-004]
depends_on: []
---

## Outcome and Value

For each fixed repository and cutoff, the opt-in evaluator obtains the fresh 50-PR sample and a reusable, time-eligible co-change history. Provider reads are bounded, shared across trials, and fail completely on missing or incomplete responses. This establishes the historical input and request counts without changing production behavior.

## Owners, Scope, Consumers, and Prohibited Changes

The existing pipeline example's sample and command owners are likely owners; the ranking trial and report owners consume sampled PRs, base commit times, history paths, and request counts. Keep GitHub access in the outer pipeline example and use the existing public path-eligibility query, extractor, verified snapshot path, and provider request policy where applicable. Preserve the existing sample qualification rules with Python, Rust, or TypeScript selected per repository. Do not change production crates, public APIs, persisted formats, gate behavior, grammar pins, or checked-in historical reports. Do not add ancestry lookups or per-trial provider reads.

## Approach

1. Reuse the current discovery, qualification, and snapshot flow with the three fixed repository cutoffs and language-specific eligibility.
2. Obtain each sampled base commit's committer timestamp once and establish the earliest timestamp as the cold build point.
3. Reuse the merged-PR listing to identify historical PRs and read each needed history file listing once through the newest trial base; reject incomplete listings and drop PRs above 30 changed files.
4. Make cold depth 10 and time-bounded incremental history available to the trial owner, together with separately accounted production and evaluation-only provider requests.
5. Keep provider data and source in memory; fail before emitting reports when required reads or rate-limit recovery fail.

## Ordered Implementation Scenarios

### Scenario 1 — Fresh deterministic sample

**Behavior.** Each repository selects the 50 most recently merged qualifying PRs strictly before its cutoff, ordered by merge time then PR number descending; TypeScript `.ts` and `.tsx` eligibility uses the public query and extractor. Too few qualifying PRs or incomplete provider data fails the run. (REQ-001, INV-001, INV-002, AC-003)

**RED.** Add focused fake-provider coverage in the existing example target for fixed cutoffs, ordering, TypeScript eligibility, and a failed/incomplete listing. Run `mise run test:pipeline`; expect the new behavior checks to fail.

**GREEN.** Rework the existing sampler and opt-in command to pass, rerunning the focused checks and previously green sampler checks.

**REFACTOR.** Reuse existing discovery and qualification rules where possible while all scenario checks remain green.

### Scenario 2 — Cold history and incremental eligibility

**Behavior.** One commit read per sampled PR establishes base committer time; the cold point is the earliest base time; the ten most recent non-mass PRs merged before it form cold history. A trial sees later non-mass PRs only when merged strictly before its base time, never itself. (REQ-001, INV-003, AC-003)

**RED.** Add focused no-network history checks for cold depth, timestamp boundaries, later extension, >30-file exclusion, and trial-PR exclusion. Run `mise run test:pipeline`; expect the history assertions to fail.

**GREEN.** Supply the bounded history to trials, then rerun these and Scenario 1 checks.

**REFACTOR.** Remove redundant per-trial preparation or reads while preserving time eligibility and the tests.

### Scenario 3 — Bounded reads and complete failure

**Behavior.** The evaluator lists merged PRs once per repository, reads each required history PR file listing once through the newest base, performs no per-trial provider reads, and separates production cold/incremental/review request counts from evaluation-only timestamp reads. Missing or incomplete responses and unmanageable rate limits fail without a partial report. (REQ-001, REQ-002, INV-002, INV-004, INV-005, AC-003)

**RED.** Add focused fake-provider call-count and failure-path checks in the example target. Run `mise run test:pipeline`; expect the counts or failure behavior to fail.

**GREEN.** Reuse fetched listings, apply the existing provider retry policy where available, and delay report emission until all repositories succeed. Rerun all prior scenario checks.

**REFACTOR.** Keep one clear read path and discard duplicated request accounting while checks remain green.

## Acceptance Criteria

- The three fixed, fresh samples qualify under their language-specific source rules and deterministic ordering.
- Every trial has a time-eligible history derived from the single repository listing and bounded file reads; mass edits cannot influence scores. TASK-002 proves that paths absent from the trial base snapshot contribute nothing.
- Fake-provider tests prove no per-trial reads, accurate cost categories, and complete failure on incomplete data or exhausted rate-limit recovery.
- Production behavior, APIs, persistence, and gate remain unchanged.

## Expected Write Set and Consumer Closure

Likely `examples/context_relevance/{main,sample}.rs`, their inline tests, and the opt-in `mise.toml` task. The trial and report owners must consume the resulting history and counts. Paths guide ownership, not implementation shape.

## Verification and Evidence

Run `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. The example is an existing Cargo test target under the pipeline owner lane. Record fake-provider call counts and failure evidence. Later integrated verification runs `mise run gate`; the live command remains outside it.

## Material Stop Conditions

Stop for spec authority if a provider limitation makes the specified single listing, cold depth, timestamp rule, or complete failure impossible without changing the approved contract; or if satisfying this task requires production API, persisted-format, or gate changes.

## Authority Links

- [Approved revision 2](../spec.md)
- [Contributor rules](../../../../AGENTS.md)
- [Design](../../../../architecture/bohmian-code-review-design.md)
- [Testing workflow](../../../../architecture/references/languages/testing-workflows.md)

## Implementation Evidence

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Three fixed, fresh samples qualify under language-specific rules and deterministic ordering | `main.rs` `REPOSITORIES` (fixed cutoffs; Python, Rust, TypeScript+TSX); `sample.rs` `eligible` via public `SymbolicContext::path_language`; existing `Discovery` order and `Screen` rules | `tests::repositories_are_the_fixed_samples`, `sample::tests::typescript_eligibility_uses_public_query`, existing discovery/listing tests | PASS |
| Time-eligible history from one repository listing and bounded file reads; mass edits excluded | `history.rs` `History::build` (cold depth 10 before earliest base time, incremental through newest base, >30-file drop), `History::for_trial` (strictly before base time, never the trial PR); `Api::select` lists PRs once, reuses qualification listings, reads other history listings one page each | `history::tests::history_is_cold_depth_plus_time_eligible_increments`, `history_excludes_trial_pr` | PASS |
| No per-trial reads, accurate cost categories, complete failure on incomplete data / exhausted rate limits | `for_trial` is pure; `Requests` separates production cold (listing pages + cold listings), incremental listings, review 0, and evaluation-only base-commit reads; reports print only after every repository succeeds; provider errors (including client rate-limit exhaustion) propagate | Counting fake reader: listings read once each, PRs 19..5 only, `ListingReads { cold: 11, incremental: 4 }`; `history_fails_on_short_or_incomplete_input` (short cold history, truncated listing, rejected entry, failing read) | PASS |
| Production behavior, APIs, persistence, and gate unchanged | Diff limited to `examples/context_relevance/{main,sample,history}.rs`, `mise.toml` opt-in task description, this task file | `git diff --stat main` | PASS |

Commands (all exit 0): `mise run test:pipeline` (example target 21 passed), `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`.

Limits: the base-commit committer time uses a direct REST read the production client does not expose, so it has no in-call rate-limit wait and fails the run on any non-success. TypeScript eligibility is the language set {TypeScript, TSX} from the public query, which also admits `.mts`/`.cts`. Non-goals (ranking arms, trial selection, report reshaping) are left to TASK-002/TASK-003.
