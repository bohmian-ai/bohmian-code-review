# Domain review: scoring and budget

Subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `0aa3a607e770d3271f872417b535d555360b4a20`.

## Boundary and coverage

Reviewed approved spec revision 4, original TASK-002, TASK-002-R1, the cumulative diff, and the evaluator's `trial.rs`, `report.rs`, and `main.rs`. Traced A through the public ranker and production candidate packer; B–D through the complete base corpus, directional lookup, document frequency, scoped lookup, scoring, candidate ordering, and common packer; then trial token accounting and aggregate/miss reporting. Governing obligations are spec REQ-004–REQ-006, INV-001–INV-004, and AC-003–AC-005, with the symbolic ranking and packing contract in `architecture/bohmian-code-review-design.md` and `architecture/references/domain/symbolic-context.md`.

The focused no-network trial lane ran successfully: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::` (5 passed). The new tests close prior FIND-TASK-002-1's denominator and FIND-TASK-002-2's same-line binding cases. No live GitHub run or checked-in repository report was available in this task's scope; the finding below follows a reachable source path that the focused fixtures do not cover.

## Material finding

**BUDGET-1 — INCORRECT: a resolved D upstream import falls back to C when its module lacks the queried declaration.** Defect class: resolved import is reclassified as unresolved by checking declaration membership before applying the scope. The candidate-owned `score` function first resolves `bound(bindings, name)` but discards the module unless it appears in `declaring` (`trial.rs:743–745`). Its `None` branch then tries every declaring file (`trial.rs:753–761`) and counts the lookup as unscoped (`trial.rs:746–751`). The spec permits C fallback for ambiguous, wildcard, implicit, or unresolved imports; for a uniquely resolved indexed module, an upstream target must declare the name **in that module**.

Reachable example: a remaining changed Python file contains `from pkg.empty import helper` and uses `helper` on a changed line; `pkg.empty` is the sole indexed base module for that import, but only `pkg.other` declares `helper`. `bindings` resolves the import to `pkg/empty.py`; `declaring("helper")` contains only `pkg/other.py`; the filter erases the resolved module and D ranks `pkg/other.py` as an upstream candidate. This can change the withheld file's D rank, score, sent outcome, and scoped/unscoped counts. The same site covers every D upstream used name with an explicit unique binding whose target module lacks that declaration; downstream uses a separate check and is not a sibling.

**Correction boundary:** retain a unique resolved module as the D scope even when it has no matching declaration, so the upstream lookup returns no candidate in that case. Keep C's full declaring-file denominator for successful scoped hits and retain unscoped fallback only for imports that do not identify one indexed base module. A focused no-network trial should compare C and D for the example above, asserting C admits `pkg/other.py`, D excludes it, and D counts the lookup as scoped.

## Result

**FAIL** — one task-contract finding. Whole-file and matched-span accounting, deterministic score/path ordering, A's separate production observation, and aggregate calculations showed no other material issue in this review.

```citations
[{"id":"BUDGET-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":744,"snippet":"                .then(|| bound(bindings, name).filter(|module| declaring.contains_key(*module)))","symbols":["score","bound","Corpus::declaring"]},{"id":"BUDGET-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":754,"snippet":"                if excluded.contains(path) || module.is_some_and(|module| module != path) {","symbols":["score"]}]
```
