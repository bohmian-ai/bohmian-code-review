---
id: TASK-002-R2
kind: remediation
status: ready
spec: SPEC-context-relevance-eval
spec_revision: 4
requirements: [REQ-004, REQ-005, INV-002, AC-003]
depends_on: [TASK-002-R1]
parent_task: TASK-002
remediates: [FIND-TASK-002-3]
---

# Preserve resolved import scope when a declaration is absent

Approved spec: `changes/active/context-relevance-eval/spec.md` revision 4. Original task: `changes/active/context-relevance-eval/tasks/TASK-002-counterfactual-trials.md`. Prior remediation: `changes/active/context-relevance-eval/review/TASK-002-r1/TASK-002-R1-correct-d-scoring-and-bindings.md`. Reviewed subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `0aa3a607e770d3271f872417b535d555360b4a20`. Validated finding: `changes/active/context-relevance-eval/review/TASK-002-r2/findings-validation.md`.

## Diagnosis and correction

**FIND-TASK-002-3 — a resolved D upstream binding is misclassified as unscoped.** At `crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs:743-745`, the shared `score` owner filters a uniquely resolved module through `declaring.contains_key` before deciding whether the reference is scoped. The defect class is a wrong eligibility decision. The `None` result then admits every indexed declarer at `trial.rs:753-761` and increments the unscoped count at `:746-751`. The same decision affects every D upstream lookup from every remaining changed head whose explicit Python or Rust binding resolves to one indexed module that does not declare the queried name. `Change::new` at `:610-623` and `Query::new` at `:687-703` feed all such lookups to this owner; downstream D at `:777-786` is a separate path without this defect.

The approved D rule at `spec.md:32` scopes a reference when its explicit binding identifies exactly one indexed module. An upstream target must declare the name in that module. Only ambiguous, wildcard, implicit, or unresolved imports retain C's unscoped lookup. For indexed `pkg/a.py` without `wanted`, indexed `pkg/b.py` declaring `wanted`, and a remaining changed head that imports `wanted` from `pkg.a` and uses it on a changed line, C may rank `pkg/b.py`; D must not. The candidate currently ranks and can send `pkg/b.py` under D. The five passing trial tests do not cover a resolved module missing the declaration, so they do not close this gap. Wave 2 reproduced the route from public `evaluate` through `Change::new`, `Query::new`, `trial`, and `score` without changing source.

Keep the uniquely resolved module as the D scope even when it has no matching declaration. Reuse `bound` and the existing upstream module filter in `score`; if that module has no declarer, no upstream file receives credit for the name. Preserve C fallback only when no unique indexed module binding exists. This one correction at the shared scorer covers both language producers without a new parser, abstraction, dependency, public API, or spec decision. Preserve the complete base declaring-file denominator from R1, the df ceiling, B/C rules, deterministic ties, D downstream behavior, production A ranking and packing, and source-free reporting.

## Acceptance and proof

1. A no-network `evaluate` trial proves a uniquely resolved explicit import to an indexed module lacking the name remains scoped: C may rank an unrelated declarer, while D leaves that withheld declarer `unranked` with no score or sent outcome from this name. The D scoped/unscoped counts reflect the resolved binding. This closes `FIND-TASK-002-3` for every remaining changed head through the shared `score` owner.
2. The same trial or an existing one proves an unresolved or ambiguous import still uses C fallback. The R1 exact D credit, same-line Python bindings, downstream scoping, and missing-base-source tests remain green.
3. No production symbolic or pipeline behavior, persisted format, public API, live-lane gate, provider flow, or report source-lifetime rule changes. The write set is the evaluator's `trial.rs` and its inline no-network test, extended only if the shared correction actually requires another evaluator file.

Record RED and GREEN for the exact focused command `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::tests::resolved_import_without_declaration_keeps_scope -- --exact`, then run the full `trial::` and `report::` focused selectors. Run sequentially: `mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Inspect the cumulative base-to-new-candidate diff for production, public API, gate, source-text, PR-context, and replay changes. Re-review the immutable cumulative candidate against the original task and both remediation tasks.
