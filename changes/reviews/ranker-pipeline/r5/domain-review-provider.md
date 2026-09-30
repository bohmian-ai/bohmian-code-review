# Domain review: provider trust and consumer pipeline

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d..9efafd309207c6cf5dd96b306ce8d22325281e5d`
**Boundary:** GitHub repository and PR identity, verified blob reads, source-free snapshot/history publication, evaluation provider access, and the consumer-facing `ReviewPipeline`.
**Authority:** `changes/reviews/ranker-pipeline/intent.md` decisions 3, 7, and 8; `architecture/references/domain/pipeline-consumer-ergonomics.md`; `AGENTS.md` product boundaries; `architecture/agent-rules.md`; R1–R4 remediation tasks.
**Result:** **PASS** for this domain.

## Coverage

I reviewed the cumulative range and traced `ReviewPipeline::context`, `refresh`, and `read` through the stage facades, the GitHub client, snapshot/history publication, and the eval crate's sampling reader. `ReviewPipeline` uses one object storage, one store implementing both persistence traits, one shared file ceiling, and a per-call client. Its cold-build decision checks for an absent pointer; `read` has no provider dependency. The eval crate's production source uses the pipeline `GitHubClient`, `Listing`, `History`, and `ColdBuild`; provider requests remain in the pipeline crate.

For `FIND-ranker-pipeline-5`, `listed_pull_request` requires the listed base repository ID to equal the requested `RepositoryIdentity`. Both closed and all-PR listings call it before returning a page. A forked or deleted head repository remains permitted. The single-PR reader independently checks its requested number and base repository, and blob reads compare the returned bytes with the requested Git blob SHA. Closed listings feed `CoChangeHistory::read`; cold build and update require its successful complete result before snapshot publication. All-PR listings feed eval discovery. No alternate production listing path bypasses the shared identity check.

## Verification and limits

The focused provider listing, cold publication, and update publication tests each passed: `cargo test -p bohmian-code-review-pipeline repository_lookup_and_pull_request_listing_are_verified --lib`, `cargo test -p bohmian-code-review-pipeline cold_build_history_is_complete_or_unpublished --lib`, and `cargo test -p bohmian-code-review-pipeline update_history_is_complete_or_unpublished --lib`. `git diff --check` for the cumulative range passed. No credentialed live GitHub request was available; the fake transport exercises controlled identity failures and the source trace establishes that production listing methods use the same conversion. The separate history reviewer owns the deeper pagination and prior-history completeness audit. I found no material issue in this boundary.

```citations
[]
```
