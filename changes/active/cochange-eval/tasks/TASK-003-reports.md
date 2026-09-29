---
id: TASK-003
kind: implementation
status: proposed
spec: SPEC-cochange-eval
spec_revision: 2
requirements: [REQ-002, REQ-003, REQ-004, INV-001, INV-002, INV-004, INV-005, AC-001, AC-002, AC-003, AC-004]
depends_on: [TASK-001, TASK-002]
---

## Outcome and Value

The opt-in command emits one complete source-free JSON report for each repository, with equal-PR paired hit@10 effects, reproducible bootstrap intervals, production request costs, and the predeclared adoption decision. Two live runs yield checked-in reports that match apart from timings.

## Owners, Scope, Consumers, and Prohibited Changes

The existing pipeline example's report and command owners and its `mise` invocation are likely owners; the checked-in cochange report files are evidence consumers. Use TASK-001's request counts and TASK-002's paired trials. Keep the command credentialed, read-only toward GitHub, explicitly invoked outside `gate` and normal CI. Emit reports to stdout for capture as checked-in artifacts, with no source text or local checkout. Do not change production ranking, packing, APIs, snapshot format, or historical evaluation reports. Do not add other outcomes, subsets, sensitivity analyses, or tuning.

## Approach

1. Aggregate hit@10 within each PR for C, E, and F, then calculate paired E − C and F − C mean differences across equally weighted PRs.
2. Bootstrap PRs 10,000 times with one fixed recorded seed and report percentile 95% intervals and the decision for each repository under the predeclared threshold.
3. Serialize the required sample, trial, history-size, cost, rerun, and validity fields without source text; finish all repository work before emitting any report.
4. Run the explicit credentialed command twice with the fixed cutoffs, compare the normalized reports, and check in one report per repository.
5. Verify the integrated example, repository gate, and unchanged production boundaries.

## Ordered Implementation Scenarios

### Scenario 1 — PR-weighted paired inference

**Behavior.** Each PR has an arm mean across its own trials and contributes one paired difference. A fixed-seed 10,000-resample PR bootstrap yields the 95% percentile interval for E − C and F − C. The decision adopts F only when every repository has at least a five-point mean improvement and an interval lower bound above zero; an interval containing zero is inconclusive. (REQ-003, INV-002, AC-003)

**RED.** Add focused report-owner checks with unequal trial counts per PR and fixed-seed expected inference and decision outcomes, including a zero-containing interval. Run `mise run test:pipeline`; expect the new inference checks to fail.

**GREEN.** Implement the PR-level summaries, bootstrap, and decision, rerunning all focused checks.

**REFACTOR.** Remove legacy all-trial and non-test aggregates, keeping the outcome limited to hit@10.

### Scenario 2 — Complete source-free reports and cost

**Behavior.** Each report includes the exact cutoff, cold point, seed, rerun command, base/head commits, selected withheld paths, trial ranks/hits, per-PR history size and means, both effects and intervals, decision outcome, separate production cold/incremental/review and evaluation timestamp costs, and all four named validity threats. No source or packing outcome appears. Incomplete provider data or a rate-limit failure produces no partial report. (REQ-002, REQ-003, REQ-004, INV-004, INV-005, AC-001, AC-003)

**RED.** Add focused fake-provider report checks in the existing example target for required fields, source-free content, request counts, and failure before output. Run `mise run test:pipeline`; expect the new report checks to fail.

**GREEN.** Compose complete reports from the prior task outputs, then rerun inference and report checks.

**REFACTOR.** Keep serialization and invocation small, reusing existing report and command surfaces.

## Proof Strategy for Live Artifacts and Boundaries

TDD does not apply to capturing already-computed live reports or inspecting static command/gate boundaries. Run the credentialed opt-in command twice with the same fixed cutoffs; compare all sample, trial, rank, effect, cost, and decision fields byte-for-byte after excluding timing fields. Check in one JSON report per repository. Inspect `mise.toml` and the final diff to prove the command remains outside `gate` and production crates and historical reports remain untouched. A missing credential or unavailable provider is a material execution blocker, never a fabricated report.

