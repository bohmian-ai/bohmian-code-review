---
id: TASK-002-R1
kind: remediation
status: ready
spec: SPEC-context-relevance-eval
spec_revision: 4
requirements: [REQ-004, REQ-005, INV-002, AC-003]
depends_on: [TASK-002]
parent_task: TASK-002
remediates: [FIND-TASK-002-1, FIND-TASK-002-2]
---

# Correct variant D scoring and import bindings

Approved spec: `changes/active/context-relevance-eval/spec.md` revision 4. Original task: `changes/active/context-relevance-eval/tasks/TASK-002-counterfactual-trials.md`. Review subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `e3d194de32f456f5136a214aaf7d8087483e7013`. The validated ledger is `changes/active/context-relevance-eval/review/TASK-002-r1/findings-validation.md`.

## Diagnosis and correction

**FIND-TASK-002-1 — scoped D upstream scores violate C's fixed formula.** `trial.rs:747` substitutes one for `declaring.len()` when an explicit import resolves, and `trial.rs:754` uses that value in `1 / (df × count)`. This is an incorrect scoring denominator at the shared `score` owner. Every D scoped upstream name is affected. With two indexed base declarers and `df=6`, D credits its selected file `1/6`, although spec C and D require `1/12`; another name can then change ranking, report scores, or sent outcome. The current fixture checks D rank but not exact credit. Keep the complete base declaring-file count for upstream credit while filtering eligible files by the uniquely imported module. Preserve B's one-point rule, C's frequency ceiling, unresolved-import fallback, and D's downstream `1/df` credit. This is a correction inside the existing scorer; no public API or new dependency is needed.

**FIND-TASK-002-2 — same-line Python imports can resolve to the wrong module.** `trial.rs:386-400` associates each import reference with the first import node whose *line* contains it. Public extractor facts carry start/end columns for nodes and a column for the reference. With `from pkg.a import x; from pkg.b import y` on one line, the `y` binding can be assigned to `pkg.a`, so D can omit the true `pkg.b` target or admit a wrong one. The same `bindings` owner feeds base candidate references at `trial.rs:267` and remaining changed head references at `trial.rs:612`; D consumes them for downstream and upstream filtering respectively. The spec permits scoping only when an explicit binding and unique indexed module are established; ambiguous bindings must use C's unscoped lookup. Associate import references with their actual statement using the available public extraction coordinates. If those facts cannot establish a unique binding, keep it unscoped. Reuse the existing shared binding owner for both source paths, preserving separate-line Python and Rust behavior, relative/wildcard/unresolved fallback, and source-free reporting.

## Acceptance and proof

1. For an imported name declared by at least two indexed base files, a no-network `evaluate` trial proves D keeps only the uniquely imported target and assigns it C's exact `1 / (df × declaring_file_count)` credit. Include a competing candidate whose order would expose the old inflated credit; verify reported score, rank, and sent result. C and B behavior remains as specified.
2. A no-network `evaluate` trial with two Python `from` imports on one line proves the actual binding is used on both remaining-head upstream and indexed-base-candidate downstream paths. A repeated-name or otherwise ambiguous same-line case falls back to C. Existing separate-line Python and Rust, unresolved-import, and missing-base-source cases remain green.
3. No production symbolic or pipeline behavior, persisted format, public API, sample discovery, or live lane gate changes. Report records remain source-free; fetched blobs stay transient and verified.

For each new focused test, record the exact `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features <selector>` command and actual RED/GREEN results. Then run sequentially: `mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Inspect the cumulative base-to-new-candidate diff and rerun task review on that immutable candidate.

Expected write set: `crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs` and its inline no-network tests. Extend it only where closing the two validated root causes actually requires another evaluator file. This task changes no approved product or architecture decision.
