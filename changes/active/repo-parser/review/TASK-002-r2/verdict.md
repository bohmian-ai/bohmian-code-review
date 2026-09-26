# TASK-002 task review r2 — verdict

## Immutable subject

- Repository: `bohmian-code-review`, branch `review-helper-setup`
- Base: `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`
- Candidate: `93b2542da4ccb26ff9468c22b35605ce0091443a` — unchanged for the whole review (`git rev-parse HEAD` checked before Wave 1, before the citation check, before Wave 2, and before writing this file; working tree clean apart from this review directory)
- Approved spec: `changes/active/repo-parser/spec.md` (SPEC-repo-parser revision 10); task maps REQ-004, REQ-013, REQ-015, REQ-016, INV-001, INV-003, INV-006
- Original task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`
- Prior review: r1 `FIX_REQUIRED` (`../TASK-002-r1/verdict.md`); remediation task `../TASK-002-r1/TASK-002-R1-validated-seam-values.md` (FIND-TASK-002-1, -2, -3) implemented in `30b5191` and `5a8cf76`
- Diff reviewed: complete cumulative `1842eaf..93b2542` (20 files, +4462/−3; code: `Cargo.toml`, `Cargo.lock`, `mise.toml`, `crates/bohmian-code-review-pipeline/{Cargo.toml, src/lib.rs, identity.rs, execution.rs, github.rs, storage.rs, persistence.rs}`; the rest is task/review documentation)

## Verdict

**FIX_REQUIRED** — Wave 2 retained three blocking findings. Two reopen prior findings as `CLASS_INCOMPLETE`: the r1 fix for the durable-seam values chose a validator whose 100-byte cap refuses the keys the crate's own constructors build (real `EXTRACTION_VERSION` hex-encodes to 106 bytes; tenants may be 128 bytes), so no real snapshot pointer can be reloaded; and one fallible item (`io::Result` return) escaped the r1 `# Errors` sweep. One is new: `GitHubHost::Enterprise { api_base }` is publicly constructible without validation, which lets a consumer send the installation token over `http://` and reach an unchecked slice that panics. FIND-TASK-002-3 is closed. The provider client, request policy, and bounds pass every scenario.

Remediation task: `changes/active/repo-parser/review/TASK-002-r2/TASK-002-R2-seam-round-trip-and-host-invariant.md`.

## Wave 1 results

| Reviewer | Report | Result | Proposed findings |
|---|---|---|---|
| task-rev A | `task-review-a.md` | FAIL | TASK-A-1 (INCORRECT) |
| task-rev B | `task-review-b.md` | FAIL | TASK-B-1 (INCORRECT), TASK-B-2 (INCORRECT) |
| repo-rev | `standards-review.md` | PASS | none (observations O1, O2) |
| domain-rev source trust | `domain-review-source-trust.md` | FAIL | TRUST-1 |
| domain-rev provider bounds | `domain-review-provider-bounds.md` | PASS | none |

## Citation check

`mise run review:verify-citations changes/active/repo-parser/review/TASK-002-r2 93b2542…` → exit 0; 18 citations, 15 OK, 3 REJECT on symbol form only (`citations.txt`): TRUST-1 `identity.rs:340` (`GitHubHost::archive_host`), `github.rs:1133` (`GitHubClient::build`), `github.rs:1115` (`GitHubHost::api_base`) — path-qualified symbol spellings; every snippet matched and TRUST-1 keeps its OK root-cause citation `identity.rs:294`. No finding was dropped (same treatment as r1).

## Acceptance matrix

