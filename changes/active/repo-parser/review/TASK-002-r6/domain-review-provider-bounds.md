# TASK-002 r6 domain review: provider bounds

Reviewer: domain-rev (provider bounds). Candidate `7983907a6aa0b01a942fb964362da73bc9e7dba6`, base `1842eaf`, read at `/tmp/cr-r6/wt`.

**Overall: PASS**. No findings.

## Reviewed boundary

`crates/bohmian-code-review-pipeline/src/github.rs`:
- `send_with_policy` (1199-1244) and its 403/429 classification;
- `rate_limit_wait` (733-754), `SECONDARY_RATE_LIMIT_WAIT` (730), `names_rate_limit` (764-771);
- `sleep_bounded` (1253-1262), `status_error` and `expect_status`, `read_bounded` (1306-1318), `BoundedReader`;
- `GitHubError::RateLimited` doc and `disposition` (282-360), `RequestLimits` (174-202);
- `UreqTransport::get` timeout wiring, `tree` `truncated`, and `pull_request_files` `has_next`.

`crates/bohmian-code-review-pipeline/src/execution.rs`: all of `CancelToken`, `Execution::check`, `remaining`, and `with_timeout`.

## Authority and source coverage

- Spec rev 11: REQ-013 (`spec.md:76`), INV-006 (`:90`), ceilings paragraph (`:95`).
- TASK-002 Scenario 2 ("typed retry or terminal outcomes; no facade can present partial input as complete").
- R4 remediation, FIND-TASK-002-9 diagnosis.
- GitHub REST rate-limit guidance, as cited by the spec and task:
  - use `retry-after` first;
  - when `x-ratelimit-remaining: 0`, wait until `x-ratelimit-reset`;
  - otherwise a secondary limit waits at least one minute, and its body names the limit.

## Assessment of FIND-TASK-002-9 (R4)

- **Resolved.** When a 403/429 has no usable headers, the code now reads a bounded 4 KiB body prefix for the documented "rate limit" wording. A 403 with that wording, and every 429, map to `RateLimited { retry_after: SECONDARY_RATE_LIMIT_WAIT }` (60 s, GitHub's documented minimum). The result has `Disposition::Retry`.
- **The reported wait is not fabricated.**
  - `retry_after` always comes from one of three sources: `retry-after`, reset minus now, or the documented 60 s constant.
  - `max_retry_wait` now only decides whether to sleep in-call (`too_long`), and it caps 5xx/transport backoff. That backoff is never reported.
  - At the default `max_retry_wait` of 60 s, a header-less secondary limit is slept once and retried, provided the deadline allows it. Otherwise it is reported without sleeping.
- **Genuine permission 403s stay terminal.** Reproduced in a scratch copy with the scripted `FakeTransport` (see below):
  - Setup: the body `{"message":"Resource not accessible by integration",...}` with `x-ratelimit-remaining: 4999`.
  - Result: `Forbidden`, `Terminal`, with exactly 1 request sent.
  - The OAuth-restriction 403 body without headers also gives `Forbidden`.
  - The real secondary-limit JSON body (message plus `documentation_url` .../`#about-secondary-rate-limits`) gives `RateLimited(60s)`.
  - Why this holds: the match is on the phrase "rate limit" with a space. Permission 403 bodies and their documentation URLs (`rate-limits` with a hyphen) do not contain it.
- **Primary limits.**
  - `retry-after` takes precedence.
  - `remaining == 0` uses the reset time, saturating at 0 when the reset has passed.
  - A reset beyond `max_retry_wait` or beyond the deadline is reported without sleeping (existing test: `RateLimited(>=3590s)`).
- **Retry budget vs deadline.**
  - Each attempt starts with `Execution::check()`.
  - The per-request timeout is `min(remaining, request_timeout)`.
  - A wait of `>=` the remaining time is reported rather than slept.
  - `max_attempts` bounds attempts, and the final failure carries the provider's wait.
- **Cancellation and deadline.**
  - `sleep_bounded` checks the execution every 100 ms.
  - Probe: a cancel 300 ms into a 3 s `retry-after` sleep returned `Interrupted(Cancelled)` in under 1 s.
  - Cancelled or expired executions send zero requests (existing test).
  - `Cancelled` is `Terminal`. `DeadlineExceeded` is `Retry`.
- **Bounded reads.**
  - `read_bounded` uses `take(limit+1)` with a saturating add, so it overflows safely and fails rather than truncating.
  - `BoundedReader` probes one byte past the limit and fails rather than truncating.
  - The secondary-limit body sniff is capped at 4096 bytes and its text never leaves the function.
- **Truncation and pagination.** The client does not collapse partial listings into complete ones:
  - `TreeListing.truncated` is surfaced as-is;
  - `PullRequestFilesPage.has_next` comes from the `Link` header;
  - rejected entries are listed rather than dropped.
- **No sibling classification path is left behind.** Every 403/429 on every operation, including the unauthenticated archive redirect hop, goes through `send_with_policy`. Because of that, `status_error`'s `403 => Forbidden` arm only covers statuses the policy has already let through.

## Verification

- Orchestrator lanes (`/tmp/cr-r6/lanes.txt`): fmt:check, lints, test:pipeline, and cargo doc all exit 0.
- Scratch copy `/tmp/cr-r6/scratch-pb`: added the probe test `probe_pb_permission_vs_secondary`, which covers the permission-403, OAuth-restriction-403, and documented secondary-body cases, a 429 with `retry-after: 0` retry, and cancellation during a retry sleep.
- `mise exec -- cargo test --all-features -p bohmian-code-review-pipeline --lib github::tests`: 7 passed, 0 failed. The probe passed, so it confirms correct behaviour rather than exposing a defect.

## Verification limits

- No real GitHub or GHES run was done. Body wording is matched against GitHub's currently documented secondary-limit text. The pre-2021 "abuse detection mechanism" wording would classify as `Forbidden`; supported GHES releases use the current wording.
- Timing behaviour is proven with the fake transport and real sleeps, not a paused clock.
- Cancellation cannot interrupt an in-flight synchronous request. It is observed at the next check, and the in-flight request is bounded by `min(remaining, request_timeout)`. This is the documented design.
- These belong to the consuming facades (TASK-003/TASK-005), not to this boundary:
  - the PR-files 3,000-file ceiling and the before/after SHA guard;
  - cancellation while streaming an archive body.

```citations
[]
```
