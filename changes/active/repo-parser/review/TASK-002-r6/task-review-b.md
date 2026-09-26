# TASK-002 task review r6: reviewer B (acceptance audit)

- Candidate: `7983907a6aa0b01a942fb964362da73bc9e7dba6`. Base: `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`.
- Subject: the original TASK-002, its Revision 11 delta (spec rev 11, REQ-015/AC-009), and the pending remediation TASK-002-R4 (FIND-TASK-002-1, -8, -9, -10, -11).
- **Overall: PASS.** No findings.

## Independent verification

These commands ran against a `git archive` export of the candidate at `/tmp/cr-r6/scratch-b`, with a separate target dir. Each ran with `mise exec -- ... --all-features`.

| Command | Result |
|---|---|
| `cargo test --locked -p bohmian-code-review-pipeline` | 23 lib tests passed, 0 doctests, exit 0 |
| `cargo test --locked --workspace --all-targets` | all suites ok (140/27/23/10/6/4/1 passed), 0 failed |
| `cargo clippy -p bohmian-code-review-pipeline --all-targets -- -D warnings` | clean |
| `cargo doc --no-deps -p bohmian-code-review-pipeline` | no pipeline warnings |
| Orchestrator `/tmp/cr-r6/lanes.txt` | `fmt:check`, `lints`, and `test:pipeline` each exit 0 |
| `rg -n 'validate_key_segment\|KeySegment' crates` | no matches (AC-R4-2) |
| `rg -n '<D: serde::\|&serde_json::' crates/bohmian-code-review-pipeline/src` | no matches (AC-R4-3) |
| `rg -n 'trait Persistence\b' crates` | no matches (Rev 11) |
| `rg -n 'impl .*(SnapshotStore\|ContextStore) for' src` | only `MemorySnapshotStore: SnapshotStore`, so no type implements both |
| `rg -n 'ApiBase\|non_exhaustive\|compile_fail\|unwrap_or\(""\)' identity.rs` | only `InvalidIdentity::ApiBase` (see note N-1) |
| `git diff 54232b2~1 54232b2 --stat`; `git diff 1842eaf 7983907 -- mise.toml` | R4 touched `github.rs`, `identity.rs`, `persistence.rs`, `storage.rs`, and `snapshot.rs` (one line: `GitHubHost::public()` call site). `mise.toml` only adds the original `test:pipeline` lane (AC-R4-8) |

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| TASK-002 S1 / REQ-004: verified identities and blob bytes; both hosts via configuration | `GitHubClient::{repository, branch_head, commit, tree, pull_request, blob}` check ids and shas against the request; `GitHubHost::{public, enterprise}` | `verified_provider_identities_cross_and_mismatches_are_rejected`, `hosts_expose_api_base_and_archive_host` pass | PASS |
| TASK-002 S1 / REQ-014: untrusted redirect rejected; no credential forwarding | `archive` follows one hop, unauthenticated, only when `is_trusted_redirect` holds (`github.rs:775-780`, `:1001-1011`) | `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` asserts `!seen[n].1` on every hop | PASS |
| TASK-002 S2 / REQ-013 / INV-006: typed retry or terminal outcomes; deadline and cancellation | a single `send_with_policy` (`github.rs:1199-1245`); `disposition()` | `rate_limits_retry_within_bounds_or_report_typed_outcomes`, `malformed_payloads_and_interruptions_are_explicit`, `check_reports_cancellation_then_deadline` | PASS |
| Rev 11 / REQ-015: `Persistence` split into `SnapshotStore` (pointer lookup, conditional publish, repository removal) and `ContextStore` (publish, exact lookup, expired listing, delete, repository removal) | `persistence.rs:131-232`. Methods equal the old trait's methods, partitioned; no method added; no pool or consumer type; `PersistenceError` shared | `rg 'trait Persistence\b'` is empty; the old trait at `b83d286~1` had exactly these 7 methods | PASS |
| Rev 11 consumer closure: test implementation and `snapshot.rs` move to the snapshot store; `lib.rs` re-exports | `testing.rs:153-223` `MemorySnapshotStore`; `ColdBuild` takes `&dyn SnapshotStore` (`snapshot.rs:316,325`); `lib.rs:37-40` | the 4 `snapshot::tests` pass | PASS |
| Rev 11: `ObjectStorage` unchanged; no OpenDAL or backend (REQ-016) | no `storage.rs` trait changes; no new dependency | diff inspection | PASS |
| AC-R4-1 (FIND-1): only the two constructor layouts reload | `ObjectKey::parse`/`canonical` (`storage.rs:96-148`) match both layouts, validate each part with `TenantId::new`, `InstallationId::new`, `NonZeroU64`, even-length lowercase hex, `u32`, `ObjectSha::new`, and `ContextId::new`, then require a byte-exact re-format | `hostile_keys()` covers every listed case (x, a/b/c, foo/bar, inst 0, repo 0, inst 01, non-hex, odd hex, bad tree, trailing segments, traversal/empty). It is exercised against `ObjectKey`, `ContextRecord.key`, and `SnapshotPointer.key` (`storage.rs` and `persistence.rs` tests). 128-byte tenant plus `EXTRACTION_VERSION` round-trip passes | PASS |
| AC-R4-2 (FIND-1 DRIFT) | `validate_key_segment` deleted; character rule folded into `validate_name`; `KeySegment` replaced by `InvalidIdentity::ObjectKey`; module and type docs describe layout parsing (`storage.rs:3-5`, `:40-46`) | rg empty | PASS |
| AC-R4-3 (FIND-8) | bare `Deserializer` imported in `identity.rs`, `persistence.rs`, and `storage.rs`; `serde_json::Error as JsonError` (`github.rs:16`) used at `:1343` | rg empty; no other crate-qualified signature types in the TASK-002 files | PASS |
| AC-R4-4 (FIND-9): secondary limit without headers → retryable 60 s; bare 403 → `Forbidden`; bare 429 → 60 s | `SECONDARY_RATE_LIMIT_WAIT` (`github.rs:730`); `names_rate_limit` reads at most 4 KiB, never copies text out (`:757-772`); routed through the common `(wait, RateLimited)` path, so the `max_retry_wait`, deadline, and attempt bounds still apply; early `max_retry_wait` return deleted; `RateLimited` doc and `# Errors` updated | test asserts `RateLimited{60s}` + `Retry` for a 403 with `x-ratelimit-remaining: 4990` and a secondary-limit body under `max_retry_wait=5s`; `Forbidden` + `Terminal` for an empty 403; `RateLimited{60s}` for a bare 429; header-driven cases unchanged | PASS |
| AC-R4-5 (FIND-10): no URL in the redirect error | `UntrustedRedirect` is a unit variant (`github.rs:343-346`) | test asserts that `to_string()` contains neither `secret` nor the URL | PASS |
| AC-R4-6 (FIND-11): public trusts only codeload.github.com; GHES trusts host plus codeload.host; GHES codeload hop unauthenticated | `GitHubHost { api_base, archive_hosts }` with private fields, computed once (`identity.rs:295-353`); `trusts_archive_host` compares ASCII case-insensitively | the identity test covers public/GHES trust and rejects `codeload.ghe.example.com.evil`, `evil.example.com`, and `codeload.github.com` for GHES. The github test follows `https://codeload.ghe.example.com/x` with `!seen[3].1`. `api_base()` values are unchanged | PASS |
| AC-R4-7 (FIND-11 DRIFT) | `Enterprise` variant, `#[non_exhaustive]`, `ApiBase` newtype, both `compile_fail` doctests, and the `unwrap_or("")` re-parse all deleted; the private fields seal the struct | doctests: 0 run; only `InvalidIdentity::ApiBase` remains (N-1) | PASS |
| AC-R4-8: write set | R4 commit limited to pipeline `src/`; `lib.rs` unchanged by R4; `mise.toml` unchanged since the original lane | `--stat` | PASS |
| Constraints: no new dependency, trait, feature, or generic layer; key layout and `Serialize` output unchanged; bearer on API only; `max_redirects(0)` | `Cargo.toml` and `Cargo.lock` untouched by R4; constructors identical; `UreqTransport::new` unchanged | diff inspection; round-trip tests | PASS |
| AGENTS.md rustdoc on new or changed items, `# Errors` on fallible ones | `parse`, `canonical`, `names_rate_limit`, both constants, `trusts_archive_host`, `enterprise`, `hostile_keys`, and the store traits are all documented; fallible items carry `# Errors` | `cargo doc` clean | PASS |
| Non-goals: no scope-binding of reloaded keys, no live run, no host in errors | none present | inspection | PASS |

