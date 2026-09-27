# TASK-002 r9 — independent task review B

**Subject:** `ab2fa3a8615431f7c32c719a0f321f2f252e68fe` → `e38c3ae165f8972037c6b823aa9c725782b4237c`. Approved `spec.md` revision 12, TASK-002 Revision 12 delta, and TASK-002-R6. Earlier TASK-002 obligations passed in r7. Snapshot cold-build internals are assessed only for compilation, boundedness, and honest buffering.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| AC-R12-1: async reqwest client, preserved trust, identity, bounds, and retry behavior | `github.rs` uses `ReqwestTransport` with redirects disabled, async response bodies, existing identity checks and one `send_with_policy`; `ureq` is absent from manifest and lockfile. | Existing async provider tests cover identities, redirects, archive limit, retries, and malformed responses. Focused R6 test passed. | PASS |
| AC-R12-2: async storage and persistence traits, `Send` futures, unchanged method sets and generic consumers | `storage.rs` and `persistence.rs` return `impl Future + Send`; in-memory implementations use `async fn`; `snapshot.rs` takes generic stores. | `snapshot::tests::cold_build_future_is_send` and the reported pipeline/gate runs. | PASS |
| AC-R12-3: remove library cancellation/deadline types and tests | `execution.rs` deleted; `lib.rs` no longer exports those types. | `rg` finds no `Execution`, `CancelToken`, or `Interrupted` in the crate. | PASS |
| AC-R12-4: dropping a provider retry wait sends no later request | `send_with_policy` awaits `tokio::time::sleep` within the caller future. | `dropped_call_sends_no_further_requests` uses a paused clock and asserts one request after drop. | PASS |
| AC-R12-5: no detached tasks | No spawn call occurs in the pipeline crate. | `rg` finds no `tokio::spawn`. | PASS |
| R6: hard 4096-byte classifier prefix, correct boundary and unchanged policy | `rate_limit_prefix` slices each chunk to remaining room before copying; `names_rate_limit` inspects only its result. Shared 403 policy and wait remain in `send_with_policy`. | Ran exact `headerless_403_inspects_only_a_bounded_prefix`: 1 passed. It asserts prefix length/capacity, both marker positions, typed outcomes and two requests. | PASS |
| Snapshot compile, bound, honest buffering | `consume_archive` buffers chunks only after `Archive::chunk` checks `max_archive_bytes`; decompression uses `BoundedReader`; `ponytail:` comment discloses the compressed buffer and TASK-003 rework. | Reported pipeline/gate runs passed; source trace confirms both byte bounds. | PASS within requested scope |
| Constraints and non-goals: no new SDK/backend, no scope drift, no caller-independent cancellation | Manifest uses reqwest/tokio; no storage backend or `async_trait`; changes stay in pipeline crate and task evidence. | `git diff --check` passed; forbidden-symbol search had no matches. | PASS |

## Findings and verdict

No proposed findings. **PASS** for the scoped Revision 12 delta and R6 remediation. The R6 correction closes prior `FIND-TASK-002-13`: a single oversized provider chunk can no longer make the classifier's own inspection buffer exceed 4096 bytes. The upstream transport still materializes one chunk, as the remediation explicitly permits. No additional cold-build judgment is made.

Verification limit: no live GitHub/GHES contract run was performed in this review. The source and fake-transport checks cover the relevant bounded prefix and request policy. Candidate HEAD was `e38c3ae165f8972037c6b823aa9c725782b4237c` during review.

```citations
[]
```
