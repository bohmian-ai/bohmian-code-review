# TASK-002 task review — sample A (r1)

## Subject

- Task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md` (text above "Implementation Evidence"); spec `SPEC-repo-parser@10`, maps REQ-004, REQ-013, REQ-015, REQ-016, INV-001, INV-003, INV-006.
- Base `1842eaf` → candidate `dafe20b` (== HEAD, working tree clean apart from this review directory).
- Read in full: `crates/bohmian-code-review-pipeline/src/{lib,identity,execution,github,storage,persistence}.rs` (github.rs 1854 lines), `Cargo.toml`, `mise.toml`, crate manifest, `git diff --stat`, AGENTS.md, `architecture/agent-rules.md`, `architecture/references/languages/rust-core.md` (trait rule), TASK-006 scope, and the vendored `ureq 3.4.2` redirect/timeout code (`src/run.rs`, `src/config.rs`) to confirm production transport semantics the fake cannot prove.
- Read-only audit; nothing tracked was modified.

## Acceptance matrix

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004 both hosts via configuration | `identity.rs:264-318` `GitHubHost::{Public, enterprise}`; `github.rs:1097` every API URL from `host.api_base()`; archive trust host from `archive_host()` | `identity::tests::hosts_expose_api_base_and_archive_host`; GHES half of `archive_redirect_…` asserts `https://ghe.example.com/api/v3/repos/...` URL and same-host rule | PASS |
| REQ-004 identity validation before source enters library (repository, commit/tree, path, blob) | `github.rs:789-796` id + full_name; `:823` branch name; `:851` commit sha; `:881` tree sha; `:923-929` blob SHA-1 recomputed via `git_blob_sha`; `:997` PR number + base repo id; tree/PR paths through `RelativePath` (`:1260`, `:1274`), invalid entries → `rejected` | `verified_provider_identities_cross_and_mismatches_are_rejected` (observed 1 passed): six mismatch cases each `IdentityMismatch`; `../escape`, `/abs` rejected; blob tamper rejected | PASS |
| REQ-004 unauthorized / oversize / unsupported PR state fail explicitly | `status_error` 401→`Unauthorized`, 403→`Forbidden`, 404→`NotFound`; `read_bounded` → `ResponseTooLarge`; `pull_request` unknown state → `MalformedResponse` | 401/403/404 cases in `rate_limits_…`; 65-byte body over 64 limit in `malformed_…` (observed) | PASS |
| REQ-004 credentials handled visibly, never leak | `InstallationToken` redacted `Debug` (`identity.rs:352`), `HttpRequest` has no `Debug` (`github.rs:42-51`); `parse_json` reports category+position only (`:1236-1244`) | `identity::tests::token_debug_is_redacted`; malformed detail asserted to start with `unexpected json shape` | PASS |
| REQ-013 retry-after / reset / bounded retry time | Single policy `send_with_policy` (`github.rs:1127-1173`): `execution.check()` before every attempt, `rate_limit_wait` parses `retry-after` then `x-ratelimit-remaining/reset`, waits capped by `max_retry_wait` and remaining deadline, `max_attempts`; `sleep_bounded` sliced at 100 ms observing cancellation | `rate_limits_retry_within_bounds_or_report_typed_outcomes` (observed 1 passed): retry-after 0 retried once; reset-in-past retried once; reset +1h → `RateLimited(≥3590s)` w/o sleeping; attempts exhausted → `RateLimited(0)`; bare 429 → `RateLimited(max_retry_wait)` | PASS |
| REQ-013 cancellation and deadline reported honestly | `Execution::check` (`execution.rs:78-83`), `GitHubError::Interrupted`; retry wait ≥ remaining → typed error, not sleep (`:1166-1170`); per-request timeout = `min(remaining, request_timeout)` (`:1139`) | `malformed_…`: cancelled and expired executions send zero requests; `retry-after: 2` under 1 s deadline → `RateLimited(2s)`; `execution::tests::check_reports_cancellation_then_deadline` | PASS |
| REQ-013 concurrency ceilings | Not implemented; client is synchronous, per-call. Task scenarios name only rate exhaustion, retry-after, cancellation, deadline; facade-level ceilings belong to TASK-003–005 | n/a for this task; flagged as open obligation for facade tasks, not a TASK-002 defect | N/A (later task) |
| REQ-013 size bounds (blob, JSON, archive) | `RequestLimits` (`:167-194`); `read_bounded` (`:1217`), `BoundedReader` fails rather than truncates (`:558-576`) | blob/JSON `ResponseTooLarge`; archive 5 bytes over 4-byte limit → read error (observed) | PASS |
| REQ-014 (supporting) archive redirect: no credential forwarding, trusted host only, one hop | `archive` (`:944-978`): redirect only when `is_trusted_redirect(location, archive_host)`; hop sent `authenticated=false`; second redirect on hop → `UnexpectedStatus` via `expect_status` | `archive_redirect_…` (observed 1 passed): hop has no `Authorization`; evil host never requested (seen len 3); GHES cross-host rejected. Production transport: `ureq` `max_redirects(0)` verified in vendored `config.rs:548-552` — with 0 the 3xx is returned as a response, never `TooManyRedirects`, so the client's own hop logic is reachable in production | PASS |
| REQ-015 narrow persistence trait, identity at every seam, no pool, no consumer type | `persistence.rs:104-179` `Persistence`: pointer read/CAS-publish, context publish/exact lookup/expired/delete, `remove_repository`; every method carries `Identity`/`SnapshotScope`; no SQL/pool types | Trait compiles under lane; in-memory conformance implementation is TASK-006's write set (`TASK-006…md:52`) | PASS (contract only, as tasked) |
| REQ-016 narrow storage trait, exact-key, create-only, delete | `storage.rs:139-163` `ObjectStorage::{create,read,delete}`; `ObjectKey` only constructible from `Identity` (`:70-89`) | `storage::tests::keys_are_scoped_by_identity_and_version`, `digest_is_sha256_hex` (observed pass). OpenDAL adapter is TASK-006 | PASS (contract only, as tasked) |
| INV-001 no clone/checkout, no symbolic duplication, no server/worker | `cargo tree -p bohmian-code-review-pipeline --depth 1`: only `bohmian-code-review`, serde, serde_json, sha1, sha2, thiserror, ureq; `rg "std::fs|tokio|axum"` → none; `Archive`/`blob` return transient bytes; no extraction/index/select code | Observed `cargo tree` output; rg | PASS |
| INV-003 blob integrity | `blob` recomputes Git SHA-1 over `blob <len>\0` (`:688-694`, `:923`) | `helpers_encode_paths_and_hash_blobs_like_git` (empty blob `e69de29…`, `hello\n` `ce01362…`) | PASS |
| INV-006 no hidden budget success | `TreeListing.truncated`, `PullRequestFilesPage.has_next`, `rejected` lists surfaced; rate exhaustion always a typed error; `Disposition` retry/terminal (`:342-355`) | tree `truncated`, `has_next`, and dispositions asserted in scenario tests | PASS |
| Scenario 1 RED/GREEN/REFACTOR | Fake provider drives real client through public `GitHubClient`; identities centralized in `identity.rs` | Observed: `verified_provider_identities_…` and `archive_redirect_…` pass; 13 API requests counted | PASS |
| Scenario 2 RED/GREEN/REFACTOR (403/429, malformed, cancellation, one policy) | One `send_with_policy` used by every operation incl. archive hop | Observed: `rate_limits_…`, `malformed_…` pass | PASS |
| Constraint: no consumer import, no server/worker loop | lib.rs deps; no loops beyond bounded retry | `cargo tree`; read | PASS |
| Constraint: never retain source on disk | No `std::fs`; bytes returned to caller | rg | PASS |
| Write set: new crate, manifest/lockfile, provider contract tests, scoped mise lane | `git diff --stat`: exactly Cargo.toml, Cargo.lock, mise.toml, crate files, task doc | Observed diff stat (11 files) | PASS |
| Tests inline (agent-rules.md:5) vs task's "outer-crate integration test" | Inline module tests exercise the public `GitHubClient`; agent-rules makes inline the default and reserves external tests for cross-crate journeys | `scripts/check-test-placement.sh` passes in `lints` | PASS |
| Rustdoc on every new item incl. private and tests (agent-rules.md:11) | All items, wire structs, fakes and tests documented | Read; `lints` exit 0 | PASS |
| No unchecked `unwrap()` on input/IO | Only `unwrap_or`/`unwrap_or("")` in production code; `expect` only in tests | rg | PASS |
| Non-goals excluded (facades, OpenDAL, tar decoding, real-GitHub run) | Absent | Read | PASS |
| Verification: focused lane, fmt, lints (gate is orchestrator's) | — | `mise run test:pipeline` 12 passed exit 0; `fmt:check` exit 0; `lints` exit 0 (50 warnings, all `bohmian-code-review-extract` C grammar build output, none from pipeline); `git diff --check` clean | PASS |
| Ponytail: dependencies `ureq`, `sha1`, `sha2` | `ureq` (no default features, rustls) is the HTTP need; `sha1` required for Git blob ids (stdlib has none); `sha2` already in workspace, used for `ObjectDigest` required by REQ-016 integrity metadata | `cargo tree` | PASS |
| Ponytail: traits | `HttpTransport` has two real impls (`UreqTransport`, scripted fake); `ObjectStorage`/`Persistence` are the task-mandated injection boundary | Read | PASS |
| Ponytail: speculative public API | `ObjectKind`/`ObjectKey::kind()` and `CallCounts::snapshot`/`Operation::ALL` have no production caller | See TASK-A-1 | DRIFT |

## Findings

### TASK-A-1 — DRIFT (non-blocking)

- Root cause: public surface added without a caller in this task or a named consumer in TASK-003–007. `ObjectKind` + `ObjectKey::kind()` reconstruct an object's kind by substring-sniffing the key (`storage.rs:97-103`), which the library never needs because the caller always knows which key constructor it used.
- Defect class: speculative public API (Ponytail rung 1: does not need to exist).
- Siblings (same class): `Operation::ALL` + `CallCounts::snapshot()` (`github.rs:219-228`, `:247-253`) exist only so one test can sum counts; `CallCounts::get` already provides accounting.
- Violated rule: `architecture/agent-rules.md:10` ("Avoid ... speculative abstraction layers"); AGENTS.md "No over-engineering. No speculative abstractions."
- Evidence: `rg "kind\(\)|ObjectKind|Operation::ALL|\.snapshot\(\)"` across `src/` hits only definitions, `lib.rs` re-exports, and tests.
- Observable consequence: none at runtime; extra exported API to maintain and a key-string heuristic that would silently misclassify if key layout ever changed.
- Testable correction: delete `ObjectKind`, `ObjectKey::kind`, `Operation::ALL`, `CallCounts::snapshot` and their `lib.rs` re-exports; rewrite the two test assertions with `calls().get(op)`. `test:pipeline`, `fmt:check`, `lints` must remain green.

No MISSING, INCORRECT, VIOLATION, or REGRESSION findings. Probed and cleared: production redirect handling under `ureq max_redirects(0)` (returns 3xx as response, `config.rs:548-552`, `run.rs:227-247`); `timeout_global` covers body transfer (`config.rs:720-726`); 403-with-`retry-after` vs plain 403 split; rate-limit wait crossing deadline reported not slept; blob raw media type; `RelativePath` rejecting `../` and absolute paths; token never attached on the archive hop; `Interrupted::Cancelled` terminal / `DeadlineExceeded` retryable.

## Verification commands and observed results

| Command | Observed |
|---|---|
| `mise run test:pipeline` | 12 passed, 0 failed, exit 0 |
| `mise run fmt:check` | exit 0 |
| `mise run lints` | exit 0; 50 `warning:` lines, all `bohmian-code-review-extract` grammar C build output |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib github::tests::verified_provider_identities_cross_and_mismatches_are_rejected -- --exact` | 1 passed |
| `... github::tests::archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials -- --exact` | 1 passed |
| `... github::tests::rate_limits_retry_within_bounds_or_report_typed_outcomes -- --exact` | 1 passed |
| `... github::tests::malformed_payloads_and_interruptions_are_explicit -- --exact` | 1 passed |
| `mise exec -- cargo tree -p bohmian-code-review-pipeline --depth 1 -e normal` | bohmian-code-review, serde, serde_json, sha1, sha2, thiserror, ureq 3.4.2 |
| `git diff --check 1842eaf..dafe20b` | clean |
| `git diff --stat 1842eaf..dafe20b` | 11 files, matches declared write set |

Not covered: `mise run gate` (orchestrator); real-GitHub contract run (explicitly outside normal CI per task).

## Overall result

PASS — TASK-A-1 is a non-blocking DRIFT cleanup.

```citations
[{"id": "TASK-A-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 97,
  "snippet": "    pub fn kind(&self) -> ObjectKind {", "symbols": ["ObjectKey", "ObjectKind"]},
 {"id": "TASK-A-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 40,
  "snippet": "pub enum ObjectKind {", "symbols": ["ObjectKind"]},
 {"id": "TASK-A-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 219,
  "snippet": "    const ALL: [Operation; 8] = [", "symbols": ["Operation"]},
 {"id": "TASK-A-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 247,
  "snippet": "    pub fn snapshot(&self) -> BTreeMap<Operation, u64> {", "symbols": ["CallCounts", "Operation"]}]
```
