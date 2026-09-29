# TASK-003 Wave 2 findings validation

## Subject and method

- Base `61d6df683b0cbe7aee34b819012e9c10c9f99802`; immutable candidate `e8ab2ce1a41c869fe38606dc3ad246df25291e63`.
- Authority: approved `changes/active/cochange-eval/spec.md` revision 2, especially Design, REQ-003, and AC-001; original `tasks/TASK-003-reports.md` and prior `TASK-003-R1-rerun-command.md`.
- Independently traced `print → run → record`, every `record`, `effect`, and `verdict` caller, the effect producers and serializer, the full cumulative code diff, and all three checked-in reports. The surviving citation-checked claim ledger contains only `DOMAIN-INF-1`. No Wave 1 prose, matrices, or intended result was used.

## Validated blocking ledger

### FIND-TASK-003-2 — E − C zero-containing intervals have no inconclusive report outcome

- **Source / status / gate / classification:** `DOMAIN-INF-1`; `CONFIRMED`; task contract; `INCORRECT`. This is distinct from the prior rerun-command defect.
- **Root cause / defect class:** The candidate-owned report shape serializes both effects but gives only F − C an interpretation. `record` computes E − C and F − C at `crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs:567-568`, passes only F − C to `report::verdict` at `:569`, and writes only `repository_f_minus_c` at `:603-607`. The E − C effect has `mean` and `ci_95` only (`report.rs:49-56`). Its inconclusive result is therefore unrepresentable in the output.
- **Siblings:** The shared `record` path writes all three reports. E − C contains zero in `changes/active/cochange-eval/reports/pydantic-ai.json:17-27` (CI `[-0.1027, 0.165]`) and `tokio.json:17-27` (CI `[-0.09, 0.1817]`); `zod.json:17-27` excludes zero and is the negative case. The no-network completeness assertion at `main.rs:939-949` checks only numeric E − C fields and F − C's verdict, preserving the omission. F − C's current outcome is present in all reports and does not need a second adoption rule.
- **Obligation:** The approved spec's Design says **for E − C and F − C** to report effects and confidence intervals and says an interval containing zero **is reported as inconclusive**, not as no effect (`spec.md:27-28`). REQ-003 requires both effects, intervals, and the decision-rule outcome (`:70-76`); AC-001 requires the three checked-in reports to satisfy it (`:101`). Scenario 1 of the original task repeats the zero-containing interval requirement. The adoption decision remains F-only.
- **Reproduction / consequence:** `print` emits `run`'s records (`main.rs:616-626`), `run` calls `record` for every repository (`:510-532`), and `record` writes the quoted shape. `jq` inspection of the three checked-in reports shows E − C has only `ci_95,mean` while pydantic-ai and tokio straddle zero; each `decision` has only `adopt_f,repository_f_minus_c,rule,threshold`. A reader gets the numbers but no reported **inconclusive** outcome for those two required effects, so two checked-in reports miss the explicit interpretation contract.
- **Smallest safe correction:** Keep the existing `Effect`, `report::verdict`, and F-only adoption logic. At the report owner, expose one interval-contains-zero predicate (reusing the existing comparison in `verdict`) and serialize an explicit E − C interval interpretation, such as `"inconclusive": true|false` inside `effects.E-C`; do not apply F's five-point adoption threshold to E. Update the three report metadata fields without changing their measured sample, trials, means, intervals, costs, or F decision. No new public API, dependency, run mode, or live provider call is needed.
- **Focused closure proof:** A no-network report test must assert an E − C interval spanning zero produces the explicit inconclusive value and one excluding zero does not, while F − C adoption still depends only on F. Assert the two checked-in zero-spanning reports say inconclusive and zod does not. Compare all other report fields with the candidate artifacts to prove only interpretation metadata changed, then run the existing pipeline, formatting, lint, and gate lanes.

## Prior finding, rejections, and limits

- `FIND-TASK-003-1` is closed. The single producer and all three reports now say `mise run live:context-relevance` (`main.rs:574`; each report line 8), and the report test asserts it (`main.rs:916`). The R1 correction remains scoped to command metadata; it does not cause this separate interpretation defect.
- No claim was rejected or downgraded to a note. No prior remediation code was made redundant by this finding.
- Validation used source tracing and checked-in report inspection. It did not rerun the credentialed live experiment; unchanged numeric artifacts and the existing reproducibility evidence are the limit of this review.

**Overall validation: one confirmed bounded task-contract finding; `FIX_REQUIRED`.**
