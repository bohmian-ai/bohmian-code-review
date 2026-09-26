# TASK-002 task review r2 — sample A (task-rev)

Subject: candidate `93b2542da4ccb26ff9468c22b35605ce0091443a` (base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`), reviewed cumulatively; HEAD re-checked before writing (`93b2542`, working tree clean apart from this review directory). Spec revision 10; original task TASK-002; prior findings FIND-TASK-002-1/2/3 from r1.

## Result: FAIL — one INCORRECT finding (TASK-A-1)

The r1 remediation closes FIND-TASK-002-2 and FIND-TASK-002-3 completely and closes FIND-TASK-002-1 at every sibling named in the remediation task, but the closure introduces an internal contradiction: `ObjectKey::snapshot` still produces a segment (`hex(extraction_version)`) that the new re-validating `Deserialize` refuses whenever the extraction version is longer than 50 bytes or empty. The library's own `EXTRACTION_VERSION` is 53 bytes, so every real `SnapshotPointer` written by the future cold/update facades fails on reload. The remediation task's own AC-R1-2 ("a constructor-built key round-trips through serde equal to itself") is only proved with `"v1"`.

## Acceptance matrix

| Requirement/AC/constraint/non-goal | Implementation evidence (path:line) | Verification evidence | Result |
|---|---|---|---|
| REQ-004 github.com and GHES via configuration | `identity.rs` `GitHubHost::{Public, enterprise}`, `api_base`/`archive_host`; `github.rs:950-993` archive trust uses `self.host.archive_host()` | `test:pipeline` 15 passed; remediation diff to `github.rs` is rustdoc only (`git diff dafe20b..HEAD -- github.rs identity.rs` non-doc lines: only `validate_name` → `pub(crate)`) so r1 PASS rows carry over | PASS |
| REQ-004 repository/commit/tree/path/blob identity verified before source enters library | `github.rs:919-936` blob SHA-1 recomputed and compared (`IdentityMismatch`); `repository`/`branch_head`/`commit`/`tree`/`pull_request` at `:787-1038` compare ids/shas; paths via `RelativePath` | `verified_provider_identities_cross_and_mismatches_are_rejected` in the 15-passed lane | PASS |
| REQ-004 unauthorized / oversize / unsupported PR state explicit | `status_error` `:1224`; `read_bounded` `:1265-1277` reads `limit+1` → `ResponseTooLarge`; unknown PR state → `MalformedResponse` | `rate_limits_…`, `malformed_…` passed | PASS |
| REQ-004 credentials never leak | redacted `InstallationToken` Debug; archive hop `authenticated=false` (`:966-972`) | `token_debug_is_redacted`, `archive_redirect_…` passed | PASS |
| REQ-013 retry-after/reset, bounded retry time, deadline, cancellation | single `send_with_policy` `:1157-1204`: wait capped by `max_retry_wait` and remaining deadline, `max_attempts`, `execution.check()` each attempt, `sleep_bounded` `:1212` cancel-aware | `rate_limits_retry_within_bounds_or_report_typed_outcomes`, `check_reports_cancellation_then_deadline` passed | PASS |
| REQ-013 concurrency ceilings | not in this task (per-call synchronous client) | — | N/A (later task, as r1 recorded) |
| REQ-014 (supporting) redirect trust, one hop, no credential forwarding | `is_trusted_redirect` `:735-740` exact host match after `https://`; `archive` `:950-993` | probed mentally: `https://codeload.github.com@evil/…` and host-with-port both refused (first `/` segment ≠ archive host) | PASS |
| REQ-014/REQ-015 keys and ids carry validated identity across the durable seam (FIND-TASK-002-1) | `storage.rs:78` `context(&Identity, &ContextId)`; `:92-97` `parse` per-segment `validate_name`; `:105-120` re-validating `Deserialize for ObjectKey`; `persistence.rs:65-95` private `ContextId`, `new` via `validate_name`, re-validating `Deserialize`; `SnapshotPointer.key`/`ContextRecord.key`/`AlreadyPublished.id` inherit through field type | scratch crate `/tmp/t002r2a`: `ContextId::new("../x")` → `Err(Name("context id"))`; `"tenants/b"` refused as `ContextId`; traversing key refused as `ObjectKey`; 3 focused tests 1 passed each | PASS for the hostile-input half; **FAIL for the round-trip half — TASK-A-1** |
| REQ-015 narrow persistence trait, no pool, no consumer type | `persistence.rs:130-206` every method carries `Identity`/`SnapshotScope` | compiles under lane; `cargo` deps: facade, serde, serde_json, sha1, sha2, thiserror, ureq | PASS (contract only) |
| REQ-016 narrow storage trait, exact-key, create-only, delete | `storage.rs:155-179` | `storage::tests::*` passed | PASS (contract only) |
| INV-001 no clone/checkout, no symbolic duplication, no server/worker | `rg 'std::fs|tokio|axum|std::process' crates/bohmian-code-review-pipeline/src` → no matches | observed | PASS |
| INV-003 blob integrity | `github.rs:929-936` git SHA-1 over `blob <len>\0` | `helpers_encode_paths_and_hash_blobs_like_git` passed | PASS |
| INV-003 pointer targets only a validated snapshot | `SnapshotPointer` re-validates `key` on reload | see TASK-A-1: a real pointer cannot be reloaded at all | FAIL (consequence of TASK-A-1) |
| INV-006 no hidden budget success | `TreeListing.truncated`, `PullRequestFilesPage.has_next`, rejected lists, typed `RateLimited` | scenario tests passed | PASS |
| Scenario 1 verified identities (RED/GREEN/REFACTOR) | fake transport drives real `GitHubClient`; identities centralized in `identity.rs` | 4 scenario tests passed | PASS |
| Scenario 2 bounded provider failure | one policy for all operations incl. archive hop | `rate_limits_…`, `malformed_…` passed | PASS |
| Constraint: no consumer import, no server/worker loop, never retain source on disk | `Cargo.toml` deps; no `std::fs` | rg | PASS |
| Non-goals excluded (facades, OpenDAL, tar decode, limiter, real-GitHub run) | absent | read | PASS |
| Write set: outer crate, workspace manifest/lockfile, contract tests, scoped `mise` lane | 11 code files + `mise.toml` `test:pipeline` (`mise.toml:54-56`) + task/review docs | `git diff --stat` | PASS |
| FIND-TASK-002-1 sibling `storage.rs` derived `Deserialize` on `ObjectKey` | replaced by `parse`-backed impl `:105-120` | `keys_are_revalidated_on_deserialization` 1 passed; scratch probe | CLOSED |
| FIND-TASK-002-1 sibling `ObjectKey::context(&str)` | now `&ContextId` `:78` | `keys_are_scoped_by_identity_and_version` unchanged expected string | CLOSED |
| FIND-TASK-002-1 sibling `ContextId(pub String)` | private field, `new`, re-validating `Deserialize` `persistence.rs:63-95` | `context_ids_are_validated_…` 1 passed | CLOSED |
| FIND-TASK-002-1 siblings `SnapshotPointer.key`, `ContextRecord.key`, `AlreadyPublished.id` | inherit through field type | `records_revalidate_their_keys_on_deserialization` 1 passed | CLOSED (but see TASK-A-1 for the pointer) |
| FIND-TASK-002-2 `# Errors` on all fallible items (17 + new) | — | own sweep: every line matching `-> Result<` in the crate has `# Errors` within 20 lines (single- and multi-line signatures) → no `MISS` | CLOSED |
| FIND-TASK-002-3 `ObjectKind`/`kind()` removed | — | `rg -n 'ObjectKind|fn kind' crates` → no matches, exit 1; `lib.rs:32` re-export gone | CLOSED |
| AC-R1-6 write set bounded to `storage.rs`, `persistence.rs`, `identity.rs`, `lib.rs`, docs | `git diff --stat dafe20b..HEAD` | matches | PASS |
| Verification lanes | — | `mise run test:pipeline` 15 passed, exit 0; orchestrator runs fmt/lints/gate | PASS |

## Proposed findings

### TASK-A-1 — INCORRECT: `ObjectKey::snapshot` builds keys its own `Deserialize` refuses for the real extraction version

- **Root cause (first owner):** `storage.rs:73` — `ObjectKey::snapshot` emits `hex(extraction_version.as_bytes())` as one key segment with no length or emptiness bound, while the remediation's `ObjectKey::parse` (`storage.rs:94`) requires every segment to pass `validate_name` (1..=100 bytes). Hex doubles the length, so any extraction version over 50 bytes, or the empty string, produces a constructor-built key that cannot be reloaded. `bohmian_code_review::EXTRACTION_VERSION` (`crates/bohmian-code-review-extract/src/lib.rs:119`) is 53 bytes → 106-byte segment.
- **Defect class:** constructor and re-validator disagree on the value domain (invariant asserted at `storage.rs:44-45` "Every `/`-separated segment is key-safe" is false for constructor output).
- **Sibling locations (one finding):**
  - `crates/bohmian-code-review-pipeline/src/storage.rs:73` — unbounded hex segment.
  - `crates/bohmian-code-review-pipeline/src/persistence.rs:38` — `SnapshotScope.extraction_version: String` accepts the same unbounded free text that feeds the key.
  - `crates/bohmian-code-review-pipeline/src/persistence.rs:51` — `SnapshotPointer.key` is where the unreloadable key is persisted and refused.
  - `crates/bohmian-code-review-pipeline/src/storage.rs:220-226` and `persistence.rs:263` — round-trip tests use `"v1"` only, so the contradiction is untested.
- **Violated obligation:** remediation AC-R1-2 (`TASK-002-R1-validated-seam-values.md`, "a constructor-built key round-trips through serde equal to itself"); spec INV-003 `spec.md:87` ("A pointer targets only a validated, complete snapshot") — a pointer that cannot be read back cannot be targeted; REQ-015 `spec.md:78` exact-key lookup; `storage.rs:41-45` the crate's own documented invariant.
- **Evidence (reproduced):** scratch crate `/tmp/t002r2a` (path deps on the candidate pipeline crate and facade) printed:
  - `ev="codegraph-kernel-572d22bf.tree-sitter-0.25.10.facts-1" len=53 hexlen=106 roundtrip=Err("object key segment must be 1..=100 bytes of [A-Za-z0-9._-] and not `.`/`..`")`
  - `ev="" len=0 hexlen=0 roundtrip=Err(...)`
  - `ev="v1" len=2 hexlen=4 roundtrip=Ok("Ok")`
- **Observable consequence:** the cold-build/update facades (TASK-003/004) will build snapshot keys from `EXTRACTION_VERSION`, publish a `SnapshotPointer`, and every subsequent `Persistence::pointer` read that deserializes the stored row fails; the whole snapshot reuse path is dead on the first real version string. The r1 fix turned a silent-admission defect into a hard reload failure for the only legitimate input.
- **Testable correction (ladder rung 2 — reuse existing validator, delete the encoder):** the real extraction version is already key-safe (`[A-Za-z0-9.-]`, 53 bytes), so drop the `hex` encoding and route `extraction_version` through the existing `validate_name(_, "extraction version")`, making `ObjectKey::snapshot` fallible (`Result<ObjectKey, InvalidIdentity>`) or giving `SnapshotScope` a validated constructor whose value `snapshot` takes. Either way add the round-trip assertion with `bohmian_code_review::EXTRACTION_VERSION` (the crate already depends on the facade) to `keys_are_revalidated_on_deserialization`, plus an empty-version rejection. Do not raise `MAX_NAME_BYTES` or special-case the hex segment in `parse` — that keeps two rules for one segment.

No DRIFT findings: the remediation added no abstraction, dependency, or config surface; `validate_name` widened to `pub(crate)` only. The `hex` encoding of the extraction version is the only speculative layer touched by this review and is subsumed into TASK-A-1's correction.

Not reported (out of contract): a well-formed key from another tenant still deserializes `Ok` (`"tenants/b/installations/9/repositories/3/contexts/x"`); the r1 remediation deliberately scoped the fix to segment safety and relies on exact-key lookup under identity for cross-tenant isolation (REQ-015). `ObjectDigest` keeps a derived transparent `Deserialize`; a corrupt digest fails closed at read-time comparison and is not key-bearing.

## Commands run (candidate `93b2542`, all read-only)

| Command | Observed |
|---|---|
| `git rev-parse HEAD` (start and end) | `93b2542da4ccb26ff9468c22b35605ce0091443a`; `git status --short` clean |
| `git diff --stat dafe20b..HEAD` | 15 files; code: `github.rs` +75, `identity.rs` +30/−1, `lib.rs` 1/1, `persistence.rs` +109/−?, `storage.rs` +88/−… |
| `git diff dafe20b..HEAD -- github.rs identity.rs \| grep -vE '///'` | only non-doc change: `fn validate_name` → `pub(crate) fn validate_name` |
| `rg -n 'ObjectKind\|fn kind' crates` | no matches, exit 1 |
| `rg -n 'std::fs\|tokio\|axum\|std::process' crates/bohmian-code-review-pipeline/src` | no matches |
| `# Errors` sweep (awk, every `-> Result<` line, 16- and 20-line lookback, all pipeline `src/*.rs`) | no `MISS` |
| `mise run test:pipeline` | 15 passed, 0 failed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib storage::tests::keys_are_revalidated_on_deserialization -- --exact` | 1 passed |
| `… persistence::tests::records_revalidate_their_keys_on_deserialization -- --exact` | 1 passed |
| `… persistence::tests::context_ids_are_validated_on_construction_and_deserialization -- --exact` | 1 passed |
| scratch crate `/tmp/t002r2a` `cargo run` | output quoted under TASK-A-1; hostile `ContextId`/`ObjectKey` inputs refused as claimed |

## Not covered

- No live GitHub/GHES run (opt-in gate outside CI). `fmt:check`, `lints`, `gate`, `cargo doc` not re-run here; orchestrator runs them in parallel. `github.rs` was spot-read at the trust/policy regions rather than re-read in full because the remediation diff to it is rustdoc only and r1 accepted the client.

## Overall: FAIL

```citations
[{"id": "TASK-A-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 73, "snippet": "            hex(extraction_version.as_bytes())", "symbols": ["ObjectKey::snapshot", "hex"]},
 {"id": "TASK-A-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 94, "snippet": "            validate_name(segment, \"object key segment\")?;", "symbols": ["ObjectKey::parse", "validate_name"]},
 {"id": "TASK-A-1", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 38, "snippet": "    pub extraction_version: String,", "symbols": ["SnapshotScope"]},
 {"id": "TASK-A-1", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 51, "snippet": "    pub key: ObjectKey,", "symbols": ["SnapshotPointer"]},
 {"id": "TASK-A-1", "path": "crates/bohmian-code-review-extract/src/lib.rs", "line": 119, "snippet": "pub const EXTRACTION_VERSION: &str = \"codegraph-kernel-572d22bf.tree-sitter-0.25.10.facts-1\";", "symbols": ["EXTRACTION_VERSION"]}]
```
