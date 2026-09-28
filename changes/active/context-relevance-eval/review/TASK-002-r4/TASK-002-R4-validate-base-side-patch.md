---
id: TASK-002-R4
kind: remediation
status: ready
spec: SPEC-context-relevance-eval
spec_revision: 4
requirements: [REQ-002, REQ-004, INV-002, INV-003, AC-003]
depends_on: [TASK-002-R3]
parent_task: TASK-002
remediates: [FIND-TASK-002-5]
---

# Validate parsed old-side patch coordinates against verified base source

Approved spec: `changes/active/context-relevance-eval/spec.md` revision 4. Original task: `changes/active/context-relevance-eval/tasks/TASK-002-counterfactual-trials.md`. Prior remediation: `changes/active/context-relevance-eval/review/TASK-002-r1/TASK-002-R1-correct-d-scoring-and-bindings.md`, `changes/active/context-relevance-eval/review/TASK-002-r2/TASK-002-R2-preserve-resolved-import-scope.md`, and `changes/active/context-relevance-eval/review/TASK-002-r3/TASK-002-R3-fail-inconsistent-head-analysis.md`. Reviewed subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `e2914e6d65a203519fefb47872040e1d2e454e20`. Validated finding: `changes/active/context-relevance-eval/review/TASK-002-r4/findings-validation.md`.

## Diagnosis and correction

**FIND-TASK-002-5 — old-side patch coordinates can disagree with the verified base blob while the trial succeeds.** The defect class is unchecked provider coordinates used as changed-name evidence. `parse_hunks` checks patch syntax and yields head changed lines, but does not compare old coordinates to base source (`crates/bohmian-code-review-select/src/diff.rs:137-150,180-247,351-362`). The evaluator's `removed_lines` derives old deletion positions (`crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs:489-507`). `Change::new` compares them only to base declaration spans (`trial.rs:566-581`) and then can return before head analysis for removed or deletion-only files (`trial.rs:581-584`). A nonempty head is checked against the head blob (`trial.rs:588-596`), which cannot validate the old side.

A syntactically valid `@@ -4 +0,0 @@\n-old` for an indexed two-line base `def old():\n    return 1\n` passes patch parsing, produces old line 4, and succeeds for a removed file without a head. The same mismatch is reachable on a modified file with an in-range head hunk. `evaluate` can then produce successful trials with the old declaration name absent. The affected siblings are all listed eligible modified, renamed, and removed paths with an indexed old path, including deletion-only and empty-head-range hunks. Their `changed` names feed `Query::new` (`trial.rs:685-702`), B–D downstream scoring (`trial.rs:761-798`), successful trial outcomes (`trial.rs:960-1009,1025-1048`), and JSON publication (`main.rs:351-405`). A withheld file's rank, score, and sent result can change. The existing R3 test checks only a head range beyond the verified head; its green result cannot close this base-side gap. The focused `trial::` lane currently passes seven tests.

This violates the spec's `REQ-002` rule that provider inconsistency fails the run, `REQ-004` requirement to include removed declarations' base names from verified source, and `INV-002` shared verified-source rule. `architecture/agent-rules.md:8` also bars an omitted file or missing facts becoming apparent success.

At the shared `Change::new` preparation boundary, validate the complete parsed old-side hunk coordinates, including context, against the already verified indexed base blob before consuming old declaration names or returning for removed/deletion-only input. An out-of-range old-side hunk must fail the run. Reuse the base bytes held by `Corpus`; keep the missing/malformed patch omission distinction and R3 head-side failure propagation. This closes the class for every sibling before the A–D trials are assembled, without changing the production parser, provider flow, public API, report format, or source lifetime. The correction belongs in the evaluator `trial.rs` and its inline no-network tests; include another evaluator file in the write set only if the shared correction demonstrably needs it.

## Acceptance and proof

1. A no-network `evaluate` case with a parsed hunk whose old span exceeds a verified two-line indexed base returns a run-level error. Cover a removed or deletion-only path that bypasses head analysis and a modified path with a valid head; neither can produce a successful trial or report.
2. A valid in-range removal or rename still contributes its old declaration name to B–D downstream lookup. Missing and syntactically malformed patches remain recorded omissions; missing required base/head blobs remain run-level failures. R1 exact D credit, R1 same-line binding, R2 resolved scope, and R3 head mismatch regressions remain green.
3. The correction changes only the opt-in evaluator and no production ranking, extraction, snapshot, PR-context, persistence, public API, Git provider behavior, report format, or gate.

Record RED and GREEN with the exact focused `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features <test-selector>` command after naming the regression. Then run the evaluator `trial::` and `report::` lanes. Run sequentially: `mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Inspect the cumulative base-to-new-candidate diff against TASK-002 and R1–R4. Route directly to `$cr-implement`.