Consolidated from `task-review-a.md` and `task-review-b.md` (independent samples; agreeing rows) with orchestrator-observed verification. Rows overridden by Wave 2 are marked.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004 both hosts via configuration | `identity.rs` `GitHubHost::{Public, enterprise}`, `api_base`/`archive_host`; `github.rs:1115` every URL from `api_base()`; archive trust from `archive_host()` | `hosts_expose_api_base_and_archive_host`; GHES half of `archive_redirect_…` | PASS via `enterprise()`; **FAIL for the bypass path — FIND-TASK-002-4** |
| REQ-004 identity verified before source enters library (repo, commit/tree, path, blob) | `github.rs:787-1038` id/full_name/sha/number/base-repo comparisons; blob SHA-1 recomputed `:929-936`; paths via `RelativePath` | `verified_provider_identities_cross_and_mismatches_are_rejected` 1 passed | PASS |
| REQ-004 unauthorized / oversize / unsupported PR state explicit | `status_error` `:1224`; `read_bounded` `:1265-1277`; unknown state → `MalformedResponse` | `rate_limits_…`, `malformed_…` | PASS |
| REQ-004 credentials never leak | redacted `InstallationToken` Debug; `HttpRequest` no Debug; bearer only when `authenticated`; archive hop unauthenticated | `token_debug_is_redacted`; `archive_redirect_…` | PASS via constructor; **bypass sends bearer over http — FIND-TASK-002-4** |
| REQ-013 retry-after/reset, bounded retry time, deadline, cancellation | single `send_with_policy` `:1157-1204`; waits capped by `max_retry_wait` and remaining deadline; `sleep_bounded` cancel-aware | `rate_limits_retry_…`, `check_reports_cancellation_then_deadline`; provider-bounds scratch run: cancel mid-sleep, no sleep past deadline, huge retry-after safe | PASS |
| REQ-013 concurrency ceilings | not in this task | — | N/A (later task, as r1 recorded) |
| REQ-013 size bounds (JSON, blob, archive) | `RequestLimits`; `read_bounded` reads `limit+1`; `BoundedReader` fails rather than truncates | `ResponseTooLarge` and archive over-limit cases | PASS |
| REQ-014 (supporting) redirect trust, one hop, no credential forwarding | `is_trusted_redirect` `:735-740`; `archive` `:950-993` | `archive_redirect_…` 1 passed | PASS (trust host mis-computed only on the FIND-TASK-002-4 bypass path) |
| REQ-014/REQ-015 keys and ids carry validated identity across the durable seam (FIND-TASK-002-1 hostile-input half) | `storage.rs:78` `context(&Identity, &ContextId)`; `:92-97` per-segment parse; `:105-120` re-validating Deserialize; `persistence.rs:63-95` private `ContextId`, `new`, re-validating Deserialize | `keys_are_revalidated_on_deserialization`, `context_ids_…`, `records_revalidate_…` 1 passed each; both samples' scratch probes refuse `../x`, empty, `.`, `a/b`, 101-byte, non-ASCII | PASS |
| AC-R1-2 constructor-built key round-trips; REQ-015 exact-key lookup; INV-003 pointer targets a validated snapshot | `storage.rs:73` hex segment unbounded; `parse` reuses `validate_name` (≤100 bytes); tenant ≤128 | scratch (A, B, Wave 2): `ObjectKey::snapshot(.., EXTRACTION_VERSION, ..)` reload `Err(…1..=100 bytes…)`; 128-byte tenant reload `Err`; `"v1"` `Ok` | **FAIL — FIND-TASK-002-1 CLASS_INCOMPLETE** |
| REQ-015 narrow persistence trait, no pool, no consumer type | `persistence.rs:130-206` | compiles under lane; `cargo tree`: facade, serde, serde_json, sha1, sha2, thiserror, ureq | PASS (contract only) |
| REQ-016 narrow storage trait, exact-key, create-only, delete | `storage.rs:155-179` | `storage::tests::*` | PASS (contract only) |
| INV-001 no clone/checkout, no symbolic duplication, no server/worker | `rg 'std::fs|tokio|axum|std::process'` → none | observed by both samples | PASS |
| INV-003 blob integrity | git SHA-1 over `blob <len>\0` | `helpers_encode_paths_and_hash_blobs_like_git` | PASS |
| INV-006 no hidden budget success | `TreeListing.truncated`, `PullRequestFilesPage.has_next`, rejected lists, typed `RateLimited` | scenario assertions; provider-bounds review | PASS |
| Scenario 1 / Scenario 2 RED-GREEN-REFACTOR | fake transport drives real `GitHubClient`; one policy for all operations incl. archive hop | four scenario tests 1 passed each; provider-bounds confirmed the tests exercise the real policy | PASS |
| Constraints: no consumer import, no server/worker loop, never retain source on disk | deps; no `std::fs` | `cargo tree`; rg | PASS |
| Non-goals excluded (facades, OpenDAL, tar decode, conformance impls, limiter, real-GitHub run, symbolic crates untouched) | absent | `git diff --stat -- crates` lists only the pipeline crate | PASS |
| Write set bounded (task + AC-R1-6) | code files as listed; r1 remediation touched only `storage.rs`, `persistence.rs`, `identity.rs`, `lib.rs`, `github.rs` (doc), docs | `git diff --stat` | PASS |
| Tests inline (agent-rules.md:5) | inline modules through the `HttpTransport` seam | `lints` placement check | PASS |
| Rustdoc + `# Errors` on every fallible item (AGENTS.md:32; agent-rules.md:11; rust-core.md:7) | 17 r1 sites documented | `-> Result<` sweep clean; **`-> io::Result<` sweep: `github.rs:566` MISS** | **FAIL — FIND-TASK-002-2 CLASS_INCOMPLETE** |
| No unchecked `unwrap()` / undocumented panic on input (AGENTS.md:30; rust-core.md:7) | no `unwrap()` in production; **`identity.rs:340` unchecked slice panics on a bypassed short `api_base`** | Wave 2 scratch: `start byte index 8 is out of bounds` | **FAIL — FIND-TASK-002-4** |
| No speculative abstraction (AGENTS.md:26; agent-rules.md:10); FIND-TASK-002-3 | `ObjectKind`/`kind()` gone; `HttpTransport` two impls; traits are the task seam | `rg -n 'ObjectKind|fn kind' crates` → none | PASS / CLOSED |
| Verification lanes | — | orchestrator at candidate: `fmt:check` 0, `lints` 0, `test:pipeline` 15 passed, `gate` 0 | PASS |