## Acceptance Criteria

- Reports for pydantic-ai, tokio, and zod contain every REQ-003 field, separate REQ-002 costs, all validity threats, and no source text or packing claim.
- Equal-PR paired effects, seeded intervals, and the repository-wide decision rule pass no-network tests.
- Two live runs with the same cutoffs produce equal reports apart from timings; all three final reports are checked in.
- The command remains opt-in and read-only, and production crates, persisted formats, public APIs, historical reports, and gate behavior remain unchanged.

## Expected Write Set and Consumer Closure

Likely `examples/context_relevance/{report,main}.rs`, their inline tests, the opt-in `mise.toml` task, and three new JSON reports under a cochange-eval report directory. The existing sample and trial records are consumers/producers whose report-facing fields may need alignment. Paths guide ownership, not implementation shape.

## Verification and Evidence

Run `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check`. Run the exact rerun command recorded in each report twice with `GITHUB_TOKEN` and fixed cutoffs, then compare timing-normalized JSON; retain the comparison result with task evidence. Inspect tracked and untracked diff for source leakage and prohibited changes. The live command is intentionally outside `gate`.

## Material Stop Conditions

Stop for spec authority if valid reports require another outcome, altered threshold, changed production behavior/API/persistence, or a changed definition of historical eligibility. A missing token, irrecoverable provider limit, or inability to obtain the required three samples blocks live artifact completion; do not substitute synthetic reports.

## Authority Links

- [Approved revision 2](../spec.md)
- [Contributor rules](../../../../AGENTS.md)
- [Design](../../../../architecture/bohmian-code-review-design.md)
- [Testing workflow](../../../../architecture/references/languages/testing-workflows.md)

## Implementation Evidence

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Reports for all three repositories contain every REQ-003 field, separate REQ-002 costs, all validity threats, no source or packing claim | `examples/context_relevance/main.rs` `record`/`run`; `reports/{pydantic-ai,tokio,zod}.json` from live run 1 | `run_report_is_complete_and_source_free`, `run_reads_each_input_once_and_reports_costs`; live run 1 exit 0 | PASS |
| Equal-PR paired effects, seeded intervals, repository-wide decision pass no-network tests | `examples/context_relevance/report.rs` `pr_means`, `effect`, `verdict`, `adopt` | `prs_weigh_equally_whatever_their_trial_counts`, `bootstrap_is_fixed_by_its_seed`, `a_zero_containing_interval_is_inconclusive`, `adoption_needs_every_repository_to_meet_the_rule` | PASS |
| Two live runs with the same cutoffs produce equal reports apart from timings | reports carry no run-timing fields, so comparison is byte-for-byte | run 1: exit 0, 267,627 bytes, sha256 `f3a33e69…da7a3`; run 2: failed `rate limited; retry after 1256s` with no report (INV-005 behaving as specified) | PENDING — rerun in a fresh rate-limit window and `cmp` against the checked-in reports |
| Command opt-in and read-only; production crates, formats, APIs, historical reports, gate unchanged | diff limited to the example target, three new reports, and this task | `mise.toml` unchanged, `live:context-relevance` not in `gate`; `git diff 61d6df6 HEAD --stat` lists only `examples/context_relevance/{main,report}.rs`, `changes/active/cochange-eval/reports/*.json`, and this task | PASS |

Commands this session: `mise run test:pipeline` (exit 0), `mise run fmt:check` (exit 0), `mise run lints` (exit 0), `mise run gate` (exit 0), `git diff --check` (exit 0), `GITHUB_TOKEN=… mise run live:context-relevance` run 1 (exit 0, 35 min), run 2 (exit 1, rate limited).

Run 1 results (F − C mean, 95% CI, verdict): pydantic-ai 0.141 [0.045, 0.242] met; tokio 0.175 [0.095, 0.267] met; zod 0.323 [0.228, 0.420] met; `adopt_f: true`.

Non-goals held: no other outcome, subset, sensitivity analysis, tuning, or production change. The legacy all-trial `summarize` aggregate was removed.
