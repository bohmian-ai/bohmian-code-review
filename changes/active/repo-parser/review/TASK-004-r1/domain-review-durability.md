# Durability and concurrency domain review

## Subject and coverage

- Base: `a77c64d8194ff3b8e0bc9f51f68b2cc5ae0095ec`; candidate: `f5a6f4bbeae1877083f79f4198316205c75a63e8` (HEAD at review).
- Boundary: default-branch snapshot update, immutable object publication, pointer compare-and-swap, interrupted calls, cleanup interaction, and review-time freshness.
- Authority: approved `changes/active/repo-parser/spec.md` revision 12, especially REQ-006, REQ-007, REQ-012, REQ-013; `changes/active/repo-parser/tasks/TASK-004-incremental-snapshot-lifecycle.md` Scenario 2 and acceptance criteria; `changes/active/repo-parser/symbolic-context-service-plan.md` section 4; `architecture/bohmian-code-review-design.md` and `architecture/agent-rules.md`.
- Source traced: complete base-to-candidate diff; `update.rs`, `snapshot.rs` (read, ingest, publication, collection), `persistence.rs` trait contract, `testing.rs` injected store/storage, and the provider's branch/tree/blob methods in `github.rs`. The complete source and test paths show record upsert before object create before pointer CAS, digest and identity validation on read, atomic CAS in the test store, no detached work, and a dropped future leaving a complete prior pointer. Reuse carries facts only for identical eligible path/blob pairs; failed changed blobs leave an incomplete attempt and do not publish.

## Verification limits

- Parent orchestrator ran `mise run gate` and `git diff --check` successfully on this candidate. I did not overlap Cargo commands with that run.
- The in-memory adapter exercises three mid-publication drop points. No external persistence or object-store implementation is part of this task, so durability beyond the documented trait contract is not established here.

## Proposed finding

### DUR-1 — Moving branch head is not reported by the update call

- **Classification:** MISSING; task contract. **Defect class:** no post-publication branch-head observation at the update facade. The candidate-owned result shape (`ColdBuildResult`/`ColdBuildOutcome`) also has no state that can say the published snapshot is complete for a pinned tree but the branch has since advanced.
- **Owner and siblings:** `Update::run` returns `cold.complete(...)` directly at `update.rs:101`; its cold-build path returns `cold.build(...)` directly at `update.rs:76`. Both paths pin once at `update.rs:74`. A separate `Update::freshness` call can detect drift, but neither `run` return path makes that check. The post-publication call is the single root-cause correction boundary, not a change to CAS or to immutable snapshot publication.
- **Obligation:** TASK-004 acceptance says “a moving branch head requests another update”; its Scenario 2 requires changed head detection and repeated convergence. The approved plan section 4 explicitly says rereading branch head after publication reports when another update is needed.
- **Evidence and reproduction:** The candidate's own racing test moves the branch to `tree_c` while the first update is waiting at the pointer swap (`update.rs:448`, `update.rs:452`). That `update.run` returns `Published(tree_b)` (`update.rs:455`) with no retry indication. Only the subsequent, independent `update.freshness` call reports `Stale(tree_c)` (`update.rs:460`). A caller invoking only the independently callable update facade receives a successful result for a tree already behind the branch head and has no signal to schedule the required next update. CAS cannot detect this because no competing pointer publication occurred.
- **Testable correction:** After a complete update publication or reuse, compare the branch head to the result's pinned tree and expose a typed need-another-update result while preserving the valid published snapshot and CAS behavior. Cover both return paths with a racing head response; assert that the same call reports the newer tree, and that a subsequent invocation converges. Do not retry indefinitely inside the facade or replace the published object.

**Overall: FAIL** on the named moving-head acceptance criterion. The publication/drop and immutable-reader paths reviewed above otherwise meet the task's durability boundary.

```citations
[{"id":"DUR-1","path":"crates/bohmian-code-review-pipeline/src/update.rs","line":74,"snippet":"        let head = cold.pin(identity).await?;","symbols":["Update","pin"]},{"id":"DUR-1","path":"crates/bohmian-code-review-pipeline/src/update.rs","line":76,"snippet":"            return cold.build(&scope, current.as_ref(), &head).await;","symbols":["Update","build"]},{"id":"DUR-1","path":"crates/bohmian-code-review-pipeline/src/update.rs","line":101,"snippet":"        cold.complete(&scope, current.as_ref(), &head, ingest, archive_too_large)","symbols":["Update","complete"]},{"id":"DUR-1","path":"crates/bohmian-code-review-pipeline/src/update.rs","line":448,"snippet":"        let (first, ()) = tokio::join!(update.run(&identity), async {","symbols":["moving_head_is_reported_stale_and_updates_converge","run"]},{"id":"DUR-1","path":"crates/bohmian-code-review-pipeline/src/update.rs","line":452,"snippet":"            repo.push(&sha('e'), &tree_c, &moved, &[]);","symbols":["moving_head_is_reported_stale_and_updates_converge"]},{"id":"DUR-1","path":"crates/bohmian-code-review-pipeline/src/update.rs","line":455,"snippet":"        let ColdBuildOutcome::Published(first) = first.expect(\"first\").outcome else {","symbols":["moving_head_is_reported_stale_and_updates_converge","ColdBuildOutcome"]},{"id":"DUR-1","path":"crates/bohmian-code-review-pipeline/src/update.rs","line":460,"snippet":"            update.freshness(&identity).await.expect(\"fresh\"),","symbols":["moving_head_is_reported_stale_and_updates_converge","freshness"]}]
```
