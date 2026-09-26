# TASK-002 r5 — task implementation review B

Subject: candidate `6899a43` against base `1842eafd`. The obligation is the original TASK-002 plus remediations R1, R2 and R3. All source was read from `/tmp/cr-r5/wt`. Scratch work is under `/tmp/cr-r5/scratch-task-rev-b/`.

## Verification run by this reviewer

- `CARGO_TARGET_DIR=/tmp/cr-r5/target-task-rev-b mise run test:pipeline` (in the wt) exited 0: 17 lib tests passed, and the `Doc-tests bohmian_code_review_pipeline` section shows 2 `compile fail ... ok` lines. `fmt:check` and `lints` exit 0 per `/tmp/cr-r5/lanes.txt`.
- `keyprobe`: a scratch crate with a path dependency on the candidate crate. It deserializes hostile but segment-safe keys. Output:
  ```
  "x" -> Ok("x")
  "a/b/c" -> Ok("a/b/c")
  "tenants/b/installations/0/repositories/0/contexts/x" -> Ok(...)
  "tenants/b/installations/9/repositories/3/snapshots/zz/notanumber/nothex" -> Ok(...)
  record with key "x" and digest "d" -> Ok("x")      # ContextRecord
  ```
- `vac.rs`: a standalone rustc probe. `ApiBase` has a **public** field and `From<String>`, and its variant can be constructed publicly. The body of doctest 2 (`*api_base = "http://evil".to_owned();`) still fails with `error[E0308]: mismatched types`.

## Requirement matrix

| Requirement / AC / constraint / non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| T2 AC: github.com and GHES are both supported through configuration (REQ-004) | `identity.rs` `GitHubHost::{Public, enterprise}`; `github.rs` `request` builds the URL from `api_base()` | `hosts_expose_api_base_and_archive_host`; the GHES half of the archive redirect test | PASS |
| T2 S1: mismatched repository, SHA, path or blob is rejected before parsing | `repository`, `branch_head`, `commit`, `tree`, `pull_request` compare ids and names; `blob` verifies `git_blob_sha`; paths go through `RelativePath` into `rejected` | `verified_provider_identities_cross_and_mismatches_are_rejected` passes | PASS |
| T2 S1: untrusted archive redirect is rejected and no credentials are forwarded | `archive` + `is_trusted_redirect`; the hop is sent with `authenticated = false` | `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` passes | PASS |
| T2 S2: 403/429, retry-after, cancellation and deadline give typed Retry/Terminal outcomes under one policy | `send_with_policy`, `rate_limit_wait`, `sleep_bounded`, `GitHubError::disposition` | `rate_limits_retry_within_bounds_or_report_typed_outcomes`, `malformed_payloads_and_interruptions_are_explicit`, `check_reports_cancellation_then_deadline` pass | PASS |
| T2 S2: partial input is not presented as complete | `TreeListing::truncated`, `rejected`, `PullRequestFilesPage::has_next`, `BoundedReader` errors instead of truncating | lane tests | PASS |
| T2: narrow storage/persistence traits; no pool, SDK or consumer type (REQ-015/016) | `storage.rs` `ObjectStorage`, `persistence.rs` `Persistence` | code read; deps are facade, serde, sha1/2, thiserror, ureq | PASS |
| INV-007: library does "not accept unscoped object keys" (storage.rs:3 restates it) | `ObjectKey` `Deserialize` → `parse` checks segments only | keyprobe: `"x"`, `"a/b/c"` and installation `0` all deserialize | **FAIL** (TASK-B-1) |
| INV-001: no clone, disk, server or symbolic duplication | no `std::fs`/`tokio`/`axum`; depends on facade only | `rg` shows no match | PASS |
| T2: scoped `mise` lane | `test:pipeline` (list form), wired into `test:rust` | lane run exits 0 | PASS |
| AC-R1-1: `ObjectKey::context` takes `&ContextId` | `storage.rs:78` | `keys_are_scoped_by_identity_and_version` | PASS |
| AC-R1-2: hostile keys refused; constructor keys round-trip. Intent: "serde cannot mint a key the constructors would never produce" (storage.rs:108) | segment-only `parse` | the listed traversal strings are refused, but keyprobe mints non-constructor keys | **FAIL** (TASK-B-1) |
| AC-R1-3: `ContextId` is validated, has a private field, and re-validates | `persistence.rs:66-96` | `context_ids_are_validated_on_construction_and_deserialization` | PASS |
| AC-R1-4 / AC-R2-4: `# Errors` on fallible items | present on every fallible fn read | code read of every `Result`-returning fn in the diff | PASS |
| AC-R1-5: `ObjectKind` removed | — | no match | PASS |
| AC-R2-1/2: `EXTRACTION_VERSION` and 128-byte tenant keys round-trip | `validate_key_segment` | `long_constructor_segments_round_trip`, `records_revalidate_their_keys_on_deserialization` | PASS (but see DRIFT in TASK-B-1) |
| AC-R2-5 / AC-R3-1: external construction and `&mut` overwrite of the GHES base are impossible **and proved by the lane** | `ApiBase(String)` private field; `#[non_exhaustive]` | the seal holds, but both doctests fail with E0308 whether or not the seal exists (vac.rs) | **FAIL** (TASK-B-2) |
| AC-R2-6: `archive_host` has no slice panic | `strip_prefix` + `split` | test passes | PASS |
| AC-R3-3: the lane runs doctests | `mise.toml` second command | lane output | PASS |
| AC-R3-4: `read_bounded` cannot overflow | `saturating_add(1)` | `read_bounded_accepts_the_maximum_limit` | PASS |
| AC-R3-5/6/7: `# Panics`, test-module rustdoc, bare `fmt` | `execution.rs:59`, `///` above each `mod tests`, `use std::fmt` | code read | PASS |
| Ponytail / superseded fixes | `#[non_exhaustive]` on `Enterprise` (added in R2) is redundant after the R3 `ApiBase` seal | reasoning plus vac.rs | **FAIL** (TASK-B-3, DRIFT) |
| Non-goals: no facade, OpenDAL, limiter, real-GitHub run; symbolic crates untouched | diff is limited to the new crate, manifests and `mise.toml` | `git diff --stat` | PASS |

