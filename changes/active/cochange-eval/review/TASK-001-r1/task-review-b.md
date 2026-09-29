# TASK-001 task implementation review B

Subject: base `a3c9ed5ccfcf8878df8fca21720f64086b3fe5ec`; candidate `f880e9e5b2621166f5240f179f2393f836e259ea`. Scope is the complete base-to-candidate diff and approved spec revision 2. This is a fresh review of source and tests, independent of the implementation summary.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Fixed repositories, cutoffs, and 50 most recent qualifying PRs in deterministic order | `main.rs:63-79,399-435`; `sample.rs:177-211,375-389` | `main.rs:575-591`; existing `sample.rs` discovery and screen tests | PASS |
| Zod source means `.ts` and `.tsx` only | `main.rs:75-77`; `sample.rs:245-249` admits every path routed to TypeScript or TSX, including `.mts` and `.cts` by `routing.rs:49-53` | `sample.rs:542-569` checks `.ts` and `.tsx` but omits `.mts` and `.cts` | FAIL |
| One sampled base committer-time read; earliest time is cold point | `main.rs:246-265,308-316,436-443` | No fake-provider test of the sampled read path; history tests use supplied times | FAIL |
| Cold depth 10, incremental history strictly before each base, mass edits dropped, trial PR excluded | `history.rs:32-47,98-145` | `history.rs:223-263,269-323` checks boundaries, mass edits, exclusion, and failures with a fake listing closure | PASS |
| Single merged-PR listing, reused history file listings, no per-trial provider read | `main.rs:402-449`; `history.rs:140-145` | `history.rs:193-208,223-249` counts closure calls, but no fake-provider test exercises `Api::select` or its cache reuse | FAIL |
| Production cold, incremental and review counts separate from evaluation-only commit reads | `main.rs:120-129,400-451,526-540` | No assertion of `Requests` or `record` categories and totals | FAIL |
| Missing or incomplete responses and exhausted rate limits fail without partial report | `main.rs:160-174,282-292,443-495`; `history.rs:32-47` | Unit tests cover some incomplete listings; no fake-provider failure path through command or exhausted rate limit | FAIL |
| Source-free, read-only, in-memory command; no production behavior, public API, format, gate, or historical-report change | Diff changes example files, task evidence, and opt-in `mise.toml` description only | `git diff --check` passes; no live command run | PASS |
| Ranking arms, trial selection, checked-in experiment reports, and production adoption remain for later tasks | No ranking or production implementation in diff | N/A for this task | PASS |

## Proposed findings

### TASK-B-1 — INCORRECT: language enum admits extra Zod extensions

- **Root cause / defect class:** `sample::eligible` treats a language enum as the complete repository-specific suffix policy. Its shape cannot distinguish `.ts` from `.mts` or `.cts`. The Zod configuration supplies `Language::Typescript`, which the existing extractor returns for all three suffixes.
- **Sibling locations:** `main.rs:75-77` supplies the Zod set; `main.rs:422-423` applies it to every PR; `sample.rs:245-249` is the shared eligibility owner; `sample.rs:542-569` lacks an exclusion check. No other repository's configured language has this particular mismatch.
- **Violated obligation:** Approved spec Sample table limits Zod source to `.ts` and `.tsx`; TASK-001 Scenario 1 and acceptance require the language-specific sample rules.
- **Evidence / consequence:** A PR with `a.mts` and `b.ts` can qualify when it has only one in-scope source file, changing which 50 PRs are sampled and therefore the experiment's ranks. `.cts` behaves the same. `routing.rs:49-53` maps both to `Language::Typescript`.
- **Testable correction:** At the shared sample eligibility boundary, require the exact Zod suffix set while still using the public path eligibility query and extractor; add one test with `.ts`, `.tsx`, `.mts`, and `.cts` that fails if either extra suffix enters `Screen::source` or qualification.

### TASK-B-2 — MISSING: fake-provider proof stops before request orchestration

- **Root cause / defect class:** The new checks exercise `History::build` with a fake closure but never drive `Api::select` or command failure through a fake provider. This leaves the actual listing cache, sampled commit reads, request categories, and no-partial-output guarantee unproved. The owner is the example's command/test boundary; this is a required proof gap rather than evidence of a demonstrated runtime bug.
- **Sibling locations:** `main.rs:308-310` sampled commit read; `main.rs:402-451` discovery, cache reuse, and request accounting; `main.rs:479-495` report emission; `history.rs:193-208` fake closure ends before those paths; `main.rs:569-593` contains only fixed-constant checks.
- **Violated obligation:** TASK-001 Scenario 3 RED and acceptance criteria require fake-provider call counts, accurate cost categories, and complete failure on incomplete data or exhausted rate-limit recovery; approved AC-003 requires no-network tests with a fake provider for the task's history path.
- **Evidence / consequence:** The existing fake counts only `History::build` closure calls. A later edit could duplicate a GitHub file read in `qualify`, misassign `Requests` counters, or emit a partial report and the current new tests would still pass. The implementation's stated call-count evidence is therefore not a check of the provider calls it claims.
- **Testable correction:** Add the smallest no-network command-level seam and fake provider check that drives selection through one repository, asserts each required file listing and base commit read count plus production/evaluation cost categories, and injects an incomplete response and exhausted rate limit to assert no report is emitted. Reuse the existing fake provider/testing support rather than adding a new test framework.

## Overall

**FAIL.** The source rules admit out-of-scope Zod files, and the task's specified fake-provider request/failure proof is absent. Review was static; no live GitHub run was available or needed to establish these findings.

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":248,"snippet":"        .is_ok_and(|language| language.is_some_and(|language| languages.contains(&language)))","symbols":["eligible"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":77,"snippet":"        languages: &[Language::Typescript, Language::Tsx],","symbols":["REPOSITORIES"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-extract/src/routing.rs","line":52,"snippet":"    (\".mts\", Language::Typescript),","symbols":["EXTENSION_MAP"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-extract/src/routing.rs","line":53,"snippet":"    (\".cts\", Language::Typescript),","symbols":["EXTENSION_MAP"]},{"id":"TASK-B-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/history.rs","line":202,"snippet":"        let history = History::build(&prs, cold_point, newest_base, async |number| {","symbols":["build"]},{"id":"TASK-B-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":388,"snippet":"    async fn select(&self, repo: &Repository) -> Result<Selected, Box<dyn Error>> {","symbols":["select"]},{"id":"TASK-B-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":574,"snippet":"    #[test]","symbols":["repositories_are_the_fixed_samples"]}]
```
