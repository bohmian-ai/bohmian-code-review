# TASK-002 r5 — task implementation review A

- Subject: candidate `6899a43` over base `1842eaf`; read from `/tmp/cr-r5/wt` only.
- Obligations: TASK-002 plus TASK-002-R1, -R2, -R3 (cumulative).
- Scratch: `/tmp/cr-r5/scratch-task-rev-a/ws` (rsync copy of the candidate), `CARGO_TARGET_DIR=/tmp/cr-r5/target-task-rev-a`.

**Overall: FAIL** (TASK-A-2 INCORRECT; TASK-A-1 DRIFT)

## Requirement matrix

| Requirement / AC / constraint / non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| T2 S1: pinned repo/branch/commit/tree/blob/PR identities verified; mismatches rejected before parsing | `github.rs:793-1036` compare id/full_name/branch/sha/tree/PR number+base repo; `git_blob_sha` check at `:935`; paths via `RelativePath` into `rejected` | `verified_provider_identities_cross_and_mismatches_are_rejected` (in 17/17 lib pass, rerun in scratch) | PASS |
| T2 S1: untrusted archive redirect rejected, no credential forwarding | `archive` `:956-990`, `is_trusted_redirect` `:741`, hop sent with `authenticated=false` | `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` | PASS |
| T2 S2 / REQ-013: rate exhaustion and retry-after yield typed retry/terminal outcomes; primary **and secondary** limits honored | `send_with_policy` `:1163-1209`, `rate_limit_wait` `:717-738` classify by headers only | Scratch test: 403 + GitHub secondary-limit message, no `retry-after`/`remaining: 0` → `Forbidden`, `Terminal` | **FAIL** (TASK-A-2) |
| T2 S2: cancellation/deadline explicit; no partial input as complete | `Execution::check` before each attempt and in `sleep_bounded`; `BoundedReader` errors instead of truncating | `malformed_payloads_and_interruptions_are_explicit`, `check_reports_cancellation_then_deadline` | PASS |
| T2 S2: 403/429, malformed payload, cancellation fake cases | `rate_limits_retry_within_bounds_or_report_typed_outcomes`, `malformed_payloads_and_interruptions_are_explicit` | lane green | PASS (cases exist; see TASK-A-2 for the missing secondary-limit case) |
| T2 AC: both GitHub hosts through configuration | `GitHubHost::{Public, enterprise}` | `hosts_expose_api_base_and_archive_host` | PASS |
| T2 AC: narrow injected storage/persistence traits, identity at every seam, no pool/SDK/consumer type | `storage.rs`, `persistence.rs` | code read; `Cargo.toml` deps: facade, serde, serde_json, sha1, sha2, thiserror, ureq | PASS |
| INV-001: no symbolic duplication, no server/worker, no disk source | facade-only dependency; no `std::fs`/tokio | `rg` in task evidence; code read | PASS |
| Credentials fail visibly and never leak | redacted `InstallationToken` Debug; `HttpRequest` no Debug; 401/403 typed | `token_debug_is_redacted` | PASS |
| Scoped `mise` lane in `test:rust` | `mise.toml` `test:pipeline` (all-targets + `--doc`) | `/tmp/cr-r5/lanes.txt`: fmt:check, lints, test:pipeline exit 0; doctests run | PASS |
| AC-R1-1 `ObjectKey::context(&ContextId)` | `storage.rs:78` | `keys_are_scoped_by_identity_and_version` | PASS |
| AC-R1-2 `ObjectKey` re-validated on reload; constructor keys round-trip | `ObjectKey::parse` `storage.rs:94`, manual `Deserialize` | `keys_are_revalidated_on_deserialization`, `records_revalidate_their_keys_on_deserialization` | PASS |
| AC-R1-3 `ContextId` validated, private, re-validating serde | `persistence.rs:66-96` | `context_ids_are_validated_on_construction_and_deserialization` | PASS |
| AC-R1-4 / AC-R2-4 `# Errors` on every fallible item | code read of every `Result`/`io::Result` fn | read-through; no missing section found | PASS |
| AC-R1-5 `ObjectKind` removed | absent | code read of `storage.rs`, `lib.rs` | PASS |
| AC-R2-1..3 long-segment round-trip; hostile set still refused; `validate_name` cap intact | `validate_key_segment` `identity.rs:149`; `validate_name` delegates, keeps 100-byte cap | `long_constructor_segments_round_trip` and the two r1 tests | PASS |
| AC-R2-5 external `Enterprise { .. }` construction fails to compile | doctest `identity.rs:310` | lane doctest ok | PASS (but now proved by `ApiBase` privacy, not `#[non_exhaustive]` — TASK-A-1) |
| AC-R2-6 `archive_host` has no indexing slice | `strip_prefix` + `split` | code read | PASS (fallback now unreachable — TASK-A-1) |
| AC-R3-1 `&mut` rewrite fails to compile | `ApiBase(String)` private field | doctest `identity.rs:317` ok | PASS |
| AC-R3-2 host accessors / trust tests unchanged | `identity.rs:370-387` | both tests pass | PASS |
| AC-R3-3 lane runs doctests | `mise.toml` second command | lanes.txt `Doc-tests` 2 passed | PASS |
| AC-R3-4 `read_bounded` cannot overflow | `saturating_add(1)` `github.rs:1273` | `read_bounded_accepts_the_maximum_limit` | PASS |
| AC-R3-5 `with_timeout` `# Panics` | `execution.rs:59-62` | code read | PASS |
| AC-R3-6 rustdoc on each `mod tests` | 5 modules | code read | PASS |
| AC-R3-7 no `std::fmt::` paths | `persistence.rs:12` `use std::fmt;` | code read; fmt/lints exit 0 | PASS |
| Non-goals (no facade, OpenDAL, limiter, real-GitHub run, symbolic crate edits) | diff stat touches only the new crate, `Cargo.toml`, `Cargo.lock`, `mise.toml` | `git diff --stat 1842eafd 6899a43` | PASS |
| Superseded remediation code deleted | `#[non_exhaustive]` (R2) and `archive_host` fallback remain after R3 made them redundant | scratch: `#[non_exhaustive]` removed → both doctests still `compile fail ... ok`, 17/17 lib tests pass | **FAIL** (TASK-A-1) |

