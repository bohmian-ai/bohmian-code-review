# TASK-002 r8 — Wave 1 task review A

## Subject and scope

- Base `ab2fa3a`; immutable candidate `3979b427ed52e298f5fa88ef0c00bb324f0acc6b` (checkout HEAD confirmed).
- Approved `changes/active/repo-parser/spec.md` revision 12 and `tasks/TASK-002-github-source-and-stage-contracts.md`, Revision 12 delta only. The earlier TASK-002 obligations passed in r7.
- `snapshot.rs` cold-build internals belong to TASK-003. I checked only async compatibility, byte bounds, and the accuracy of its buffering comment.
- Reviewed the base-to-candidate diff and current provider, trait, test-double, and cold-build consumers. No cargo commands were run in this reviewer role; the orchestrator owns executable verification.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| AC-R12-1: async `reqwest` client; old HTTP dependency removed | `github.rs:75-90,134-191,850-870`; workspace and pipeline manifests use `reqwest`; `Cargo.lock` has `reqwest`, no `ureq` package | Source and lockfile inspection; orchestrator lanes pending | PASS |
| AC-R12-1: existing trust, redirect, body and retry rules preserved | `github.rs:1040-1057,1071-1100,1211-1217,1271-1310,1357-1372`; `ReqwestTransport` disables redirects and applies request timeout | Converted `github::tests` cover identities, redirect, body limits, rate classes; orchestrator run pending | PASS |
| AC-R12-2: async `Send` storage/persistence contracts, native `async fn` implementers, generic facade consumers | `storage.rs:216-243`; `persistence.rs:137-210,230-282`; `testing.rs` implementations; `snapshot.rs:235-305,440-588` awaits store calls | Source inspection; `snapshot::tests::cold_build_future_is_send` exists; orchestrator compile pending | PASS |
| AC-R12-3: remove library cancellation and deadlines | `execution.rs` deleted; module/re-exports removed in `lib.rs`; provider signatures no longer accept `Execution`; no `Execution`, `CancelToken`, or `Interrupted` references in pipeline crate | Targeted `rg` was empty; orchestrator static check pending | PASS |
| AC-R12-4: dropping retry wait stops further requests | `github.rs:1271-1310` awaits `tokio::time::sleep`; no independent task survives the future; `github.rs:1845-1860` times out a paused-clock call and checks one request | Test source inspected; orchestrator focused run pending | PASS |
| AC-R12-5: no detached tasks; owned concurrency only | No `tokio::spawn` or detached task API in pipeline crate; provider work is sequential | Targeted `rg` was empty; orchestrator static check pending | PASS |
| TASK-003 cold-build exception: compiles, stays bounded, buffering note honest | `snapshot.rs:653-675` buffers only chunks admitted by `Archive::chunk` at `github.rs:594-605`; expanded data crosses `BoundedReader`; `snapshot.rs:1756-1760` checks a `Send` future | Source inspection; orchestrator compilation pending | PASS |
| Non-goals and scope: no consumer loop, checkout, model work, or extra provider framework | Diff remains in pipeline crate/contracts and their tests, manifests/lockfile, task evidence | Diff inspection | PASS |

The archive comment accurately names the current in-memory compressed buffer and its `max_archive_bytes` ceiling. The decompressed tar reader has a separate expansion ceiling. Its streaming replacement is TASK-003 work.

## Proposed findings

None. The changed transport remains the existing single fake/production seam; the async trait shape directly serves the required `Send` futures. No new speculative layer or unnecessary dependency was found in the scoped delta.

## Result

**PASS**, subject to the orchestrator's executable checks. No finding from this sample.

```citations
[]
```
