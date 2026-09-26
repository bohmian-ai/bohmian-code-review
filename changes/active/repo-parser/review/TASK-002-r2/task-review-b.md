# TASK-002 task review r2 — sample B (task implementation acceptance)

Subject: `/tmp/t002r2-subject.md` (base `1842eaf`, candidate `93b2542`, cumulative range). Read-only; scratch crate under `/tmp/t002r2b`.

## Overall result: FAIL

Two findings. TASK-B-1 is a regression introduced by the FIND-TASK-002-1 remediation: the re-validating `ObjectKey` parser is stricter than the key constructors, so every snapshot key built with the workspace's own `EXTRACTION_VERSION` fails to reload. TASK-B-2 is one sibling of FIND-TASK-002-2 the r1 sweep did not catch (`io::Result`, not `Result<`).

## Acceptance matrix

| Requirement / AC / constraint / non-goal | Implementation evidence (path:line) | Verification evidence | Result |
|---|---|---|---|
| REQ-004 both hosts through configuration | `identity.rs:290-343` `GitHubHost::{Public, enterprise}`, `api_base`, `archive_host`; `github.rs:1115` every API URL from `host.api_base()` | `identity::tests::hosts_expose_api_base_and_archive_host`; GHES half of `archive_redirect_…` (1 passed) | PASS |
| REQ-004 repository / commit-tree / path / blob identity verified before source enters library | `github.rs:795-796` id + `full_name`; `:829` branch; `:857` commit; `:887` tree; `:929-930` blob SHA-1 recomputed; `:1003` PR number + base repo id; `:1319-1323`, `:1340` paths via `RelativePath`, shas via `ObjectSha` | `verified_provider_identities_cross_and_mismatches_are_rejected` 1 passed | PASS |
| REQ-004 invalid identity / unauthorized / oversize / unsupported PR state fail explicitly | `github.rs:1224-1235` `status_error`; `:1265-1276` `read_bounded` → `ResponseTooLarge`; `:1009-1017` unknown state → `MalformedResponse`; identity constructors `identity.rs:70,107,196,251,308,361` | `malformed_payloads_and_interruptions_are_explicit`, `rate_limits_…` 1 passed each | PASS |
| REQ-004 credentials fail visibly, never leak | `identity.rs:378-383` redacted `Debug`; `HttpRequest` no `Debug` (`github.rs:42-51`); bearer only when `authenticated` (`:1132-1134`); archive hop `authenticated=false` (`:965-971`) | `token_debug_is_redacted`; `archive_redirect_…` | PASS |
| REQ-013 retry-after / reset / bounded retry time, single policy | `github.rs:1157-1203` `send_with_policy`; `rate_limit_wait` `:711-732`; `too_long` `:1196-1198` | `rate_limits_retry_within_bounds_or_report_typed_outcomes` 1 passed | PASS |
| REQ-013 cancellation and deadline reported honestly | `execution.rs:78-83`; `github.rs:1168-1169` per-attempt check + timeout = min(remaining, request_timeout); `sleep_bounded` `:1212-1221` | `check_reports_cancellation_then_deadline`; `malformed_…` (Cancelled, DeadlineExceeded asserted `:1880`, `:1885`) | PASS |
| REQ-013 concurrency ceilings | absent | adjudicated N/A for this task in r1 verdict (later task / change review) | N/A |
| REQ-013 size bounds (JSON, blob, archive) | `RequestLimits` `:173-186`; `read_bounded` reads `limit+1` `:1267`; `BoundedReader` fails rather than truncates `:564-582` | `ResponseTooLarge { limit: 64 }` asserted `:1853`; archive over-limit in `archive_redirect_…` | PASS |
| REQ-014 (supporting) archive redirect trusted host only, one hop, no credential forwarding | `:958-976`, `is_trusted_redirect` `:735-740`; `ureq` `max_redirects(0)` `:114` | `archive_redirect_…` 1 passed | PASS |
| REQ-014/REQ-015 keys and ids carry validated identity across the durable seam | `storage.rs:78` `context(&Identity, &ContextId)`; `storage.rs:92-97,116-119` re-validating `Deserialize`; `persistence.rs:74-76,91-94` `ContextId::new` + `Deserialize` | `keys_are_revalidated_on_deserialization`, `context_ids_…`, `records_revalidate_…` 1 passed each; scratch: `ContextId::new("..")` → `Err(Name("context id"))`, 101-byte and non-ASCII refused | PASS for hostile input; **FAIL on round-trip — TASK-B-1** |
| REQ-015 exact-key lookup; pointer reload usable | `persistence.rs:45-56` `SnapshotPointer.key: ObjectKey`; `:137-140` `snapshot_pointer` returns a deserialized pointer | scratch: `ObjectKey::snapshot(id, EXTRACTION_VERSION, 1, tree)` → serde reload `Err("object key segment must be 1..=100 bytes …")` | **FAIL — TASK-B-1** |
| REQ-015 narrow persistence trait, no pool, no consumer type | `persistence.rs:131-206`; every method carries `Identity`/`SnapshotScope` | compiles; `rg` no consumer import (`lib.rs:12-32`) | PASS (contract only, as tasked) |
| REQ-016 narrow storage trait, create-only, exact-key read, delete | `storage.rs:155-179` | `storage::tests` | PASS (contract only) |
| INV-001 no clone/checkout, no symbolic duplication, no server/worker loop | deps `crates/bohmian-code-review-pipeline/Cargo.toml:11-18`; only facade import is `RelativePath` (`github.rs:14`) | `rg 'std::fs\|tokio\|axum\|\.unwrap\(\)'` → no matches | PASS |
| INV-003 blob integrity; pointer targets validated snapshot | `git_blob_sha` `:695-700`, compared `:929-934`; `SnapshotPointer` carries `tree`, `key`, `digest` | `helpers_encode_paths_and_hash_blobs_like_git`; pointer reload broken → see TASK-B-1 | PASS (blob) / **FAIL (pointer reload) — TASK-B-1** |
| INV-006 no hidden budget success | `TreeListing.truncated` `:451`, `rejected` `:455`; `PullRequestFilesPage.has_next` `:548`; typed `RateLimited` | scenario asserts `:1551`, `:1596`, `:1780-1800` | PASS |
| Scenario 1 (verified identities; mismatch rejected before parsing) | above | `verified_provider_identities_…`, `archive_redirect_…` | PASS |
| Scenario 2 (403/429, malformed, cancellation, deadline → typed outcomes) | `send_with_policy`; `parse_json` `:1286-1299` never echoes body | `rate_limits_…`, `malformed_…` | PASS |
| AC: retry/deadline states explicit | `GitHubError::disposition` `:350-360` | asserted in scenario tests | PASS |
| Constraint: no consumer import, no server/worker loop, never retain source on disk | see INV-001 | `rg` | PASS |
| Non-goals excluded (facades, OpenDAL adapter, conformance impls, limiter, real-GitHub run, symbolic crates untouched) | absent | `git diff --stat 1842eaf..HEAD -- crates` lists only the pipeline crate | PASS |
| Write set (crate, workspace manifest/lockfile, provider contract tests, scoped `mise` lane) | `Cargo.toml`, `Cargo.lock`, `mise.toml:54-57` `test:pipeline` + `test:rust` dep `:66`, crate | `git diff --stat` 9 code files | PASS |
| Tests inline in owning module | all `#[cfg(test)] mod tests` inline | read | PASS |
| Rustdoc + `# Errors` on every fallible item (AGENTS.md:32) | all 17 r1 sites now documented (`identity.rs:86,123,136,178,278`; `github.rs:98,133,1082,1104,1146,1208,1240,1260,1281,1313,1334,1428,1468`) | own sweep over `-> Result<` → no MISS; sweep over `io::Result` → **`github.rs:566` MISS** | **FAIL — TASK-B-2** |
| FIND-TASK-002-1 sibling: `storage.rs` transparent `Deserialize` on `ObjectKey` | replaced by `parse` `:92-97` + impl `:105-120` | 7 hostile strings refused (`:227-240`); scratch confirms | CLOSED (but see TASK-B-1) |
| FIND-TASK-002-1 sibling: `ObjectKey::context(&str)` | `:78` takes `&ContextId` | `keys_are_scoped_by_identity_and_version` unchanged expected string | CLOSED |
| FIND-TASK-002-1 sibling: `ContextId(pub String)` / derived `Deserialize` | `persistence.rs:65` private; `:74-76`; `:84-95` | `context_ids_are_validated_…` | CLOSED |
| FIND-TASK-002-1 sibling: `SnapshotPointer.key`, `ContextRecord.key` reload | inherit `ObjectKey::deserialize` | `records_revalidate_their_keys_on_deserialization` | CLOSED for hostile keys; round-trip broken for real keys (TASK-B-1) |
| FIND-TASK-002-1 sibling: `AlreadyPublished { id: ContextId }` | inherits validated `ContextId` `persistence.rs:20-23` | type-level | CLOSED |
| FIND-TASK-002-2 (17 sites) | all 17 present with `# Errors` | sweep | CLOSED for the 17; **one further sibling open — TASK-B-2** |
| FIND-TASK-002-3 `ObjectKind` / `kind()` / re-export / test asserts | deleted | `rg -n 'ObjectKind\|fn kind' crates` → exit 1 | CLOSED |
| AC-R1-6 remediation write set | `git diff --stat dafe20b..HEAD` → `github.rs`, `identity.rs`, `lib.rs`, `persistence.rs`, `storage.rs` + review/task docs | observed | PASS |

