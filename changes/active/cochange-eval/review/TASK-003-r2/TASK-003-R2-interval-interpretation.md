---
id: TASK-003-R2
kind: remediation
status: proposed
spec: SPEC-cochange-eval
spec_revision: 2
requirements: [REQ-003, AC-001]
depends_on: [TASK-003-R1]
parent_task: TASK-003
remediates: [FIND-TASK-003-2]
---

# Report zero-containing E − C intervals as inconclusive

## Subject and diagnosis

Approved spec: `changes/active/cochange-eval/spec.md` revision 2. Original task: `changes/active/cochange-eval/tasks/TASK-003-reports.md`. Prior remediation: `changes/active/cochange-eval/review/TASK-003-r1/TASK-003-R1-rerun-command.md`. Review base: `61d6df683b0cbe7aee34b819012e9c10c9f99802`; cumulative candidate: `e8ab2ce1a41c869fe38606dc3ad246df25291e63`.

`FIND-TASK-003-2` is a report-shape defect. The shared producer at `crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs:567-569,602-607` computes both effects but applies `report::verdict` only to F − C; `effects.E-C` contains only `mean` and `ci_95`. Pydantic-ai and tokio have E − C intervals spanning zero (`reports/pydantic-ai.json:17-27`, `reports/tokio.json:17-27`), yet neither says **inconclusive**. Zod uses the same shape, though its E interval excludes zero. The no-network report assertion at `main.rs:939-949` checks E's numeric fields but no interpretation. Thus the candidate and its proof miss the spec's instruction that an interval containing zero be reported as inconclusive (`spec.md:27-28`), as applied to both reported effects by REQ-003 and AC-001. Readers must infer the two E outcomes themselves.

## Correction outcome and boundary

At the existing report owner, expose the interval-zero interpretation already used by F's verdict and report it explicitly for E − C. Use the same inclusive zero comparison on the 95% interval. Keep F's five-point threshold and repository-wide adoption decision exclusive to F; do not apply that threshold to E. The output must represent E as inconclusive when its interval contains zero and not inconclusive when it excludes zero. The correction belongs to the candidate-owned report/inference surface and its checked-in outputs, with no spec or public API decision.

The write set is `crates/bohmian-code-review-pipeline/examples/context_relevance/{main,report}.rs` and `changes/active/cochange-eval/reports/{pydantic-ai,tokio,zod}.json`. If the existing interval predicate can be reused without changing `report.rs`, leave that file untouched. Change only interpretation metadata in the reports; preserve every sample, trial, rank, mean, interval, cost, validity threat, rerun command, and F decision value. No provider or production behavior, persistence, public API, gate, historical report, extra outcome, or new dependency belongs in this correction.

## Acceptance and proof

- `FIND-TASK-003-2` closes at the shared producer and all three report siblings: pydantic-ai and tokio explicitly report E − C as inconclusive; zod explicitly reports its E − C interval as not inconclusive. F's current decision and `adopt_f` stay unchanged.
- A focused no-network report check exercises both a zero-containing E − C interval and an interval excluding zero, and confirms F-only adoption. The check must fail against the current candidate and pass after correction.
- Compare the corrected reports with the candidate artifacts after removing only the new interpretation field to show all numeric and existing metadata is unchanged. Run the exact focused example test through `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features tests::run_report_is_complete_and_source_free -- --exact`, confirming it selects one test. Then run `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check`.

Route this task directly to `$cr-implement`. A later review reassesses the full cumulative candidate against the original TASK-003.