## Findings

### TASK-B-1 — INCORRECT (+ DRIFT): `ObjectKey` reload admits keys no constructor produces, including unscoped ones

- **Root cause:** `ObjectKey::parse` (`storage.rs:94`) is the first owner of the decision. It validates each `/` segment against the key-safe alphabet and ignores the key grammar. `ObjectKey::prefix`, `snapshot` and `context` produce exactly `tenants/<TenantId>/installations/<nonzero>/repositories/<nonzero>/(snapshots/<lowercase hex>/<u32>/<ObjectSha> | contexts/<ContextId>)`. This candidate owns the type: its private `String` field and hand-written `Deserialize` are an implementation choice.
- **Defect class:** a validated seam value whose re-validator is weaker than its constructors (the parser accepts a superset of what the constructors produce).
- **Sibling locations (one finding):**
  - `storage.rs:94` `parse` (owner)
  - `storage.rs:108-109` doc that claims the opposite
  - `storage.rs:3-5` and `:41-45` module and type invariants ("never accepts an unscoped key", "only produced by snapshot and context")
  - `persistence.rs:52` `SnapshotPointer.key` and `persistence.rs:111` `ContextRecord.key` (they inherit through the field type)
  - `persistence.rs:206` `remove_repository -> Vec<ObjectKey>` (keys come back from a consumer implementation)
- **Violated obligation:**
  - INV-007 (`spec.md:91`, "do not accept unscoped object keys")
  - REQ-014 (`spec.md:77`, identities scope keys and every read)
  - R1 intended outcome ("every value that crosses the storage/persistence seam is constructible only through validation") and AC-R1-2
  - the candidate's own contract at `storage.rs:108`