## Proposed findings

### TASK-B-1 — INCORRECT (regression from the FIND-TASK-002-1 remediation): `ObjectKey::parse` refuses keys the constructors produce

- **Root cause:** `ObjectKey::parse` (`storage.rs:92-97`) re-validates every `/` segment through `validate_name`, whose 100-byte cap was designed for GitHub owner/repository names (`identity.rs:16`). The constructors emit segments under different bounds: `ObjectKey::snapshot` writes `hex(extraction_version)` (`storage.rs:73`), which is 106 bytes for the workspace's own `EXTRACTION_VERSION` (`crates/bohmian-code-review-extract/src/lib.rs:119`, 53 bytes), and `TenantId` admits up to 128 bytes (`identity.rs:13`, `:71`). The first owner of the wrong decision is `parse` choosing a validator whose bound does not match what `snapshot`/`prefix` emit.
- **Defect class:** constructor/parser invariant mismatch (round-trip broken at the durable seam).
- **Sibling locations (one finding):** `storage.rs:92-97` (`parse`), `storage.rs:70-74` (`snapshot` hex segment, unbounded), `storage.rs:52-59` (`prefix` tenant segment, ≤128 vs ≤100), `persistence.rs:51` (`SnapshotPointer.key` reload), `persistence.rs:137-140` (`Persistence::snapshot_pointer` returns a deserialized pointer → every cold-built pointer reload fails), `persistence.rs:110` (`ContextRecord.key`, same class if a tenant id exceeds 100 bytes).
- **Violated obligation:** remediation AC-R1-2 (`TASK-002-R1-validated-seam-values.md` "a constructor-built key round-trips through serde equal to itself"); spec REQ-015 `spec.md:78` ("exact-key lookup"); INV-003 `spec.md:87` ("A pointer targets only a validated, complete snapshot"); `storage.rs:44-45` doc claim ("a persisted key is re-validated on reload").
- **Evidence:** scratch crate `/tmp/t002r2b` (path dependency on the candidate crate):
  ```
  EXTRACTION_VERSION len=53 hex_len=106
  snapshot key round-trip: Err(Error("object key segment must be 1..=100 bytes of [A-Za-z0-9._-] and not `.`/`..`", line: 0, column: 0))
  128-byte tenant key round-trip: Err(Error("object key segment must be 1..=100 bytes of [A-Za-z0-9._-] and not `.`/`..`", line: 0, column: 0))
  ```
  The existing tests pass only because they use `"v1"` (`storage.rs:220`, `persistence.rs:263`) and single-letter tenants.