## Ponytail ladder and DRIFT sweep

- `GitHubHost` is a plain struct with two private fields and one predicate. That is the smallest shape that seals the host and supports two archive hosts. No doctest scaffolding remains.
- `ObjectKey::canonical` reuses the crate's existing validators and stdlib `NonZeroU64`/`u32` parsing. The byte-exact round-trip removes any need for per-rule canonicality checks. No new error type was added; one variant replaces another.
- `names_rate_limit` uses stdlib `Read::take`. It adds no dependency and no type.
- The store split adds no method, and the test double shrank to the snapshot store only. The context half of the old `MemoryPersistence` was dead for TASK-002 and was deleted, not ported. AC-009 contract tests belong to TASK-003 and TASK-005 by their `maps:`.
- Earlier-round code the new shape made redundant has all been removed: `validate_key_segment`, `KeySegment`, `ApiBase` newtype, `#[non_exhaustive]`, the doctests, `archive_host()`, and the early `max_retry_wait` return. Kept: `is_key_safe`, still used by `TenantId` and `validate_name`, and `read_bounded_accepts_the_maximum_limit`, whose R3 fix still holds.

## Notes (not findings)

- **N-1.** AC-R4-7's literal regex `ApiBase` still matches `InvalidIdentity::ApiBase`. The same packet requires that `enterprise` "keeps its current validation and error" and that "`InvalidIdentity` keeps its other variants". The newtype the AC targets is gone, so the regex is broader than its intent.
- **N-2.** Commit `b83d286` ("refactor ... split Persistence") also carries rustfmt changes and a behavior fix in out-of-scope TASK-003 WIP files. The fix is `eligibility.rs` `glob_matches` gaining a trailing `**` arm, plus a raw-header tar fixture in `snapshot.rs` tests. Per `common.md` these are lane-greening edits to TASK-003 code and are not judged here. The trailing-`**` arm matches gitattributes semantics. The TASK-003 reviewer should own it.

```citations
[]
```
