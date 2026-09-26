# TASK-002 r5 domain review: provider bounds

Candidate `6899a43` (base `1842eaf`), read only from `/tmp/cr-r5/wt`.

## Reviewed boundary

- Execution: `execution.rs` (`CancelToken`, `Execution::{new, with_timeout, remaining, check}`, `Interrupted`).
- Request policy: `github.rs` `send_with_policy`, `sleep_bounded`, `rate_limit_wait`, `RequestLimits`, `CallCounts`, `GitHubError::disposition`, `Disposition`.
- Bounded reads: `read_bounded`, `BoundedReader`, `archive` (redirect hop and stream), `blob`, `get_json`, `pull_request_files`.
- Pagination and truncation: `TreeListing::truncated`, `PullRequestFilesPage::has_next`.
- Transport: `UreqTransport::{new, get}` against `ureq 3.4.2` / `ureq-proto 0.6.4` from `Cargo.lock`.

## Authority and source coverage

- Spec REQ-013 (`spec.md:76`), INV-006 (`spec.md:90`), operational ceilings (`spec.md:95`). Plan: `symbolic-context-service-plan.md:69` and S12 (`:88`, "Provider primary/secondary limits → Bounded retry or visible rate-limited outcome").
- TASK-002 Scenario 2 and acceptance. Remediation tasks R1, R2, and R3. R3 FIND-6 added the `read_bounded` `saturating_add` and the `with_timeout` `# Panics` doc, both inside this boundary.
- ureq 3.4.2 source. `config.rs:257-259, 321-327`: with `max_redirects(0)`, the 3xx response is always returned and `TooManyRedirects` is never raised, so the archive redirect path works in production. `timings.rs:91-114`: the global timeout starts at call creation and is never reset, so `timeout_global` also bounds body reads. JSON, blob, and archive bodies therefore cannot outlive `min(remaining, request_timeout)`.

## Traces

- **Deadline and cancellation.** `send_with_policy` calls `Execution::check` before every attempt. The per-attempt timeout is `remaining.min(request_timeout)`, which ureq enforces through the body read. Before any sleep, `too_long` rejects a wait that is at least the remaining deadline and returns the typed failure. Otherwise `sleep_bounded` re-checks every 100 ms. Every exit is typed. Cancellation is not observed during a body transfer, but the deadline still bounds that transfer. I do not report this.
- **Retry.** Attempts are counted, and `max_attempts` bounds the loop. 5xx and transport failures back off by `attempt` seconds, capped by `max_retry_wait`. Waits from `retry-after` and `x-ratelimit-reset` are honoured or surfaced as `RateLimited`. `CallCounts` records every send, including the redirect hop.
- **Bounded reads.** `read_bounded` uses `take(limit+1)`, which makes an oversized body a typed `ResponseTooLarge` and never a silent truncation. `BoundedReader` returns an error at the archive limit rather than truncating.
- **Pagination.** `truncated` and `has_next` are passed through verbatim. The spec lists the provider's 3,000 PR-file cap as a known ceiling (`spec.md:95`). A facade can detect it by counting files, so the source contract does not hide partial input.
- **Owner shapes.** `Execution` has private fields and its only panic is documented. `RequestLimits` has public fields: `max_attempts: 0` behaves as 1, and a zero `request_timeout` shows up as `Transport(Timeout)` failures. Neither produces hidden success, so I do not report them.
- **Superseded guards.** Neither remediation guard is redundant. `saturating_add` still prevents overflow. The `# Panics` doc still describes a reachable panic. `too_long`'s `wait >= left` check is not duplicated by `sleep_bounded`: it returns the rate-limit error rather than a later `DeadlineExceeded`.

## Verification

- `cargo test --locked -p bohmian-code-review-pipeline --all-features --lib` at the candidate passed, 17 of 17. The lane doctests passed, 2 of 2 (`/tmp/cr-r5/lanes.txt`).
- The candidate's own test encodes the behaviour behind DOM-PB-1 (`github.rs:1808`, `:1823`), so no scratch reproduction was needed.
- Limits: I made no real-GitHub call. The claim about GitHub's secondary-limit responses comes from GitHub's published REST rate-limit guidance, which the spec adopts by name ("secondary limits"). I did not exercise it live.

## Findings

### DOM-PB-1: a secondary rate limit without wait headers is reported as terminal `Forbidden`, or with a made-up wait (INCORRECT)