- **Observable consequence:** with the only real extraction version, `Persistence::snapshot_pointer` can never return a pointer that a serde-backed implementation persisted; the cold facade (TASK-003) would rebuild every snapshot and the update facade (TASK-004) could never find its predecessor. Long-but-valid tenant ids hit the same wall for context records.
- **Testable correction:** in `ObjectKey::parse`, validate each segment as non-empty, not `.`/`..`, and `is_key_safe` without the name length cap (or give `validate_name` a caller-chosen cap); add a round-trip test using `bohmian_code_review::EXTRACTION_VERSION` and a 128-byte tenant. Nothing else changes; key layout and `Serialize` output stay identical.

### TASK-B-2 — INCORRECT (FIND-TASK-002-2 not closed for every sibling): `BoundedReader::read` lacks `# Errors`

- **Root cause:** the r1 sweep matched only `-> Result<`; `BoundedReader::read` returns `io::Result<usize>` and produces its own error (`io::Error::other(...)` on limit overrun) but carries a one-line summary only (`github.rs:565-566`).
- **Defect class:** missing mandatory rustdoc section on a new fallible item (same class as FIND-TASK-002-2).
- **Sibling locations:** `github.rs:565-566` only; every other `io::Result`/`Result<` site in the crate now has the section (own sweep, both patterns).
- **Violated obligation:** `AGENTS.md:32` ("Fallible functions need `# Errors`"); `architecture/references/languages/rust-core.md:7`.
- **Observable consequence:** the only place archive over-limit becomes an error to a caller reading the stream is undocumented, while `archive`'s own doc (`:948-949`) defers to it.
- **Testable correction:** add `# Errors` naming the propagated inner `io::Error` and the `other` error returned once `remaining == 0` and a further byte is available; rerun the sweep with both `-> Result<` and `-> io::Result<`.

