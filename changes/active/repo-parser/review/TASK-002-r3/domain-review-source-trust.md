# Domain review: SOURCE TRUST boundary (TASK-002, round 3)

Subject: base `1842eaf`, candidate `dbd6de3` (HEAD). Crate `crates/bohmian-code-review-pipeline`.

## Reviewed boundary

Public API (`GitHubHost`, `InstallationToken`, `GitHubClient`, `HttpTransport`, `ObjectStorage`, `Persistence`) through request building (`GitHubClient::request` / `build`), transport (`UreqTransport`, `send_with_policy`), response validation (`repository`, `branch_head`, `commit`, `tree`, `blob`, `archive`, `pull_request`, `pull_request_files`, `validate_tree_entry`, `validate_pull_request_file`), error surfaces (`GitHubError`, `TransportError`, `Debug`/`Display`), and the durable seam (`ObjectKey`, `ContextId`, `ContextRecord`, `SnapshotPointer`, `SnapshotScope` deserialization).

## Authority and source coverage

- Spec: `changes/active/repo-parser/spec.md` REQ-004 (line 61), REQ-014 (line 77), REQ-015/016 (lines 78-79), INV-003 (line 87).
- Rules: `AGENTS.md` line 29 (domain values at trust boundaries), line 30 (no unchecked unwrap on input).
- Source read in full: `identity.rs`, `github.rs` (lines 1-1420 production code plus tests 1500-1740), `storage.rs`, `persistence.rs`, `execution.rs`, `lib.rs`, `Cargo.toml`; `Cargo.lock` ureq pin (3.4.2) and ureq 3.4.2 `src/error.rs`, `src/util.rs`, `src/run.rs` for error-text URL echo.
- Nothing under `review/` was read.

## Verification limits

- No live GitHub call. Transport behaviour verified against ureq 3.4.2 source, not a network.
- Provider-bounds (retry/deadline/decompression limits) are owned by the sibling reviewer and were not assessed here.
- 15-minute budget; `blob`/`archive` decompression interaction with ureq's automatic `Content-Encoding: gzip` handling was noted but not probed (bounds, not trust).

## Re-verification of prior-round items

