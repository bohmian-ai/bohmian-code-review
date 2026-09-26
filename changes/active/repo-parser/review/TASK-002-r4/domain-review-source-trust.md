# TASK-002 r4 domain review: source trust / credential boundary

- Reviewer: `domain-rev` (source trust)
- Subject: base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`, candidate `801d9572cf6b0364b5ab33e858388d86f54cb91e` (verified `git rev-parse HEAD`)
- Overall: **PASS**

## Reviewed boundary

`crates/bohmian-code-review-pipeline/src/{identity.rs,github.rs,storage.rs,persistence.rs,execution.rs,lib.rs}` traced from `GitHubHost::enterprise` -> `api_base()`/`archive_host()` -> `GitHubClient::request`/`build` (Authorization attach) -> `send_with_policy` -> `UreqTransport::get`, plus `archive` redirect trust, `blob` SHA-1 verification, `tree`/`pull_request_files` path validation, and the `ObjectKey`/`ContextId`/`TenantId`/`ObjectSha`/`RepositoryIdentity` durable-seam re-validation.

## Authority and source coverage

- `changes/active/repo-parser/spec.md` REQ-004 (verify repository/commit/tree/path/blob identity before source enters the library), REQ-014 (credentials never in logs/persistence; redirects and paths prevent credential forwarding and traversal), INV-001, INV-003, INV-006.
- `AGENTS.md` ownership boundaries: pipeline crate is the only Git-provider IO owner; never retains or persists source.
- Prior obligation consulted: `review/TASK-002-r3/...` FIND-TASK-002-4 (enum payload externally writable via `&mut`).
- Cumulative diff `/tmp/task002-r4-code.diff`; fix diff `/tmp/task002-r4-r3fix.diff` (192 lines: `ApiBase` newtype, second compile_fail doctest, `read_bounded` saturating_add + test, doc-test lane in `mise.toml`, rustdoc/import tidy).

## Evidence by item

### 1. `GitHubHost::Enterprise` payload seal

`identity.rs:326-341`: variant is `#[non_exhaustive]`; payload is `pub struct ApiBase(String)` with a private field, derives only `Debug, Clone, PartialEq, Eq` (no `Default`, `Serialize`, `Deserialize`, `From`). `GitHubHost` itself derives no serde traits. No `pub fn` returns `&mut` into the host. `lib.rs` re-exports `GitHubHost` but not `ApiBase`; `ApiBase` remains reachable as `identity::ApiBase`, which is harmless because it is unconstructible.

Reproduced from a scratch crate outside the repo (`/tmp/trust-scratch`, path dependency, `cargo check --offline`):

| Probe | rustc result |
|---|---|
| `GitHubHost::Enterprise { api_base: "http://x".to_owned() }` | E0639 cannot create non-exhaustive variant (+E0308) |
| `if let Enterprise { api_base, .. } = &mut h { *api_base = "http://evil".to_owned(); }` | E0308 mismatched types (`ApiBase` vs `String`) |
| `identity::ApiBase("http://x".to_owned())` | E0603 tuple struct constructor is private |
| `let _: ApiBase = Default::default()` | E0277 `Default` not satisfied |
| `serde_json::from_str::<GitHubHost>("{}")` | E0277 `Deserialize` not satisfied |

Both checked-in `compile_fail` doctests (`identity.rs:310`, `:317`) pass in `cargo test --doc -p bohmian-code-review-pipeline --all-features` and fail for the intended reasons (E0639 and E0308 respectively, per the scratch reproduction). The only remaining `&mut` write is swapping in another `ApiBase` obtained from another `GitHubHost::enterprise(..)`, which is by construction validated.

`GitHubHost::enterprise` (`identity.rs:351-370`): requires `https://` prefix, nonempty host, no `@` in host, no `?`/`#`/whitespace anywhere, no trailing slash. Unit test `hosts_expose_api_base_and_archive_host` covers `http://`, userinfo, trailing slash, query, empty host -> `Err(InvalidIdentity::ApiBase)`. `api_base()` and `archive_host()` read only `ApiBase.0`, so `request()` (`github.rs:1121`) and `is_trusted_redirect` (`github.rs:741`) can only see `https://` bases. `is_trusted_redirect` also independently requires `https://` on the Location, so even the Public host cannot be redirected to cleartext.

### 2. Credential handling

