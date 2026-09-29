---
id: SPEC-cochange-eval
revision: 2
status: approved
---

# Co-change ranking experiment

Drafted 2026-09-29. Revision 2 approved by the maintainer 2026-09-29. Follows the [completed context-relevance evaluation](../../completed/2026/context-relevance-eval.md), whose results made C the production default.

## Question

Does co-change history (files that changed together in earlier merged PRs) rank related files higher than C alone? History is built once at cold build, extended as PRs merge, and read at review time with no provider calls.

This experiment measures ranking only. Whether a ranked file is then sent to the review model depends on production packing (the candidate file cap and token allowance), which is a separate decision for the follow-up production change. No packing outcome is measured or claimed here.

## Design

- **Control:** C, as defined in the completed evaluation (directional references with rarity weighting, document-frequency ceiling 100).
- **Treatments:**
  - **E**, co-change alone. For each remaining changed file `d` touched by `n(d) > 0` history PRs, each candidate `f` scores `n(d, f) / n(d)`, where `n(d, f)` counts those PRs that also touched `f`. Scores are summed over remaining changed files.
  - **F**, C and E combined by reciprocal-rank fusion: `1 / (60 + rank_C) + 1 / (60 + rank_E)`, where an unranked side contributes 0.
  - In every arm, ties sort by canonical path and a file with score 0 is unranked.
- **Paired:** all three arms rank the same candidates in the same trials.
- **Primary outcome:** hit@10, meaning the withheld file ranks 10th or better. No other outcome is reported.
- **Unit of analysis:** the PR. A PR's score in each arm is the mean hit@10 over its trials, so every PR counts equally.
- **Effect:** for E − C and F − C, the mean per-PR difference with a 95% percentile bootstrap confidence interval: 10,000 resamples over PRs with a fixed recorded seed.
- **Decision rule, fixed before any run:** adopt co-change (F) as the ranking signal only if, in every repository, F − C ≥ 5 points and the interval's lower bound is above 0. An interval containing 0 is reported as inconclusive, not as no effect.

## Sample

| Repository | Language | PRs merged before (exclusive) |
|---|---|---|
| `pydantic/pydantic-ai` | Python | `2026-08-23T19:15:42Z` |
| `tokio-rs/tokio` | Rust | `2026-05-21T13:41:55Z` |
| `colinhacks/zod` | TypeScript (`.ts`, `.tsx`) | `2026-09-01T00:00:00Z` |

The pydantic-ai and tokio cutoffs are the earliest merge times in the completed evaluation's samples, so no PR used to choose C is reused.

For each repository, the sample is the 50 most recently merged PRs into the default branch before the cutoff. It keeps the completed evaluation's selection rules, with the language set per repository:
- 2–15 changed source files;
- at least one withholdable file, meaning modified (not added or renamed) with declarations in the base snapshot;
- at least one other file with changed lines;
- ordered by merge time, then PR number, both descending.

## Trials

Each sampled PR contributes up to 5 trials. When it has more than 5 withholdable files, 5 are chosen by a deterministic pseudo-random rule fixed by the PR number and path. When it has 5 or fewer, all are used. The report records the chosen files.

A trial withholds one file and ranks every indexed file in the PR's base snapshot using the remaining changed files, as in the completed evaluation. The remaining changed paths are excluded from candidates.

## History

- **History PR:** a PR merged into the default branch, represented by its changed file paths. A PR with more than 30 changed files is a mass edit and is dropped.
- **Merged before a commit:** by definition, a PR is merged before a commit when the PR's merge timestamp is earlier than that commit's committer timestamp. History eligibility is defined by time, not Git ancestry, so no ancestry lookups are needed.
- **Cold build point:** the earliest committer timestamp among the sampled PRs' base commits. Every trial's base commit is therefore at or after the cold build point.
- **Cold-build history:** the 10 most recent non-mass-edit PRs merged before the cold build point.
- **Trial history:** the cold-build history plus every non-mass-edit PR merged at or after the cold build point and before the trial's base commit. This simulates incremental updates. A PR's base commit predates its merge, so a trial PR is never in its own history.
- A history path absent from the trial's base snapshot contributes nothing.

## Requirements

