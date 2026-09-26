# TASK-002 r2 — domain review: SOURCE TRUST

- Candidate: `93b2542da4ccb26ff9468c22b35605ce0091443a` (base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`), reviewed cumulatively.
- Reviewer: `domain-rev` (source trust). Read-only; scratch crate at `/tmp/t002r2-trust` (path dependency on the candidate crate).
- Overall: **FAIL** (one material finding, same defect class as FIND-TASK-002-1, sibling not swept by r1).

## Reviewed boundary

Everything that decides whether provider data or seam values are trusted:

- `crates/bohmian-code-review-pipeline/src/identity.rs` (full): `TenantId`, `InstallationId`, `validate_name`, `RepositoryIdentity`, `Identity`, `ObjectSha`, `GitHubHost`, `InstallationToken`.
- `crates/bohmian-code-review-pipeline/src/storage.rs` (full): `ObjectKey` (constructors, `parse`, re-validating `Deserialize`, `Display`), `ObjectDigest`, `ObjectStorage`.
- `crates/bohmian-code-review-pipeline/src/persistence.rs` (full): `ContextId`, `SnapshotScope`, `SnapshotPointer`, `ContextRecord`, `Persistence`.
- `crates/bohmian-code-review-pipeline/src/github.rs` lines 1–1360 (production code) plus `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` test: identity verification in `repository`, `branch_head`, `commit`, `tree`, `blob`, `archive`, `pull_request`, `pull_request_files`; `git_blob_sha`; `is_trusted_redirect`; `build` (header attachment); `send_with_policy`; `read_bounded`; `parse_json`; `validate_tree_entry`; `validate_pull_request_file`.
- `crates/bohmian-code-review-pipeline/src/lib.rs` (re-exports).
- Supporting: `crates/bohmian-code-review-core/src/relative_path.rs` (`RelativePath::new` rejection set), ureq 3.4.2 `src/run.rs:224-247` and `src/config.rs:257-259` (redirect semantics under `max_redirects(0)`).

## Authority and source coverage

| Obligation | Where enforced at candidate | Verified how | Result |
|---|---|---|---|
| REQ-004 repository identity verified before trust | `github.rs:795-802` exact numeric `id` + `full_name` (ASCII case-insensitive, acceptable because id is exact and GitHub names are case-insensitive) | read; existing test `verified_provider_identities_cross_and_mismatches_are_rejected` | PASS |
| REQ-004 branch / commit / tree identity | `github.rs:829` exact `wire.name != branch`; `:857` `wire.sha != *sha`; `:887` `wire.sha != *tree`; all shas deserialized through re-validating `ObjectSha` (`identity.rs:275-286`) | read | PASS |
| INV-003 blob identity | `github.rs:929-935` recomputes `blob <len>\0<bytes>` SHA-1 (`:694-700`) and compares exactly to lowercase hex | read; test `helpers_encode_paths_and_hash_blobs_like_git` | PASS |
| REQ-004 PR identity | `github.rs:1003` exact `number` and `base.repo.id == repository.id()`; deleted-fork base (`None`) is a mismatch | read | PASS |
| REQ-014 path validation | tree and PR paths (incl. `previous_filename`) through `RelativePath::new` (`github.rs:1319-1320`, `:1341-1350`), rejecting empty/absolute/UNC/backslash/NUL/`.`/`..`/too-long (`relative_path.rs:37-63`); per-entry rejection retained as `RejectedPath` with raw text never used as identity | read | PASS |
| REQ-014 archive redirect: scheme, host, single hop, no credential forwarding | `is_trusted_redirect` (`github.rs:735-740`) requires `https://` and exact host token before first `/` (so `host@evil`, `host?x`, `host:443` variants are rejected); hop sent with `authenticated=false` (`:965-971`, `:1132-1134`); a second 3xx on the hop is `UnexpectedStatus` via `expect_status(200)` (`:972`); non-archive operations never follow 3xx (`get_json` → `expect_status`) | read; test asserts no `Authorization` on hop and no request to untrusted host; ureq confirmed to return the 3xx response when `max_redirects(0)` (`config.rs:257-259` `max_redirects_do_error` is false at 0) | PASS |
| REQ-014 credentials never in logs/errors | `InstallationToken` has no `Display`, redacted `Debug` (`identity.rs:378-383`); `expose()` used only at `github.rs:1133`; `HttpRequest` has no `Debug`; `GitHubClient` has no `Debug`; `TransportError::Failed` carries ureq error text (no headers); `MalformedResponse.detail` is category+position only (`:1290-1298`) | `rg expose\(` → one site | PASS |
| REQ-004 oversize input | `read_bounded` (`:1265-1277`) `take(limit+1)` for JSON and blob; `BoundedReader` for archive | read | PASS |
| REQ-014/REQ-015 seam values validated, private fields, re-validating `Deserialize`, key layout, tenant scoping | `TenantId`, `InstallationId`, `ObjectSha`, `ContextId`, `ObjectKey` all private-field newtypes with manual `Deserialize` through the constructor; `RepositoryIdentity` via `try_from = RawRepository`; `ObjectKey` only from `snapshot`/`context` with identity prefix; `parse` refuses empty/`.`/`..`/non-key-safe segments | read; storage/persistence tests cover traversal, empty, dot, whitespace | PASS |
| FIND-TASK-002-1 closure (r1) | `ObjectKey::context(&Identity, &ContextId)` (`storage.rs:78`); `ObjectKey` manual `Deserialize` (`:105-120`); `ContextId` private field + `new` + manual `Deserialize` (`persistence.rs:63-95`); `SnapshotPointer.key`, `ContextRecord.key`, `AlreadyPublished.id` inherit through field types | read; tests `keys_are_revalidated_on_deserialization`, `records_revalidate_their_keys_on_deserialization`, `context_ids_are_validated_on_construction_and_deserialization` | CLOSED for the sites r1 listed |
| FIND-TASK-002-1 sibling sweep (this review's mandate) | one remaining sibling: `GitHubHost::Enterprise { api_base }` is a public enum-variant field, so the validating constructor `GitHubHost::enterprise` is bypassable | scratch crate (below) | **FAIL — TRUST-1** |

Sibling sweep results that are NOT findings (verified, recorded so the orchestrator need not re-check):

- `ObjectDigest` keeps a derived transparent `Deserialize` (`storage.rs:135-137`); `serde_json::from_str::<ObjectDigest>("\"zz\"")` is `Ok` (scratch run). Not a finding: the digest never enters a key, path, URL, or output text; its only use is exact equality against `ObjectDigest::of(bytes)`, so a corrupt or hostile persisted digest can only cause a read to be refused. No trust decision depends on its spelling.
- `SnapshotScope.extraction_version: String` (pub, free text) is hex-encoded before entering the key (`storage.rs:73`); `format_version: u32` is decimal. No admission path for traversal.
- `Repository.full_name` / `default_branch`, `BranchHead.name`, `PullRequest.base_ref`/`head_ref` are provider text returned to the caller; `default_branch` fed back into `branch_head` is percent-encoded (`encode_segment`) and the response name is compared exactly. No unvalidated text reaches a key or a trust decision.
- `is_trusted_redirect` uses `eq_ignore_ascii_case` on the host only — DNS hostnames are case-insensitive; the comparison is otherwise exact (no prefix/suffix match, port must match).
- GHES `api_base` with a port (`https://ghe.example.com:8443/api/v3`) yields `archive_host() == "ghe.example.com:8443"`, so the redirect must carry the same port; consistent.
- `TreeEntryMode::Symlink` entries are kept in `TreeListing::entries`; symlink handling belongs to archive extraction / ingest (later tasks), not to this listing contract.

## Findings

### TRUST-1 — `GitHubHost::Enterprise` admits an unvalidated API base through its public variant field

- **Classification:** material; same defect class as FIND-TASK-002-1 (sibling not swept by r1).
- **Root cause / first owner:** `crates/bohmian-code-review-pipeline/src/identity.rs:294-297` declares `Enterprise { api_base: String }` as a public enum variant with a public field. Every other identity value in the module is a private-field newtype whose only admission path is the validating constructor; `GitHubHost` alone exposes a second admission path (`GitHubHost::Enterprise { api_base: .. }`) that skips `GitHubHost::enterprise` (`identity.rs:308-325`).
- **Defect class:** validated domain value with a public construction path that bypasses its invariant.
- **Sibling locations (one finding):**
  - `crates/bohmian-code-review-pipeline/src/identity.rs:294-297` — public variant field (admission point).
  - `crates/bohmian-code-review-pipeline/src/identity.rs:340` — `archive_host` slices `api_base["https://".len()..]` assuming the invariant; on an unvalidated value it mis-slices (`http://ghe.internal/api/v3` → `"he.internal"`) or panics (`"ab"` → `start byte index 8 is out of bounds`).
  - `crates/bohmian-code-review-pipeline/src/github.rs:1115` and `:1132-1134` — `request` builds `{api_base}{resource}` and `build` attaches `Authorization: Bearer <token>` regardless of scheme, so a bypassed `http://` base sends the installation token in cleartext.
- **Violated obligation:** `identity.rs:3-6` (module invariant: "Construction rejects anything the library would otherwise have to sanitize later"); `AGENTS.md:29` (domain values at trust boundaries); `AGENTS.md:30` / `rust-core.md:7` (no unchecked panic on input; "possible panics are documented or removed" — `archive_host` documents none); spec REQ-014 `spec.md:77` ("Archive redirects and paths prevent credential forwarding") and REQ-004 `spec.md:61` (GHES configured instances validated); r1 remediation AC-R1-3 wording for the same class ("the field is not publicly constructible").
- **Evidence (scratch crate `/tmp/t002r2-trust`, `cargo run`):**
  ```
  api_base=http://ghe.internal/api/v3 archive_host="he.internal"
  sent=[("http://ghe.internal/api/v3/repos/o/r", true)]      # true = Authorization header present
  thread 'main' panicked at .../identity.rs:340:25: start byte index 8 is out of bounds of `ab`
  ```
  The same crate's `GitHubHost::enterprise("http://ghe.example.com")` is `Err(InvalidIdentity::ApiBase)` (existing test `hosts_expose_api_base_and_archive_host`), confirming the constructor and the variant disagree.
- **Observable consequence:** a consumer that builds the variant directly (the type signature invites it; nothing in the rustdoc forbids it) gets a client that sends the installation token over plaintext HTTP, computes a wrong archive trust host (so a redirect to the real GHES host is rejected and a redirect to a host whose name happens to equal the mis-sliced text is accepted), and can panic inside `archive`. The trust guarantee `identity.rs:3-6` advertises is false at the type level for this value, exactly as it was for `ObjectKey`/`ContextId` before r1.
- **Testable correction (smallest):** make the variant's payload non-constructible outside the module — e.g. `Enterprise(EnterpriseBase)` where `pub struct EnterpriseBase(String)` has a private field, or turn `GitHubHost` into a struct with a private `api_base: Option<String>` and keep `Public`/`enterprise` as constructors. Keep `api_base()` / `archive_host()` signatures and the existing tests unchanged. Acceptance: a doc-test or `compile_fail` proving `GitHubHost::Enterprise { api_base: "http://x".into() }` does not compile from outside the crate; `archive_host` can then rely on the invariant without a documented panic.

## Verification limits

- No live GitHub/GHES call; provider behaviour verified against the scripted fake and ureq 3.4.2 source only.
- I did not re-run the full pipeline test lane (orchestrator runs `test:pipeline`/`gate` in parallel); I ran only the scratch probe.
- `TransportError::Failed(ureq_error.to_string())` for the archive hop: ureq's `BadUri`-style errors can echo the request URI, which for a private-repo codeload redirect carries a provider-minted short-lived `?token=` query. I did not reproduce a case where a Location passing `is_trusted_redirect` also fails ureq URI parsing, so this is recorded as an unverified edge, not a finding.
- Provider bounds (retry/rate-limit arithmetic, timeouts) are `domain-rev-provider-bounds`' scope and were only read for their trust consequences.

```citations
[{"id": "TRUST-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 294, "snippet": "    Enterprise {", "symbols": ["GitHubHost", "Enterprise", "api_base"]},
 {"id": "TRUST-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 340, "snippet": "                api_base[\"https://\".len()..].split('/').next().unwrap_or(\"\")", "symbols": ["GitHubHost::archive_host"]},
 {"id": "TRUST-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1133, "snippet": "            headers.push((\"Authorization\", format!(\"Bearer {}\", self.token.expose())));", "symbols": ["GitHubClient::build", "InstallationToken::expose"]},
 {"id": "TRUST-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1115, "snippet": "        let url = format!(\"{}{resource}\", self.host.api_base());", "symbols": ["GitHubClient::request", "GitHubHost::api_base"]}]
```
