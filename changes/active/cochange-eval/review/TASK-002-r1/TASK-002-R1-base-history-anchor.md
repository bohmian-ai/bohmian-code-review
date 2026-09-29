---
id: TASK-002-R1
kind: remediation
status: proposed
spec: SPEC-cochange-eval
spec_revision: 2
parent_task: TASK-002
remediates: [FIND-TASK-002-1]
---

# Exclude absent-base history anchors from E

## Subject and diagnosis

Approved spec: `changes/active/cochange-eval/spec.md` revision 2. Original task: `changes/active/cochange-eval/tasks/TASK-002-ranking.md`. Review base: `3fecce95f055268359d7d1d87df6308ea504329e`; cumulative candidate: `1b6d31eeeb7c6b8edd1989abb19f6f2cf161d4c8`.

**FIND-TASK-002-1 (INCORRECT; source IDs `TASK-B-1`, `HIST-1`).** `rank_e` in `crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs:419-428` selects history PRs for every remaining changed path, while checking base-corpus membership only for the credited candidate path. `evaluate` at lines 533-550 admits added changed paths absent from the base and passes them through every trial. This missing source-side membership rule is the sole implementation site; F at lines 438-446 inherits the resulting E rank. The approved spec's History section says a path absent from the trial base snapshot contributes nothing, and TASK-002 scenario 2 requires that boundary.

For a sampled PR with a modified, withholdable base file and an added source file whose path was used in an earlier merged PR, the added file remains after withholding. An earlier co-change involving that path and an indexed candidate produces E credit despite the path's absence from the trial base. This can alter the withheld E/F ranks and hit@10. `findings-validation.md` traces the reachable path from sample screening through the example facade and scoring. Existing tests cover an absent destination path and an added path with no matching history, so they cannot detect this case.

## Required outcome and correction

At the existing `rank_e` owner, a remaining changed path contributes to E only when it belongs to the trial's indexed base corpus. Reuse `Corpus::declarations`, the membership mechanism already used for candidate eligibility and withheld selection. Apply that rule before counting history touched by the path, across every trial. Keep the complete remaining-change input for C, changed-path exclusion, selection, and trial reporting; no upstream filtering, provider read, public API, history-store, or production selector change is needed. This closes the one root-cause class and preserves the approved C/E/F formulas for eligible paths.

## Acceptance and proof

- An added remaining path absent from the base yields no E contribution even if eligible earlier history contains it with indexed candidates; F receives no E rank credit from it. This covers the `evaluate` to `rank_e` to `rank_f` path.
- An indexed remaining changed path still contributes `n(d,f)/n(d)` normally. Candidate membership, changed-path exclusion, time filtering, C ranking, trial selection, and ranking-only serialization remain intact.
- Add one focused in-memory regression in the existing example target that distinguishes those two anchors and checks the resulting E and F ranks or unranked outcomes. Run its exact focused command and `mise run test:pipeline`, then the task's symbolic journey, formatting, lint, diff check, and gate lanes.

The write set is `crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs`, including its inline test. The correction stays within the approved experiment. Do not add packing or model-exposure outcomes, other arms, a production co-change store, public API changes, or unrelated refactoring. Route this task directly to `$cr-implement`.
