# Domain review: paired inference and report decision

**Subject:** `61d6df683b0cbe7aee34b819012e9c10c9f99802` → `e8ab2ce1a41c869fe38606dc3ad246df25291e63`. **Boundary:** per-PR hit@10 means, paired E − C and F − C effects, seeded percentile intervals, interval interpretation, and the repository-wide F decision. **Overall: FAIL** on the report interpretation finding below.

## Authority and coverage

| Authority | Boundary checked | Result |
|---|---|---|
| `changes/active/cochange-eval/spec.md` revision 2, Design, REQ-003, INV-002, AC-003 | Equal PR weighting, 10,000 seeded PR resamples, both effects, inconclusive intervals, and adoption only when every F effect meets the threshold | Calculation and F decision pass; E interval interpretation fails |
| `changes/active/cochange-eval/tasks/TASK-003-reports.md`, Scenarios 1–2 | Inference and report outcomes | Report interpretation fails |
| `changes/active/cochange-eval/review/TASK-003-r1/TASK-003-R1-rerun-command.md` | Prior command correction in cumulative candidate | Report producer now records the executable `mise run live:context-relevance` command |

Inspected the complete base-to-candidate source diff, `report.rs`, `main.rs` report construction and invocation, `trial.rs` outcome production and arm mapping, and all three checked-in JSON reports. `pr_means` groups trials by PR before division; `effect` resamples paired treatment-minus-C PR differences 10,000 times with the recorded fixed seed and takes the empirical 2.5% and 97.5% endpoints. `run` gathers every repository verdict before setting the same `adopt_f` value in each report. No statistical calculation defect was found.

## Verification and limits

Independently recomputed every checked-in PR's C/E/F hit mean and each repository's E − C and F − C mean directly from the JSON trial records: all match. The E − C intervals include zero for pydantic-ai `[-0.10266666666666666, 0.165]` and tokio `[-0.08999999999999998, 0.18166666666666664]`; zod's E interval excludes zero. Source tracing confirms the F threshold and all-repository decision. The task records two byte-identical live outputs and passing focused and gate checks. I did not rerun credentialed GitHub work, Cargo checks, or independently replay all bootstrap draws; the source, pinned-seed test, and recorded live evidence cover those paths.

## Proposed finding

**DOMAIN-INF-1 — MISSING: zero-containing E − C intervals are not reported as inconclusive.**

- **Root cause and defect class:** `record` interprets only `f_minus_c` through `verdict` and writes only `repository_f_minus_c` under `decision`. Its candidate-owned report shape leaves E − C as a raw interval with no outcome label. The report producer is the first owner of this omission.
- **Violated obligation:** The approved spec's decision rule explicitly says an interval containing zero is *reported as inconclusive*, and REQ-003 requires both treatment effects and the decision-rule outcome. TASK-003 Scenario 1 repeats the zero-containing interval requirement.
- **Sibling sweep:** The same `record` path writes all three reports. Pydantic-ai and tokio have zero-containing E intervals without an `inconclusive` E outcome. Zod uses the same report shape, though its E interval excludes zero. F's interval interpretation and adoption are present and correct.
- **Observable consequence:** Readers get a numeric E interval and a decision field labeled only for F, so the two E results that the spec calls inconclusive are never stated as such. The reported F adoption remains correct.
- **Testable correction:** At the existing report owner, state the interval interpretation for both treatment effects while preserving F's separate threshold and repository-wide adoption decision. A focused no-network report check should assert an explicit inconclusive E outcome for a zero-containing interval; checked-in reports should carry the corresponding interpretation.

```citations
[{"id":"DOMAIN-INF-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":569,"snippet":"    let verdict = report::verdict(&f_minus_c);","symbols":["record"]},{"id":"DOMAIN-INF-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":602,"snippet":"        \"effects\": {\"E-C\": e_minus_c, \"F-C\": f_minus_c},","symbols":["record"]},{"id":"DOMAIN-INF-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":606,"snippet":"            \"repository_f_minus_c\": verdict,","symbols":["record"]}]
```
