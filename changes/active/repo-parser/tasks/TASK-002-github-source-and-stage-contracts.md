---
id: TASK-002
kind: implementation
status: review
spec: SPEC-repo-parser@10
depends_on: [TASK-001]
maps: [REQ-004, REQ-013, REQ-015, REQ-016, INV-001, INV-003, INV-006]
---

## Outcome and Value

The new outer library can obtain and verify GitHub/GHES repository, commit, tree, blob, and PR identities through provider APIs. It defines narrow storage and persistence traits that later stage facades and any consuming crate can implement.

## Owners, Scope, Consumers, and Prohibited Changes

The new library crate owns provider IO, trust boundaries and dependency-injection traits. Callers supply credentials, trait implementations, deadlines and cancellation. The crate imports no consumer type and runs no server or worker loop. Never clone, check out, or retain source on disk. Provider limits and credential handling must fail visibly.

## Approach

1. Add the outer crate to the workspace with domain values for verified host/installation/repository and pinned Git identities.
2. Define narrow injected storage and persistence capabilities needed by later stage facades.
3. Implement bounded GitHub/GHES source calls, authentication, response validation, and rate/error classification without duplicating symbolic algorithms.

## Ordered Implementation Scenarios

### Scenario 1 — Verified provider identities

**Behavior.** A controlled fake provider returns pinned branch/tree/blob/PR data; mismatched repository, path, SHA, or untrusted archive redirect is rejected before symbolic parsing.

**RED.** Add an outer-crate integration test that supplies valid and mismatched provider responses; it fails before verified source access exists.

**GREEN.** Implement only the source operations and identity checks needed by cold, update, and PR facades; rerun the scenario.

**REFACTOR.** Centralize the common validated identities while keeping provider-specific response handling explicit.

### Scenario 2 — Bounded provider failure

**Behavior.** Rate exhaustion, retry-after, cancellation, and deadline yield typed retry or terminal outcomes; no facade can present partial input as complete.

**RED.** Add fake-provider cases for 403/429, malformed payload, and cancellation; each initially fails its expected outcome.

**GREEN.** Add bounded request accounting and error classification; rerun the first scenario and these failures.

**REFACTOR.** Keep one provider request policy for all stages without introducing a general provider framework.

## Acceptance Criteria

Both GitHub hosts are supported through configuration; verified identities and blob bytes cross the trust boundary, and provider retry/deadline states remain explicit. TASK-003–TASK-005 implement and prove the individual facades; the integrated journey later proves their composition.

## Expected Write Set and Consumer Closure

New outer crate, workspace manifest/lockfile, provider contract tests, and a scoped `mise` lane. Later cold/update/PR tasks consume the same source and capability contracts.

## Verification and Evidence

Run the new crate's focused test lane, `mise run fmt:check`, `mise run lints`, and `mise run gate`. Record fake-provider requests and failures. The controlled real-GitHub contract run remains an integrated acceptance gate outside normal CI.

## Material Stop Conditions

Stop if GitHub/GHES cannot provide the required pinned identity or if the dependency contracts require importing a consumer crate or raw SQL pools.

## Authority Links

[Revision 10 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md), [library design](../../../../architecture/bohmian-code-review-design.md).

## Implementation Evidence

