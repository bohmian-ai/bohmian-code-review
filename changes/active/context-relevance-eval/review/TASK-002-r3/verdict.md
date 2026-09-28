# TASK-002 cumulative task review, round 3

**Verdict: FIX_REQUIRED.** Immutable subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `012aa47dbf2b7885d1e9ba25eb1b22c861113629`. Authority: approved `changes/active/context-relevance-eval/spec.md` revision 4, original `tasks/TASK-002-counterfactual-trials.md`, and R1/R2 remediation tasks. `HEAD` remained at the pinned candidate throughout review. The cumulative diff changes the opt-in evaluator example, one `mise.toml` description, and task/review evidence; it does not change production crates or the gate.

## Acceptance matrix

| Obligation | Candidate evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004, AC-003: withhold one modified indexed file, retain its base version, exclude remaining changed paths, use full indexed base corpus and fail on missing base source | `trial.rs` `Corpus::new`, `Change::new`, `trial`, `evaluate`; verified blob fetch in `main.rs` | No-network withholding and missing-source assertions; trial lane 6/6 | PASS |
| REQ-004, INV-002, AC-004: A uses public production ranking and separate production packing | `trial.rs` `rank_a` and `trial` call the public facade | Baseline equality and packing tests; trial lane 6/6 | PASS |
| REQ-004, REQ-005: B directional names; C rarity, ambiguity, and ties; D explicit import scoping and fallback | `trial.rs` `Corpus`, `Query`, `bindings`, `score` | Variant test and focused R1/R2 regressions; trial lane 6/6 | PASS |
| REQ-002, REQ-004, INV-002–003: parsed patches and verified heads agree or fail the run; only authorized missing/malformed patches become omissions | `trial.rs:557-596`, `evaluate` | Wave 2 source-path reproduction of a parsed patch beyond the verified head; existing tests omit this case | **FAIL: FIND-TASK-002-4** |
| REQ-005, AC-003: whole/spans/omitted outcomes, 100,000-token common allowance, A production observation, patch and reserve counts | `trial.rs` `Outcome`, `outcome`, `trial`; `main.rs` report assembly | No-network packing test; trial lane 6/6 | PASS |
| REQ-006, INV-004: source-free trial, aggregates, distributions, and misses | `report.rs`, `main.rs` serialization | Report test 1/1, source-text serialization assertion | PASS |
| INV-001, INV-005, AC-005 and non-goals: opt-in read-only evaluator, no production/public API/PR-context/replay/gate change | Cumulative diff and `mise.toml` | Diff inspection, `git diff --check`, `mise run gate` | PASS |
| INV-003: deterministic results on the same supplied sample | Ordered candidate and report output; TASK-001 sample ordering | No-network ordering assertions; live paired rerun is TASK-003 scope | PASS for TASK-002 scope |

## Review result

Wave 1 reports: `task-review-a.md` **FAIL** (`TASK-A-1` proposed), `task-review-b.md` **PASS**, `standards-review.md` **PASS**, `domain-review-source.md` **FAIL** (`SOURCE-1`), and `domain-review-budget.md` **PASS**. All required reviewers completed. The citation check exited zero: `SOURCE-1` citations passed, while every `TASK-A-1` citation was `REJECT` because its snippet did not match the candidate; that claim was dropped. Wave 2 independently confirmed **FIND-TASK-002-4** from `SOURCE-1` under the task-contract and written repository-rule gates. Its source path, sibling sweep, consequence, and correction are in `findings-validation.md`. There are no notes.

Prior `FIND-TASK-002-1` (D denominator), `FIND-TASK-002-2` (same-line Python bindings), and `FIND-TASK-002-3` (resolved import scope) are **CLOSED** by source tracing and passing focused regressions. No prior remediation code became redundant. The new failure-classification defect has a separate root cause.

## Verification limits

This review reran `trial::` (6 passed), `report::` (1 passed), `mise run gate` (exit 0), and `git diff --check` (exit 0). The standards reviewer ran all four original named tests with exact selectors (one test each, all passed); Wave 2 ran the exact `changed_lines_reject_invalid_and_deletion_only_input` component test (passed). The new gap was reproduced by a traced source path and a concrete parsed-patch/head mismatch, without adding a test or changing reviewed source. No credentialed live run was performed; TASK-003 owns it. There was no sub-reviewer timeout or missing required report.

Remediation: `TASK-002-R3-fail-inconsistent-head-analysis.md` in this directory, covering **FIND-TASK-002-4**. Route it to `$cr-implement`.
