---
id: SPEC-cochange-eval
revision: 1
status: approved
---

# Co-change ranking and extractor-gap evaluation

Drafted 2026-09-29. Revision 1 approved by the maintainer 2026-09-29. Builds on the [completed context-relevance evaluation](../../completed/2026/context-relevance-eval.md) and its [reports](../../completed/2026/context-relevance-eval-reports/).

## Objective and user value

The context-relevance evaluation showed that symbolic name matching cannot reach many related files: under variant C, 24% of Python and 64% of Rust withheld files share no name with the rest of their PR. This change measures two things before any production decision:

1. whether **co-change history** (files that changed together in recent merged PRs) finds related files that names cannot, when that history is built once at cold build, extended incrementally as PRs merge, and read at review time with no provider calls; and
2. how much of the Rust gap is an **extractor gap**: names inside item-wrapping macro bodies (for example `cfg_rt! { … }`) that `rustlang.rs` does not extract, compared with files that are lexically unrelated.

The owner has already decided that C becomes the production default in a separate change and that D is not adopted. This change neither revisits nor implements that.

## Definitions

- **Harness, sample, trial, candidate universe, A–D, token accounting, packing, and test-path split** keep their meaning from the completed evaluation. The 50-PR samples per repository and the cutoff `2026-09-01T00:00:00Z` are reused unchanged, so every new number compares directly with A–D on the same trials.
- **History PR**: a PR merged into the default branch, represented by its changed file paths. A **mass edit** is a history PR with more than 30 changed files; it is dropped from history.
- **Cold build point**: the base commit of the oldest PR in the repository's sample. The evaluation simulates one cold build there.
- **Cold-build history (depth N)**: the N most recent non-mass-edit PRs merged before the cold build point.
- **Trial history (depth N)**: cold-build history plus every non-mass-edit PR merged after the cold build point and before the trial PR's base commit, which simulates incremental updates. A PR counts as merged before a commit when its merge commit is an ancestor of that commit. The trial PR is never in its own history (leakage guard).
- **Remaining changed files**: the trial PR's changed Python or Rust source files other than the withheld file, as in the completed evaluation.

## Ranking variants

| ID | Adds | Scoring |
|---|---|---|
| E | Co-change alone | For each remaining changed file `d` with `n(d) > 0` history PRs touching it, each candidate `f` receives `n(d, f) / n(d)`, where `n(d, f)` counts those PRs that also touched `f`. Sum over remaining changed files; sort by descending score, then canonical path. A file with score 0 is unranked. |
| F | C and E combined | Reciprocal-rank fusion with `k = 60`: `1 / (60 + rank_C) + 1 / (60 + rank_E)`, where an unranked side contributes 0. Sort by descending score, then canonical path. A file unranked by both is unranked. |

E and F use the same candidate universe as A–D. A history path absent from the base snapshot's indexed files contributes nothing. E and F are computed at the design depth N = 10 and, as a sensitivity check fixed before any run, at N = 50 and N = 200. None of N, the mass-edit threshold, or `k` is tuned on this sample.

## Requirements

