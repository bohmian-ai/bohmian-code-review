# TASK-002 cumulative task review, round 2

**Verdict: FIX_REQUIRED.** Immutable subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `0aa3a607e770d3271f872417b535d555360b4a20`. Authority: approved `changes/active/context-relevance-eval/spec.md` revision 4, original `tasks/TASK-002-counterfactual-trials.md`, and prior `review/TASK-002-r1/TASK-002-R1-correct-d-scoring-and-bindings.md`. The candidate remained at the pinned commit. Review covered the complete base-to-candidate diff; the only production-code changes are in the opt-in evaluator example and one `mise.toml` description.

## Acceptance matrix

| Obligation | Candidate evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004, AC-003: one modified file withheld, base candidate retained, other changed paths excluded, full indexed base corpus with missing-source failure | `trial.rs` `Corpus::new`, `Change::new`, `trial`, `evaluate`; verified blob retrieval in `main.rs` | No-network withholding and missing-base-source assertions; focused trial lane 5/5 | PASS |
| INV-002, AC-004: A uses public production ranking and separate production packing | `trial.rs` `rank_a` and `trial` call the public facade | Baseline equality and packing tests; focused trial lane 5/5 | PASS |
| REQ-004: B directional distinct-name lookup; C rarity, ambiguity, df ceiling and stable ties | `trial.rs` `Corpus`, `Query`, `score` | Variant scoring test and R1 denominator regression test | PASS |
| REQ-004, REQ-005, AC-003: D scopes uniquely bound upstream imports and otherwise uses C fallback | `trial.rs:743-761` | Wave 2 traced a reachable resolved binding discarded by the scorer; existing tests omit this case | **FAIL: FIND-TASK-002-3** |
| REQ-005, AC-003: whole and span costs, common 100,000-token sent outcomes, separate A production observation, patch and reserve counts, omissions | `trial.rs` `Outcome`, `outcome`, `trial`; `main.rs` report assembly | No-network whole, spans-only, omitted, unranked and malformed-patch test | PASS |
| REQ-006, INV-004: source-free trials, all/non-test aggregates, hit rates, scores and misses | `report.rs` summaries; `main.rs` serialization | Report test and trial source-text serialization assertion | PASS |
| INV-001, INV-005, AC-005 and non-goals: opt-in read-only evaluator without production, persisted-format, public-API, PR-context or gate changes | Cumulative diff and `mise.toml` | Diff inspection and clean `git diff --check` | PASS |
| INV-003: deterministic ordering for the same supplied sample | Ordered containers and score/path tie break in `trial.rs`; TASK-001 sample order | No-network ordering assertions; live paired rerun belongs to TASK-003 | PASS for TASK-002 scope |

## Review result

Wave 1 reports are preserved as `task-review-a.md` (**FAIL**, `TASK-A-1`), `task-review-b.md` (**PASS**), `standards-review.md` (**FAIL**, `STD-R2-1`), `domain-review-source.md` (**PASS**), and `domain-review-budget.md` (**FAIL**, `BUDGET-1`). All five required reviewers completed. The citation check exited zero; `citations.txt` rejects `BUDGET-1` and `STD-R2-1` for invalid candidate symbol citations, so neither enters the validated ledger. All `TASK-A-1` citations passed. There are no nonblocking notes.

Wave 2 independently confirmed one blocking task-contract defect, **FIND-TASK-002-3**: `score` erases an explicit, uniquely resolved D upstream module when it does not declare the queried name, then admits unrelated declarers through C fallback. A withheld unrelated file can acquire a false D rank, score and sent outcome. The full root-cause trace, sibling sweep, reproduction and minimum correction boundary are in `findings-validation.md`. The implementation task is `TASK-002-R2-preserve-resolved-import-scope.md` in this directory; route it to `$cr-implement`.

Prior `FIND-TASK-002-1` (D denominator) and `FIND-TASK-002-2` (same-line Python binding) are **CLOSED** by the shared scorer and bindings changes plus passing focused regressions. The new eligibility finding has a distinct root cause; neither prior ID is reopened. No R1 remediation code became redundant.

## Verification limits

This review reran `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::` (5 passed), `report::` (1 passed), `mise run gate` (exit 0), and `git diff --check` (exit 0). The confirmed gap was reproduced by a source path and concrete in-memory input, without adding a test or changing the reviewed code. No credentialed live evaluation or paired rerun was performed; TASK-003 owns those observations. There was no sub-reviewer timeout or missing required report.
