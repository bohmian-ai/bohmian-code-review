# TASK-002 independent task review A

Subject: `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41` → `0aa3a607e770d3271f872417b535d555360b4a20`; approved spec revision 4, original TASK-002, and TASK-002-R1. Reviewed the cumulative diff and the evaluator's source path. No CodeGraph index exists.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004 / AC-003–004: withhold one modified base file, preserve base candidate, exclude other changes, and match production A ranking | `trial.rs:964–1049`, `rank_a` at 828–845 | `withholding_keeps_base_file_rankable_and_a_is_production`; focused trial lane passed 5/5 | PASS |
| REQ-004: full indexed base reference corpus and missing-source failure | `Corpus::new` at `trial.rs:225–277`; `Api::trials` at `main.rs:216–275` fetches blobs via verified client | `variants_score_direction_rarity_and_imports` checks a remote candidate and missing base blob | PASS |
| REQ-004 / R1: B and C directional, rarity, ambiguity, and deterministic scores; D retains C denominator | `trial.rs:716–819`, especially `759`, and `813–818` | `variants_score_direction_rarity_and_imports`, `scoped_upstream_keeps_declaring_count_in_credit` | PASS |
| REQ-004 / R1: D scopes a reference only by its explicit, uniquely resolved import and never ranks an upstream file outside that module | `bindings` at `trial.rs:378–423`; `score` at `743–761` | Existing same-line test covers statement association, but not a resolved module lacking the declaration | FAIL: TASK-A-1 |
| REQ-005 / AC-003: whole and span token counts, common 100,000-token packing, separate production A result, patch counts and omissions | `trial.rs:879–1012`; `main.rs:183–195` | `packing_records_whole_spans_omitted_and_unranked`; focused trial lane passed | PASS |
| REQ-006: source-free per-trial and all/non-test aggregate report with misses | `report.rs:1–185`; `main.rs:351–398` | `summaries_and_misses_follow_outcomes`; serialization assertion in trial test | PASS |
| INV-001–005 / AC-005: read-only evaluator, no production or gate changes, no `PrContext` or replay | Cumulative diff touches only the opt-in example, `mise.toml` description, and task/review records | Diff inspection; no production files changed | PASS |

## Proposed finding

### TASK-A-1 — INCORRECT: a resolved D binding becomes an unscoped lookup when its module lacks the name

- **Root cause and defect class:** `score` discards an otherwise uniquely resolved upstream import binding when the bound module is absent from `declaring`. This is a wrong eligibility decision in the shared D scorer, not a public API shape defect.
- **Location and siblings:** `crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs:743–745` is the sole upstream owner. It affects every used name in every remaining head whose explicit Python or Rust binding resolves to an indexed module that does not declare the name. `Query::new` feeds that owner for all remaining changed heads (`trial.rs:686–705`); `Change::new` builds their bindings (`618–623`). Downstream D at `777–785` keeps a resolved module and does not have this fallback.
- **Violated obligation:** Spec revision 4, D rule: scope when explicit binding identifies exactly one indexed base module; an upstream target must declare the name **in that module**. Only ambiguous, wildcard, implicit, or unresolved imports retain C's unscoped lookup. TASK-002 Scenario 2 and AC-003 require this behavior.
- **Reachable consequence:** Given indexed `pkg/a.py` with no `x`, indexed `pkg/b.py` declaring `x`, and a remaining changed `pkg/caller.py` with `from pkg.a import x` and `x()` on its changed line, `bindings` resolves `x` to `pkg/a.py`. `declaring("x")` contains only `pkg/b.py`; the filter makes `module` `None`, so D credits and ranks `pkg/b.py` exactly as C would. If `pkg/b.py` is withheld, its D rank and sent outcome are falsely positive although it is outside the explicit import's module. This path runs from public `evaluate` through `Change::new`, `Query::new`, `score`, and `trial`.
- **Testable correction:** Preserve the resolved module even when it lacks the name, then let D's existing module filter exclude all other declarers. A focused no-network `evaluate` trial with the above files should yield C ranking `pkg/b.py` and D `unranked` for `pkg/b.py`, while unresolved and ambiguous bindings still fall back to C. This also checks the sole sibling class.

Prior `FIND-TASK-002-1` and `FIND-TASK-002-2` appear closed by `trial.rs:759` and the coordinate-based binding association at `391–406`; their focused regression tests pass. The proposed finding is a distinct D eligibility defect.

Verification: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::` passed 5/5 at this candidate. No live GitHub run was attempted; TASK-003 owns live invocation. Overall result: **FAIL**.

```citations
[{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":743,"snippet":"            let module = (rule == Rule::Scoped)","symbols":["score","bound"]},{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":744,"snippet":"                .then(|| bound(bindings, name).filter(|module| declaring.contains_key(*module)))","symbols":["score","bound"]},{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":754,"snippet":"                if excluded.contains(path) || module.is_some_and(|module| module != path) {","symbols":["score"]}]
```
