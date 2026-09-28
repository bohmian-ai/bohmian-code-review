# TASK-002 independent task review B

Subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `e3d194de32f456f5136a214aaf7d8087483e7013`; approved `spec.md` revision 4 and original `TASK-002-counterfactual-trials.md`. I inspected the complete five-file diff, the public ranking and packing calls, and the evaluator's trial tests. The candidate remained at the requested commit during this review.

| Requirement, criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004: one withheld modified file remains a base candidate; all remaining changed paths excluded; complete base candidate corpus | `trial.rs` `evaluate`, `Corpus::new`, `excluded`, `rank_a` | `withholding_keeps_base_file_rankable_and_a_is_production`; `variants_score_direction_rarity_and_imports` missing-blob case | PASS |
| REQ-004/INV-002/AC-004: A uses public ranking and separate production packing | `rank_a` calls `SymbolicContext::rank_files`; `trial` calls `build_candidate_context`; `outcome` packs experimental results | Baseline equality assertion and focused trial lane (3 passed) | PASS |
| REQ-004: B directional distinct-name scoring over base references | `Corpus::new`, `Query::new`, `score` | Python and Rust directional test | PASS |
| REQ-004: C exact document frequency, ceiling, weighting, and deterministic ties | `document_frequency`, `score` | Scoring test asserts a C score and rank | PASS |
| REQ-004: D retains C's weighting while restricting lookup by unique explicit imports | `bindings`, `python_module`, `rust_module`, `score` | Test asserts D ranks, but no D score or unchanged C credit | FAIL (TASK-B-1) |
| REQ-004: removed or renamed declaration names include base side; missing indexed base blob fails | `Change::new` reads base declaration facts and removed lines; `Corpus::new` requires every source | Test exercises removal and missing indexed blob; rename path is handled by `previous_path` | PASS |
| REQ-005: rank, score, names, directions, token counts, common whole/spans/omitted outcome and separate production selection | `Outcome`, `trial`, `outcome`, `snippet_tokens`, `production_selected` | Packing test covers all four outcomes; focused lane passed | PASS |
| REQ-005: remaining patches and prompt reserve counted separately; withheld patch excluded | `Api::qualify` sums listed patches; `trial` subtracts withheld patch; `PROMPT_RESERVE` in report | Withholding test checks subtraction | PASS |
| REQ-006/INV-004: source-free trial records, summaries, score distributions, test split, and every miss | `report.rs`, `main.rs` JSON record | Report unit test and trial JSON serialization assertion | PASS |
| INV-001/INV-005/AC-005 and non-goals: opt-in only; no production behavior, gate, persisted format, PR-context workflow, replay, or write to GitHub | Diff is confined to evaluator example, task evidence, and live-lane description | Complete base-to-candidate diff inspection | PASS |
| INV-003: deterministic sample and results for fixed provider inputs | Ordered maps, sorted candidate ranking, existing TASK-001 discovery | No live paired run in this task; TASK-003 owns live rerun | PASS for this task's deterministic implementation; live proof pending TASK-003 |
| AC-003: no-network withholding and sent-outcome proof | In-memory `PullRequestFile` and source-map fixtures call `evaluate` | Focused lane passed: 3 tests, 0 failures | PASS for trial behavior; fixtures supply data directly rather than exercising `Api::trials` |

## Proposed finding

### TASK-B-1 — INCORRECT: import scoping changes rarity credit

- **Root cause / defect class:** `score` replaces the global declaring-file count with one when D identifies an import-bound module. This makes D's score formula different from C's, beyond D's permitted lookup restriction. The candidate-owned `score` rule implementation permits the defect; no spec-named public surface needs changing.
- **Location and siblings:** `crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs:747` computes `count = 1` for scoped D upstream lookups; line 754 applies that count to the upstream weight. All D upstream hits reached through `query.used` share this root cause. C and B use the intended count and are unaffected.
- **Violated obligation:** Spec revision 4 ranking table says D is “C, with the fixed explicit-import rule”; C prescribes upstream credit `1 / (df(name) × declaring_file_count(name))`. Scenario 2 and TASK-002 acceptance require approved scoring for B–D.
- **Reachable consequence:** With two base files declaring `helper`, `df(helper)=6`, and an explicit import uniquely identifying one, C gives each upstream declaring hit `1/12`. D should retain `1/12` for the permitted target, but the current code gives it `1/6`. That can change D's withheld score, relative rank, score distributions, and sent outcome under a budget. The existing Python and Rust test fixtures have exactly this ambiguous declaration setup yet assert only D rank, not D's formula.
- **Testable correction:** Preserve the C global `declaring_file_count(name)` denominator when D filters to the imported module. Add one focused assertion using an explicit import with at least two declaring base files, checking D's exact score and a competing downstream hit or rank; retain unresolved-import fallback and both-direction maximum credit.

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::` passed: 3 tests, 0 failures. The task packet records the broader lanes as passed, but I did not repeat them because the scoring defect is visible in the source and the focused lane does not assert its formula. No live GitHub run or paired reproduction reports exist yet; TASK-003 owns those artifacts.

Overall result: **FAIL** (TASK-B-1).

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":747,"snippet":"            let count = if module.is_some() { 1 } else { declaring.len() };","symbols":["score","Rule::Scoped"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":754,"snippet":"                    _ => 1.0 / (df * count) as f64,","symbols":["score","Rule::Scoped"]}]
```