- **REQ-001 — Same inputs.** Run on the completed evaluation's two repositories, cutoff, sample-selection rules, and trials. A run whose sample differs from the checked-in reports fails, rather than producing incomparable numbers.
- **REQ-002 — History retrieval.** Per repository, list merged PRs once, reading one file listing per history PR needed by any depth, from the 200 cold-build PRs through the newest trial's base commit. Nothing is fetched per trial or per review. A missing or incomplete provider response fails the run. History is held in memory for the run and is never persisted.
- **REQ-003 — Variants E and F.** Every trial records, for E and F at each depth, the same measurements as A–D: rank or `unranked`, score, whole and snippet token counts, and the common 100,000-token packing outcome. E additionally records the contributing remaining files with their `n(d, f)` and `n(d)`.
- **REQ-004 — Cost.** The report records, per depth, the provider requests a cold build needs (merged-PR listing pages plus one file listing per history PR) and the requests per incremental update (one per merged PR). Review-time provider requests are zero by construction.
- **REQ-005 — History coverage.** Each trial records its history size in PRs and how many remaining changed files appear in no history PR. Aggregates report, per depth, the share of trials in which no remaining changed file appears in history (cold start).
- **REQ-006 — Extractor-gap diagnosis.** For each trial, record:
  - **lexical reach**: whether the withheld file's base text shares at least one identifier token with the remaining files' changed head lines, counting only identifiers of at least three characters that are not language keywords and that appear lexically in no more than 100 base files (C's ceiling);
  - for Rust only, **macro share**: how many remaining changed head lines fall inside a macro invocation body (`name!{…}`, `name!(…)`, `name![…]`), and whether every shared identifier from lexical reach occurs inside a macro invocation body in either file.

  Aggregates, over all trials and non-test trials, report the share lexically reachable among trials unranked by C, and for Rust the share of those attributable to macro bodies. This is an upper-bound diagnostic from lexical tokens, not a ranking variant; it does not change A–F.
- **REQ-007 — Report.** Each run writes one source-free JSON report per repository containing A–F per-trial results and aggregates in the completed evaluation's format, plus the cost, coverage, and diagnosis fields above. A–D results must equal the checked-in reports for the same trials.
- **REQ-008 — Invocation.** The evaluation remains the explicitly invoked, credentialed, read-only `live:context-relevance` command, outside `gate` and normal CI.

## Constraints and invariants

- **INV-001 — No production change.** Production ranking, packing, extraction, snapshot and PR-context behavior, persisted formats, public APIs, and the gate are unchanged. E, F, history retrieval, and the diagnosis live only in the opt-in evaluation command.
- **INV-002 — A–D unchanged.** Adding E, F and the diagnosis must not change any A–D trial result.
- **INV-003 — Reproducible.** A second run with the same cutoff reproduces the sample, per-trial results, and cost counts, apart from timings. A missing or incomplete provider response fails a run. Provider data is otherwise trusted as returned; the evaluation does not cross-check it for internal consistency.
- **INV-004 — No leakage.** A trial's history contains only PRs merged before that trial PR's base commit.
- **INV-005 — Source-free and read-only.** Reports contain paths, commit IDs, PR numbers, names, identifiers, ranks, scores, and counts, never source text. No writes, clones, or checkouts; fetched data is held in memory for the run only.
- **INV-006 — Bounded provider use.** A rate limit the existing GitHub client cannot wait out fails the run with no partial report.

## Non-goals

Adopting C, E, or F in production; a production co-change store or snapshot-format change; fixing the Rust macro extraction gap; a lexical-fallback ranking variant; TypeScript or other repositories; tuning N, `k`, or the mass-edit threshold on this sample; a pass/fail threshold.

## Acceptance criteria

- **AC-001 — Reports.** Checked-in reports for both repositories satisfy REQ-001–REQ-007, with the exact rerun command and cutoff, and A–D results identical to the completed evaluation's reports.
- **AC-002 — Reproducibility.** A second run with the same cutoff reproduces the sample, per-trial results, and cost counts exactly.
- **AC-003 — Scoring correctness.** A no-network test with a fake provider proves cold-build depth, incremental extension up to the trial base commit, the mass-edit drop, the leakage guard, E's scoring, F's fusion and tie-breaking, the unranked rule, and the cost and coverage counts.
- **AC-004 — Diagnosis correctness.** A no-network test proves lexical reach (length, keyword, and document-frequency filters) and Rust macro-body detection for brace, parenthesis, and bracket invocations, including nested bodies.
- **AC-005 — Boundaries.** Production crates and the gate are unchanged apart from the opt-in command and its tests.

## Open decisions

None.

## Revision history

- Revision 1 (2026-09-29): approved by the maintainer. History is built once at a simulated cold build and extended incrementally; history unit is merged PRs; design depth N = 10 with a fixed 50/200 sensitivity check; provider data is trusted apart from missing or incomplete responses; F uses reciprocal-rank fusion with `k = 60`.