- Single `expose()` call site in production code: `github.rs:1139`, inside `build()`, gated by `authenticated`. Archive hop passes `authenticated = false` (`github.rs:971-976`); test at `github.rs:1695` asserts the hop carries no Authorization.
- `InstallationToken` has a redacting `Debug` and no `Display`; `HttpRequest` has no `Debug`; `GitHubClient` has no `Debug`. Errors: `TransportError::Failed(String)` wraps `ureq::Error::to_string()`, which never contains request headers; the request URL contains no credential. `MalformedResponse.detail` carries only serde category/position, never body text.
- Redirect: one hop only (`expect_status(&hop, 200, ..)` turns a second 3xx into `UnexpectedStatus`), trusted only when `Location` is `https://<archive_host>/...` with an exact, case-insensitive host match (`github.rs:741-746`); `user@host`, port, or suffix-host variants fail the equality.
- ureq 3.4.2 config: `max_redirects(0)`; verified in `ureq-3.4.2/src/config.rs:257` that `max_redirects_do_error()` is `max_redirects > 0 && ...`, so ureq returns the 3xx response instead of erroring, matching the client's assumption. ureq's own debug logging redacts `authorization`/sensitive headers (`util.rs:211-251`).

### 3. Identity / blob verification

- `git_blob_sha` (`github.rs:701-706`): SHA-1 over `blob <len>\0<bytes>`; `blob()` compares to the requested `ObjectSha` and returns `IdentityMismatch` otherwise; bounded by `read_bounded(max_blob_bytes)` before hashing. Known-answer test for `hello\n`.
- `repository()` checks numeric id and `full_name`; `branch_head()` checks branch name; `commit()`/`tree()` check returned sha equals requested; `pull_request()` checks number and base repo id. Wire shas deserialize through `ObjectSha` (validated `Deserialize`).
- Paths: `validate_tree_entry` / `validate_pull_request_file` build `RelativePath` (core rejects empty, absolute, `\\`, NUL, `.`/`..`/empty segments, over-long) and `ObjectSha`; rejects go to `TreeListing::rejected` / `PullRequestFilesPage::rejected` with reason, never silently dropped.

### 4. Durable seam validation

`TenantId`, `InstallationId`, `ObjectSha`, `ContextId` implement manual `Deserialize` through their constructors; `RepositoryIdentity` uses `try_from = RawRepository`; `ObjectKey::deserialize` re-validates every `/` segment with `validate_key_segment` (refuses empty, `.`, `..`, non `[A-Za-z0-9._-]`). `ObjectKey` is only minted from `Identity` + validated parts. No API in the pipeline crate persists source: `Archive.reader` and `blob()` bytes are returned to the caller only; `ContextRecord`/`SnapshotPointer` hold ids, keys, digests, timestamps.

### 5. `read_bounded` change

`github.rs:1273`: `take((limit as u64).saturating_add(1))` then `bytes.len() > limit` -> `ResponseTooLarge`. No over-read (still at most `limit+1` bytes consumed) and no truncation-as-success (any body longer than `limit` errors; at `usize::MAX` the take bound is `u64::MAX`, unreachable in practice). Test `read_bounded_accepts_the_maximum_limit` covers the overflow edge.

## Verification run

- `cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features`: 17 passed.
- `cargo test --locked -p bohmian-code-review-pipeline --doc --all-features`: 2 compile_fail passed.
- Scratch-crate seal probes above.

## Verification limits

- No live GitHub/GHES call; transport behavior inferred from ureq 3.4.2 source and the scripted `FakeTransport` tests.
- `fmt:check`/`lints` lanes not re-run here (outside the trust boundary; other reviewers own them).
- Did not audit the `sha1`/`sha2`/`ureq` dependency trees for CVEs beyond the pinned lock versions.

## Proposed findings

None.

## Non-blocking observations

- O-1: `GitHubError::UntrustedRedirect { location }` echoes the raw untrusted `Location` header into error text (`github.rs:341-345`); it never carries the installation token, but a consumer logging it would record whatever the provider put in that URL. ureq redacts `Location` in its own logs for this reason.
- O-2: `ApiBase` is a public type in the public `GitHubHost::Enterprise` variant but is not re-exported from `lib.rs`; only reachable as `identity::ApiBase`. Cosmetic.
- O-3: `ObjectDigest` derives plain `Deserialize` without hex re-validation; a malformed persisted digest fails closed at integrity check time rather than at load, so no trust impact.

## Overall

PASS

```citations
[]
```
