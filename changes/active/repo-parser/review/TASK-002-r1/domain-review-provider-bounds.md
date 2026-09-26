# TASK-002 r1 — Domain review: provider budget, retry, deadline, and cancellation boundary

Reviewer: `domain-rev` (provider bounds). Read-only review of candidate `dafe20b` against base `1842eaf`.

## Reviewed boundary

Request policy, retry/backoff, rate-limit classification, deadline and cancellation handling, request accounting, and "no partial input presented as complete" in `crates/bohmian-code-review-pipeline`:

- `src/execution.rs` (full): `Execution`, `CancelToken`, `Interrupted`.
- `src/github.rs` (full): `GitHubClient::send_with_policy` / `request` / `build`, `rate_limit_wait`, `sleep_bounded`, `read_bounded`, `BoundedReader`, `status_error` / `expect_status`, `GitHubError` / `Disposition`, `CallCounts`, `RequestLimits`, all eight operations, and the four tests.
- `src/lib.rs`: re-exports of the error/execution types.

## Authority and source coverage

- Spec `changes/active/repo-parser/spec.md`: REQ-013 (line 76), INV-006 (line 90), REQ-012 (line 75), ceilings paragraph (line 95).
- Task `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`: Scenario 2 (lines 36-44) including the REFACTOR note "one provider request policy for all stages"; acceptance criteria (line 48).
- Repository rules: `AGENTS.md` (outer crate is the only crate with provider IO; never report partial as complete).

Traced paths (all in `crates/bohmian-code-review-pipeline/src/github.rs`):

1. **Single policy.** Every operation reaches the transport through `send_with_policy` (lines 1127-1174): `repository`/`branch_head`/`commit`/`tree`/`pull_request` via `get_json` -> `request`; `blob`, `archive` (first hop), `pull_request_files` via `request`; the archive redirect hop calls `send_with_policy` directly (line 959) with `authenticated = false`. No operation bypasses the policy; `CallCounts::record` is inside the loop (line 1140) so retries and the redirect hop are counted.
2. **Interrupt checks.** `execution.check()` runs before every attempt (line 1138) and before every 100 ms sleep slice (`sleep_bounded`, lines 1177-1186; `SLEEP_SLICE` line 40). Cancellation is polled, not a blind `thread::sleep` for the whole wait. Verified by `malformed_payloads_and_interruptions_are_explicit`: a cancelled or expired execution sends zero requests.
3. **Per-request timeout.** `timeout = remaining.min(request_timeout)` (line 1139) is passed to the transport; `UreqTransport::get` applies it as `timeout_global` (line 139). ureq 3.4.2 `Config::timeout_global` is documented "end-to-end, from DNS lookup to finishing reading the response body" and `timings.rs` always checks `Timeout::Global`, so body reads via `read_bounded` and the archive `BoundedReader` cannot overrun the deadline in production.
4. **Rate-limit classification** (`rate_limit_wait`, lines 704-726): `retry-after` integer seconds wins; else `x-ratelimit-remaining == 0` with `x-ratelimit-reset` epoch seconds, `saturating_sub(now)` so a past reset (clock skew) yields a zero wait rather than a panic or huge wait. 403 without either signal -> `Forbidden` (terminal, line 1149); 429 without either -> `RateLimited { retry_after: max_retry_wait }` returned immediately (line 1151-1154), not slept.
5. **Bounds on waiting.** Backoff for 5xx/transport is `attempt` seconds capped by `max_retry_wait` (line 1144). Before any sleep: `too_long = wait > max_retry_wait || remaining.is_none_or(|left| wait >= left)` and `attempt >= max_attempts` (lines 1166-1170) return the typed failure instead of sleeping. So the client never sleeps past the deadline, never sleeps longer than `max_retry_wait`, and `retry-after: 0` repeated forever is bounded by `max_attempts` (verified: two `retry-after: 0` responses under `max_attempts = 2` yield `RateLimited { retry_after: 0 }`).
6. **Typed outcomes / disposition** (`GitHubError::disposition`, lines 343-354): `RateLimited` -> `Retry { after: Some }`; `DeadlineExceeded`, `Unavailable`, `Transport` -> `Retry { after: None }`; `Cancelled`, `Forbidden`, `Unauthorized`, `NotFound`, `MalformedResponse`, `ResponseTooLarge`, `IdentityMismatch`, `UntrustedRedirect`, `UnexpectedStatus` -> `Terminal`.
7. **Partial completeness.** `TreeListing.truncated` is copied from the wire (line 900); `PullRequestFilesPage.has_next` is derived from `Link: ... rel="next"` (lines 1051-1053) and the client never aggregates pages, so no facade in this task can present a partial collection as complete. `read_bounded` uses `take(limit + 1)` while streaming (line 1219) and returns `ResponseTooLarge` on overflow; the archive `BoundedReader` errors (never truncates) at `max_archive_bytes` (lines 558-576). `ResponseTooLarge` vs `MalformedResponse` are distinct terminal variants; `parse_json` never echoes body text.

## Verification performed

```
mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib github::tests::rate_limits_retry_within_bounds_or_report_typed_outcomes -- --exact   # ok
mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib github::tests::malformed_payloads_and_interruptions_are_explicit -- --exact          # ok
mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib execution::tests::check_reports_cancellation_then_deadline -- --exact               # ok
mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib github                                                                              # 5 passed
mise exec -- cargo clippy --locked -p bohmian-code-review-pipeline --all-features --all-targets                                                            # no warnings in the pipeline crate
```

## Verification limits (observations, not findings)

None of these violates a cited REQ/INV/task line; recorded so later tasks and the change review can decide.

- `retry-after` is parsed as integer seconds only (line 707-709). GitHub documents integer seconds for secondary limits; an HTTP-date form would fall through to the `x-ratelimit-*` check and, on a bare 403, be classified `Forbidden`. No spec or task line requires HTTP-date parsing.
- A deadline that expires *during* an in-flight request surfaces as `Transport(TransportError::Timeout)` (line 1165, then `too_long` on `remaining() == None`), not `Interrupted::DeadlineExceeded`. Both map to `Retry { after: None }`, so the caller-facing disposition is identical and typed; only the variant differs.
- The `Archive.reader` stream is bounded by bytes and by the ureq global timeout, but it does not observe `CancelToken` while the caller reads it. The consuming facade (TASK-003 cold build) owns the read loop and holds the `Execution`; cancellation during archive streaming is that facade's check, not a TASK-002 obligation.
- Body-read failures after the policy returned a 200 (`read_bounded` -> `Transport(Failed)`) are not retried in-client; they are bounded (one read) and typed retryable, so REQ-013's "bounded retry time" holds.
- The provider's 3,000 PR-file ceiling (spec line 95) is not detectable from this client alone (`PullRequestWire` omits `changed_files`); `has_next` reports only what the provider advertises. REQ-010's "every available page up to the provider's documented ceiling" belongs to the PR-context facade (TASK-005), which can bound by page count.
- Not exercised: a real GitHub/GHES rate-limit response (AC-006 opt-in contract run is out of this task's scope); ureq timeout behaviour is verified by reading `ureq-3.4.2/src/config.rs` and `timings.rs`, not by a live slow server.

## Findings

None. No root-cause defect was found in the reviewed boundary against REQ-013, INV-006, REQ-012, or task Scenario 2.

## Overall result

**PASS**

```citations
[]
```
