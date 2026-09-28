# TASK-002 cumulative task review, round 4

**Verdict: FIX_REQUIRED.** Immutable subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `e2914e6d65a203519fefb47872040e1d2e454e20`. Authority: approved `changes/active/context-relevance-eval/spec.md` revision 4, original `tasks/TASK-002-counterfactual-trials.md`, and R1–R3 remediation tasks. The cumulative diff changes the opt-in evaluator example, task/review evidence, and one live-lane description in `mise.toml`; production crates, public APIs, and the gate are unchanged. The candidate remained at the pinned commit.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004, AC-003: withhold one modified indexed file, retain its base version, exclude remaining changed paths, use the complete indexed base corpus, and fail on missing base source | `trial.rs` `Corpus::new`, `Change::new`, `evaluate`; `main.rs` verified blob fetch | No-network withholding and missing-source assertions; focused `trial::` 7 passed | PASS |
| REQ-004, INV-002, AC-004: A uses public production ranking and distinct production packing | `trial.rs` `rank_a` and `trial` | Baseline equality and packing assertions; focused `trial::` 7 passed | PASS |
| REQ-004–005: B directional names, C rarity and ambiguity, D explicit import scope and fallback | `trial.rs` `Query`, `bindings`, `score` | Variant and R1/R2 regressions passed | PASS |
| REQ-002, REQ-004, INV-002–003: parsed patch coordinates agree with verified base and head source or fail the run | `trial.rs:489-623` | R3 head mismatch regression passed; Wave 2 reproduced an unchecked old-side coordinate path | **FAIL: FIND-TASK-002-5** |
| REQ-005, AC-003: rank, names, directions, token counts, whole/spans/omitted outcomes, patch and reserve counts, and separate A production result | `trial.rs` `Outcome`, `outcome`, `trial`; `main.rs` report assembly | No-network packing assertions; focused `trial::` 7 passed | PASS |
| REQ-006, INV-004: source-free trials, aggregates, score distributions, and misses | `report.rs`, `main.rs` serialization | `report::` 1 passed and source-text serialization assertion | PASS |
| INV-001, INV-005, AC-005 and non-goals: opt-in read-only evaluator; no production, public API, PR-context, replay, or gate change | Complete base-to-candidate diff | Boundary inspection and cumulative `git diff --check` passed | PASS |
| INV-003: deterministic ordering on consistent supplied input | Ordered candidates and report output | No-network ordering assertions; live paired rerun is TASK-003 scope | PASS for TASK-002 scope |

## Review result

Wave 1: `task-review-a.md` **PASS**, `task-review-b.md` **FAIL** (`TASK-B-1`), `standards-review.md` **PASS**, `domain-review-source.md` **PASS**, and `domain-review-budget.md` **PASS**. All required reports arrived. The mandatory citation check exited zero and accepted both `TASK-B-1` citations; no citations were rejected. Wave 2 independently traced and reproduced the claim, confirming **FIND-TASK-002-5** under the task-contract and written repository-rule gates. The validated ledger, sibling sweep, consequence, and correction are in `findings-validation.md`. There are no notes.

Prior `FIND-TASK-002-1` (D denominator), `FIND-TASK-002-2` (same-line Python bindings), `FIND-TASK-002-3` (resolved import scope), and `FIND-TASK-002-4` (post-parse head-analysis failure) remain **CLOSED** by source tracing and passing focused regressions. Finding 5 concerns absent old-side validation against the base blob; finding 4 corrected a different error-classification path after head analysis. No earlier remediation code became redundant.

## Verification limits

Reviewers reran the evaluator `trial::` lane (7 passed), `report::` (1 passed), the R3 exact regression (1 passed), and the cumulative whitespace check (exit 0). The R3 task record reports the required pipeline, journey, format, lint, and full-gate lanes passing; this review did not repeat those broad lanes. The old-side mismatch was reproduced by a concrete source path without editing the immutable candidate. No credentialed live GitHub run was performed; TASK-003 owns the paired live run. There was no sub-reviewer timeout or missing required report.

Remediation: `TASK-002-R4-validate-base-side-patch.md` in this directory, covering **FIND-TASK-002-5**. Route it directly to `$cr-implement`.
