---
id: TASK-002-R3
kind: remediation
status: ready
spec: SPEC-context-relevance-eval
spec_revision: 4
requirements: [REQ-002, REQ-004, INV-002, INV-003, AC-003]
depends_on: [TASK-002-R2]
parent_task: TASK-002
remediates: [FIND-TASK-002-4]
---

# Fail a trial when a parsed patch conflicts with verified head analysis

Approved spec: `changes/active/context-relevance-eval/spec.md` revision 4. Original task: `changes/active/context-relevance-eval/tasks/TASK-002-counterfactual-trials.md`. Prior remediations: `changes/active/context-relevance-eval/review/TASK-002-r1/TASK-002-R1-correct-d-scoring-and-bindings.md` and `changes/active/context-relevance-eval/review/TASK-002-r2/TASK-002-R2-preserve-resolved-import-scope.md`. Reviewed subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `012aa47dbf2b7885d1e9ba25eb1b22c861113629`. Validated finding: `changes/active/context-relevance-eval/review/TASK-002-r3/findings-validation.md`.

## Diagnosis and correction

**FIND-TASK-002-4 — parsed patch and verified head disagreement becomes a successful omission.** The root cause is incorrect failure classification in the shared `Change::new` owner. At `crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs:590-596`, every failure from public-facade ranking or extraction after a patch has parsed and a verified head has been supplied becomes `Ok(change.omit(...))`. The catch-all hides an inconsistent patch/head pair, including a changed-line range beyond the head source. A syntactically valid patch `@@ -1 +4 @@` can pass `parse_hunks` while a two-line verified head causes `ChangedLines::new` to return `RangeBeyondSource`. `evaluate` then succeeds with no names from that changed file.

The shared decision affects every parsed, nonempty Python or Rust changed head whose facade analysis fails. Its sibling effects are A's head filtering in `rank_a` (`trial.rs:832-843`), B–D query construction (`trial.rs:610-623,687-703`), and successful trials and report output (`trial.rs:964-1012,1027-1049`; `main.rs:269-276,351-406`). A withheld candidate may receive a different rank, score, and sent outcome because another changed file's names disappeared. Existing six trial tests pass without a parsed-patch/head-mismatch case. Spec `REQ-002` permits omissions for missing or malformed *patches*; `REQ-004`, `INV-002`, and `INV-003` require changed-head facts from verified source and require provider inconsistency to fail the run. `architecture/agent-rules.md:8` forbids an omitted file or budget failure becoming apparent success.

At `Change::new`, propagate post-parse facade analysis failures using the existing `TrialError::Symbolic` path. Keep missing and syntactically malformed patch omissions where they already belong. This closes the class once before A and B–D consume the change, without a new parser, abstraction, public API, dependency, or spec decision. Preserve existing missing-head and missing-base failures, verified blob access, production A ranking and packing, B–D scoring and scope, report format, and source lifetime. Do not modify production symbolic or pipeline behavior, live provider flow, PR-context, replay, or gates.

## Acceptance and proof

1. A no-network `evaluate` trial with a parsed, nonempty patch whose changed head range exceeds its supplied head source returns `TrialError::Symbolic` and produces no successful trial or report. This proves the shared owner closes the failure for every remaining changed head and all four variants.
2. A missing or syntactically malformed patch remains a recorded omission; a missing required head or indexed base blob remains a run-level failure. The R1 exact D credit and same-line binding tests, R2 resolved-scope test, A baseline, packing, and report tests remain green.
3. The correction changes only the evaluator `trial.rs` and its inline no-network tests, unless the shared correction demonstrably requires another evaluator file. No public or production behavior, persisted format, provider flow, or gate changes.

Record RED and GREEN for the exact focused command after naming the regression test, then run `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::` and `report::`. Run sequentially: `mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Inspect the cumulative base-to-new-candidate diff for boundaries and review it against the original task plus R1–R3. Route directly to `$cr-implement`.
