# Domain review: inference and adoption decision

**Subject:** `61d6df683b0cbe7aee34b819012e9c10c9f99802` → `b75079fa2c32710924a230c224eda8b55aeb456e`.
**Boundary:** PR-weighted paired hit@10 effects, fixed-seed percentile bootstrap, interpretation of intervals containing zero, and the repository-wide F adoption decision. **Overall: FAIL** on the proposed report interpretation finding below.

## Authority and source coverage

| Authority | Boundary checked | Result |
|---|---|---|
| `changes/active/cochange-eval/spec.md`, revision 2, Design and REQ-003 | Every PR counts equally; E − C and F − C use 10,000 seeded PR resamples; zero-containing intervals are inconclusive; F needs a five-point effect and positive lower bound in every repository | Inference and F decision pass; E interpretation fails |
| `changes/active/cochange-eval/tasks/TASK-003-reports.md`, Scenario 1 and acceptance criteria | Paired inference, report outcomes, and focused fake-provider checks | Inference passes; report interpretation fails |
| `AGENTS.md` and `architecture/agent-rules.md` | No production ranking change; report evidence must satisfy the approved task | No production-ranking change observed in this boundary |
| `architecture/references/languages/spec-driven-development.md` and `architecture/references/languages/testing-workflows.md` | Review against the approved revision and runnable focused evidence | Applied |

Inspected the complete base-to-candidate diff, `report.rs`, the report construction and invocation in `main.rs`, the existing `trial.rs` outcome and arm mapping, all three checked-in JSON reports, `mise.toml`, and the task's verification record. `pr_means` groups by PR and divides each arm's hit total by that PR's trial count. `effect` samples paired per-PR treatment-minus-C values 10,000 times from a fixed SplitMix64 seed and selects the 2.5% and 97.5% empirical endpoints. `run` computes all three F verdicts before setting `adopt_f` in each report. Those paths match the specified calculation and decision threshold.

## Verification

- `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features report::tests -- --nocapture`: **4 passed**. The first attempt omitted the example's `testing` feature and selected no tests; the corrected command above is the result used here.
- Independently recomputed every checked-in PR's C/E/F hit mean and the E − C/F − C mean from each JSON trial record with Python: all three reports match. The E − C intervals cross zero in `pydantic-ai.json` and `tokio.json`.
- The task records two byte-identical live report runs and passing `test:pipeline`, `fmt:check`, `lints`, and `gate`. I did not rerun credentialed GitHub work or independently replay all 10,000 bootstrap draws; the focused pinned-seed test passed.

## Proposed finding

**DOMAIN-INF-1 — MISSING; intervals that cross zero lack an explicit inconclusive outcome.**

- **Defect class/root cause:** Report construction interprets only F − C. `record` computes both treatment effects, but passes only `f_minus_c` to `verdict` and serializes only `repository_f_minus_c` under `decision`. The candidate-owned report shape has no place for E − C's interpretation.
- **Violated obligation:** The approved revision 2 decision rule says, “An interval containing 0 is reported as inconclusive, not as no effect”; REQ-003 requires both effects and the decision-rule outcome. TASK-003 Scenario 1 repeats the zero-containing interval requirement.
- **Sibling locations:** The same report construction feeds all three reports. The missing status is observable in `changes/active/cochange-eval/reports/pydantic-ai.json` (`E-C` CI `[-0.10266666666666666, 0.165]`) and `changes/active/cochange-eval/reports/tokio.json` (`E-C` CI `[-0.08999999999999998, 0.18166666666666664]`). Zod's E − C interval excludes zero, but its report uses the same shape.
- **Consequence:** The two inconclusive E outcomes are left as unlabeled numeric intervals despite the specification's reporting rule. The F adoption result remains correct.
- **Testable correction:** At the report owner, report an explicit interpretation for each treatment effect using the existing interval classification rule; preserve F's separate threshold-based adoption decision. A focused report check should assert that a zero-containing E − C interval is labeled inconclusive, then regenerate the three live artifacts and verify the rerun comparison.

```citations
[{"id":"DOMAIN-INF-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":569,"snippet":"    let verdict = report::verdict(&f_minus_c);","symbols":["record","verdict"]},{"id":"DOMAIN-INF-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":602,"snippet":"        \"effects\": {\"E-C\": e_minus_c, \"F-C\": f_minus_c},","symbols":["record","Effect"]},{"id":"DOMAIN-INF-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":606,"snippet":"            \"repository_f_minus_c\": verdict,","symbols":["record","Verdict"]}]
```