| Item | Result |
|---|---|
| (a) single admission path + re-validate on Deserialize | `TenantId`, `InstallationId`, `RepositoryIdentity` (`try_from = RawRepository`), `ObjectSha`, `ContextId`, `ObjectKey` all re-validate on `Deserialize` and have private fields. `InstallationToken` has no `Deserialize`. **`GitHubHost::Enterprise` FAILS: field is `pub` and writable through `&mut` pattern binding — see TRUST-1.** |
| (a) struct-expression construction fails externally | Verified in scratch crate: `E0639 cannot create non-exhaustive variant using struct expression`; pattern without `..` fails `E0638`. Crate doctest `compile_fail` passes. |
| (b) `archive_host` no panic | `unwrap_or("")` on both `strip_prefix` and `split().next()`; no panic path. (Returns `""` for a forged non-https base, see TRUST-1 consequence.) |
| (c) bearer only over https; not forwarded on hop; archive host only; one hop | `request()` prefixes `host.api_base()` (constant `https://api.github.com` or validated `https://…`); `archive()` calls `send_with_policy(..., authenticated=false)` for the hop; `is_trusted_redirect` requires literal `https://` prefix and host equality up to first `/` (rejects `@`, `:port`, `\`, `?`, `#` variants by inequality); hop result must be 200, never re-followed. `UreqTransport` sets `max_redirects(0)`, `http_status_as_error(false)`. Test at github.rs:1659 exercises trusted, untrusted, and GHES cases. Holds — except when the host itself is forged (TRUST-1). |
| (d) provider identity comparisons; blob SHA-1; RelativePath | `repository`: id equality + `full_name` case-insensitive; `branch_head`: name equality; `commit`/`tree`: sha equality; `pull_request`: number + base repo id; `blob`: `git_blob_sha` recomputed over bytes (`blob <len>\0`); tree/PR-file paths and `previous_filename` through `RelativePath::new`, shas through `ObjectSha::new`. Holds. |
| (e) token never echoed | `InstallationToken` custom `Debug`, no `Display`; `HttpRequest` and `GitHubClient` derive no `Debug`; `resource` strings never contain the token (header-only). ureq 3.4.2 `Error::BadUri` echoes the URI only for "missing scheme/host", which cannot occur for `https://`-prefixed URLs the client builds; `Http(http::Error)` for invalid URI characters prints no URI; `RequireHttpsOnly` is not configured. `UntrustedRedirect{location}` echoes only an untrusted `Location` (never the installation token). Holds. |
| (f) persisted keys/records re-validate | `ObjectKey::parse` per-segment via `validate_key_segment` (no length cap); `ContextRecord`/`SnapshotPointer`/`SnapshotScope` re-validate through field types; tests at storage.rs:`keys_are_revalidated_on_deserialization`, persistence.rs:`records_revalidate_their_keys_on_deserialization`. `ObjectDigest` deserializes any string, but a wrong digest fails closed at read-back comparison; not material. Holds. |

## Findings

### TRUST-1 — `GitHubHost::Enterprise.api_base` is publicly mutable, so a client can be handed an unvalidated (non-https) API base and send the bearer token to it

- **Defect class:** validated domain value with a public writable field (admission path not closed; `#[non_exhaustive]` only blocks construction, not mutation).
- **Root cause:** `crates/bohmian-code-review-pipeline/src/identity.rs:318-322` — the variant field `api_base: String` is `pub` (enum variant fields are always public). `#[non_exhaustive]` prevents `GitHubHost::Enterprise { .. }` struct expressions from another crate, but any crate holding a `&mut GitHubHost` may bind `GitHubHost::Enterprise { api_base, .. }` and assign a new `String`.
- **Sibling locations:** consumers of the invariant — `identity.rs:353-358` (`api_base()` returns the field verbatim), `identity.rs:361-369` (`archive_host()` returns `""` for a non-`https://` base), `github.rs:1121` (`request()` builds `"{api_base}{resource}"`), `github.rs:1139` (`build()` attaches `Authorization: Bearer …` to that URL when `authenticated`), `github.rs:741-746` (`is_trusted_redirect` would accept `https:///…` when `archive_host` is `""`).
- **Violated obligation:** `identity.rs:307-308` documents "`GitHubHost::enterprise` is the only way to obtain an `Enterprise` host, so every API base a client sees is validated" — false. Spec REQ-014 (`spec.md:77`): "Credentials … never appear in logs …; archive redirects and paths prevent credential forwarding" and REQ-004 (`spec.md:61`) "configured GHES instances"; `AGENTS.md:29` requires domain values at trust boundaries — a domain value whose invariant can be broken after construction is not one.
- **Evidence (scratch crate `/tmp/ghost_probe`, path dependency on the candidate crate):**
  ```
  $ cargo build   # GitHubHost::Enterprise { api_base: "http://x".to_owned() }
  error[E0639]: cannot create non-exhaustive variant using struct expression
  $ cargo build   # if let GitHubHost::Enterprise { api_base } = h
  error[E0638]: `..` required with variant marked as non-exhaustive
  $ cargo run     # if let GitHubHost::Enterprise { api_base, .. } = &mut h { *api_base = "http://evil".to_owned(); }
  http://evil
  ```
  The third program compiles and prints the forged base from `GitHubHost::api_base()`.
- **Observable consequence:** `GitHubClient::new(forged_host, token, limits).repository(..)` sends `GET http://evil/repos/o/r` with `Authorization: Bearer <installation token>` in cleartext to an arbitrary host. The `https`-only, no-userinfo, no-query guarantees of `GitHubHost::enterprise` are bypassed, and `archive_host()` degrades to `""`, which `is_trusted_redirect` matches against a `https:///…` `Location`. Requires the consumer to hold and mutate the host, so exploitation needs a bug or hostile code on the consumer side; the library's own stated invariant is nonetheless not enforced by the type system, which is what the invariant exists for.
- **Testable correction (smallest):** make the payload a newtype with a private field so external code cannot produce a `String`-typed replacement:
  ```rust
  /// Validated `https://host[/path]` base; only `GitHubHost::enterprise` builds one.
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct ApiBase(String);
  pub enum GitHubHost { Public, Enterprise(ApiBase) }   // or Enterprise { api_base: ApiBase }
  ```
  Keep `api_base()`/`archive_host()` signatures. Drop `#[non_exhaustive]` (no longer needed) or keep it. Add a second `compile_fail` doctest:
  ```rust
  /// ```compile_fail
  /// let mut h = bohmian_code_review_pipeline::GitHubHost::enterprise("https://x.example").unwrap();
  /// if let bohmian_code_review_pipeline::GitHubHost::Enterprise(base) = &mut h { *base = "http://evil".to_owned().into(); }
  /// ```
  ```
  (any `From<String>`/`pub` constructor for `ApiBase` must be absent so the test remains a compile failure). Existing test `hosts_expose_api_base_and_archive_host` continues to cover accessor values.

No other material findings.

## Commands run

| Command | Result |
|---|---|
| `git rev-parse HEAD` | `dbd6de34…` |
| `cargo test -p bohmian-code-review-pipeline --all-features` | 16 unit tests pass; 1 doctest (`compile_fail`) passes |
| scratch crate `/tmp/ghost_probe` — struct expression | `E0639` (construction blocked) |
| scratch crate — exhaustive pattern without `..` | `E0638` |
| scratch crate — `&mut` binding assignment | compiles, prints `http://evil` (TRUST-1) |
| `grep -n "BadUri(\|RequireHttpsOnly(" ~/.cargo/registry/src/*/ureq-3.4.2/src -r` | `BadUri` only for missing scheme/host (`util.rs:330,337`), `RequireHttpsOnly` only under https-only config (`run.rs:150`) — neither reachable with the client's `https://` URLs |
| `grep -n "derive.*Debug" github.rs` | no `Debug` on `HttpRequest`, `HttpResponse`, `GitHubClient` |

## Verdict

**FAIL** — TRUST-1: `GitHubHost::Enterprise` admission path is not closed; the documented invariant that every API base a client sees is validated is bypassable from an external crate, which can route the bearer token to a plaintext/arbitrary host. Everything else re-verified for (a)-(f) holds at the candidate.

```citations
[
  {"id":"TRUST-1","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":321,"snippet":"api_base: String,","symbols":["GitHubHost","Enterprise","api_base"]},
  {"id":"TRUST-1","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":307,"snippet":"/// [`GitHubHost::enterprise`] is the only way to obtain an `Enterprise`","symbols":["enterprise"]},
  {"id":"TRUST-1","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":318,"snippet":"#[non_exhaustive]","symbols":["non_exhaustive"]},
  {"id":"TRUST-1","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":367,"snippet":".unwrap_or(\"\"),","symbols":["archive_host"]},
  {"id":"TRUST-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1121,"snippet":"let url = format!(\"{}{resource}\", self.host.api_base());","symbols":["request","api_base"]},
  {"id":"TRUST-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1139,"snippet":"headers.push((\"Authorization\", format!(\"Bearer {}\", self.token.expose())));","symbols":["build","expose"]}
]
```
