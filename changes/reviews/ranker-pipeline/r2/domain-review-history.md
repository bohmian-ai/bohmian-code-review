# Domain review: co-change history and snapshot publication

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d..efc381f776e397ace4f43f9f4ab45fc4187436a5`
**Result:** FAIL

## Boundary and coverage

Reviewed decision 7 in `changes/reviews/ranker-pipeline/intent.md:13`, the pipeline history contract in `architecture/bohmian-code-review-design.md:22`, and R1 FIND-ranker-pipeline-2. Traced `CoChangeHistory::read` through GitHub's closed-PR page and file-listing reads, `ColdBuild::history/build/complete/publish`, `Update::run` including its cold fallback, and PR-context history filtering. Inspected the focused history, cold-build, and update tests. `mise run test:pipeline` exited 0 during this review. Its unproved-bound fixtures have **zero** qualifying PRs; they do not exercise a bound after ten qualifying PRs during an update. No credentialed live read was available, and the provider-concurrency claim is grounded in source and fake-provider paths rather than a live GitHub race.

## Proposed finding

### HIST-1 — prior-boundary proof is replaced by the cold-depth count

**Classification:** INCORRECT; R1 `FIND-ranker-pipeline-2` is `CLASS_INCOMPLETE`. **Defect class:** the shared history reader treats a cold-build completion threshold as proof of incremental-history completeness. The owner is `CoChangeHistory::read`; its current control flow permits success without a join to `prior.before`. This is a rule error in the reader, not a caller-specific validation gap.

At the tenth closed-PR page, `has_next=true`, `prior` is present, `horizon` is still newer than `prior.before`, and ten eligible merged PRs have been read. The reader passes the `history.prs.len() < COLD_DEPTH` check and breaks with `joined=false`; it then omits `prior.prs` and returns `Ok`. A page 11 may contain further PRs merged since the previous snapshot. Both the incremental `Update::run` path and its fallback through `ColdBuild::build` reach this owner; successful results can reach `ColdBuild::complete` and publish a new pointer. Decision 7 requires the update to extend the previous history with **all** merges since its commit, bounded and complete or failing. The architecture applies the ten-PR proof to **cold builds**, while its update rule requires reaching the previous snapshot boundary. This is the same completeness class R1 sought to close.

**Sibling sinks:** `update.rs:120-124` (fallback and incremental reads) and `snapshot.rs:723-729,848-857` (history accepted and included in a publishable snapshot). They share the same reader; they do not need separate fixes.

**Observable consequence:** a successful refresh may persist a history missing both prior entries and one or more new merges beyond page ten. Subsequent PR context ranks from this silently incomplete history. The existing no-publication tests pass because they keep `found=0`, so they do not establish this case.

**Testable correction:** in the shared reader, an incremental read at an advertised page bound must fail unless it has proved the prior boundary. Cold reads may still stop once ten eligible PRs are proved. Preserve the ten-page cap, full-page rechecks, and 1,000-entry retention. A focused fake-provider update with ten eligible new PRs in the first ten pages, `has_next=true`, and the prior boundary not yet reached must return an incomplete-history error and keep the old pointer; exercise the fallback build path through the same owner. A proved join must still prepend new entries to prior history.

```citations
[{"id":"HIST-1","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":399,"snippet":"            if pages.len() == MAX_DISCOVERY_PAGES as usize {","symbols":["CoChangeHistory::read"]},{"id":"HIST-1","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":400,"snippet":"                if history.prs.len() < COLD_DEPTH {","symbols":["CoChangeHistory::read","COLD_DEPTH"]},{"id":"HIST-1","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":406,"snippet":"                break false;","symbols":["CoChangeHistory::read"]},{"id":"HIST-1","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":432,"snippet":"        if let Some(prior) = prior.filter(|_| joined) {","symbols":["CoChangeHistory::read"]},{"id":"HIST-1","path":"crates/bohmian-code-review-pipeline/src/update.rs","line":120,"snippet":"            return cold","symbols":["Update::run","ColdBuild::build"]},{"id":"HIST-1","path":"crates/bohmian-code-review-pipeline/src/update.rs","line":124,"snippet":"        let history = cold.history(identity, &head, Some(&prior.history)).await?;","symbols":["Update::run","ColdBuild::history"]},{"id":"HIST-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":723,"snippet":"        let history = self.history(identity, head, prior).await?;","symbols":["ColdBuild::build","ColdBuild::history"]},{"id":"HIST-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":855,"snippet":"            history,","symbols":["ColdBuild::complete","ColdBuild::publish"]}]
```
