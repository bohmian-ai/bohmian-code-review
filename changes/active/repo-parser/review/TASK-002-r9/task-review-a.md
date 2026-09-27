# TASK-002 Revision 12 delta: independent task review A

Subject: `ab2fa3a8615431f7c32c719a0f321f2f252e68fe` → `e38c3ae165f8972037c6b823aa9c725782b4237c`. Authority: approved `changes/active/repo-parser/spec.md` revision 12, original TASK-002 Revision 12 delta, and TASK-002-R6. Earlier TASK-002 behavior passed in r7; this review tests the delta and preservation boundaries. The candidate remained at the named commit during inspection.

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| AC-R12-1: async `reqwest`, no `ureq`, trusted redirects, bounded bodies, identity and retry policy preserved | `github.rs` uses `ReqwestTransport`, async client methods, `send_with_policy`, `read_bounded`, `Archive::chunk`, and unchanged identity checks; manifest/lock use `reqwest` | `cargo tree --depth 1 --all-features` lists `reqwest` and tokio; `rg` finds no `ureq`; focused rate-policy test passed | PASS |
| AC-R12-2: async injected traits with `Send` futures, generic facades, no `async_trait` | `storage.rs` and `persistence.rs` trait methods return `impl Future<Output = ...> + Send`; `testing.rs` implements them with `async fn`; snapshot facade is generic | Pipeline crate compiles under focused test; no `async_trait` in crate or manifest | PASS |
| AC-R12-3: remove library cancellation types and deadline parameters | `execution.rs` deleted; exports removed from `lib.rs`; provider methods take no execution argument | `rg` finds no `Execution`, `CancelToken`, or `Interrupted` in crate | PASS |
| AC-R12-4: dropping a retrying call stops further requests | `send_with_policy` awaits `tokio::time::sleep` in its own future, with no detached work | Exact focused `dropped_call_sends_no_further_requests` passed (1 selected, 0 failed) | PASS |
| AC-R12-5: no detached tasks | No `tokio::spawn`; provider request and wait remain in the caller's future | `rg` finds no `tokio::spawn` in pipeline crate | PASS |
| R6/FIND-TASK-002-13: at most 4096 bytes copied for headerless 403 inspection; classification and wait unchanged | `github.rs:827-840` preallocates the 4096-byte prefix and slices every chunk to remaining room before `extend_from_slice`; `names_rate_limit` consumes only that prefix; shared policy handles its boolean | Exact focused `headerless_403_inspects_only_a_bounded_prefix` passed (1 selected, 0 failed); direct capacity assertion detects the former copy-then-truncate error; focused rate-policy test passed | PASS |
| R6: no extra retry when the marker is outside/inside the prefix and wait exceeds budget | Test checks `Forbidden` past byte 4096, `RateLimited` when ending at byte 4096, and exactly two requests for two calls | Same focused test passed | PASS |
| Consumer closure: TASK-003 cold-build internals compile, stay bounded, and mark buffering honestly | `snapshot.rs:653-656` explicitly says compressed archive is buffered in memory; `Archive::chunk` enforces `max_archive_bytes`, `BoundedReader` enforces expansion, and entry size is capped before reading | Pipeline crate compiled; this review does not assess the cold-build algorithm, owned by TASK-003 | PASS |
| No unrequested product surface or prohibited behavior | Diff changes the provider transport, async contracts and compilation consumers; no server, Git checkout, consumer dependency, or new general framework | Source and manifest review; `git diff --check` passed | PASS |

R6 closes `FIND-TASK-002-13`: the former `extend` then `truncate` could retain a large allocation; the new prefix buffer never exceeds the bound. `HttpBody::chunk` still materializes one transport chunk, as the remediation explicitly excludes; the classifier does not copy its excess. `read_bounded` and `Archive::chunk` are siblings with separate pre-copy length checks.

Verification limit: this reviewer reran the three exact provider tests and compile, dependency, search, and diff checks. The candidate's recorded broader `mise run gate` evidence was not independently rerun here. The approved user scope excludes a full TASK-003 behavior audit.

Proposed findings: none. Overall: **PASS**.

```citations
[]
```
