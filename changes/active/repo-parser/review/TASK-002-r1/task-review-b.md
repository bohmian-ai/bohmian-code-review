# TASK-002 task review (sample B, r1)

## Subject

- Repository: `/Users/stevenforrester/Documents/GitHub/bohmian-code-review`, branch `review-helper-setup`.
- Base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`, candidate `dafe20bd6112b17aed345278d85025fe1a0aaeb2` (HEAD, clean tree at start).
- Task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md` (text above "Implementation Evidence"). Spec `changes/active/repo-parser/spec.md` rev 10; mapped REQ-004, REQ-013, REQ-015, REQ-016, INV-001, INV-003, INV-006.
- Read in full: `crates/bohmian-code-review-pipeline/src/{lib,identity,execution,github,storage,persistence}.rs` (github.rs all 1854 lines), `Cargo.toml`, `crates/bohmian-code-review-pipeline/Cargo.toml`, `mise.toml` diff, `git diff --stat 1842eaf..dafe20b`.
- Mode: read-only. No tracked file modified. A scratch copy under `/tmp/t2mut` (excluding `.git`/`target`) was mutated for one probe.

## Acceptance matrix

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004 both hosts via configuration | `identity.rs:264-318` `GitHubHost::{Public, enterprise}`, `api_base`/`archive_host`; every URL built from `host.api_base()` at `github.rs:1097` | `identity::tests::hosts_expose_api_base_and_archive_host` (rejects http, userinfo, trailing slash, query, empty host); GHES half of `archive_redirect_...` asserts GHES URL and same-host redirect rule | PASS |
| REQ-004 identity validated before source enters symbolic library | `TenantId`/`InstallationId`/`RepositoryIdentity`/`ObjectSha` validate on `new` and on `Deserialize` (`identity.rs:70,102,175,230` and custom `Deserialize` impls); `repository` checks numeric id + full_name (`github.rs:789`), `branch_head` name (`:823`), `commit` sha (`:851`), `tree` sha (`:881`), `blob` git-hash (`:924`), `pull_request` number + base repo id (`:997`) | `verified_provider_identities_cross_and_mismatches_are_rejected`: six mismatch cases each yield `IdentityMismatch`; mutation probe (below) shows the blob-hash check is load-bearing | PASS |
| Scenario 1 "mismatched path" rejected | Tree entries and PR files pass through `RelativePath::new` from the facade crate (`github.rs:1260,1274,1281`); invalid ones land in `rejected`, never as identities | Same test: `../escape` and `/abs` rejected; `weird` mode `777777` and status `teleported` rejected; counts asserted | PASS |
| Scenario 1 untrusted archive redirect rejected, credentials never forwarded (REQ-014 boundary) | `archive` follows exactly one redirect, only when `is_trusted_redirect(location, host.archive_host())`, and the hop is built with `authenticated=false` (`github.rs:954-967`); `is_trusted_redirect` requires `https://` and exact host match (`:729-734`) | `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials`: asserts no `Authorization` on hop, untrusted host never requested (seen count unchanged), GHES same-host accepted and `codeload` rejected for GHES; helper test rejects `http://` and `codeload.github.com.evil`; mutation probe shows the check is load-bearing | PASS |
| Verified blob bytes (INV-003) | `blob` reads at most `max_blob_bytes` then compares `git_blob_sha` (SHA-1 over `blob <len>\0`) to the requested id (`github.rs:919-930`) | `helpers_encode_paths_and_hash_blobs_like_git` pins empty-blob and `hello\n` ids to Git's known values; tampered body yields `IdentityMismatch` | PASS |
| REQ-013 primary/secondary limits, retry-after/reset, bounded retry time | One policy `send_with_policy` (`github.rs:1127-1173`): `execution.check()` before each attempt, timeout = min(remaining, request_timeout), 403/429 classified by `rate_limit_wait` (`retry-after`, then `x-ratelimit-remaining: 0` + reset), waits capped by `max_retry_wait` and remaining deadline, `max_attempts`, cancellable `sleep_bounded` | `rate_limits_retry_within_bounds_or_report_typed_outcomes`: 403+retry-after 0 retried once (2 calls), 429+reset retried, reset 1h out → `RateLimited(>=3590s)` `Retry{after}`, attempts exhausted → `RateLimited(0)`, bare 403 → `Forbidden` Terminal, 401/404 typed, bare 429 → `RateLimited(max_retry_wait)` without sleeping | PASS |
| REQ-013 concurrency ceilings | Not implemented; client is synchronous, one request in flight per call; evidence section states caller worker pool bounds concurrency | Task scenarios do not name a concurrency ceiling; Scenario 2 REFACTOR forbids a general provider framework | PARTIAL, deferred (no finding; see Observations) |
| REQ-013 cancellation and deadline reported honestly; Scenario 2 cancellation/deadline outcomes | `Execution::check` (`execution.rs:78`) → `Interrupted::{Cancelled, DeadlineExceeded}`; `GitHubError::Interrupted(#[from])`; retry wait that would cross deadline is reported, not slept (`github.rs:1166-1170`) | `malformed_payloads_and_interruptions_are_explicit`: cancelled/expired executions send zero requests; `retry-after: 2` under 1 s deadline → `RateLimited(2s)`; `execution::tests::check_reports_cancellation_then_deadline` | PASS |
| Scenario 2 malformed payload, 5xx, transport | `parse_json` reports category + position only (`github.rs:1232-1255`), `read_bounded` → `ResponseTooLarge` (`:1217-1229`); 5xx and transport retried under backoff then surfaced with `Retry{after: None}` | Same test: wrong shape → `MalformedResponse` Terminal; 65 B over 64 B limit → `ResponseTooLarge{limit:64}`; 503→502 → `Unavailable{502}` Retry; Timeout→Failed → `Transport(Failed)` | PASS |
| INV-006 / Scenario 2 "no facade can present partial input as complete" for every collection | `TreeListing.truncated` + `rejected` (`github.rs:441-450`); `PullRequestFilesPage.has_next` + `rejected` (`:536-543`); `Archive` wraps `BoundedReader` which errors instead of truncating at the limit (`:551-576`) | tree test asserts `truncated` and `rejected`; PR-files test asserts `has_next` from `Link rel="next"` and `rejected`; archive test reads 5 B under a 4 B limit and gets an IO error, not a short read | PASS |
| REQ-015 narrow persistence trait: no pool, no consumer type, identity on every method, conditional publication, cleanup | `persistence.rs:104-179` `Persistence`: every method takes `Identity`/`SnapshotScope`; `publish_snapshot_pointer` is compare-and-set; `expired_contexts`, `delete_context`, `remove_repository` cover REQ-014/REQ-006 cleanup; `cargo tree` shows no SQL/pool/consumer dependency | Trait conformance tests are TASK-006 scope per task doc; `cargo tree -p bohmian-code-review-pipeline --depth 1 -e normal` observed: facade, serde, serde_json, sha1, sha2, thiserror, ureq | PASS (contract only, as tasked) |
| REQ-016 narrow storage trait; keys scoped by identity | `storage.rs:139-163` `ObjectStorage::{create,read,delete}` create-only; `ObjectKey` constructible only via `snapshot`/`context` from `Identity` (`:56-89`); `ObjectDigest` SHA-256 | `storage::tests::keys_are_scoped_by_identity_and_version`, `digest_is_sha256_hex`; OpenDAL adapter explicitly out of scope for this task | PASS |
| INV-001 no clone/checkout, no disk source, no symbolic duplication, no server/worker loop | `rg "std::fs|tokio|axum|std::net|std::process" crates/bohmian-code-review-pipeline/src` → no matches; only `RelativePath` imported from facade; blob/archive return transient bytes/streams | Observed | PASS |
| INV-003 facts bound to verified identity | Every sha/tree/blob is an `ObjectSha` verified against request; `SnapshotPointer` carries key + digest; pointer publication is conditional | As above | PASS (remaining INV-003 obligations belong to TASK-003/004) |
| Credentials fail visibly, never leak | `InstallationToken` redacted `Debug` (`identity.rs:352`), `HttpRequest` has no `Debug`; `TransportError::Failed` carries `ureq` error display only; JSON errors never echo body | `identity::tests::token_debug_is_redacted`; 401/403 typed | PASS |
| Constraint: imports no consumer type, no Git provider SDK, no model SDK, no Node/SQLite/PG/PyO3 | `Cargo.toml` deps: facade, serde, serde_json, sha1, sha2, thiserror, ureq (rustls, platform-verifier, no default features) | `cargo tree` observed above | PASS |
| Write set: new crate, workspace manifest/lockfile, provider contract tests, scoped `mise` lane | `git diff --stat`: Cargo.toml, Cargo.lock, mise.toml, 7 crate files, task doc only | Observed | PASS |
| Non-goals: no facades, no OpenDAL, no tar/gzip decode, no real-GitHub run | None present in crate | `rg` for opendal/flate2/tar → none | PASS |
| Verification: focused lane, fmt, lints (gate is orchestrator's) | `mise.toml` `test:pipeline` wired into `test:rust` (which `gate` depends on) | Observed below | PASS |
| RED discipline (`architecture/references/languages/spec-driven-development.md:206-208`) | Evidence admits tests written alongside; only one behavioral RED recorded; crate-absent compile failure does not count | Gap closed by read-only mutation probe below (tests fail on removed identity and redirect checks); recorded, not a finding per contract | PASS (evidence supplied here) |
| Dependency ladder: `ureq` | No HTTP client in workspace; stdlib has none; rung 7 | Minimal feature set | ACCEPT |
| Dependency ladder: `sha1` | Git blob ids are SHA-1; `sha2` cannot produce them; rung 5 fails | Pinned against Git known values | ACCEPT |
| Ponytail: `ObjectKind` / `ObjectKey::kind()` | Unused outside its own test; substring sniff of the key text; no task scenario or later-facade need stated | Only `storage::tests` references it | DRIFT → TASK-B-1 |

## Findings

### TASK-B-1 — DRIFT: unused key-kind classifier on `ObjectKey`

- Root cause: `storage.rs:38-45` defines `ObjectKind` and `storage.rs:97-103` `ObjectKey::kind()` classifies a key by `contains("/contexts/")`. Nothing in the crate or task consumes it; `lib.rs:32` exports it publicly.
- Defect class: speculative public surface (YAGNI), plus a string-sniffing heuristic that would be wrong for any future key shape.
- Siblings: none (only `storage.rs:40`, `storage.rs:97`, `lib.rs:32`, and the two asserts in `storage::tests::keys_are_scoped_by_identity_and_version` at `storage.rs:192,198`).
- Violated rule: `AGENTS.md` "Avoid ... future-proofing, and unnecessary features"; `~/.claude/CLAUDE.md` "No speculative abstractions".
- Evidence: `rg -n "ObjectKind|\.kind\(\)" crates/bohmian-code-review-pipeline/src` → definitions, the `lib.rs` re-export, and two test asserts only.
- Consequence: public API that later tasks may build on instead of carrying the kind explicitly; the classifier silently returns `Snapshot` for any key that is not a context key.
- Testable correction: delete `ObjectKind`, `ObjectKey::kind`, the `lib.rs` re-export, and the two asserts; `mise run test:pipeline`, `mise run lints` stay green. If a later facade needs the kind, it already knows which constructor it called.

No MISSING, INCORRECT, VIOLATION, or REGRESSION findings.

## Observations (not findings)

- REQ-013 "concurrency ceilings": not implemented in this task. The task's scenarios and acceptance criteria do not name it, and the client is per-call synchronous; the facade tasks (TASK-003–005) or the caller's worker pool must bound it. The change reviewer should confirm it lands somewhere before the integrated journey.
- Tests are inline (`github::tests`) rather than an external "outer-crate integration test" as the task's RED step phrases it. `architecture/agent-rules.md:5` makes inline the default and reserves external tests for cross-crate facade journeys; the inline tests drive the public `GitHubClient` through the `HttpTransport` seam, which is the intent.

## Verification commands and observed results

| Command | Observed |
|---|---|
| `mise run test:pipeline` | 12 passed, 0 failed, exit 0 |
| `mise run fmt:check` | exit 0 |
| `mise run lints` (clippy `-D warnings` + `scripts/check-test-placement.sh`) | exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib github::tests::verified_provider_identities_cross_and_mismatches_are_rejected -- --exact` | 1 passed |
| `... github::tests::rate_limits_retry_within_bounds_or_report_typed_outcomes -- --exact` | 1 passed |
| `... github::tests::malformed_payloads_and_interruptions_are_explicit -- --exact` | 1 passed |
| `... github::tests::archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials -- --exact` | 1 passed |
| `mise exec -- cargo tree -p bohmian-code-review-pipeline --depth 1 -e normal` | facade, serde, serde_json, sha1, sha2, thiserror, ureq |
| `rg -n "std::fs\|tokio\|axum\|std::net\|std::process" crates/bohmian-code-review-pipeline/src` | no matches |
| Mutation probe in `/tmp/t2mut` copy: blob-hash check (`github.rs:924`) and redirect trust check (`github.rs:956`) disabled with `if false &&`; `cargo test --locked -p bohmian-code-review-pipeline --lib github::tests` | `verified_provider_identities_cross_and_mismatches_are_rejected` FAILED at `github.rs:1552` (blob mismatch expected); `archive_redirect_...` FAILED (untrusted redirect was followed: "scripted response for every request"); 3 passed, 2 failed |

Not covered: `mise run gate` (orchestrator's), real-GitHub contract run (explicitly outside CI), ureq runtime behavior against a live host (fake transport only, as the task specifies).

## Overall result

PASS

```citations
[
  {"id": "TASK-B-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 40, "snippet": "pub enum ObjectKind {", "symbols": ["ObjectKind"]},
  {"id": "TASK-B-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 97, "snippet": "    pub fn kind(&self) -> ObjectKind {", "symbols": ["ObjectKey::kind", "ObjectKind"]},
  {"id": "TASK-B-1", "path": "crates/bohmian-code-review-pipeline/src/lib.rs", "line": 32, "snippet": "pub use storage::{ObjectDigest, ObjectKey, ObjectKind, ObjectStorage, StorageError};", "symbols": ["ObjectKind"]}
]
```
