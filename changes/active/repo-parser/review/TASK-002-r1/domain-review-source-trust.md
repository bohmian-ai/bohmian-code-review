# TASK-002 r1 — Domain review: source trust / provider identity boundary

Reviewer: `domain-rev` (source trust). Read-only review of candidate `dafe20bd` against base `1842eafd`.

## Reviewed boundary

Everything in `crates/bohmian-code-review-pipeline` that decides whether provider data or a caller-supplied identifier is trusted before it crosses into the symbolic library or the injected storage/persistence contracts:

- `src/identity.rs` (full): `TenantId`, `InstallationId`, `RepositoryIdentity`, `Identity`, `ObjectSha`, `GitHubHost`, `InstallationToken`.
- `src/github.rs` (full, 1854 lines): request build, `send_with_policy`, redirect trust, blob hashing, tree/PR-file validation, error types, tests.
- `src/storage.rs`, `src/persistence.rs`, `src/lib.rs` (full).

## Authority and source coverage

- Spec `changes/active/repo-parser/spec.md`: REQ-004 (line 61), REQ-014 (77), REQ-015 (78), REQ-016 (79), INV-001 (85), INV-003 (87).
- Task `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md` Scenario 1 (lines 26-34) and acceptance (48).
- `AGENTS.md` line 29 (domain values at trust boundaries).
- `crates/bohmian-code-review-core/src/relative_path.rs` (what `RelativePath::new` rejects).
- Observed: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features` → 12 passed, 0 failed.

## Trace results (no finding)

- **Repository identity.** `repository()` (github.rs:781-802) compares numeric `id` exactly and `full_name` case-insensitively; a mismatch is `IdentityMismatch` before any field is returned. `commit`/`tree` compare the returned sha to the requested sha; `blob` recomputes `sha1("blob <len>\0" + bytes)` (github.rs:689-694, verified against the Git test vectors at 1834-1839) and rejects tampered bytes. `pull_request` verifies number and `base.repo.id`. Renamed repos: `UreqTransport` sets `max_redirects(0)` and `http_status_as_error(false)` (github.rs:112-116), so a provider 301 surfaces as `UnexpectedStatus` rather than a silent follow.
- **Tree / PR-file entries.** Every path goes through `RelativePath::new` (rejects absolute, `..`, `.`, empty segment, backslash, NUL, over-long); mode/status are closed enums; sha through `ObjectSha::new`. Invalid entries land in `rejected`, never in `entries` (github.rs:889-897, 1058-1066). `truncated` is carried verbatim as a public flag; nothing in this crate presents a truncated listing as complete. Wire `ObjectSha` fields deserialize through `ObjectSha::new` (identity.rs:254-260), so a malformed sha in a commit/branch/tree/PR body is `MalformedResponse`, not an `Ok`.
- **Archive redirect.** `is_trusted_redirect` (github.rs:729-734) requires literal `https://` and an exact (case-insensitive) match of the whole authority component against `archive_host()`; relative, `http://`, `host.evil`, `host@evil`, and cross-GHES targets fail closed (tests 1615-1626, 1654-1663, 1841-1852). The hop is sent with `authenticated=false` (github.rs:959-965, asserted at 1613); a second 3xx from the archive host is `UnexpectedStatus` via `expect_status(&hop, 200)`, so there is no loop.
- **Credentials.** `HttpRequest` has no `Debug`; `InstallationToken` `Debug` is redacted; `GitHubClient` derives no `Debug`; error variants carry only resource paths, status, limits, and the untrusted `Location`. The `Authorization` header is only ever attached in `build()` when `authenticated` is true, and the only `false` caller is the archive hop.
- **Response bounds.** `read_bounded` (github.rs:1217-1229) reads through `take(limit + 1)` and never trusts `content-length`, so allocation is bounded before the size check. Archive streams are wrapped in `BoundedReader` which errors rather than truncates.
- **Failure downgrades.** `IdentityMismatch`, `UntrustedRedirect`, `MalformedResponse`, `ResponseTooLarge` all map to `Disposition::Terminal` (github.rs:344-354); no verification failure is retryable.

## Findings

### TRUST-1 — `ObjectKey` scoping invariant is not enforced by the type

- **Classification.** Trust-boundary contract defect (cross-tenant key scoping), REQ-014 / REQ-015 / REQ-016.
- **Root cause (first wrong owner).** `ObjectKey::context` (storage.rs:84) accepts a free-text `context_id: &str` and interpolates it into the key without validation, and `ObjectKey` derives transparent `Deserialize` (storage.rs:52), so a key can also be materialized from persisted text without passing through either constructor.
- **Defect class.** Unvalidated identifier admitted at a trust boundary (missing domain value; violates AGENTS.md:29 "Use domain values such as `RelativePath` at trust boundaries").
- **Siblings (same root cause, one finding).**
  - storage.rs:84 — `context(identity, context_id: &str)`; no alphabet/segment check.
  - storage.rs:52 — `Deserialize` on `ObjectKey` bypasses `snapshot`/`context`.
  - persistence.rs:61 — `pub struct ContextId(pub String);` — public unvalidated field, no `new`, is the value downstream workflows hand back for reads.
  - persistence.rs:51 and persistence.rs:83 — `SnapshotPointer.key` / `ContextRecord.key` are the reload path that feeds unvalidated keys into `ObjectStorage::read`/`delete`.
