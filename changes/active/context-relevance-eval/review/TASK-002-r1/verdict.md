# TASK-002 review verdict

**Verdict: FIX_REQUIRED.** Subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `e3d194de32f456f5136a214aaf7d8087483e7013`; approved `changes/active/context-relevance-eval/spec.md` revision 4 and original `tasks/TASK-002-counterfactual-trials.md`. The candidate remained at the specified commit. The complete five-file base-to-candidate diff was reviewed.

## Acceptance matrix

| Obligation | Candidate evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004, AC-003: one withheld modified file remains a base candidate; remaining changed paths excluded; every indexed base file considered | `trial.rs` `Corpus::new`, `Change::new`, `excluded`, `evaluate` | No-network withholding test and missing-base-blob assertion; focused trial lane passed 3 tests | PASS |
| INV-002, AC-004: A ranks and packs through the unmodified public facade, separate from the common experimental result | `trial.rs` `rank_a`, `trial`, `outcome` | Baseline equality test and common-packing test | PASS |
| REQ-004: B directional distinct-name lookup and C document frequency, ceiling, weights, stable ties | `trial.rs` `Corpus::new`, `Query`, `score` | Directional/rarity test checks C score and ranks | PASS |
| REQ-004, REQ-005: D retains C's credit while scoping only verified explicit bindings, otherwise falling back | `trial.rs` `bindings`, `score` | Independent source-path reproduction; existing test does not cover exact D score or same-line imports | **FAIL: FIND-TASK-002-1, FIND-TASK-002-2** |
| REQ-005: rank, names, direction, whole/snippet counts, common sent outcome, separate A production selection, patch and prompt accounting | `trial.rs` `Outcome`, `snippet_tokens`, `outcome`, `trial`; `main.rs` report assembly | No-network packing test covers whole, spans-only, omitted, unranked, malformed patch | PASS |
| REQ-006, INV-004: source-free trials and all/non-test summaries, scores, rates, misses | `trial.rs` serializable records; `report.rs` summaries; `main.rs` JSON | Report test and source-text serialization assertion | PASS |
| INV-001, INV-005, AC-005: opt-in, read-only evaluator; production crates, gate, persistence and PR-context workflow unchanged | Diff touches evaluator example, task evidence, and mise description only | Diff inspection and `git diff --check` | PASS |
| INV-003: deterministic per-trial ordering with fixed input | Ordered corpus/score/report containers | Source inspection; live paired rerun and checked-in reports belong to TASK-003 | PASS for TASK-002 scope |

## Review waves and citation check

Wave 1: task A **FAIL** (`TASK-A-1`, `TASK-A-2`); task B **FAIL** (`TASK-B-1`); standards **PASS** (no written-rule findings); source domain **FAIL** (`DOMAIN-SOURCE-1`); budget domain **FAIL** (`DOMAIN-BUDGET-1`). Each required report is present. The citation check exited zero, but rejected one citation for `TASK-A-1` because `Change::omit` is not a symbol in candidate `trial.rs`; per the review contract, the entire claim was dropped. The exact check output is `citations.txt`. All citations for the retained claims passed.

Wave 2 independently reproduced two blocking root causes through the public `evaluate` path and approved formula/import rule. Its complete ledger, evidence, sibling sweep, and correction boundaries are in `findings-validation.md`:

| Finding | Wave 1 sources | Blocking consequence |
|---|---|---|
| **FIND-TASK-002-1** | `TASK-A-2`, `TASK-B-1`, `DOMAIN-BUDGET-1` | D uses one instead of the full base declaring-file count, inflating scoped upstream scores and possibly changing rank or sent result. |
| **FIND-TASK-002-2** | `DOMAIN-SOURCE-1` | Same-line Python imports can bind a reference to the wrong module, changing D upstream and downstream candidate selection. |

No nonblocking notes or prior `FIND-TASK-002-*` remediation IDs exist. No approved spec revision is needed: both corrections stay in the private evaluator. The decision-complete implementation task is `TASK-002-R1-correct-d-scoring-and-bindings.md` in this directory; route it to `$cr-implement`.

## Verification limits

The reviewers ran focused no-network trial tests and inspected the complete source paths. The candidate task packet records passing pipeline, public journeys, format, lints, diff check, and gate; these broad lanes were not rerun for this review. Wave 2 reproduction used traced source paths and hand-computed examples, with no new executable test or live GitHub run. TASK-003 owns the live paired rerun and checked-in reports. All required independent reviewers completed; there is no sub-reviewer timeout gap.