Candidate: commit `1696545` on `review-helper-setup` (base `1842eaf`). New crate `crates/bohmian-code-review-pipeline` (`identity`, `execution`, `github`, `storage`, `persistence` modules), workspace manifest and lockfile (`ureq`, `sha1`), and the `test:pipeline` lane in `mise.toml`. Tests are inline per `architecture/agent-rules.md`; the scripted `FakeTransport` drives the real request policy, parsing, and identity verification through the public `GitHubClient`.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Both GitHub hosts supported through configuration (REQ-004) | `identity::GitHubHost::{Public, enterprise}` with `api_base`/`archive_host`; `GitHubClient::request` builds every URL from the host | `identity::tests::hosts_expose_api_base_and_archive_host`; GHES half of `github::tests::archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` | PASS |
| Verified identities cross the trust boundary (REQ-004, INV-003, S1 of this task) | `TenantId`, `InstallationId`, `RepositoryIdentity`, `Identity`, `ObjectSha` validate on construction and deserialization; `GitHubClient::{repository, branch_head, commit, tree, pull_request}` compare provider ids/names/shas to the request and fail with `IdentityMismatch`; tree and PR-file paths go through `RelativePath`, invalid entries land in `rejected` | `github::tests::verified_provider_identities_cross_and_mismatches_are_rejected`; `identity::tests::identities_validate_key_safe_spellings`; `identity::tests::object_sha_accepts_only_lowercase_hex` | PASS |
| Verified blob bytes (REQ-004, INV-003) | `GitHubClient::blob` fetches raw bytes under `max_blob_bytes` and checks `git_blob_sha` against the requested id | blob half of `verified_provider_identities_cross_and_mismatches_are_rejected`; `github::tests::helpers_encode_paths_and_hash_blobs_like_git` | PASS |
| Untrusted archive redirect rejected; credentials never forwarded (REQ-014) | `GitHubClient::archive` follows one redirect only to `GitHubHost::archive_host`, sends the hop unauthenticated, bounds the stream with `BoundedReader` | `github::tests::archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` (asserts no `Authorization` on the hop, no request to the untrusted host, GHES same-host rule, stream limit error) | PASS |
| Provider retry/deadline states explicit (REQ-013, INV-006, S2 of this task) | One policy in `GitHubClient::send_with_policy`: `Execution::check` before every attempt, `retry-after`/`x-ratelimit-reset` waits bounded by `max_retry_wait` and remaining deadline, `max_attempts`, cancellable `sleep_bounded`; `GitHubError::disposition` types `Retry`/`Terminal`; `CallCounts` records every request | `github::tests::rate_limits_retry_within_bounds_or_report_typed_outcomes`; `github::tests::malformed_payloads_and_interruptions_are_explicit`; `execution::tests::check_reports_cancellation_then_deadline` | PASS |
| Credentials fail visibly and never leak (REQ-004, REQ-014) | `InstallationToken` redacted `Debug`, `HttpRequest` has no `Debug`; 401 → `Unauthorized`, 403 without rate headers → `Forbidden`; JSON errors carry category and position, never body text | `identity::tests::token_debug_is_redacted`; 401/403 cases in `rate_limits_retry_within_bounds_or_report_typed_outcomes`; malformed case in `malformed_payloads_and_interruptions_are_explicit` | PASS |
| Narrow injected storage and persistence contracts (REQ-015, REQ-016, INV-007) | `storage::{ObjectStorage, ObjectKey, ObjectDigest, StorageError}`; `persistence::{Persistence, SnapshotScope, SnapshotPointer, ContextId, ContextRecord, PersistenceError}`; every key and method carries `Identity`; no pool, SDK, or consumer type | `storage::tests::keys_are_scoped_by_identity_and_version`; `storage::tests::digest_is_sha256_hex`; trait conformance is TASK-006's scope | PASS |
| No symbolic duplication, no server/worker, no source retention (INV-001) | Crate depends on `bohmian-code-review` facade only; no extraction/index/select code; `Archive` and `blob` return transient bytes; nothing writes disk | `cargo tree -p bohmian-code-review-pipeline --depth 1`; `rg -n "std::fs|tokio|axum" crates/bohmian-code-review-pipeline` → no matches | PASS |
| Scoped `mise` lane | `mise.toml` `test:pipeline`, wired into `test:rust` | `mise run test:pipeline` → 12 passed | PASS |

RED/GREEN record. The crate did not exist at base, so every scenario failed to compile before this change. Tests were written alongside the client, not strictly before it; the one observed RED was `rate_limits_retry_within_bounds_or_report_typed_outcomes` failing with "scripted response for every request" because a 429 without rate headers slept the full `max_retry_wait` and retried. The policy was corrected to report `RateLimited { retry_after: max_retry_wait }` without sleeping, and in-call backoff is now capped by `max_retry_wait`; the test then passed.

Fake-provider requests and failures observed: 13 API requests in the identity scenario (2 `Repository`, 2 `Blob`, all authenticated to the API host); 3 `Archive` requests per host in the redirect scenario (trusted hop unauthenticated, untrusted hop never sent); rate-limit scenario: 403+`retry-after: 0` retried once, 429+reset-in-past retried once, reset one hour out → `RateLimited(≥3590s)`, two `retry-after: 0` under `max_attempts: 2` → `RateLimited(0s)`, bare 403 → `Forbidden`, 401 → `Unauthorized`, 404 → `NotFound`, bare 429 → `RateLimited(5s)`; malformed scenario: wrong JSON shape → `MalformedResponse` terminal, 65-byte body over a 64-byte limit → `ResponseTooLarge`, 503 then 502 → `Unavailable(502)` retryable, timeout then reset → `Transport` retryable, cancelled and expired executions send zero requests, `retry-after: 2` under a 1 s deadline → `RateLimited(2s)` without sleeping.