- **Evidence:** keyprobe output above. `"x"`, `"a/b/c"`, installation/repository `0`, and a non-hex version with a non-numeric format and non-SHA tree all deserialize as `ObjectKey`. A `ContextRecord` with `key: "x"` deserializes.
- **Observable consequence:** a `Persistence` implementation (the consumer-owned side of the seam) can hand the library a pointer or record naming an unscoped or constructor-impossible key. Every TASK-003..005 facade receives it as a trusted `ObjectKey` and passes it to `ObjectStorage::read`/`delete`. The segment check blocks only `..` traversal. The documented invariant that a storage backend "only ever sees tenant-, installation-, and repository-scoped paths" is false. This is distinct from the deferred TASK-A-1 (binding a *well-formed* key to the caller's identity): here the key is not even scoped.
- **Correction (unrepresentable shape):**
  1. `parse` should accept only the constructor grammar and check each component with the validators that already exist: `TenantId::new`, `InstallationId::new`, a nonzero repository id, even-length lowercase hex, `u32`, `ObjectSha::new`, `ContextId::new`. Anything else is refused.
  2. Equivalently, `parse` can decompose the key and require `ObjectKey::snapshot(..)`/`context(..)` of the parsed parts to equal the raw text.
  3. The wire format and `Serialize` output stay unchanged.
- **DRIFT this deletes:**
  - `identity.rs:149` `validate_key_segment` and `identity.rs:38` `InvalidIdentity::KeySegment` (both added in R2) exist only to serve segment-wise parsing. Once `parse` routes through the component validators, they are unused.
  - `validate_name` returns to its pre-R2 inline check.
- **Tests:**
  - `keyprobe`'s five strings must be `Err`.
  - The existing hostile set and `long_constructor_segments_round_trip` must stay green.

### TASK-B-2 — INCORRECT: the `GitHubHost` seal doctests pass for a reason unrelated to the seal

- **Root cause:** both `compile_fail` doctests (`identity.rs:311`, `identity.rs:320`) assign or construct from a `String` where the field type is `ApiBase`. Rustdoc's `compile_fail` accepts any compile error, and E0308 (mismatched types) fires whether or not `ApiBase` is sealed.
- **Defect class:** a regression proof that cannot fail, so it proves nothing.
- **Sibling locations:** `identity.rs:311` (construction doctest), `identity.rs:320` (`&mut` overwrite doctest).
- **Violated obligation:**
  - AC-R3-1 and AC-R2-5 intent: an executable guard of the seal
  - FIND-TASK-002-5's rationale in R3 (the seal must have a CI regression guard)
  - `AGENTS.md` test rule ("never weaken a gate")
- **Evidence:** `vac.rs` gives `ApiBase` a public field and `impl From<String>`, and allows public variant construction. The doctest-2 body still errors with `E0308: mismatched types`. Adding `pub` to `ApiBase.0` or giving it a public constructor would therefore leave `test:pipeline` and `gate` green while reopening FIND-TASK-002-4, because the attacker would write `api_base.0 = ...` or `*api_base = ApiBase::from(...)`.
- **Correction:** replace the bodies with ones whose only possible error is the seal. For example:
  - `bohmian_code_review_pipeline::identity::ApiBase("http://x".to_owned())`: private tuple constructor, E0423/E0603
  - `if let GitHubHost::Enterprise { api_base, .. } = &mut h { api_base.0 = "http://evil".to_owned(); }`: private field, E0616

  Record RED by making the field `pub` in a scratch copy and observing that the doctests then compile.

### TASK-B-3 — DRIFT: `#[non_exhaustive]` on `GitHubHost::Enterprise` is superseded by the R3 `ApiBase` seal

- **Root cause:** R2 added `#[non_exhaustive]` (`identity.rs:328`) to block external construction. R3 made the payload an `ApiBase` with a private field, and that alone makes external construction impossible without an already validated `ApiBase`; a cloned validated value is harmless. R3 left the attribute in place ("keep or drop").
- **Defect class:** remediation code made redundant by a later, stronger fix.
- **Sibling locations:** `identity.rs:328` only. Its sole proof is the vacuous doctest at `identity.rs:311` (see TASK-B-2).
- **Violated obligation:** revised review rule (superseded-fix deletion); `AGENTS.md` "no ... unnecessary features"; Ponytail rung 1.
- **Consequence:** it only forces external matches to write `..` and adds a second, misleading "this is the seal" mechanism.
- **Correction:** delete the attribute. The seal is covered by the private `ApiBase` field and proved by the corrected TASK-B-2 doctests.

## Overall

**FAIL**. TASK-B-1 (INCORRECT + DRIFT), TASK-B-2 (INCORRECT), TASK-B-3 (DRIFT).

```citations
[
{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":94,"snippet":"    fn parse(raw: &str) -> Result<ObjectKey, InvalidIdentity> {","symbols":["ObjectKey::parse"]},
{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":108,"snippet":"    /// Deserializes through segment re-validation so serde cannot mint a key","symbols":["ObjectKey::deserialize"]},
{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":3,"snippet":"//! The library never accepts an unscoped key: every [`ObjectKey`] is built","symbols":["ObjectKey"]},
{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/persistence.rs","line":52,"snippet":"    pub key: ObjectKey,","symbols":["SnapshotPointer"]},
{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/persistence.rs","line":111,"snippet":"    pub key: ObjectKey,","symbols":["ContextRecord"]},
{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/persistence.rs","line":206,"snippet":"    fn remove_repository(&self, identity: &Identity) -> Result<Vec<ObjectKey>, PersistenceError>;","symbols":["Persistence::remove_repository"]},
{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":149,"snippet":"pub(crate) fn validate_key_segment(raw: &str) -> Result<(), InvalidIdentity> {","symbols":["validate_key_segment"]},
{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":38,"snippet":"    KeySegment,","symbols":["InvalidIdentity::KeySegment"]},
{"id":"TASK-B-2","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":311,"snippet":"/// let _ = bohmian_code_review_pipeline::GitHubHost::Enterprise { api_base: \"http://x\".to_owned() };","symbols":["GitHubHost"]},
{"id":"TASK-B-2","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":320,"snippet":"///     *api_base = \"http://evil\".to_owned();","symbols":["GitHubHost","ApiBase"]},
{"id":"TASK-B-3","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":328,"snippet":"    #[non_exhaustive]","symbols":["GitHubHost::Enterprise"]}
]
```