- **Root cause.** The owner is the 403/429 arm of `send_with_policy` (`github.rs:1182-1191`), which decides only from `rate_limit_wait` (`github.rs:717`). That function returns `None` when neither `retry-after` nor `x-ratelimit-remaining: 0` is present. The arm then does one of two things:
  - A 403 becomes `GitHubError::Forbidden`, which has `Disposition::Terminal` (`:1185`).
  - A 429 becomes `RateLimited { retry_after: self.limits.max_retry_wait }` (`:1188`). This is a caller config value presented as "Wait the provider requested" (`:284`), which contradicts `Disposition::Retry`'s contract, "provider's requested wait when it gave one" (`:266`).
- **Defect class.** The case "rate limited, provider gave no wait" cannot be represented. `RateLimited.retry_after: Duration` cannot say "no provider wait", so the policy either drops the rate-limit classification or substitutes config. The owner's shape (a non-optional `Duration` payload) permits this, while `Disposition::Retry { after: Option<Duration> }` already models the missing case.
- **Siblings (one finding).** `github.rs:1185`, `:1188`, `:284`, `:266`, and `:717`. The test locks in the wrong behaviour at `github.rs:1808-1811` (a 403 with no headers is asserted `Forbidden`/`Terminal`) and `:1823-1829` (a 429 with no headers is asserted `retry_after == max_retry_wait`).
- **Violated obligation.**
  - REQ-013: "honor installation-specific primary/secondary limits".
  - Plan S12: "visible rate-limited outcome".
  - INV-006: rate exhaustion is reported.
  - TASK-002 Scenario 2: 403/429 yield a correctly typed retry or terminal outcome.
  - GitHub's REST guidance: secondary-limit responses are 403 or 429. When neither `retry-after` nor `x-ratelimit-remaining: 0` is present, the client should wait at least one minute.
- **Evidence.** Candidate test lines 1808 and 1823 as cited. `Disposition` maps `Forbidden` to `Terminal` (`:350-359`).
- **Consequence.**
  - A secondary-limit 403 is reported to the consumer as a terminal credential or permission failure. A cold build, update, or review then fails permanently where it should have retried later, and the operator is told the installation lacks access.
  - A header-less 429 under a small `max_retry_wait` (the crate's own tests use 5 s and `ZERO`) tells the caller to retry after 0–5 s. GitHub warns that retrying early against a secondary limit can get the integration banned.
- **Testable correction.**
  1. Change `RateLimited.retry_after` to `Option<Duration>`, set to `None` when the provider gave no wait. `disposition` then passes it straight through to `Retry { after }`.
  2. For a 403 without rate headers, read the small error body (bounded by `max_json_bytes`). Classify it as `RateLimited { retry_after: None }` when GitHub's error payload identifies a rate limit: `message` contains "rate limit" or `documentation_url` references rate limiting. Otherwise keep `Forbidden`.
  3. Update the two test cases:
     - 403, no headers, body `{"message":"You have exceeded a secondary rate limit..."}` gives `RateLimited { retry_after: None }` with `Disposition::Retry { after: None }`.
     - 403, no headers, body `{"message":"Resource not accessible by integration"}` still gives `Forbidden`/`Terminal`.
     - 429, no headers, gives `RateLimited { retry_after: None }`, independent of `max_retry_wait`.

## Overall

FAIL: DOM-PB-1.

```citations
[
  {"id":"DOM-PB-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1185,"snippet":"None if response.status == 403 => return Err(GitHubError::Forbidden),","symbols":["GitHubClient::send_with_policy","GitHubError::Forbidden"]},
  {"id":"DOM-PB-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1188,"snippet":"retry_after: self.limits.max_retry_wait,","symbols":["GitHubClient::send_with_policy","GitHubError::RateLimited","RequestLimits::max_retry_wait"]},
  {"id":"DOM-PB-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":284,"snippet":"/// Wait the provider requested.","symbols":["GitHubError::RateLimited"]},
  {"id":"DOM-PB-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":266,"snippet":"/// provider's requested wait when it gave one.","symbols":["Disposition::Retry"]},
  {"id":"DOM-PB-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":717,"snippet":"fn rate_limit_wait(response: &HttpResponse) -> Option<Duration> {","symbols":["rate_limit_wait"]},
  {"id":"DOM-PB-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1808,"snippet":"fake.push(403, &[], Vec::new());","symbols":["rate_limits_retry_within_bounds_or_report_typed_outcomes"]},
  {"id":"DOM-PB-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1823,"snippet":"fake.push(429, &[], Vec::new());","symbols":["rate_limits_retry_within_bounds_or_report_typed_outcomes"]}
]
```