- **REQ-001 — Retrieval.** Per repository, the command lists merged PRs once and reads one file listing per history PR from the cold-build history through the newest trial's base commit. It reads one commit per sampled PR to obtain its base commit's committer timestamp. Nothing is fetched per trial. A missing or incomplete provider response fails the run.
- **REQ-002 — Production cost.** The report states the provider requests that production would need at depth 10:
  - cold build: merged-PR listing pages plus one file listing per history PR;
  - incremental update: one file listing per merged PR;
  - review time: zero requests.

  The evaluation's own base-commit timestamp lookups are reported separately and are not part of production cost.
- **REQ-003 — Report.** Each run writes one source-free JSON report per repository containing:
  - the cutoff, cold build point, bootstrap seed, and exact rerun command;
  - the sample with base and head commits;
  - per trial, the withheld path and each arm's rank or `unranked` and hit@10;
  - per PR, its history size and the per-arm mean;
  - the E − C and F − C effects with confidence intervals and the decision-rule outcome;
  - the cost figures and the threats to validity below.
- **REQ-004 — Invocation.** The experiment is an explicitly invoked, credentialed, read-only command in `mise`, outside `gate` and normal CI. The existing evaluation command may be reworked for it. The completed evaluation's checked-in reports remain historical records and need not be reproducible by the reworked command.

## Constraints and invariants

- **INV-001 — No production change.** Production ranking, packing, extraction, snapshot and PR-context behavior, persisted formats, public APIs, and the gate are unchanged. Extending sample eligibility to TypeScript uses the existing public path-eligibility query and extractor.
- **INV-002 — Reproducible.** A second run with the same cutoffs reproduces the sample, trials, ranks, and effects, apart from timings. A missing or incomplete provider response fails a run. Provider data is otherwise trusted as returned and is not cross-checked for internal consistency.
- **INV-003 — No leakage.** A trial's history contains only PRs merged before its base commit, as defined above.
- **INV-004 — Source-free and read-only.** Reports contain paths, commit IDs, PR numbers, ranks, scores, and counts, never source text. The command performs no writes, clones, or checkouts, and holds fetched data in memory for the run only.
- **INV-005 — Bounded provider use.** A rate limit the existing GitHub client cannot wait out fails the run with no partial report.

## Threats to validity

The report states each of these:
- **Circular label:** the label is co-change, and E predicts from earlier co-change, which favors E.
- **One repository per language:** results cannot separate language from codebase.
- **Proxy for relevance:** co-changed files stand in for the files a reviewer needed.
- **Small sample:** 50 PRs per repository detect only large effects.

## Non-goals

Variants A, B and D; other outcomes or subsets; packing or model-exposure outcomes; history-depth sensitivity; the Rust macro diagnosis; adopting any variant in production; a production co-change store or snapshot-format change; tuning depth, the mass-edit threshold, or the fusion constant on this sample.

## Acceptance criteria

- **AC-001 — Reports.** Checked-in reports for all three repositories satisfy REQ-003.
- **AC-002 — Reproducibility.** A second run with the same cutoffs reproduces the reports exactly, apart from timings.
- **AC-003 — Experiment correctness.** No-network tests with a fake provider prove:
  - trial selection: at most 5 per PR, all when fewer, deterministic;
  - history: the cold build point precedes every trial base, cold depth 10, incremental extension, the mass-edit drop, and exclusion of the trial PR;
  - E and F scoring, ties, and the unranked rule;
  - per-PR means, paired differences, the seeded bootstrap interval, and the decision rule.
- **AC-004 — Boundaries.** Production crates and the gate are unchanged apart from the opt-in command and its tests.

## Open decisions

None.

## Revision history

- **Revision 2 (2026-09-29):** approved by the maintainer. Redesigned as a controlled experiment on reviewer and maintainer feedback:
  - C as the sole control, with E and F as treatments; hit@10 at the PR level; a pre-registered decision rule;
  - up to 5 trials per PR;
  - fresh non-overlapping samples, with `colinhacks/zod` added;
  - history eligibility by merge and commit timestamps, with the cold build point at the earliest sampled base commit, which resolves the ancestry-retrieval and cold-point findings;
  - dropped A, B, D, depth sensitivity, and the macro diagnosis;
  - narrowed the question and decision rule to ranking alone, because production packing limits mean rank does not establish that a file reaches the review model.
- **Revision 1 (2026-09-29):** approved by the maintainer; superseded by revision 2.
