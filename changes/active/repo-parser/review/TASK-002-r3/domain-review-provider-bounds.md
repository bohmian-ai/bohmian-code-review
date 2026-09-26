# Domain review — provider bounds (TASK-002 r3)

Subject: base `1842eaf`, candidate `dbd6de3` (HEAD). Crate `crates/bohmian-code-review-pipeline`.
Reviewed boundary: request policy, retry/rate-limit, deadline, cancellation, size budgets, partial-input signalling.

## Authority and source coverage

- Spec: `changes/active/repo-parser/spec.md` REQ-013 (line 76), INV-006 (line 90); REQ-012/REQ-018 partial-progress wording.
- Task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md` Scenario 2 (lines 40–48), acceptance table.
- Rules: `AGENTS.md`, `architecture/agent-rules.md:11`, `architecture/references/languages/rust-core.md:7`.
- Source read in full: `crates/bohmian-code-review-pipeline/src/execution.rs`, `crates/bohmian-code-review-pipeline/src/github.rs` (non-test 1–1365, test harness 1366–1502, Scenario 2 tests 1747–1908, archive test 1659–1745). `ureq` 3.4.2 `timeout_global` semantics checked in the registry source (`config.rs:720-726`).

Traced paths:

| Operation | Policy entry | Status handling | Body bound |
|---|---|---|---|
| `repository`, `branch_head`, `commit`, `tree`, `pull_request` | `get_json` → `request` → `send_with_policy` (github.rs:1095–1123) | `expect_status(200)` → `status_error` | `read_bounded(max_json_bytes)` → `parse_json` |
| `blob` (925) | `request` → `send_with_policy` | `expect_status(200)` | `read_bounded(max_blob_bytes)` + Git SHA check |
| `archive` (956) | `request` for API hop; `send_with_policy` unauthenticated for the redirect hop | 200 / trusted redirect / `status_error` | `BoundedReader { remaining: max_archive_bytes }` |
| `pull_request_files` (1044) | `request` → `send_with_policy` | `expect_status(200)` | `read_bounded(max_json_bytes)`; `has_next` from `Link rel="next"` |

Every provider request goes through `send_with_policy` (1163–1209): `Execution::check` before each attempt (1174); per-request timeout `remaining.min(request_timeout)` (1175); 403/429 classified via `rate_limit_wait` (717–738: `retry-after` seconds, else `x-ratelimit-remaining: 0` + `x-ratelimit-reset` epoch), 403 without rate signal → `Forbidden` (terminal), 429 without rate signal → `RateLimited{max_retry_wait}` without sleeping; 5xx/transport → linear backoff capped at `max_retry_wait`; a wait is slept only if `wait <= max_retry_wait`, `wait < remaining`, and `attempt < max_attempts` (1202–1207); `sleep_bounded` (1218–1227) re-checks cancellation/deadline every 100 ms. `GitHubError::disposition` (350–360) types `RateLimited`/`DeadlineExceeded`/`Unavailable`/`Transport` as `Retry`, everything else `Terminal`.

Partial-input signalling: `TreeWire.truncated` is a required field (636), surfaced as `TreeListing.truncated` (912); a payload without it is `MalformedResponse`, never silently complete. `PullRequestFilesPage.has_next` (548, 1063–1065). Invalid entries land in `rejected`, not dropped. `read_bounded` (1271–1283) reads `limit + 1` and fails with `ResponseTooLarge` rather than truncating. `BoundedReader::read` (572–587) hands back at most the budget and, once exhausted, probes one byte and returns `io::Error::other("… exceeds archive byte limit")` instead of EOF; documented under `# Errors` (567–571) and on `archive` (954–955).

## Verification limits

- The synchronous `ureq` transport cannot observe cancellation while a request is in flight; cancellation latency there is bounded by `min(remaining, request_timeout)`, which is the design the repository chose (synchronous default). Only cancellation before an attempt and during retry sleeps was verified.
- No real-GitHub contract run (out of normal CI by spec). Provider header shapes were exercised through the scripted fake and the /tmp probe only.
- Concurrency ceilings named in REQ-013 are not part of the client; they belong to the caller's worker pool and TASK-002 Scenario 2 does not claim them. Not assessed as a defect here.

## Findings

None material. Observations (non-blocking, no correction required for this task):

- O1 — `archive` rustdoc (945–955) states only the byte bound on the returned stream. Because `UreqTransport` sets `timeout_global` (github.rs:145) and ureq 3.4.2 applies that end-to-end through body reading, the stream is also bounded by `min(remaining deadline, request_timeout)` from request start (60 s by default). This is correct for REQ-013 (the deadline is never exceeded), and `HttpRequest.timeout` already says "including body transfer" (49–50), but the cold-build task consuming `archive` should size `request_timeout` for archive streaming. Documentation-only; TASK-003 territory.
- O2 — For a 429 with no rate-limit headers, `RateLimited { retry_after: max_retry_wait }` (1186–1190) yields `Disposition::Retry { after: Some(max_retry_wait) }`, whereas the `Disposition::Retry` doc (265–266) describes `after` as the provider's requested wait. The value is a client cap, not a provider hint. Semantics are still honest (a retryable, non-slept outcome) and the test at 1822–1829 pins it; a doc tweak would suffice.
- O3 — A 403 whose `retry-after` is an HTTP-date rather than seconds classifies as `Forbidden` (terminal). GitHub documents integer seconds for secondary limits, so this is not a defect under the spec's provider contract.

## Commands run and observed results

1. `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features github::tests` → 5 passed, 0 failed (`rate_limits_retry_within_bounds_or_report_typed_outcomes`, `malformed_payloads_and_interruptions_are_explicit`, `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials`, `verified_provider_identities_cross_and_mismatches_are_rejected`, `helpers_encode_paths_and_hash_blobs_like_git`).
2. Scratch probe `/tmp/bounds-probe` (path dependency on the candidate crate, scripted transport through the public `GitHubClient`), `cargo run`:
   - Cancellation tripped from another thread 300 ms into a 5 s `retry-after` sleep → `Interrupted(Cancelled)` after 324 ms, one request sent.
   - `retry-after` equal to `max_retry_wait` (1 s) is slept and the retry succeeds (1.05 s); `retry-after` above it returns `RateLimited { retry_after: 2s }` in 12 µs without sleeping.
   - Deadline 1.5 s with two consecutive `retry-after: 1` responses → first wait slept, second reported as `RateLimited { retry_after: 1s }` at 1.07 s, 2 requests; the sleep never crossed the deadline.
   - Archive of exactly `max_archive_bytes` (5) reads fully; one byte more fails the reader with `… exceeds archive byte limit`.
   - Blob of exactly `max_blob_bytes` verifies; one byte more → `ResponseTooLarge { limit: 3 }`.
   - `"truncated": true` surfaces as `TreeListing.truncated == true`; a tree payload missing `truncated` → `MalformedResponse("unexpected json shape …")`.
   - PR files with no `Link` header or only `rel="prev"` → `has_next == false`.
   - 429 with HTTP-date `retry-after` → `RateLimited { retry_after: 60s }`; 403 with HTTP-date `retry-after` → `Forbidden`.
3. `git status --short` before and after → clean (no tracked file modified; only this report added).

## Verdict

PASS

```citations
[]
```
