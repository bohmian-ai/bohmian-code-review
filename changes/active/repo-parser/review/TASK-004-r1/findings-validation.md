# Wave 2 findings validation

Subject: `a77c64d8194ff3b8e0bc9f51f68b2cc5ae0095ec` → `f5a6f4bbeae1877083f79f4198316205c75a63e8`. Approved authority: `changes/active/repo-parser/spec.md` revision 12 and the original TASK-004.

## Claim disposition

| Source ID | Result | Gate | Reason |
|---|---|---|---|
| `DUR-1` | **REJECTED** | None | It correctly observes that `Update::run` can publish its pinned tree after the branch moves. It does not establish a violated obligation: the independently callable `Update::freshness` returns a typed `Stale` result for precisely that state, and TASK-004 explicitly makes invocation timing the caller's decision. The task requires a moving head to request another update at the facade boundary, not that every `run` response re-read the branch. |

The reachable race is explicit. `Update::run` reads a pointer and pins one branch head at `update.rs:72-74`; its cold and incremental paths both publish or reuse against that pinned head (`update.rs:75-102`, `snapshot.rs:524-555`, `snapshot.rs:634-679`). `ColdBuild::publish` records the object and swaps the pointer for that tree (`snapshot.rs:695-736`). If the branch moves from B to C before the swap, the B publication is a complete immutable snapshot, while `Update::freshness` reads the pointer and pins the then-current head and returns `Stale` (`update.rs:109-123`). The focused test moves the head during publication, asserts `Published(B)`, then asserts `Freshness::Stale` with C (`update.rs:445-462`); it also proves a second update converges and a lost concurrent swap yields `PointerConflict` (`update.rs:464-484`). The exact test passed: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib update::tests::moving_head_is_reported_stale_and_updates_converge -- --exact` (1 passed).

Authority supports that split: spec REQ-007 says callers can invoke the update facade separately after a push and a review can report a stale pointer without waiting; AC-003 requires stale detection after a missed update. TASK-004 lines 16 and 38-48 say callers decide when to invoke update, repeated calls converge, a conflicting pointer swap returns a retry state, and a review-time check returns typed staleness. No consumer scheduler is in this task (`TASK-004:50-52`; spec's consumer boundary). The candidate exposes the pinned `head` and publication `outcome` in `ColdBuildResult` (`snapshot.rs:395-421`), so `Published` describes the pinned build; it does not certify the branch remains at that tree forever.

The proposed post-publication re-read would add provider IO on both `update.rs:76` and `update.rs:101`, while a push can still arrive after that read. It is therefore neither a durable guarantee of currentness nor the smallest correction. The existing `freshness` method is the repository mechanism for the caller's latest-head decision. Both cited paths share the same pinned-head contract; no sibling correction is needed. `ColdBuild::run` also uses the same pin/build path (`snapshot.rs:499-504`) and is outside this incremental claim. The only production caller surface in this repository is the exported facade; other `Update::run` calls are tests. The head producer is `ColdBuild::pin` (`snapshot.rs:511-516`), and pointer state is produced by the snapshot store and `ColdBuild::publish`.

## Final ledger and recommendation

No retained blocking findings. No notes. No remediation recommendation. Wave 2 result: **PASS**.

Verification limit: this validation used the focused moving-head test and source trace; it did not rerun the full repository gate. A concurrent push after the final observation can always make a pointer stale, so callers use `freshness` or a later update invocation when they need a new observation.