Scope note: TASK-A-1 of r3 (binding a reloaded `SnapshotPointer.key`/`ContextRecord.key` to its scope) is an explicit R3 non-goal assigned to TASK-003..005 and is not re-raised. Once those facades compare reloaded keys against the constructor-derived key, re-check whether `ObjectKey::parse` is still needed outside `Persistence::remove_repository`.

## Findings

### TASK-A-1 — DRIFT: the GHES host seal is enforced three times, and one enforcement can never trigger

- **Root cause:** R2 sealed `GitHubHost::Enterprise` construction with `#[non_exhaustive]` (`identity.rs:328`). R3 then made the payload a private-field newtype `ApiBase` (`identity.rs:340`). That alone makes the variant impossible to build or overwrite from outside the crate, so the R2 attribute no longer does anything. The accessor `archive_host` still re-parses the validated base at every call and keeps a `""` fallback (`identity.rs:381-385`) for a state `enterprise()` already rules out.
- **Defect class:** remediation-round guard superseded by a later shape change. The owner's shape (an enum whose variant field needs a separate public-but-unexported newtype to be sealed) is what made three layers necessary.
- **Sibling locations:** `identity.rs:328` (`#[non_exhaustive]`); `identity.rs:340` (`pub struct ApiBase`: public, not re-exported, no accessor, reachable only as `identity::ApiBase`); `identity.rs:381-385` (`archive_host` re-parse plus unreachable `unwrap_or("")`); `identity.rs:310-322` (two `compile_fail` doctests that now prove Rust field privacy rather than a crate decision).
- **Violated obligation:** r5 review rule on superseded-fix deletion; `AGENTS.md` "Avoid ... future-proofing, and unnecessary features"; R3's own note that `#[non_exhaustive]` "becomes redundant once the payload is unconstructible".
- **Evidence:** in the scratch copy, deleting `#[non_exhaustive]` still gives `identity::GitHubHost (line 310) - compile fail ... ok` and `(line 317) - compile fail ... ok`, and `cargo test --all-targets --all-features` passes all 17 candidate tests. `enterprise()` refuses any base without the `https://` prefix, so `strip_prefix("https://")` in `archive_host` cannot fail on a value that exists.
- **Observable consequence:** external callers have to write `Enterprise { .. }` for no reason. The public type `ApiBase` is exposed but cannot be used. A reader sees three seals and a fallback and cannot tell which one carries the invariant.
- **Testable correction:** minimum fix: delete `#[non_exhaustive]`. Better, since `GitHubHost` is surface this candidate owns: replace the enum and `ApiBase` with `pub struct GitHubHost { api_base: String, archive_host: String }`, add `GitHubHost::public()`, and have `enterprise()` compute `archive_host` once during validation. The accessors then return the fields, which deletes `ApiBase`, the attribute, the re-parse and its fallback, and both doctests (private struct fields are sealed by the language). Keep `hosts_expose_api_base_and_archive_host` and the archive-redirect test unchanged apart from `GitHubHost::Public` becoming `GitHubHost::public()`.

