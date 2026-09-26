---
reviewer: domain-rev (provider bounds / budget safety)
subject: TASK-002 cumulative, base 1842eafdbc884d29f45fcee1c952ed3c89dd3e15, candidate 801d9572cf6b0364b5ab33e858388d86f54cb91e (HEAD verified)
result: PASS
findings: []
---

## Reviewed boundary

Timeouts, deadlines, cancellation, retry policy, bounded reads, and integer arithmetic on caller-supplied limits in `crates/bohmian-code-review-pipeline/src/execution.rs` and `crates/bohmian-code-review-pipeline/src/github.rs`, plus a panic/allocation sweep over the cumulative diff (`/tmp/task002-r4-code.diff`, 3277 lines) and the r3 fix diff (`/tmp/task002-r4-r3fix.diff`).

## Authority and source coverage

- `changes/active/repo-parser/spec.md` REQ-005 (deadline + cancellation on cold build), REQ-012 (typed retry/terminal, partial != complete), REQ-013 (retry-after/reset, bounded retry time, size and duration bounds), INV-006 (no hidden budget success), INV-002 (truncation visible).
- `AGENTS.md` Rust rules: no unchecked `unwrap()` on input, `expect()` only for named invariants, `# Panics` documented.
- Prior obligation `review/TASK-002-r3/TASK-002-R3-host-payload-seal-and-lane-doctests.md` FIND-TASK-002-6, AC-R3-4, AC-R3-5.
- Source read in full: `execution.rs` (108 lines); `github.rs` transport (`:104-170`), `RequestLimits` (`:172-199`), `BoundedReader` (`:557-587`), `rate_limit_wait` (`:717-739`), `archive` (`:956-1000`), `pull_request_files` (`:1044-1090`), `send_with_policy` (`:1162-1210`), `sleep_bounded` (`:1218-1227`), `read_bounded` (`:1271-1283`), `disposition` (`:350-360`), tests `:1840-1944`.
- ureq 3.4.2 source in the local registry: `src/config.rs:257` (`max_redirects_do_error = max_redirects > 0 && will_error`), `src/run.rs:238-247` (3xx returned as a response when not following), `src/run.rs:61,676` (global timeout applied through `RecvBody`).

## Trace

### 1. `read_bounded` / `BoundedReader`

- `github.rs:1273` `body.take((limit as u64).saturating_add(1))`; `:1276` `if bytes.len() > limit`. For `limit = usize::MAX` the take is `u64::MAX`, the body is read whole, `len <= limit` holds, bytes are returned. For `limit = N`, exactly `N` bytes pass; `N+1` bytes trip `len > limit` and return `GitHubError::ResponseTooLarge { limit: N }` (typed, never truncated). For `limit = 0`, any byte is `ResponseTooLarge { limit: 0 }`, an empty body passes the bound (matches the rustdoc "at most `limit` bytes").
- `BoundedReader::read` (`:571-586`): `want = min(remaining, buf.len())` so `remaining -= read` cannot underflow; on `remaining == 0` it probes one byte and returns a typed `io::Error::other` instead of a short read. `max_archive_bytes = 0` degrades to "empty archive only", consistent.
- Arithmetic sweep (`rg` for `+ 1`, `as u64`, `as usize`, `checked_`, `saturating_`, `wrapping_` in both files): the only casts are `limit as u64` (widening), `buf.len() as u64` (widening), `read as u64` (widening), `operation as usize` (enum discriminant index into a fixed array), `reset.saturating_sub(now)` (`:736`). No unchecked `+ 1` remains on a caller bound.
- Inline test `github::tests::read_bounded_accepts_the_maximum_limit` (`:1937-1943`) calls the real private `read_bounded` with `black_box(usize::MAX)`; not a re-implementation. Run: `cargo test --locked -p bohmian-code-review-pipeline --all-features --lib -- --exact github::tests::read_bounded_accepts_the_maximum_limit` -> `1 passed`. RED-before-fix reasoning: with `limit as u64 + 1` and `limit = usize::MAX` the addition overflows `u64`; under the `test:pipeline` profile (overflow-checks on) it panics, so the assertion is never reached. Confirmed outside the repo: `catch_unwind(|| black_box(usize::MAX) as u64 + 1)` is `Err` in the scratch crate's debug profile.
- Scratch crate `/tmp/bounds-scratch` (path dependency on the pipeline crate, public API only, `HttpTransport` fake): exact-`N` body succeeds; `N+1` returns `ResponseTooLarge { limit: N }`; zero limit rejects one byte and accepts an empty body (parse then fails as `MalformedResponse`, not the reader); `usize::MAX` returns the body. 4 passed.

