# Domain review: budget safety and reporting

Subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `012aa47dbf2b7885d1e9ba25eb1b22c861113629`.

## Boundary and authority coverage

Reviewed approved spec revision 4, original TASK-002, both remediation tasks, the complete base-to-candidate diff, `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, and the symbolic-context reference. Traced the evaluator from `Api::qualify` patch totals through `Api::trials`, `evaluate`, `trial`, `outcome`, `snippet_tokens`, the public A ranker and production packer, and `report::summarize`/`misses` into the JSON record. The applicable contract is spec REQ-004–REQ-006, INV-002–INV-004, AC-003–AC-005, and TASK-002 scenario 3.

The common packer starts each A–D ranked list with 100,000 candidate tokens, visits files in rank order without a file cap, and uses merged matched spans when a whole file does not fit. Both whole and span counts use the specified UTF-8 byte estimate. A's separate `production_selected` observation calls the public production packer with its existing file cap and the same candidate allowance; it does not substitute that result for A's common `sent` outcome. `Trial::diff_tokens` removes the withheld patch from the listing total, while `PROMPT_RESERVE` is reported separately. Missing and malformed changed-file patches appear in the sample's omissions and source-file omissions appear in each trial. The report computes deterministic rank-hit shares, unranked/whole/spans shares, sorted score distributions, and PR/path misses over all and non-test trials. No report field contains fetched source bytes.

The prior budget-domain finding `FIND-TASK-002-1` remains closed: `score` uses the complete declaring-file count for scoped D upstream credit. The prior `FIND-TASK-002-3` also remains closed at the shared scorer: a uniquely bound indexed module remains the scope even when it lacks the name, so no unrelated declarer receives upstream credit. The scoped/unscoped counters follow that decision.

## Verification and limits

At candidate HEAD, `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::` exited 0 with six tests passed, including the whole/spans/omitted/unranked trial and both remediation regressions. The aggregate assertions in `report.rs` and source trace support report counts; this domain reviewer did not run the live GitHub evaluation, which belongs to TASK-003, or re-run the broader gate. The focused fixture does not exhaust every 100,000-token boundary or every multi-span shape; those are source-traced here.

## Proposed findings and result

No material finding. **PASS** for the reviewed budget and reporting boundary.

```citations
[]
```