## Validated blocking ledger (from `findings-validation.md`)

| ID | Sources | Status | Gate | Root cause | Correction owner |
|---|---|---|---|---|---|
| FIND-TASK-002-1 | TASK-A-1, TASK-B-1 | CLASS_INCOMPLETE | 2 (AC-R1-2), 1 (REQ-006/REQ-015 pointer reload) | `ObjectKey::parse` re-validates with `validate_name` (GitHub-name domain, ≤100 bytes) while `snapshot` emits unbounded hex and tenants admit ≤128 bytes | `identity.rs` segment validator reused by `storage.rs:92-97` |
| FIND-TASK-002-2 | TASK-B-2 | CLASS_INCOMPLETE | 3 (AGENTS.md:32; rust-core.md:7) | closure sweep matched `-> Result<` only; `io::Result` escaped | rustdoc on `github.rs:566` |
| FIND-TASK-002-4 | TRUST-1 | CONFIRMED | 4 (bearer over cleartext via bypass); 3 (rust-core.md:7 undocumented panic; AGENTS.md:29) | public struct variant `GitHubHost::Enterprise { api_base }` bypasses `enterprise()`; `archive_host` slices unchecked | `identity.rs:294-297`, `:340` |

## Notes (non-blocking)

- Well-formed foreign-scope keys still deserialize; scope binding is at facade lookup under `Identity` (r1 accepted scope). Change review should confirm the facades bind it.
- repo-rev O1: `file.patch.clone()` at `github.rs:1355` avoidable; preference, not a rule.
- repo-rev O2: no RED record for the three r1 remediation tests; process note, same treatment as r1.
- source-trust: ureq `TransportError::Failed` text may echo a codeload `?token=` URI on a URI-parse failure; unreproduced.
- provider-bounds: checked-in tests never exercise a non-zero sleep (proved only by the reviewer's scratch run); `read_bounded` `limit as u64 + 1` and `Instant::now() + timeout` overflow only for absurd caller limits; retry-after HTTP-date unparsed; archive stream not cancel-aware (r1 notes carried forward).

## Rejected

None. Every Wave 1 finding was reproduced by Wave 2.

## Prior-finding closure

| ID | Status | Remaining siblings |
|---|---|---|
| FIND-TASK-002-1 | CLASS_INCOMPLETE | `storage.rs:92-97`, `:73`, `:52-59`; `persistence.rs:51`, `:110`; `identity.rs:140-150`; tests `storage.rs:220`, `persistence.rs:263` |
| FIND-TASK-002-2 | CLASS_INCOMPLETE | `github.rs:565-566` |
| FIND-TASK-002-3 | CLOSED | none (`rg -n 'ObjectKind|fn kind' crates` → no output) |

## Verification limits

- No live GitHub/GHES contract run (opt-in integrated gate outside CI); redirect, header, and timeout behaviour proved against the scripted fake and vendored `ureq` 3.4.2 source only.
- Wave 2 did not re-run `lints`/`fmt:check`/`gate`; the orchestrator ran them at the candidate (all exit 0) and the corrections are validator, attribute, slice, and doc changes the same lanes re-prove.
- Every reviewer finished within budget (Wave 1 longest 5 min 4 s; Wave 2 4 min 47 s); no report was missing; no sub-reviewer gap was recorded. Review wall clock 14:22–14:37.

## Orchestrator-observed commands (candidate `93b2542`)

| Command | Observed |
|---|---|
| `mise run fmt:check` | exit 0 |
| `mise run lints` | exit 0 |
| `mise run test:pipeline` | 15 passed, 0 failed; exit 0 |
| `mise run gate` | exit 0 (8 `test result: ok` lines) |
| `mise run review:verify-citations <dir> 93b2542` | exit 0; 15 OK / 3 REJECT (symbol form only) |
