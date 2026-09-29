# Domain review: co-change history and snapshot publication

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d..cd93bdadfa5cb17ed54752168e4a445c2e660068`
**Result:** PASS

## Boundary and authority

Reviewed intent decision 7, `architecture/bohmian-code-review-design.md:7,22,40`, `architecture/agent-rules.md:7`, the pipeline consumer reference, and R1/R2/R3 `FIND-ranker-pipeline-2`. Traced the full cumulative history path: provider closed-PR and file pages; `Listing::complete` and mass-edit exclusion; `CoChangeHistory::read`; direct and fallback `Update::run`; cold build, snapshot publication and read-back; `PrContext::review_history`; and public `ReviewPipeline::refresh` and `context`. Inspected the corresponding tests and the candidate's R3 delta. No other r4 review conclusions were used.

The shared reader now stops a cold read after ten proved non-mass-edit PRs and rejects an incremental read that reaches the ten-page bound with `has_next` before reaching the prior time or true listing exhaustion. Every accepted update appends prior history and caps it at 1,000 entries. Earlier listing pages are re-read in full, including unmerged entries. Incomplete or rejected PR-file listings fail before a history entry can be persisted. `ColdBuild::build` and `Update::run` consume the reader's `Result` before any snapshot publication. PR context filters persisted entries by strict base-commit time and excludes its own PR. The same-tree reuse rule remains explicit.

## Verification and limits

Focused commands all passed (one test each):

- `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib history::tests::update_with_ten_new_prs_at_the_page_bound_fails -- --exact`
- `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib update::tests::update_history_is_complete_or_unpublished -- --exact`
- `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib snapshot::tests::cold_build_history_is_complete_or_unpublished -- --exact`
- `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib history::tests::entry_moving_below_the_first_fails -- --exact`

The provider race proof uses routed fake responses. No credentialed live GitHub race was run. This review did not assess ranker formulas, fact validation, provider base-repository identity, or evaluation report identity; those belong to other domains.

## Proposed findings

None. `FIND-ranker-pipeline-2` is closed for this boundary. Its former ten-qualified-PR counterexample now returns `UnprovedHistory`, and both cold and update publication tests retain the prior pointer on failure.

```citations
[]
```