### 2. `with_timeout` / deadline / cancellation / retry

- `execution.rs:56-62`: `# Panics` present and accurate (`Instant + Duration` panics when unrepresentable, e.g. `Duration::MAX`). `Execution::new` has no panic path. `check()` orders `Cancelled` before `DeadlineExceeded`.
- `send_with_policy` (`:1170-1208`): `execution.check()?` precedes every attempt; the per-request timeout is `remaining.min(request_timeout)`; no request is sent after interruption (asserted at `:1893-1896`).
- Retry waits: `rate_limit_wait` honors `retry-after` then `x-ratelimit-remaining == 0` with `x-ratelimit-reset` (saturating); server error and transport failures use linear backoff capped by `max_retry_wait`. `too_long = wait > max_retry_wait || remaining.is_none_or(|left| wait >= left)` so a wait that reaches the deadline is reported (`RateLimited { retry_after }`) rather than slept (`:1899-1909` tests it). `sleep_bounded` re-checks cancellation and deadline every 100 ms slice. `attempt` is `u32`, bounded by `max_attempts`; `Duration::from_secs(u64)` cannot panic for any header value.
- Dispositions (`:350-360`): `RateLimited` -> `Retry { after: Some }`; `DeadlineExceeded`, `Unavailable`, `Transport` -> `Retry { after: None }`; everything else including `Cancelled`, `ResponseTooLarge`, `UntrustedRedirect` -> `Terminal`.
- Partial listing: `TreeListing.truncated` (`:451`, from the wire field, tested at `:1627`) and `PullRequestFilesPage.has_next` (`:548`, from `Link rel="next"`, tested at `:1603`) are surfaced to the caller; nothing collapses them into a complete result.

### 3. ureq configuration

- `UreqTransport::new` (`:112-116`): `http_status_as_error(false)`, `max_redirects(0)`, fixed user agent. ureq 3.4.2 returns the 3xx response when `max_redirects == 0` (`config.rs:257`, `run.rs:243-246`), so the manual single-hop archive redirect at `:962-978` receives the `Location` and enforces `is_trusted_redirect` plus credential stripping itself.
- `timeout_global(Some(request.timeout))` (`:145`) covers resolve, connect, send, response headers and `RecvBody` (`run.rs:676`), so a stalled body read under `read_bounded` or the returned `Archive` stream fails within `request_timeout ∧ remaining`. Body size is not limited in ureq; every body path goes through `read_bounded` (`:934`, `:1066`, `:1104`) or `BoundedReader` (`:984-988`), so the size bound is uniform and the two mechanisms agree.

### 4. Panic / allocation sweep of the cumulative diff

- Non-test `expect`/`unwrap`: none on input. `usize::try_from(...).unwrap_or(buf.len())` (`:583`) is infallible-by-construction with a safe fallback.
- Allocations sized by input: `Vec::with_capacity(wire.tree.len())` and `Vec::with_capacity(wire.len())` derive from JSON already bounded by `max_json_bytes`; `String::with_capacity(raw.len())` is a percent-encoder over a branch name; `read_to_end` grows to at most `limit + 1`. `page: u32` and `number: u64` are only formatted into a URL.
- `self.0[operation as usize]` indexes a fixed-size array by a fieldless enum; cannot be out of range.

## Verification limits

- No real GitHub/GHES traffic; ureq behavior verified from the pinned 3.4.2 registry source, not by a live redirect or a stalled socket.
- `read_bounded` RED-before-fix reasoned from the expression and reproduced on the raw expression in a scratch crate; the base tree was not checked out (per instructions).
- Wall-clock overshoot of `sleep_bounded` not measured.

## Proposed findings

None.

## Non-blocking observations

- O-1: `sleep_bounded` (`github.rs:1221-1224`) discards the `Duration` returned by `check()`; a slice is `min(left, 100ms)` rather than `min(left, 100ms, remaining)`, so the last slice can drift up to 100 ms past the deadline before `DeadlineExceeded` is reported. Bounded and harmless; a one-token cap would remove it.
- O-2: `RequestLimits::max_attempts` is documented as "attempts per operation including the first" (`github.rs:174`); a caller-supplied `0` behaves as `1` because `attempt >= max_attempts` is evaluated after the first send. Worth a doc word ("at least one attempt is always made").
- O-3: After `archive()` returns, the `Archive` stream is bounded by bytes and by the ureq global timeout fixed at request time, but reads do not observe the `CancelToken`; the consumer of the stream (TASK-003+) should check `Execution` between tar entries.

```citations
[]
```