Non-goals confirmed excluded: no cold/update/PR facade, no OpenDAL adapter, no tar/gzip decoding, no concurrency limiter, no real-GitHub run. Provider concurrency ceilings (REQ-013) are left to the caller's worker pool because the client is synchronous and per-call; the real-GitHub contract run remains the opt-in integrated gate. Only the listed files changed.

Commands run in this session (all exit 0): `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`.

Focused commands (current GREEN closure, `mise exec -- ` prefix):

| Command | Observed |
|---|---|
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib github::tests::verified_provider_identities_cross_and_mismatches_are_rejected -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib github::tests::archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib github::tests::rate_limits_retry_within_bounds_or_report_typed_outcomes -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib github::tests::malformed_payloads_and_interruptions_are_explicit -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib github::tests::helpers_encode_paths_and_hash_blobs_like_git -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib identity::tests::identities_validate_key_safe_spellings -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib identity::tests::object_sha_accepts_only_lowercase_hex -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib identity::tests::hosts_expose_api_base_and_archive_host -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib identity::tests::token_debug_is_redacted -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib execution::tests::check_reports_cancellation_then_deadline -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib storage::tests::keys_are_scoped_by_identity_and_version -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib storage::tests::digest_is_sha256_hex -- --exact` | 1 passed, exit 0 |

### TASK-002-R1 remediation evidence

Remediation task: `changes/active/repo-parser/review/TASK-002-r1/TASK-002-R1-validated-seam-values.md`. Commits `30b5191` (FIND-TASK-002-1, -3) and `5a8cf76` (FIND-TASK-002-2) on top of `dafe20b`.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| AC-R1-1 `ObjectKey::context` takes `&ContextId` | `storage.rs` `ObjectKey::context(identity: &Identity, context_id: &ContextId)` | `storage::tests::keys_are_scoped_by_identity_and_version` (unchanged expected string) | PASS |
| AC-R1-2 `ObjectKey` re-validated on deserialization; constructor keys round-trip | `storage.rs` manual `Deserialize for ObjectKey` via private `ObjectKey::parse`, every `/` segment through `identity::validate_name` | `storage::tests::keys_are_revalidated_on_deserialization` (7 hostile strings incl. `../x`, `./x`, empty segment, leading/trailing `/`, space, empty); `persistence::tests::records_revalidate_their_keys_on_deserialization` (`SnapshotPointer` and `ContextRecord` refuse a traversing key, round-trip otherwise) | PASS |
| AC-R1-3 `ContextId` validated, private field, re-validating `Deserialize` | `persistence.rs` `ContextId(String)`, `ContextId::new` → `validate_name(raw, "context id")`, manual `Deserialize` | `persistence::tests::context_ids_are_validated_on_construction_and_deserialization` (`../x`, `""`, `.` → `Err(InvalidIdentity::Name("context id"))`; `"a/b"` refused by serde) | PASS |
| AC-R1-4 `# Errors` on all 17 sites + new fallible items (`validate_name`, `ObjectKey::parse`, `Deserialize for ObjectKey`, `ContextId::new`, `Deserialize for ContextId`) | `identity.rs`, `github.rs`, `storage.rs`, `persistence.rs` | sweep from `findings-validation.md` → no `MISS`; multi-line signatures (`get_json`, `request`, `send_with_policy`, `expect_status`, `parse_json`, `Deserialize for InstallationId`) checked with a 14-line lookback → none missing; `cargo doc --no-deps -p bohmian-code-review-pipeline` → 0 warnings | PASS |
| AC-R1-5 `ObjectKind` / `kind()` removed | deleted from `storage.rs` and `lib.rs` re-export | `rg -n 'ObjectKind|fn kind' crates` → no matches, exit 1 | PASS |
| AC-R1-6 write set bounded | — | `git diff --stat dafe20b..HEAD` → `github.rs`, `identity.rs`, `lib.rs`, `persistence.rs`, `storage.rs`, plus this document and the remediation task | PASS |

