# TASK-002 task review r6 — reviewer A (acceptance audit)

- Candidate: `7983907a6aa0b01a942fb964362da73bc9e7dba6` (read at `/tmp/cr-r6/wt`), base `1842eaf`.
- Scope: TASK-002 original obligations, the Revision 11 delta (REQ-015/AC-009 store split), and remediation TASK-002-R4 (FIND-TASK-002-1, -8, -9, -10, -11). TASK-003 behavior in `snapshot.rs`, `eligibility.rs`, `testing.rs` is out of scope. Only the edits made so they compile against the new traits and keep lanes green were checked.

## Overall: PASS

I found no MISSING, INCORRECT, DRIFT, VIOLATION or REGRESSION finding that survived a reachable-path check.

## Verification run by this reviewer

These commands ran in scratch copy `/tmp/cr-r6/scratch-a`. `mise` refused the untrusted scratch config, so I called `cargo` directly with the same arguments.

- `cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features`: 23 passed, 0 failed.
- `cargo doc --no-deps -p bohmian-code-review-pipeline --all-features`: no pipeline warnings.
- Orchestrator lanes at the candidate (`/tmp/cr-r6/lanes.txt`):
  - `fmt:check` exit 0
  - `lints` exit 0, including clippy and test placement
  - `test:pipeline` exit 0; the `--doc` step runs 0 doctests, as expected after the R4 deletion
- Sweeps at the candidate:
  - `rg -n 'validate_key_segment|KeySegment' crates` finds nothing.
  - `rg -n '<D: serde::|&serde_json::|serde::Deserializer|serde_json::Error' src` matches only the import `use serde_json::Error as JsonError;` at `github.rs:16`.
  - A broader sweep for crate-qualified types finds only body-level `serde::de::Error::custom` and `serde_json::error::Category::*` match arms. R4 explicitly allows body-level paths.
  - `rg -n 'ApiBase|non_exhaustive|compile_fail|unwrap_or\(""\)' identity.rs` matches only the retained `InvalidIdentity::ApiBase` error variant, at `identity.rs:46,318,324,333,475`.
    - Read literally, AC-R4-7 would forbid this variant. R4 step 5 requires `enterprise` to "keep its current validation and error", and the two conflict. The newtype, attribute, doctests and re-parse the AC targets are all gone.
  - `rg -n 'trait Persistence\b' crates` finds nothing.
  - The only remaining `Persistence` identifier (not counting `PersistenceError`) is the TASK-003 variant `ColdBuildError::Persistence`.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Rev 11: `Persistence` replaced by two narrow traits | `persistence.rs` `SnapshotStore` (`snapshot_pointer`, `publish_snapshot_pointer`, `remove_repository`) and `ContextStore` (`publish_context`, `context`, `expired_contexts`, `delete_context`, `remove_repository`); `PersistenceError` shared; no pool or consumer type | sweep above; diff `b83d286` only splits and moves methods, adds none; each `remove_repository` doc names only its own records | PASS |