- **Violated obligation.** storage.rs:3-5 (module doc: "The library never accepts an unscoped key: every `ObjectKey` is built here from a validated `Identity`, so a consumer's storage implementation only ever sees tenant-, installation-, and repository-scoped paths") and storage.rs:49-51 ("Keys are only produced by `ObjectKey::snapshot` and `ObjectKey::context`"). Spec REQ-014 (spec.md:77): "Tenant, installation and repository identities scope snapshot/context keys and every read". REQ-015 (spec.md:78) "exact-key lookup". The task's own evidence row (task doc line 78) claims "every key ... carries `Identity`".
- **Evidence (reproduced).** Scratch binary linking the candidate crate by path:
  ```
  ObjectKey::context(&identity_a, "../../../../../tenants/b/installations/1/repositories/2/contexts/x")
    → tenants/a/installations/42/repositories/7/contexts/../../../../../tenants/b/installations/1/repositories/2/contexts/x
  serde_json::from_str::<ObjectKey>("\"tenants/b/installations/1/repositories/2/contexts/x\"")
    → Ok, kind()=Context
  ContextId("../x".into()) → constructs
  ```
  All three succeed; the tenant-`a` context key resolves under tenant `b` on any path-normalizing backend (local filesystem is one of the four REQ-016 backends), and a deserialized key needs no identity at all.
- **Observable consequence.** The scoping guarantee the storage contract advertises to consumers is false at the type level. Any facade (TASK-005 read-back receives a `ContextId` from a downstream workflow; TASK-003/004 reload `SnapshotPointer.key`) that builds or reloads a key gets no protection from the type and must re-implement the check, or a hostile/buggy id or row reaches another tenant's object. `ObjectKey` is `pub use`d from `lib.rs`, so this is public API today.
- **Testable correction.** (a) Give `ContextId` a validating constructor (key-safe alphabet, e.g. the existing `is_key_safe` in identity.rs, bounded length) with a private field and a validating `Deserialize`, mirroring `TenantId`; make `ObjectKey::context` take `&ContextId`. (b) Make `ObjectKey` deserialize through a parser that requires the exact `tenants/<key-safe>/installations/<u64>/repositories/<u64>/(snapshots/<hex>/<u32>/<40-hex>|contexts/<key-safe>)` shape (or store the components and derive the string), so a reloaded key cannot name a path the constructors could not produce. (c) Tests: `ObjectKey::context` with `..`/`/` in the id is a compile error or `Err`; `serde_json::from_str::<ObjectKey>` of an out-of-shape string is `Err`; `ContextId::new("../x")` is `Err`.

## Verification limits

- Facade composition is TASK-003+; this review did not evaluate whether facades call `repository()` before `branch_head()`. Note for those tasks: the branch endpoint carries no repository id, so `branch_head` alone cannot detect a repository re-created under a transferred/renamed `owner/name`; `repository()` in the same execution is the intended check (identity.rs:139-141) and must precede it.
- `TransportError::Failed(other.to_string())` (github.rs:144) can echo a `ureq::Error::BadUri` text; for the archive hop that text is the provider's short-lived codeload URL (which the client already records in `UntrustedRedirect`). Not an installation-token leak; not reported as a finding.
- No live GitHub/GHES contract run was performed (opt-in per AC-006); redirect and header behavior were verified against the scripted fake and source only.
- Time budget: all listed sources were read in full; no area was skipped.

## Overall result

**FAIL** — TRUST-1.

```citations
[{"id": "TRUST-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 84,
  "snippet": "    pub fn context(identity: &Identity, context_id: &str) -> ObjectKey {", "symbols": ["ObjectKey", "context"]},
 {"id": "TRUST-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 52,
  "snippet": "#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]", "symbols": ["ObjectKey"]},
 {"id": "TRUST-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 3,
  "snippet": "//! The library never accepts an unscoped key: every [`ObjectKey`] is built", "symbols": ["ObjectKey"]},
 {"id": "TRUST-1", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 61,
  "snippet": "pub struct ContextId(pub String);", "symbols": ["ContextId"]},
 {"id": "TRUST-1", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 51,
  "snippet": "    pub key: ObjectKey,", "symbols": ["SnapshotPointer", "ObjectKey"]},
 {"id": "TRUST-1", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 83,
  "snippet": "    pub key: ObjectKey,", "symbols": ["ContextRecord", "ObjectKey"]},
 {"id": "TRUST-1", "path": "changes/active/repo-parser/spec.md", "line": 77,
  "snippet": "- **REQ-014 — Security and source lifetime.**", "symbols": []},
 {"id": "TRUST-1", "path": "AGENTS.md", "line": 29,
  "snippet": "- Use `&str`, `&[u8]`, slices, and references when ownership is unnecessary.", "symbols": []}]
```
