# Domain review: GitHub source trust and identity

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d..cd93bdadfa5cb17ed54752168e4a445c2e660068`
**Boundary:** provider identity and source checks through PR listing, co-change history, snapshot publication, and evaluation.
**Authority:** `changes/reviews/ranker-pipeline/intent.md` decisions 3 and 7; `architecture/bohmian-code-review-design.md:5,22,24`; `AGENTS.md` pipeline ownership; R1/R2/R3 remediation tasks.
**Result:** **PASS** for this domain.

## Coverage

Reviewed the cumulative provider changes in `github.rs`, `history.rs`, `snapshot.rs`, `update.rs`, and eval `live.rs`/`sample.rs`, including their direct callers and focused fake-provider tests. Repository discovery validates numeric ID and name; branch and commit reads validate their requested identifiers; blob reads verify Git hashes. The listed-PR conversion now retains `base.repo.id` and requires it to equal the requested `RepositoryIdentity` before returning either a closed or all-PR page. It permits a forked or deleted head repository. Closed pages feed the shared co-change reader, which completes each selected PR file listing before a snapshot may publish. All-PR pages feed evaluation discovery, whose subsequent base commits and head blobs are fetched through the same provider client.

## Prior-finding closure

- **FIND-ranker-pipeline-5: closed.** `ListedRefWire.repo` and `listed_pull_request` in `github.rs:750-902` enforce the requested base repository ID. Both listing methods call that conversion (`github.rs:1611,1706`), so absent or foreign IDs fail the whole page before history or eval sees it. The fake transport test covers absent, null, and foreign IDs for both methods, and accepts a matching base ID with a forked head. `snapshot::tests::cold_build_history_is_complete_or_unpublished` covers the publication sink on a foreign listing.
- **Related history sink:** The shared reader's successful result is required before cold build or update publication (`snapshot.rs:723-752`, `update.rs:120-124`). Its bounded, complete-or-fail and page re-read behavior is assessed by the separate history reviewer; this domain found no alternate provider path around it.

## Verification and limits

`cargo test -p bohmian-code-review-pipeline repository_lookup_and_pull_request_listing_are_verified --lib` and `cargo test -p bohmian-code-review-pipeline cold_build_history_is_complete_or_unpublished --lib` each passed (one test each). `git diff --check` for the cumulative range passed. No authenticated live GitHub request was available; the fake transport proves the trust decision for controlled responses, while the current source shows that both production listing methods use that same conversion. No material finding in this boundary.

```citations
[]
```
