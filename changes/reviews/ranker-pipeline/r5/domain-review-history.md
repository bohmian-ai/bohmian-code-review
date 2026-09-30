# Domain review: co-change history and snapshot publication

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d..9efafd309207c6cf5dd96b306ce8d22325281e5d`
**Result:** PASS

## Boundary and authority

Reviewed decision 7 in `changes/reviews/ranker-pipeline/intent.md`, the persisted-history and pipeline-journey contracts in `architecture/bohmian-code-review-design.md:7,22,40`, the IO and complete-history rules in `AGENTS.md` and `architecture/agent-rules.md`, the pipeline consumer reference, and the R1–R3 `FIND-ranker-pipeline-2` remediation. The R4 typed-evaluation-error change does not alter this boundary.

Traced the cumulative path from `GitHubClient::closed_pull_requests` and complete PR-file listings through `CoChangeHistory::read`, cold build, direct and fallback update, snapshot serialization and pointer publication, public `ReviewPipeline::refresh`/`context`, and PR-base-time filtering in `PrContext::review_history`. The reader proves ten eligible PRs for a cold build or the prior boundary/listing end for an update, fails on an unproved ten-page cap, and re-reads earlier pages in full. Incomplete/rejected file listings fail before publication; mass edits are dropped only after complete reads. Accepted updates append prior history and retain at most 1,000 entries. History is source-free; review ranking excludes the PR itself and mergers at or after its base commit time. The explicit same-tree path reuses its prior history.

## Verification and limits

The following exact focused commands each exited 0 and selected one test:

- `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib history::tests::update_with_ten_new_prs_at_the_page_bound_fails -- --exact`
- `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib update::tests::update_history_is_complete_or_unpublished -- --exact`
- `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib history::tests::entry_moving_below_the_first_fails -- --exact`
- `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib pull_request::tests::persisted_history_ranks_only_earlier_merges_but_its_own -- --exact`

The page-movement proof uses a routed fake provider; no credentialed live GitHub race was run. This domain review does not assess ranker formulas, symbolic fact validation, or evaluation error types.

## Proposed findings

None. `FIND-ranker-pipeline-2` remains closed at the history boundary; the previously reachable unjoined incremental page cap now returns `UnprovedHistory` before either publication path.

```citations
[]
```
