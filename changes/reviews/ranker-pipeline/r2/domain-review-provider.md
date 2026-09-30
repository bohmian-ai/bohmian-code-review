# Domain review: provider trust and consumer pipeline

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d..efc381f776e397ace4f43f9f4ab45fc4187436a5`
**Boundary:** Pipeline-owned GitHub reads and identity verification; eval reads through pipeline types; `ReviewPipeline` snapshot/context publication and read-back.
**Authority:** `changes/reviews/ranker-pipeline/intent.md` decisions 3, 7, 8; `architecture/references/domain/pipeline-consumer-ergonomics.md`; `AGENTS.md:23`; `architecture/bohmian-code-review-design.md:22,24`; prior `FIND-ranker-pipeline-3`.
**Result:** **FAIL**.

## Coverage and limits

Reviewed the cumulative candidate diff and current source for `github.rs`, `history.rs`, `pull_request.rs`, `review.rs`, and eval `live.rs`, plus existing single-PR identity validation and focused tests. The new eval `Api` holds only `GitHubClient`; discovery, listing, base commit, blobs, and snapshots route through pipeline types, closing prior `FIND-ranker-pipeline-3`'s direct-HTTP class. `ReviewPipeline` uses one storage/store, shares `max_file_bytes`, accepts the client per call, cold-builds only on an absent pointer, wraps update for refresh, and reads contexts without a client. The history reader now fails an unproved page bound and rereads full prior pages. No live authenticated run was available; provider behavior was traced through the fake-transport tests and source. I did not modify source.

## Proposed finding

### DOMAIN-PROVIDER-1 — listed PRs lose base repository identity

- **Classification:** VIOLATION; source-trust boundary. **Defect class:** the new listed-PR wire type omits the base repository id, so its shared conversion cannot verify that the returned PR belongs to the requested repository. The candidate-owned response shape permits the defect.
- **Owner and siblings:** `github.rs:749-753` (`ListedRefWire`) is the first loss of identity. `listed_pull_request` at `github.rs:863-899` validates number/time but has no repository identity input. Both `closed_pull_requests` at `github.rs:1588-1613` and `pull_requests` at `github.rs:1686-1691` consume it without comparing a base repository id. Existing `pull_request` at `github.rs:1476-1482` shows the same provider boundary already checks that id for a single PR.
- **Obligation:** `AGENTS.md:23` and `architecture/bohmian-code-review-design.md:22` assign Git identity verification to the pipeline. Decision 7 requires source-free history of PRs merged into *the repository's* default branch; R1 `FIND-ranker-pipeline-3` specifically moved eval listing into the pipeline to retain its identity policy.
- **Evidence and reachable consequence:** A 200 listing for `/repos/A/pulls` whose entry has the requested base branch and valid times/SHAs but `base.repo.id = B` is accepted because that field is never decoded. The closed listing feeds `CoChangeHistory::read` at `history.rs:408-416`, then snapshot/update publication, so foreign changed paths can affect F ranking. The all-PR listing feeds `Api::select` at `live.rs:277-290`, allowing a foreign PR into the eval sample. The existing fake listing test omits `base.repo` entirely and expects success, so it cannot catch this violation.
- **Testable correction:** Reuse the existing `PullRepoWire` identity field in the listed-PR response and reject each entry whose base repository id differs from the requested `RepositoryIdentity` before either page is returned. Cover both listing operations with a fake response naming another id; retain the current number/time/branch checks. This closes both sibling sinks at the common provider owner.

```citations
[{"id":"DOMAIN-PROVIDER-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":749,"snippet":"struct ListedRefWire {","symbols":["ListedRefWire","listed_pull_request"]},{"id":"DOMAIN-PROVIDER-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1596,"snippet":"            let pr = listed_pull_request(wire, &resource)?;","symbols":["closed_pull_requests","listed_pull_request"]},{"id":"DOMAIN-PROVIDER-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1689,"snippet":"            .map(|wire| listed_pull_request(wire, &resource))","symbols":["pull_requests","listed_pull_request"]},{"id":"DOMAIN-PROVIDER-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1476,"snippet":"        let base_repo = wire.base.repo.as_ref().map(|repo| repo.id);","symbols":["pull_request"]},{"id":"DOMAIN-PROVIDER-1","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":408,"snippet":"            let page = client","symbols":["CoChangeHistory::read"]},{"id":"DOMAIN-PROVIDER-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":278,"snippet":"            let listed = self","symbols":["Api::select"]}]
```
