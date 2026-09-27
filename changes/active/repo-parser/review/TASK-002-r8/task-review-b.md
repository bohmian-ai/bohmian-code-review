# TASK-002 Revision 12 delta — task review B

Subject: base `ab2fa3a`; candidate `3979b427ed52e298f5fa88ef0c00bb324f0acc6b`. Scope is only the Revision 12 delta of `tasks/TASK-002-github-source-and-stage-contracts.md`; the remaining TASK-002 obligations passed in `review/TASK-002-r7`. `snapshot.rs` cold-build internals are judged only for compilation, bounds, and the honest buffering note because TASK-003 rewrites them.

| Requirement, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| AC-R12-1: async reqwest, no ureq; preserve host and redirect trust, identity, retries, bounded bodies | `github.rs:75-198, 897-1212, 1271-1373`; manifest replaces `ureq` with `reqwest` | Existing async `github::tests` cover identities, redirects, retries and oversized JSON; task records `test:pipeline` and `gate` passing. Independent source trace finds the 403 body cap broken as below. | FAIL |
| AC-R12-2: three async dependency traits with `Send` futures, generic stores, unchanged method sets; no `async_trait` | `storage.rs:213-244`, `persistence.rs:127-286`, `testing.rs`; `snapshot.rs` uses generic `O` and `S` | `snapshot::tests::cold_build_future_is_send`; task records full crate lane and gate passing | PASS |
| AC-R12-3: remove library cancellation types and deadlines | `execution.rs` deleted; `lib.rs` removes module and re-exports; provider methods have no execution argument | `rg` over pipeline crate, manifests and lockfile found no `Execution`, `CancelToken`, `Interrupted`, or `ureq` | PASS |
| AC-R12-4: dropping retry wait stops requests | `github.rs:1271-1311` awaits `tokio::time::sleep` in the caller's future | `github::tests::dropped_call_sends_no_further_requests` uses paused tokio time and observes one request | PASS |
| AC-R12-5: no detached tasks; concurrency only in caller-owned `JoinSet` | No spawn call in pipeline crate; current cold build is sequential | `rg` found no `tokio::spawn` or `spawn_blocking` | PASS |
| Consumer closure and bounded compile-only snapshot port | `snapshot.rs:440-590` awaits provider/store calls; `snapshot.rs:649-675` buffers compressed bytes through bounded `Archive::chunk`, then applies `BoundedReader` to expansion. The `ponytail:` note says it buffers and TASK-003 will rework it. | Task records 29 passing pipeline tests, `fmt:check`, `lints`, `gate`; no cargo commands run by this reviewer | PASS |
| No added storage backend, consumer dependency, cold-build behavioral redesign, or unrelated changes | Diff contains only pipeline crate, manifests/lockfile, and task evidence | Base-to-candidate changed-file list inspected | PASS |

## Proposed finding

### TASK-B-1 — INCORRECT — headerless 403 inspection no longer obeys its byte cap

- **Violated obligation:** AC-R12-1 requires bounded bodies to keep their prior behavior. Before this port, `names_rate_limit` used `Read::take(RATE_LIMIT_BODY_BYTES)`. The current rustdoc also promises at most that many bytes are read.
- **Defect class and root cause:** The rate-limit classifier accumulates a whole transport chunk before applying its 4 KiB bound. Its body-consumption shape permits the defect: `HttpBody::chunk` yields unrestricted `Vec<u8>` chunks, and the classifier copies one in full.
- **Owner and siblings:** `github.rs:815-825` is the wrong decision. I swept the other consumers of `HttpBody::chunk`: `read_bounded` checks the chunk length before copying (`github.rs:1357-1373`), while `Archive::chunk` checks the configured archive limit before returning it (`github.rs:586-606`). Those paths do not share the excess-copy symptom. The classifier is called from the common `send_with_policy` path for a 403 without rate headers (`github.rs:1288-1296`), so every provider operation reaches it.
- **Evidence and consequence:** A 403 fake body delivered as one 1 MiB chunk makes line 819 append the entire 1 MiB to `body`, even though line 823 later truncates it to 4096 bytes. A provider can therefore force memory and copy work beyond the promised inspection cap before the retry/forbidden decision. This is a regression in the requested bounded provider behavior.
- **Testable correction:** At the classifier owner, copy only the remaining prefix from each chunk; add one focused fake-body test whose first chunk exceeds 4096 bytes and assert the classifier's bounded accumulation and unchanged classification. Do not change retry semantics.

Overall: **FAIL** (one task-contract finding). The existing passing checks prove compilation and common responses; they do not exercise an oversized single 403 chunk.

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":819,"snippet":"            Ok(Some(chunk)) => body.extend_from_slice(&chunk),","symbols":["names_rate_limit","RATE_LIMIT_BODY_BYTES"]}]
```
