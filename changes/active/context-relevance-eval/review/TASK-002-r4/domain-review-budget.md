# Domain review: scoring, budget, and report correctness

Subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`; candidate `e2914e6d65a203519fefb47872040e1d2e454e20`.

## Boundary and coverage

Reviewed approved spec revision 4, TASK-002, R1–R3 remediation, the cumulative evaluator diff, `AGENTS.md`, `architecture/agent-rules.md`, and the symbolic-context reference. Traced `Api::qualify` and `Api::trials` into `Corpus::new`, `Change::new`, `Query::new`, A's public `rank_files` and `build_candidate_context`, B–D `score`, the common `outcome` packer, and JSON aggregation in `report.rs` and `main.rs`. The applicable obligations are REQ-004–REQ-006, INV-002–INV-004, AC-003–AC-005, and TASK-002 scenarios 1–3.

The index export defines the full candidate corpus; each indexed base path requires a blob and yields reference facts. A calls the production ranker and packer with the 100,000-token candidate allowance and production file cap. The separate common packer visits each variant's ranked files without a file cap, sends whole files that fit, then merged matched spans if the whole file does not fit. It counts UTF-8 bytes divided by four, rounded up, and records unranked, whole, spans-only, or omitted. The withheld patch is excluded from `diff_tokens`; the 4,096-token prompt reserve is reported separately. B counts each matched name once per file; C applies document frequency, the ceiling, declaring-file split, and maximum credit when both directions match; D scopes only resolved explicit bindings. Scores tie by canonical path. Aggregates include rank hits, sent shares, score distributions, non-test trials, and PR/path misses. Serialized trial and report fields contain no source text.

R1's declaring-file-count correction remains present in the shared C/D scorer and its focused regression. R2's resolved import scope remains in effect even when the resolved module lacks the declaration; its regression is present. R3 propagates a parsed patch/head analysis conflict as `TrialError::Symbolic`, before either A or B–D uses the change; the regression checks a range beyond verified head source. No new budget or scoring defect was found in the cumulative candidate.

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::` exited 0: seven tests passed, including A fidelity, B–D scoring and import scope, whole/spans/omitted/unranked packing, missing base source, and the R3 patch/head failure. The aggregate assertions in `report::tests::summaries_and_misses_follow_outcomes` were source-inspected; this reviewer did not rerun that focused test or the broad gate. Live GitHub sampling and checked-in repository reports belong to TASK-003 and were not exercised here. The fixture does not exhaust every exact 100,000-token boundary or multi-span rounding shape; those paths were source-traced.

## Proposed findings and result

No material finding. **PASS** for this domain boundary.

```citations
[]
```