| Rev 11: identity, object-before-row, no transaction across IO, exact key kept on both traits | trait docs on both traits restate the identity refusal, no open transaction across IO, and publish-after-object | read of `persistence.rs` | PASS |
| Rev 11: `ObjectStorage` unchanged; no OpenDAL or backend (REQ-016) | `storage.rs` trait unchanged; `Cargo.toml` deps: flate2, serde, serde_json, sha1, sha2, tar, thiserror, ureq, bohmian-code-review | Cargo manifest read | PASS |
| Rev 11 consumer closure: in-memory store and partial cold build move to snapshot store; no type implements both | `testing.rs` `MemorySnapshotStore` only (context half removed); `snapshot.rs` holds `&dyn SnapshotStore`; `lib.rs` re-exports `SnapshotStore`, `ContextStore` | diff `b83d286`; 23 tests pass | PASS |
| Rev 11: stop if a facade needs the other store's records | the only facade (ColdBuild) uses the snapshot store alone | read of `snapshot.rs` field types | PASS |
| AC-R4-1: only the two constructor layouts reload | `storage.rs` `ObjectKey::parse`/`canonical` | `hostile_keys()` has 18 strings, covering every AC case, and is exercised by `keys_are_revalidated_on_deserialization` and `records_revalidate_their_keys_on_deserialization` (`ObjectKey`, `ContextRecord.key`, `SnapshotPointer.key`); `long_constructor_segments_round_trip` passes | PASS |
| AC-R4-2: no `validate_key_segment`/`KeySegment`; layout docs | `InvalidIdentity::ObjectKey`; module, type, `parse` and `Deserialize` docs describe layout parsing | sweep | PASS |
| AC-R4-3: imported bare types in signatures | `Deserializer` imported in `identity.rs`, `persistence.rs`, `storage.rs`; `JsonError` alias in `github.rs` | sweep | PASS |
| AC-R4-4: header-less secondary limit is retryable with an honest wait; plain 403 stays `Forbidden` | `github.rs` `SECONDARY_RATE_LIMIT_WAIT`, `RATE_LIMIT_BODY_BYTES`, `names_rate_limit`; `send_with_policy` routes 429 and body-named 403 through the existing `RateLimited` path; the early `max_retry_wait` return is gone; the `RateLimited` doc and `# Errors` are updated | `rate_limits_retry_within_bounds_or_report_typed_outcomes` passes: secondary 403 gives `RateLimited(60s)`/`Retry`, bare 403 gives `Forbidden`, bare 429 gives 60s | PASS |
| AC-R4-5: no URL in redirect error | `GitHubError::UntrustedRedirect` unit variant, message "untrusted archive redirect" | archive test asserts that neither `secret` nor the URL appears in `to_string()` | PASS |
| AC-R4-6: host trust (public, GHES host, `codeload.<host>`) | `GitHubHost { api_base, archive_hosts }` with private fields; `public()`, `enterprise()`, `trusts_archive_host`; `is_trusted_redirect(&str, &GitHubHost)` | `hosts_expose_api_base_and_archive_host`; the archive test follows `https://codeload.ghe.example.com/x` with no credentials and rejects `codeload.github.com` on GHES | PASS |
| AC-R4-7: seal scaffolding gone; struct literal cannot be built outside the crate | enum, `#[non_exhaustive]`, `ApiBase` newtype, `compile_fail` doctests and `unwrap_or("")` are all deleted; fields are private | sweep (see the note above) | PASS |
| AC-R4-8: write set | commit `54232b2` touches `github.rs`, `identity.rs`, `persistence.rs`, `storage.rs`, and `snapshot.rs` (one line, `GitHubHost::public()` call site); `mise.toml` is untouched | `git diff --stat` | PASS |
| R4 constraints: no dependency, trait or feature added; key layout, `Serialize`, bearer attach, single unauthenticated hop, retry policy and `read_bounded` preserved | diff of `54232b2` | read of the diff; the archive and rate-limit tests keep their earlier expectations | PASS |
| TASK-002 original (Scenarios 1–2, REQ-004/013/016, INV-001/003/006) | unchanged apart from the above | 23 tests pass; the earlier rounds' matrices were re-checked by running the tests, not taken from the evidence section | PASS |

## Ponytail and DRIFT review

- `validate_key_segment` and `KeySegment` are deleted. `is_key_safe` stays because `TenantId` and `validate_name` both use it.
- The `GitHubHost` seal is now plain private fields, so R2's `#[non_exhaustive]`, R3's `ApiBase` newtype and the doctests were correctly removed. Nothing redundant is left over from R1–R3.
- `names_rate_limit` and its constant are the minimum needed for FIND-9.
- The R4 recommendation prescribed `canonical` + `parse`; each is a single responsibility.

## Notes checked and dismissed (not findings)

- REQ-015 lists "expired-snapshot cleanup" for the snapshot store. The approved Rev 11 delta fixes that trait's method set and forbids adding methods, and TASK-003 owns expired-snapshot cleanup.
- AC-009's separate in-memory context store is owned by TASK-005, which maps it.
- A 403 body is read at most 4 KiB through the transport's bounded stream and is never copied into an error.
- Authority matching rejects userinfo, port or query tricks, for example `codeload.github.com@evil`, because the whole authority must equal a trusted host.
- Non-canonical numbers such as `+1` or `07` are refused by the byte-for-byte re-format.
- The `eligibility.rs` and `snapshot.rs` edits in `b83d286` are rustfmt output, trait renames, and test fixes that keep lanes green. Their TASK-003 behavior is out of scope.

```citations
[]
```
