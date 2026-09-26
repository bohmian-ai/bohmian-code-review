# TASK-002 r2 — domain review: provider bounds / budget safety / cancellation

Reviewer: `domain-rev` (BOUNDS-*). Subject: `/tmp/t002r2-subject.md`, base `1842eaf`, candidate `93b2542`, cumulative range. Read-only.

## Reviewed boundary

Request policy and bounded execution in `crates/bohmian-code-review-pipeline/src/execution.rs` (102 lines, whole file) and `crates/bohmian-code-review-pipeline/src/github.rs` (1929 lines, whole file), traced end to end:

- `Execution::check` / `remaining` (execution.rs:67-82) — consulted at the top of every `send_with_policy` attempt (github.rs:1121), on every 100 ms slice of `sleep_bounded` (github.rs:1170), and for the `too_long` decision (github.rs:1153). The archive redirect hop goes through the same `send_with_policy` (github.rs:961), so it is checked too.
- `send_with_policy` (github.rs:1109-1160): `attempt` counts from 1; the loop exits on `attempt >= max_attempts || too_long`, so attempts are bounded by `max_attempts` (a `max_attempts` of 0 still makes exactly one attempt and then fails — bounded). Wait sources: `rate_limit_wait` (github.rs:751-773) for 403/429, `Duration::from_secs(attempt).min(max_retry_wait)` backoff for 5xx and transport errors. `too_long = wait > max_retry_wait || remaining.is_none_or(|left| wait >= left)` returns the typed failure instead of sleeping; `sleep_bounded` never sleeps beyond `wait` and re-checks cancellation/deadline per slice. Typed outcomes verified: `RateLimited{retry_after}` (headers present, or 429 with no headers → `max_retry_wait`, documented at github.rs:1099-1101 and asserted by the test at github.rs:1745), `Forbidden` (403 without rate headers), `Unavailable{status}` (last 5xx), `Transport(err)` (last transport error), `Interrupted` via `From` on `sleep_bounded(..)?` and `execution.check()?`.
- Per-request timeout: `remaining.min(request_timeout)` (github.rs:1122) passed to `HttpRequest.timeout`; `UreqTransport` applies it as `timeout_global` (github.rs:143), which in vendored `ureq 3.4.2` (`src/config.rs:720-726`) is "end-to-end, from DNS lookup to finishing reading the response body". So JSON/blob body reads and the archive stream are all bounded by the deadline-derived timeout even though cancellation is not polled during a read.
- `RequestLimits` (github.rs:170-200): `max_json_bytes` applied by `read_bounded` in `get_json` (github.rs:1050) and `pull_request_files` (github.rs:1013); `max_blob_bytes` in `blob` (github.rs:927); `max_archive_bytes` via `BoundedReader` in `archive` (github.rs:975-979). `read_bounded` (github.rs:1224-1236) takes `limit+1` and fails with `ResponseTooLarge` when `len > limit` — fails, never truncates. `BoundedReader::read` (github.rs:562-580) probes one byte once `remaining == 0` and returns an `io::Error` on any further byte; `remaining -= read` cannot underflow because `want <= remaining`.
- Accounting: `CallCounts::record` fires before every `transport.get`, including retries and the redirect hop (github.rs:1123). Snapshot omits zero entries; `get` is exact.
- Truncation/pagination surfaced: `TreeListing.truncated` from the wire (github.rs:906), `TreeListing.rejected` / `PullRequestFilesPage.rejected` carry every dropped entry with a reason, `PullRequestFilesPage.has_next` from `Link: ...rel="next"` (github.rs:1010-1012). No facade in this candidate collapses these into a "complete" claim (INV-006).
- Arithmetic: `retry-after`, `x-ratelimit-remaining`, `x-ratelimit-reset` parsed via `str::parse::<u64>` after `trim()`; `Duration::from_secs(u64)` cannot overflow; `reset.saturating_sub(now)`; `now_epoch_seconds` maps a pre-epoch clock to 0 rather than panicking; `Instant::checked_duration_since` for remaining. Scratch run confirmed a `retry-after` of `u64::MAX` yields `RateLimited{retry_after: 18446744073709551615s}` without panic.
- Error swallowing: none found. Every non-2xx/non-retried status goes through `status_error` → typed error; `parse_json` never echoes body text; `rate_limit_wait`'s parse failures fall through to the stricter outcome (`Forbidden` / immediate `RateLimited`), never to a silent success.

## Authority and source coverage

