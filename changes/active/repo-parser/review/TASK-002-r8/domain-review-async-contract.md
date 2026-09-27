# TASK-002 r8: async contract domain review

**Subject:** `ab2fa3a..3979b427ed52e298f5fa88ef0c00bb324f0acc6b`, approved repo-parser spec revision 12, TASK-002 Revision 12 delta. **Result: PASS.**

## Boundary and coverage

| Boundary | Authority | Candidate source and evidence | Result |
|---|---|---|---|
| Injectable storage and persistence futures are `Send`; facades are generic over stores | TASK-002 AC-R12-2; spec REQ-012, revision 12 | `storage.rs:216-245`, `persistence.rs:137-285`, `snapshot.rs:228-235,396-404`; `testing.rs` implements the traits with `async fn`; `snapshot.rs:1753-1765` checks `ColdBuild::run` and `Snapshots::collect` futures are `Send` | PASS |
| Caller drop cancels a provider retry wait | TASK-002 AC-R12-4; spec REQ-013 | `github.rs:1271-1311` awaits transport then `tokio::time::sleep`; no spawned work outlives the future. `github.rs:1845-1860` drops a waiting call with `timeout`, advances paused time, and checks exactly one request | PASS |
| No library cancellation API or detached task | TASK-002 AC-R12-3/5; spec revision 12 | `execution.rs` deleted; `lib.rs` removes its module and exports. Repository search of pipeline source and root manifest found no `Execution`, `CancelToken`, `Interrupted`, `tokio::spawn`, `spawn_blocking`, `JoinSet`, or `async_trait` | PASS |
| Snapshot compile closure and honest temporary archive bound | TASK-002 consumer closure; user-scoped TASK-003 exception | `snapshot.rs:440-594` awaits injected stores and provider, records before create, then checks object before pointer publication. `snapshot.rs:653-675` acknowledges compressed buffering; `github.rs:578-606,1091-1096` bounds archive bytes, and `BoundedReader` limits expansion | PASS within stated TASK-003 scope |

## Verification limits

- Per delegation, I ran no cargo commands. The candidate's task evidence reports passing pipeline, format, lint and gate lanes; this review directly inspected source and existing tests, but did not independently execute them.
- Synchronous tar expansion in `snapshot.rs` does not yield to drop while that function runs. The user limits this review to compilation, resource bounds, and the honest `ponytail:` buffering note; TASK-003 owns its rewrite.
- No live provider or consumer implementation was exercised. The fake provider's paused-clock test establishes the retry-wait drop path, not arbitrary network cancellation timing.

## Proposed findings

None.

```citations
[]
```
