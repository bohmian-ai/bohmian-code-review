---
id: SPEC-context-relevance-eval
revision: 4
status: approved
---

# Context relevance evaluation

Drafted 2026-09-27. Revision 4 approved by the maintainer. Architecture overview: [review context architecture](https://claude.ai/artifact/KsGpUXokH17SP7SNmw9To2).

## Objective and user value

Measure whether the pipeline picks code that is actually related to a pull request, and whether three proposed ranking changes pick better, before any production change. The completed [repo-parser change](../../completed/2026/repo-parser.md) proves the pipeline is internally consistent (updates equal from-scratch builds, ranking follows its rule) but not that its choices help a review model. This change produces a reproducible baseline and a comparison so the next change is decided by evidence.

The method is leave-one-out co-change recall. Files changed together in one merged PR are treated as related. For each PR, one changed file is withheld from the PR's change set, the remaining changes are ranked against the PR's base commit, and the evaluation records where the withheld file ranks and whether it would be sent.

## Ranking variants

Each variant after A adds exactly one change to the one before, so the difference between neighbours measures that change.

Terms: **used names** are names referenced on the changed lines of a remaining changed file at its head version. **Changed names** are names of declarations that the changed lines add, modify or remove, taken from both the head version and the base version, so a removed or renamed declaration contributes its old name.

| ID | Adds | Lookup and scoring |
|---|---|---|
| A | Production baseline | Names of changed-line declarations and references find files that declare them; 1 point per distinct shared name, exactly as the pipeline ranks today |
| B | References, by direction | Used names find files that declare them (upstream). Changed names find files that reference them (downstream). 1 point per distinct name per file, even when both directions match |
| C | Noise control | B, with the fixed rarity and ambiguity rule below |
| D | Import scoping | C, with the fixed explicit-import rule below |

For C, document frequency `df(name)` is the number of distinct eligible base files that declare or reference that exact name. Ignore a name when `df(name) > 100`. Otherwise, each downstream file referencing the name receives `1 / df(name)`; each upstream file declaring it receives `1 / (df(name) × declaring_file_count(name))`. If a file matches in both directions, use the larger credit for that name and record both directions. Sum credits over distinct names, then sort by descending score and canonical path. The report records the formula and ceiling.

For D, inspect the file containing the reference: a remaining changed file for an upstream hit, or a base candidate file for a downstream hit. Use only import facts returned by the public extractor. Scope that reference only when an explicit import binding and the repository-relative Python or Rust module path identify exactly one indexed base module. The upstream target must declare the name in that module; the downstream import must point to the changed file's module. Ambiguous, wildcard, implicit, or unresolved imports retain C's unscoped lookup. No separate compiler, language server, or general import resolver is part of this evaluation. The report counts names scoped and names left unscoped.

A uses production ranking on the counterfactual inputs. B, C and D are evaluation-only prototypes built from the public extraction output; they are not production code.

## Requirements

- **REQ-001 — Repositories.** The evaluation runs against `pydantic/pydantic-ai` (Python) and `tokio-rs/tokio` (Rust) through the real GitHub API.
- **REQ-002 — PR sample.** For each repository, the sample is the 50 most recently merged PRs into the default branch before a fixed cutoff date recorded with the run, filtered to PRs with 2–15 changed Python or Rust source files of which at least one is withholdable (REQ-003) and at least one other has changed lines. Order by merge timestamp descending, then PR number descending to break ties. The opt-in command reads one complete PR file listing per PR (following its pages once), checks each path with the public language-eligibility query, and uses the existing fixed-commit base snapshot to establish declaration facts. This is test-harness setup: treat the listed PR metadata and files as the run's input. An unusable PR identity or merge timestamp, rejected file entry, incomplete listing, or incomplete base snapshot fails the run; do not repair or cross-check the input through a second PR-context workflow. A missing or malformed per-file patch is recorded as an omission and cannot satisfy the changed-lines condition; it does not by itself fail the run. Record the PR's base and head commits. Discovery may query GitHub's API directly; no new public library API is needed.
- **REQ-003 — Withholdable file.** A changed Python or Rust source file is withholdable when the PR file listing marks it modified (not added or renamed) and the base snapshot contains declarations for it.
- **REQ-004 — Trial.** One trial per withholdable file per variant. The withheld file is removed from the PR's change set and remains a normal candidate at its base version. The remaining changed files contribute names from their changed lines at the head commit. The candidate universe for every variant is every indexed file in the same complete base snapshot. Variant A loads that snapshot's declarations and uses public ranking. B–D obtain base-version reference facts for that entire candidate universe from hash-verified blobs and the public extraction output; a missing base blob fails the run rather than silently shrinking the candidate set. A's production packing result is recorded separately from the common experimental packing result used to compare all four variants.
- **REQ-005 — Measurements.** Each trial records the withheld file's rank (or `unranked` when it shares no name), its score, the matched names, the direction that found it (upstream, downstream or both), its whole-file token count and its snippet token count (for an upstream hit the matched declarations; for a downstream hit the enclosing declaration around each reference), and whether the file would be sent under one common experimental packing policy: whole files in rank order within a 100,000-token candidate-context allowance, no file-count cap, and matched spans when the whole file does not fit. Count tokens with the pipeline's current estimate of one token per four UTF-8 bytes, rounded up. Count the remaining changed files' patches and the 4,096-token prompt reserve separately and report those counts; the candidate-context allowance is after those allocations. The withheld file's patch is excluded from the remaining diff. For A, also record whether unmodified production packing would select it under the same 100,000-token candidate allowance. Changed files omitted for a missing or malformed patch are recorded, not silently dropped.
- **REQ-006 — Report.** Each run writes one JSON report per repository with the cutoff, sample, per-trial results and, per variant, hit rate at 1, 5, 10 and 50, share unranked, share sent whole, share sent as spans only, and the score distribution of hits and misses (to choose a minimum-score threshold later). Aggregates are reported over all trials and over trials whose withheld file is not a test file, using each repository's conventional test paths. Every miss lists its path and PR.
- **REQ-007 — Invocation.** The evaluation is an explicitly invoked, credentialed, read-only command in this repository, outside `mise run gate` and normal CI, in the same style as the existing live lanes.

## Constraints and invariants

- **INV-001 — No production change.** Production ranking, packing, extraction, snapshot and PR-context behavior, persisted formats and public APIs are unchanged. The evaluation uses the existing fixed-commit snapshot and public symbolic facade for base facts and baseline behavior, and the existing GitHub client for source blobs needed by trials. Private PR discovery, experimental ranking, packing, and reporting stay in the opt-in command. Sample qualification does not run `PrContext`, fetch a second PR file listing, or add a transport replay layer.
- **INV-002 — Faithful baseline.** Variant A uses the production build and ranking code unmodified, and its separately reported production packing result uses the production packer unmodified. All four variants use the same experimental packing policy for comparative sent outcomes. B–D share A's verified source, extraction and candidate set and differ only in lookup and scoring.
- **INV-003 — Reproducible.** A second run with the same cutoff must produce the same sample and per-trial results, apart from wall-clock timings. This is checked by comparing runs, not by building an in-run PR-list reconciliation or replay mechanism. Provider inconsistency fails a run.
- **INV-004 — Source-free report.** Reports contain paths, commits, names, ranks, scores and counts only, never source text.
- **INV-005 — Read-only.** No write to either repository; no clone or checkout; fetched source is held in memory for the run only.

## Non-goals

Changing production ranking, packing, snapshot format or default limits; hand-labelled relevance sets; model- or LLM-based judging; measuring files a reviewer needed that the PR did not change; compiler- or language-server-based resolution; repositories beyond the two named; a pass/fail threshold. The results are reported, not gated.

## Acceptance criteria

- **AC-001 — Reports.** Checked-in reports for both repositories satisfy REQ-002–REQ-006, with the exact rerun command, cutoff and pinned commits.
- **AC-002 — Reproducibility.** A second run with the same cutoff reproduces the sample and per-trial results exactly.
- **AC-003 — Trial correctness.** A no-network test with a fake provider proves a trial withholds exactly one file, keeps it rankable at its base version, excludes the remaining changed paths, scores each variant as defined, and reports rank, `unranked`, whole, spans-only and omitted outcomes correctly.
- **AC-004 — Baseline fidelity.** A test proves variant A's ranking equals the production pipeline's ranking for the same inputs.
- **AC-005 — Boundaries.** Production crates and the gate are unchanged apart from the new opt-in command and its tests. Sample qualification uses one PR file listing and the base snapshot, with no `PrContext` call or transport replay layer.

## Open decisions

None.

## Follow-up (not in this change)

If the results justify it, a separate change revises the design and snapshot format to store referenced names, adopts the winning scoring, decides whole-file versus snippet packing (proposed: whole files for upstream definitions, enclosing-declaration snippets for downstream callers), sets a production context budget using these measurements, removes the 15-file cap and sets a minimum score from this evaluation's score distributions.

## Revision history

- Revision 4 (2026-09-28): specified which sample failures stop the run versus become file omissions, fixed the full base-snapshot candidate universe for all variants, and limited import scoping to public extraction facts. Approved by the maintainer.
- Revision 3 (2026-09-27): clarified sample discovery as a small evaluation harness step using one trusted PR file listing, path eligibility, and base snapshot declarations; inconsistent provider input fails the run; rerun comparison proves reproducibility. Approved by the maintainer after rejecting TASK-001's production PR-context and replay implementation.
- Revision 2 (2026-09-27): defined C and D scoring, set a 100,000-token candidate-context comparison allowance and fixed token accounting, separated A's production packing result from common experimental packing, and allowed private PR discovery in the opt-in evaluation command. Approved by the maintainer 2026-09-27.
- Revision 1 (2026-09-27): cumulative variants A–D (directional references with base-version names, rarity weighting with ambiguity split and document-frequency ceiling, import scoping), whole-file and snippet token measurements at a 200,000-token budget with no file cap, and test-file split reporting. Approved by the maintainer 2026-09-27.