Non-goals confirmed excluded: key string layout and `Serialize` output unchanged; no trait method, request policy, or provider behavior changed; no new dependency, validator, helper struct, or trait; no facade, OpenDAL adapter, conformance implementation, or limiter; symbolic crates untouched. `validate_name` widened to `pub(crate)` only.

Commands run in this session (all exit 0): the four focused `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib <test> -- --exact` runs above (1 passed each), `mise run test:pipeline` (15 passed), `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`.

### TASK-002-R2 remediation evidence

Remediation task: `changes/active/repo-parser/review/TASK-002-r2/TASK-002-R2-seam-round-trip-and-host-invariant.md`. Commits `5cfa554` (FIND-TASK-002-1) and `ebb3869` (FIND-TASK-002-2, -4) on top of `e54fa2b` (review record over candidate `93b2542`).

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| AC-R2-1 snapshot and context keys with `EXTRACTION_VERSION` and a 128-byte tenant round-trip | `identity.rs` `validate_key_segment` (no length cap) with `InvalidIdentity::KeySegment`; `validate_name` delegates to it and keeps the 100-byte cap; `storage.rs` `ObjectKey::parse` calls `validate_key_segment` | `storage::tests::long_constructor_segments_round_trip` — RED before fix: `round trip: Error("object key segment must be 1..=100 bytes ...")` at `storage.rs:252`; GREEN after | PASS |
| AC-R2-2 `SnapshotPointer` and `ContextRecord` keys built with `EXTRACTION_VERSION` round-trip | same as above | `persistence::tests::records_revalidate_their_keys_on_deserialization` extended to a 128-byte tenant and `EXTRACTION_VERSION` — RED before fix: same error at `persistence.rs:257`; GREEN after | PASS |
| AC-R2-3 hostile strings still refused; `ContextId::new` and `validate_name` unchanged | `validate_name` keeps `MAX_NAME_BYTES`; hostile set unchanged | `storage::tests::keys_are_revalidated_on_deserialization` (7 hostile strings), `persistence::tests::context_ids_are_validated_on_construction_and_deserialization`, `identity::tests::identities_validate_key_safe_spellings` all pass unchanged | PASS |
| AC-R2-4 no `MISS` in `# Errors` sweep; `cargo doc` clean | `github.rs` `BoundedReader::read` gained `# Errors` | sweep from the remediation task → no `MISS` line; `cargo doc --no-deps -p bohmian-code-review-pipeline` → 0 warnings from the pipeline crate (only pre-existing vendored-grammar C build warnings from `bohmian-code-review-extract`) | PASS |
| AC-R2-5 external `GitHubHost::Enterprise { .. }` construction fails to compile; `enterprise("http://...")` still `Err(ApiBase)` | `identity.rs` `#[non_exhaustive]` on `Enterprise`, `compile_fail` doctest on `GitHubHost` | `cargo test --doc -p bohmian-code-review-pipeline --all-features` → `identity::GitHubHost (line 310) - compile fail ... ok`; `hosts_expose_api_base_and_archive_host` covers `http://ghe.example.com` → `Err(ApiBase)` | PASS |
| AC-R2-6 `archive_host` has no indexing slice | `identity.rs` `archive_host` uses `strip_prefix("https://")` + `split('/').next()` | `rg 'api_base\[' identity.rs` → no match; `identity::tests::hosts_expose_api_base_and_archive_host -- --exact` → 1 passed | PASS |
| AC-R2-7 write set bounded | — | `git diff --stat 93b2542..HEAD` code files: `github.rs`, `identity.rs`, `persistence.rs`, `storage.rs`, plus this document; the `review/TASK-002-r2/` files come from the review record commit `e54fa2b`, not this remediation | PASS |

Non-goals confirmed excluded: key string layout, `Serialize` output, hex encoding, trait method sets, provider client behaviour, and request policy unchanged; no dependency, helper struct, trait, or whole-key length cap added; no facade, OpenDAL adapter, conformance implementation, or limiter; symbolic crates, grammar pins, and fixtures untouched.

Commands run in this session (all exit 0 unless noted): `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib -- storage::tests persistence::tests` (RED: 2 failed before the fix, then 10 passed with `identity::tests`), `... --all-features --doc` (1 passed), `... --lib identity::tests::hosts_expose_api_base_and_archive_host -- --exact` (1 passed), `# Errors` sweep (no `MISS`), `mise run test:pipeline` (16 passed), `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`.