### TASK-A-2 — INCORRECT: secondary rate limits sent as a 403 without rate headers are reported as terminal `Forbidden`

- **Root cause:** `rate_limit_wait` (`github.rs:717`) decides whether a response is a rate limit from the `retry-after` and `x-ratelimit-remaining: 0` headers alone. When both are absent, `send_with_policy` (`github.rs:1185`) maps a 403 to `GitHubError::Forbidden`, whose `disposition()` is `Terminal`. GitHub documents that a secondary rate limit comes back as 403 or 429 and may carry neither header. In that case the only signal is the response `message`, and clients are told to "wait for at least one minute before retrying".
- **Defect class:** provider rate-limit classification that reads headers only and misclassifies a documented response.
- **Sibling locations:** `github.rs:717` (`rate_limit_wait`, the owner); `github.rs:1185` (bare 403 → `Forbidden`); `github.rs:1187-1189` (bare 429 reports `retry_after: max_retry_wait`, which is a caller setting and can be below GitHub's documented one-minute minimum; the test at `:1818-1826` asserts 5 s); `github.rs:1808-1811` (the test fixes bare 403 as `Forbidden`/`Terminal`, with no secondary-limit case).
- **Violated obligation:** spec REQ-013 (`spec.md:76`, "honor installation-specific primary/secondary limits, retry-after/reset information"); TASK-002 Scenario 2 ("Rate exhaustion ... yield typed retry or terminal outcomes"); INV-006 (rate exhaustion must be reported, not disguised).
- **Evidence / reproduction:** a scratch test in `/tmp/cr-r5/scratch-task-rev-a/ws` pushes `403` with `x-ratelimit-remaining: 4999` and body `{"message":"You have exceeded a secondary rate limit. Please wait a few minutes before you try again.", ...}`. `client.repository(..)` returned `SCRATCH Forbidden Terminal`.
- **Observable consequence:** a cold build or PR overlay that trips a secondary limit (for example a burst of blob GETs) reports a permanent permission failure. The consumer then stops retrying, or treats the installation as lacking access, when waiting a minute would have worked.
- **Testable correction:** in `send_with_policy`, when a 403/429 has no rate headers, read the body through `read_bounded` (`max_json_bytes`). If `message` contains "rate limit" (case-insensitive), or the status is 429, return `RateLimited { retry_after: max(60 s, provider hint) }` so the disposition is `Retry`. Keep a 403 with any other message as `Forbidden`. Add one fake case: 403 with the secondary-limit message and no rate headers → `RateLimited { retry_after >= 60 s }`, `Disposition::Retry`. Keep the existing empty-body 403 → `Forbidden` case, and change the bare-429 expectation to the 60 s floor.

## Verification story

- Tests reviewed: all five test modules. The fake transport drives the real policy.
- Build and lanes: `/tmp/cr-r5/lanes.txt` shows `fmt:check`, `lints`, and `test:pipeline` exiting 0 with 2 doctests. The scratch run showed 17/17 candidate lib tests passing and 2/2 doctests passing with `#[non_exhaustive]` removed.
- Security: token redaction, the unauthenticated redirect hop, and the host seal all hold. Key traversal is refused on reload. Cross-scope key binding is deferred to TASK-003..005 per R3.

```citations
[
{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":328,"snippet":"    #[non_exhaustive]","symbols":["GitHubHost::Enterprise"]},
{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":340,"snippet":"pub struct ApiBase(String);","symbols":["ApiBase"]},
{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":381,"snippet":"            GitHubHost::Enterprise { api_base } => api_base","symbols":["GitHubHost::archive_host"]},
{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":385,"snippet":"                .unwrap_or(\"\"),","symbols":["GitHubHost::archive_host"]},
{"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":717,"snippet":"fn rate_limit_wait(response: &HttpResponse) -> Option<Duration> {","symbols":["rate_limit_wait"]},
{"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1185,"snippet":"                        None if response.status == 403 => return Err(GitHubError::Forbidden),","symbols":["GitHubClient::send_with_policy"]},
{"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1187,"snippet":"                            return Err(GitHubError::RateLimited {","symbols":["GitHubClient::send_with_policy"]},
{"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1808,"snippet":"        fake.push(403, &[], Vec::new());","symbols":["rate_limits_retry_within_bounds_or_report_typed_outcomes"]}
]
```