- spec.md:76 (REQ-013), :85 (INV-001), :90 (INV-006); task file Scenario 2 (tasks/TASK-002-...md:36-44); AGENTS.md error/no-`unwrap` rules; r1 verdict notes (verdict.md:77-84) for previously accepted non-blocking observations.
- Prior findings FIND-TASK-002-1/-3 are outside this boundary. FIND-TASK-002-2 (`# Errors`) in this boundary: every fallible item in execution.rs and github.rs — `Execution::check`, `HttpTransport::get`, `UreqTransport::get`, `get_json`, `request`, `send_with_policy`, `sleep_bounded`, `expect_status`, `read_bounded`, `parse_json`, `validate_tree_entry`, `validate_pull_request_file`, `FakeTransport::get`, `FakeRef::get`, and each public client method — now carries `# Errors`. Closed for every sibling in this boundary.

## Scenario 2 test verification

Tests at github.rs:1640-1854 construct a real `GitHubClient::with_transport` with `FakeTransport` (github.rs:1379-1430), whose only role is to pop scripted `(status, headers, body)` / `TransportError` entries; `send_with_policy`, `rate_limit_wait`, `sleep_bounded`, `read_bounded`, `parse_json` and the identity checks are the production code. Covered through the public API: 403 + `retry-after: 0` retried then succeeds (2 calls counted); 429 with `x-ratelimit-remaining: 0` retried; far reset → `RateLimited` with `Disposition::Retry{after}`; exhausted attempts → `RateLimited`; bare 403 → `Forbidden`/`Terminal`; 401/404 typed; bare 429 → `RateLimited{max_retry_wait}` not slept; malformed JSON → `MalformedResponse` with category-only detail, `Terminal`; oversized JSON → `ResponseTooLarge{limit}`; 503→502 → `Unavailable{502}`, `Retry`; `Timeout` then `Failed` → `Transport(Failed)`; pre-cancelled and expired executions → `Interrupted::Cancelled` / `DeadlineExceeded` with zero requests sent; `retry-after: 2` under a 1 s deadline → `RateLimited` without sleeping.

Commands run at candidate (all pass): `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib github::tests` (5 passed, 0.00 s) and `... --lib execution::tests` (1 passed).

Scratch reproduction (`/tmp/t002r2-bounds`, path-dependency on the crate, own transport):
- 429 `retry-after: 5` with the token cancelled from another thread at 250 ms → `Interrupted(Cancelled)` after 323 ms, exactly 1 request sent.
- 503×3 under a 2.5 s deadline with `max_attempts: 5` → attempt 1 (timeout 2.50 s), 1 s backoff slept, attempt 2 (timeout 1.43 s), 2 s backoff ≥ remaining → `Unavailable{503}` returned at 1.07 s, never sleeping past the deadline.
- `retry-after: 18446744073709551615` → typed `RateLimited`, no panic.

## Verification limits

- No live GitHub/GHES run; `ureq` timeout semantics confirmed from vendored 3.4.2 source only.
- The inline tests never exercise a non-zero sleep (all scripted waits are 0 or over-cap), so cancellation observed mid-sleep and slice-bounded backoff are proven here only by the scratch run above, not by a checked-in test. Recorded as a verification note, not a finding: Scenario 2 as written requires typed outcomes for 403/429, malformed payload, cancellation, and deadline, which the checked-in tests do exercise through the real policy.
- Not covered: ureq-level behaviour for `timeout_global(Some(Duration::ZERO))` when `remaining()` is exactly zero (the `Some(0)` case of `checked_duration_since`); it is at worst an immediate `Transport(Timeout)` on one attempt, bounded.

## Proposed findings

None material.

Observations deliberately not raised as findings (caller-supplied absurd limits or already-accepted r1 notes; no written rule violated inside the diff):
- `read_bounded` computes `limit as u64 + 1` (github.rs:1226); overflows only for `max_json_bytes`/`max_blob_bytes == usize::MAX`.
- `Execution::with_timeout` uses `Instant::now() + timeout` (execution.rs:59), which panics only for a `Duration` near `MAX`.
- `retry-after` HTTP-date form is not parsed; in-flight deadline surfaces as `Transport(Timeout)` with the retry disposition; archive stream is bounded by the request timeout and byte cap but is not cancel-aware (all recorded in r1 verdict.md:81 as consumer-task concerns).

## Overall

**PASS** for the provider bounds / budget safety / cancellation boundary.

```citations
[]
```