### Residual observations (not findings)

- A well-formed key from another scope (`"tenants/b/installations/9/repositories/3/contexts/x"`) still deserializes (`scratch: true`). Scope binding needs the caller `Identity` at reload, which the facades (TASK-003–005) carry; segment validation was the remediation's accepted scope. Recorded for change review, not raised.
- Ponytail ladder over the changed abstractions: `HttpTransport` has two real implementations, `ObjectStorage`/`Persistence` are the task-mandated seams, `CallCounts`/`RequestLimits` are consumed by the scenarios. No DRIFT after the `ObjectKind` deletion.

## Commands run (all at candidate `93b2542`, read-only)

| Command | Observed |
|---|---|
| 9× `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib <name> -- --exact` (storage ×2, persistence ×2, github ×4, execution ×1) | each `1 passed; 0 failed`, exit 0 |
| `cargo run` in `/tmp/t002r2b` (path dep on candidate crate + facade) | output quoted under TASK-B-1; also `ContextId::new("..")` → `Err(Name("context id"))`, 101-byte and `é` ids refused, foreign well-formed key accepted |
| `# Errors` sweep (awk, 14-line lookback, `-> .*Result<` incl. `io::Result`) over `crates/bohmian-code-review-pipeline/src/*.rs` | one MISS: `github.rs:566` |
| `rg -n 'ObjectKind\|fn kind' crates` | no matches, exit 1 |
| `rg -n 'std::fs\|tokio\|axum\|\.unwrap\(\)' crates/bohmian-code-review-pipeline/src` | no matches, exit 1 |
| `git diff --stat 1842eaf..HEAD -- Cargo.toml mise.toml crates` | 9 files, +3094 |
| `git diff --stat dafe20b..HEAD` | 5 crate source files + review/task documents |
| `git log --oneline 1842eaf..HEAD` | 6 commits (`1696545` … `93b2542`) |

## Not covered (15-minute budget)

- Did not re-read `github.rs:1340-1929` test bodies line by line; relied on assertion survey (`rg`) plus passing runs and the r1 matrix for scenario coverage.
- Did not run `mise run gate`/`lints`/`fmt:check` (orchestrator runs them in parallel).
- Did not probe `GitHubHost::enterprise` with backslash/port spellings beyond the existing test set.

```citations
[
 {"id": "TASK-B-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 92, "snippet": "    fn parse(raw: &str) -> Result<ObjectKey, InvalidIdentity> {", "symbols": ["ObjectKey", "parse", "validate_name"]},
 {"id": "TASK-B-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 94, "snippet": "            validate_name(segment, \"object key segment\")?;", "symbols": ["validate_name"]},
 {"id": "TASK-B-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 73, "snippet": "            hex(extraction_version.as_bytes())", "symbols": ["ObjectKey", "snapshot", "hex"]},
 {"id": "TASK-B-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 13, "snippet": "const MAX_TENANT_BYTES: usize = 128;", "symbols": ["MAX_TENANT_BYTES", "TenantId"]},
 {"id": "TASK-B-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 16, "snippet": "const MAX_NAME_BYTES: usize = 100;", "symbols": ["MAX_NAME_BYTES", "validate_name"]},
 {"id": "TASK-B-1", "path": "crates/bohmian-code-review-extract/src/lib.rs", "line": 119, "snippet": "pub const EXTRACTION_VERSION: &str = \"codegraph-kernel-572d22bf.tree-sitter-0.25.10.facts-1\";", "symbols": ["EXTRACTION_VERSION"]},
 {"id": "TASK-B-1", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 51, "snippet": "    pub key: ObjectKey,", "symbols": ["SnapshotPointer", "ObjectKey"]},
 {"id": "TASK-B-2", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 565, "snippet": "    /// Reads at most the remaining budget; a further byte is an error.", "symbols": ["BoundedReader", "read"]},
 {"id": "TASK-B-2", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 566, "snippet": "    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {", "symbols": ["BoundedReader", "read"]}
]
```
