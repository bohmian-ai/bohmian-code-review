---
id: SPEC-context-relevance-eval
revision: 1
status: approved
---

# Context relevance evaluation

Drafted 2026-09-27. Revision 1 is approved. Architecture overview: [review context architecture](https://claude.ai/artifact/KsGpUXokH17SP7SNmw9To2).

## Objective and user value

Measure whether the pipeline picks code that is actually related to a pull request, and whether three proposed ranking changes pick better, before any production change. The completed [repo-parser change](../../completed/2026/repo-parser.md) proves the pipeline is internally consistent (updates equal from-scratch builds, ranking follows its rule) but not that its choices help a review model. This change produces a reproducible baseline and a comparison so the next change is decided by evidence.

The method is leave-one-out co-change recall. Files changed together in one merged PR are treated as related. For each PR, one changed file is withheld from the PR's change set, the remaining changes are ranked against the PR's base commit, and the evaluation records where the withheld file ranks and whether it would be sent.

## Ranking variants

Each variant after A adds exactly one change to the one before, so the difference between neighbours measures that change.

Terms: **used names** are names referenced on the changed lines of a remaining changed file at its head version. **Changed names** are names of declarations that the changed lines add, modify or remove, taken from both the head version and the base version, so a removed or renamed declaration contributes its old name.

| ID | Adds | Lookup and scoring |
|---|---|---|
| A | Production baseline | Declared names only; 1 point per distinct shared name, exactly as the pipeline ranks today |
| B | References, by direction | Used names find files that declare them (upstream). Changed names find files that reference them (downstream). 1 point per distinct name per file |
| C | Noise control | B, with each name weighted by rarity (inverse of the number of files that declare or reference it), an upstream name's credit split evenly across the files that declare it, and names above a fixed document-frequency ceiling ignored |
| D | Import scoping | C, with a name resolved to the module a file's import names when one does, before lookup |

A matches the production pipeline exactly. B, C and D are evaluation-only prototypes built from the public extraction output; they are not production code. The rarity formula and document-frequency ceiling are recorded in each report.

## Requirements

- **REQ-001 — Repositories.** The evaluation runs against `pydantic/pydantic-ai` (Python) and `tokio-rs/tokio` (Rust) through the real GitHub API.
- **REQ-002 — PR sample.** For each repository, the sample is the 50 most recently merged PRs into the default branch before a fixed cutoff date recorded with the run, filtered to PRs with 2–15 changed eligible files of which at least one is withholdable (REQ-003) and at least one other has changed lines. The sample is fully determined by repository, cutoff and filter, and every PR is pinned to its recorded base and head commits.
- **REQ-003 — Withholdable file.** A changed eligible file is withholdable when it was modified (not added or renamed) and has declarations at the PR's base commit.
- **REQ-004 — Trial.** One trial per withholdable file per variant. The withheld file is removed from the PR's change set and remains a normal candidate at its base version. The remaining changed files contribute names from their changed lines at the head commit. Variant A runs through the existing verified fixed-commit build, public ranking and packing. B–D rank against an index built from the same verified base source.
- **REQ-005 — Measurements.** Each trial records the withheld file's rank (or `unranked` when it shares no name), its score, the matched names, the direction that found it (upstream, downstream or both), its whole-file token count and its snippet token count (for an upstream hit the matched declarations; for a downstream hit the enclosing declaration around each reference), and whether the file would be sent under the proposed packing: whole files in rank order within a 200,000-token review budget less the prompt reserve and the remaining diff, no file-count cap, and matched spans when the whole file does not fit. Changed files omitted for a missing or malformed patch are recorded, not silently dropped.
- **REQ-006 — Report.** Each run writes one JSON report per repository with the cutoff, sample, per-trial results and, per variant, hit rate at 1, 5, 10 and 50, share unranked, share sent whole, share sent as spans only, and the score distribution of hits and misses (to choose a minimum-score threshold later). Aggregates are reported over all trials and over trials whose withheld file is not a test file, using each repository's conventional test paths. Every miss lists its path and PR.
- **REQ-007 — Invocation.** The evaluation is an explicitly invoked, credentialed, read-only command in this repository, outside `mise run gate` and normal CI, in the same style as the existing live lanes.

## Constraints and invariants

- **INV-001 — No production change.** Production ranking, packing, extraction, snapshot and PR-context behavior, persisted formats and public APIs are unchanged. The evaluation consumes only existing public facades.
- **INV-002 — Faithful baseline.** Variant A uses the production build, ranking and packing code unmodified. B–D share A's verified source, extraction and candidate set and differ only in index contents and scoring.
- **INV-003 — Reproducible.** Rerunning with the same cutoff produces the same sample and the same per-trial results, apart from wall-clock timings.
- **INV-004 — Source-free report.** Reports contain paths, commits, names, ranks, scores and counts only, never source text.
- **INV-005 — Read-only.** No write to either repository; no clone or checkout; fetched source is held in memory for the run only.

## Non-goals

Changing production ranking, packing, snapshot format or default limits; hand-labelled relevance sets; model- or LLM-based judging; measuring files a reviewer needed that the PR did not change; compiler- or language-server-based resolution; repositories beyond the two named; a pass/fail threshold. The results are reported, not gated.

## Acceptance criteria

- **AC-001 — Reports.** Checked-in reports for both repositories satisfy REQ-002–REQ-006, with the exact rerun command, cutoff and pinned commits.
- **AC-002 — Reproducibility.** A second run with the same cutoff reproduces the sample and per-trial results exactly.
- **AC-003 — Trial correctness.** A no-network test with a fake provider proves a trial withholds exactly one file, keeps it rankable at its base version, excludes the remaining changed paths, scores each variant as defined, and reports rank, `unranked`, whole, spans-only and omitted outcomes correctly.
- **AC-004 — Baseline fidelity.** A test proves variant A's ranking equals the production pipeline's ranking for the same inputs.
- **AC-005 — Boundaries.** Production crates and the gate are unchanged apart from the new opt-in command and its tests.

## Open decisions

None.

## Follow-up (not in this change)

If the results justify it, a separate change revises the design and snapshot format to store referenced names, adopts the winning scoring, decides whole-file versus snippet packing (proposed: whole files for upstream definitions, enclosing-declaration snippets for downstream callers), raises the default review budget to about 200,000 tokens, removes the 15-file cap and sets a minimum score from this evaluation's score distributions.

## Revision history

- Revision 1 (2026-09-27): cumulative variants A–D (directional references with base-version names, rarity weighting with ambiguity split and document-frequency ceiling, import scoping), whole-file and snippet token measurements at a 200,000-token budget with no file cap, and test-file split reporting. Approved by the maintainer 2026-09-27.
