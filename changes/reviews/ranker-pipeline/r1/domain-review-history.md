# Domain review: co-change history and snapshot boundary

Subject: `322a2b9e0984062625d54274798a9c32cb01142d` through `100249c3031d645ce8db9148b4b6fa43e71481d1`.

## Boundary and coverage

Reviewed decision 7 in `changes/reviews/ranker-pipeline/intent.md:13` and the history contract in `architecture/bohmian-code-review-design.md:22`. Traced closed-PR discovery and PR-file listings through `github.rs` and `history.rs`, cold build and update through `snapshot.rs` and `update.rs`, snapshot publication and store CAS through `snapshot.rs` and `persistence.rs`, and review-time filtering through `pull_request.rs`. Also inspected the history and update tests. The complete candidate gate passed according to the orchestrator; these two cases are not exercised by it. No source was edited.

## Proposed findings

### DOMAIN-HISTORY-1 — incomplete discovery is accepted as complete

- **Classification:** INCORRECT; task contract and durability boundary.
- **Defect class/root cause:** `CoChangeHistory::read` turns exhaustion of the bounded closed-PR discovery pages into a successful history, even when the provider advertises more pages and the required history window has not been established. The owner's `joined` flag only controls whether prior entries are appended; it does not distinguish proved completion from an exhausted bound.
- **Owner and siblings:** `history.rs:385-386` is the decision; `history.rs:413-417` persists its incomplete result. Both `ColdBuild::history` (`snapshot.rs:739-752`) and `Update::run` (`update.rs:120-124`) call this owner. Thus cold build can record fewer than the ten most recent eligible PRs, and update can discard prior history or omit PRs merged since it, with the same root cause.
- **Violated obligation:** Decision 7 requires bounded, complete-or-fail co-change history and an update that extends history with PRs merged since the prior snapshot. The design says cold build reads the ten most recent eligible PRs within the page bound and update reads every newly merged PR (`architecture/bohmian-code-review-design.md:22`).
- **Evidence and consequence:** At page 10 with `has_next = true`, no threshold reached, and only nine eligible PRs found, the loop breaks `false`; the function returns `Ok(history)` and `ColdBuild::complete` can publish it. The existing `empty_and_page_bounded_history_is_valid` test (`history.rs:664-711`) explicitly expects success when all ten pages have `has_next = true` and never prove the prior boundary. A review then ranks over silently missing co-change data.
- **Testable correction:** Keep the ten-page bound, but return a history error when that bound is hit before the cold depth or the prior snapshot boundary is proved. Preserve valid empty history only when the provider proves there are no eligible earlier PRs. Exercise both cold build and update with a tenth page advertising a next page and missing required history; assert no new snapshot pointer is published.

### DOMAIN-HISTORY-2 — page-one first entry cannot prove a stable paged listing

- **Classification:** INCORRECT; task contract and source-trust boundary.
- **Defect class/root cause:** Multi-page closed-PR discovery treats equality of only the first page's first `(number, updated_at)` as proof that no page shifted. Updating a lower-ranked PR can shift page boundaries while that first entry remains unchanged.
- **Owner and siblings:** `history.rs:407-410` is the shared stability check for both cold and update history reads. The producer is `GitHubClient::closed_pull_requests` (`github.rs:1494-1568`), which exposes only `first`, `oldest_update`, and merged entries; the two callers are `ColdBuild::history` and `Update::run` as above.
- **Violated obligation:** Decision 7 requires complete-or-fail history. The design explicitly requires a multi-page read to fail as a retry state if the listing moved (`architecture/bohmian-code-review-design.md:22`).
- **Evidence and consequence:** Suppose page one starts with PR A updated at time 100. Between reading page one and page two, a merged PR from a later page updates to time 90 and moves into page one. A remains first. That moved PR was absent from the page-one response already read and is absent from the later pages now read; the re-read still has the same `first`, so the check passes. Cold build or update may publish a snapshot missing that PR's changed paths. The current `listing_changed_mid_read_fails` test (`history.rs:615-625`) changes the *first* entry, so it does not cover this reachable shift.
- **Testable correction:** Validate enough of the page boundary or listing identity to detect a shift before publishing; a first-entry-only comparison is insufficient. In a two-page scripted provider test, keep the first entry fixed while moving a lower entry across the boundary; require the retry-state error and no pointer publication for both cold build and update.

## Verification limits

The gate result covers existing tests, not the page-cap or non-first-entry pagination cases above. The concurrent provider mutation is established by the source path and paginated ordering; I did not run an external GitHub request. No source-trust, snapshot-store-clock, or pointer CAS issue was found in the reviewed path beyond these findings.

Overall: **FAIL**.

```citations
[{"id":"DOMAIN-HISTORY-1","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":385,"snippet":"            if pages == MAX_DISCOVERY_PAGES {","symbols":["CoChangeHistory::read"]},{"id":"DOMAIN-HISTORY-1","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":413,"snippet":"        if let Some(prior) = prior.filter(|_| joined) {","symbols":["CoChangeHistory::read"]},{"id":"DOMAIN-HISTORY-2","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":407,"snippet":"        if pages > 1 {","symbols":["CoChangeHistory::read"]},{"id":"DOMAIN-HISTORY-2","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":409,"snippet":"            if again.first != first {","symbols":["CoChangeHistory::read"]}]
```
