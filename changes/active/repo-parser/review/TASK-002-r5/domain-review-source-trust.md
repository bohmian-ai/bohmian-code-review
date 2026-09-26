# TASK-002 r5 — Domain review: SOURCE TRUST

Candidate `6899a43` (base `1842eaf`), read only from `/tmp/cr-r5/wt`. Owning crate `crates/bohmian-code-review-pipeline`.

**Overall: FAIL** (DOM-ST-1, DOM-ST-2, DOM-ST-3)

## Reviewed boundary

- Host configuration: `GitHubHost::{Public, enterprise}`, sealed `ApiBase` payload, `api_base()`, `archive_host()`.
- Request construction: `GitHubClient::request` (`api_base + resource`), `encode_segment` for branch names, `full_name()` from key-safe owner/name.
- Credential attachment: `InstallationToken` (ASCII printable, redacted `Debug`), `GitHubClient::build` adds `Authorization: Bearer` only when `authenticated`; only `request()` (API base) passes `true`; the archive hop passes `false`.
- Archive redirect trust: `is_trusted_redirect`, `archive()` single-hop follow, `UreqTransport` with `max_redirects(0)`.
- Identity verification: repository (id + full_name), branch (name), commit (sha), tree (sha + per-entry `RelativePath`/mode/`ObjectSha`), PR (number + base repo id), PR files (per-file path/status/sha), blob (Git SHA-1 over `blob <len>\0<bytes>` after bounded read).
- Durable seam: `TenantId`, `InstallationId`, `RepositoryIdentity`, `ObjectSha`, `ContextId`, `ObjectKey` construction and `Deserialize`; `RelativePath` (core, unchanged) `Deserialize` through `RelativePath::new`.
- Error text: `GitHubError`, `TransportError::Failed(ureq::Error::to_string())`, `parse_json` detail, `RejectedPath`.

## Authority and source coverage

Spec REQ-004, REQ-014, REQ-015, REQ-016, context-ID definition; TASK-002; R1–R3 remediation tasks. Read in full: `github.rs` (production part, tests skimmed), `identity.rs`, `storage.rs`, `persistence.rs`, `lib.rs`, `core/relative_path.rs` validation. ureq 3.4.2 (Cargo.lock pin) source: `config.rs:257-258,325-330` — `max_redirects(0)` returns the 3xx response instead of following or erroring; `error.rs:234` — only `BadUri` echoes URL text, and it cannot fire for an `https://host/...` URL that already passed the host check. Header values never appear in ureq error Display.

## Verified sound (no finding)

- `is_trusted_redirect` requires `https://` and exact (case-insensitive) authority equality; `@`-userinfo, port, `?`, `#`, backslash, and relative forms all fail equality. Not bypassable.
- Bearer is never attached to a redirect hop; ureq cannot follow redirects itself.
- `ApiBase` cannot be constructed or overwritten outside the crate (two `compile_fail` doctests); `enterprise()` rejects non-https, userinfo, query, fragment, whitespace, trailing slash.
- Branch names keep `/` and `.` in `encode_segment`; a `..` segment could only reroute within the same API host, and the `wire.name != branch` check rejects any response that is not the named branch. No credential or identity escape.
- Blob verification hashes the exact bounded bytes and compares to the requested lowercase `ObjectSha`.
- `TenantId`, `InstallationId`, `RepositoryIdentity`, `ObjectSha`, `ContextId`, `RelativePath` re-validate on deserialization through their constructors.

## Findings

### DOM-ST-1 — Untrusted redirect error echoes the full Location, including its credential query

- **Classification:** INCORRECT (VIOLATION of REQ-014).
- **Root cause / defect class:** the error variant's owner shape carries the raw provider `Location` string (`UntrustedRedirect { location: String }`) and Displays it verbatim; raw untrusted URL where a credential-free value belongs. GitHub archive redirects for private repositories carry a short-lived access token in the query (`?token=...`), which is exactly the URL that reaches this branch when the host check rejects it.
- **Locations:** `crates/bohmian-code-review-pipeline/src/github.rs:341` (variant + Display), `github.rs:969` (construction from raw header).
- **Obligation:** REQ-014 "Credentials ... never appear in logs"; `GitHubError` is `Display`/`Debug` and consumers log it.
- **Evidence:** scratch probe (`/tmp/cr-r5/scratch-source-trust`, real `GitHubClient::archive` over an injected transport, GHES base `https://ghe.example.com/api/v3`, 302 to `https://codeload.ghe.example.com/o/r/legacy.tar.gz/abc?token=SECRETTOKEN123`) printed `untrusted archive redirect to https://codeload.ghe.example.com/o/r/legacy.tar.gz/abc?token=SECRETTOKEN123`.
- **Consequence:** a rejected archive redirect puts a usable archive credential into caller logs/metrics.
- **Correction:** store only the rejected authority (scheme + host, or `<unparseable>`) in the variant, never path or query. Test: redirect to `https://evil.example/x?token=SECRET` yields `UntrustedRedirect` whose `Display` and `Debug` do not contain `SECRET` or the path.

### DOM-ST-2 — `ObjectKey` deserialization accepts keys no constructor can produce, contradicting its own invariant

- **Classification:** INCORRECT.
- **Root cause / defect class:** `ObjectKey::parse` (added by R1/R2) validates only the per-segment alphabet, not the constructor grammar; the owner type therefore admits unscoped and foreign-scope keys through serde while documenting that it cannot. Weaker guard in place of an unrepresentable shape.
- **Locations:** `crates/bohmian-code-review-pipeline/src/storage.rs:94` (`parse`), `storage.rs:108` (Deserialize doc claim "serde cannot mint a key the constructors would never produce"), `storage.rs:44` (type doc claim of scope). Reached through `SnapshotPointer.key` and `ContextRecord.key`, which `Persistence::snapshot_pointer`/`context` hand back to the library.
- **Obligation:** REQ-014/REQ-015 (tenant/installation/repository identities scope every key and read); AGENTS.md validated domain values at trust boundaries; the type's own documented invariant.
- **Evidence:** scratch probe: `serde_json::from_str::<ObjectKey>` returned `Ok` for `"a"`, `"x/y/z"`, and `"tenants/OTHER/installations/9/repositories/3/contexts/x"`.
- **Consequence:** a persisted row can reload a key that is not tenant/installation/repository-scoped, or that names another identity's scope; the type gives downstream readers no guarantee that a reloaded key has the constructor shape.
- **Correction:** make `ObjectKey::parse` accept exactly the two constructor shapes — `tenants/{TenantId}/installations/{nonzero u64}/repositories/{nonzero u64}/` followed by `snapshots/{lowercase hex}/{u32}/{ObjectSha}` or `contexts/{ContextId}` — validating each segment through its owning type; the segment-alphabet-only rule becomes superseded and is deleted. Test: `"a"`, `"x/y/z"`, `"tenants/a/installations/0/repositories/7/contexts/c"` are refused; constructor-built snapshot/context keys (including 128-byte tenant and real `EXTRACTION_VERSION`) still round-trip. (Binding a reloaded key to the *caller's* identity remains the read facade's check in TASK-005 by comparing against `ObjectKey::context(identity, &record.id)`; not claimed here.)

### DOM-ST-3 — GHES archive trust set omits the subdomain-isolation codeload host

- **Classification:** MISSING (REQ-004 GHES support, REQ-005 archive ingest).
- **Root cause / defect class:** `GitHubHost::archive_host` returns a single host equal to the API host for `Enterprise`; the trust rule is modeled as one string, so GHES with subdomain isolation enabled (GitHub's recommended configuration, which serves archives from `codeload.<HOSTNAME>` instead of `<HOSTNAME>/_codeload/`) is unrepresentable as trusted.
- **Locations:** `crates/bohmian-code-review-pipeline/src/identity.rs:378` (`archive_host`), `crates/bohmian-code-review-pipeline/src/github.rs:741` (`is_trusted_redirect`, single-host comparison).
- **Obligation:** REQ-004 "supports github.com and configured GHES instances"; AC-006 GHES endpoint variation contract.
- **Evidence:** the probe in DOM-ST-1 is exactly this case: a GHES 302 to `codeload.ghe.example.com` is refused. Candidate test coverage asserts only the same-host form.
- **Consequence:** every archive request on a subdomain-isolated GHES fails with `UntrustedRedirect` (terminal), forcing cold builds onto the 200-blob fallback bound and incomplete snapshots, and (with DOM-ST-1) logging the archive token.
- **Correction:** for `Enterprise`, trust exactly `{<api host>, codeload.<api host>}` (credentials still never forwarded); for `Public`, `codeload.github.com` only. Test: GHES redirects to `https://codeload.ghe.example.com/...` and `https://ghe.example.com/_codeload/...` are followed without `Authorization`; `https://codeload.evil.example/...` and `https://x.codeload.ghe.example.com/...` are refused.

## Verification limits

- No live GitHub or GHES instance was contacted. The private-repo `?token=` codeload query and the GHES subdomain-isolation `codeload.HOSTNAME` mapping are taken from GitHub/GHES public documentation and observed provider behavior, not from a live capture in this session; the library-side behavior in DOM-ST-1..3 is reproduced against the candidate.
- Scratch probe: `/tmp/cr-r5/scratch-source-trust` (path dependency on `/tmp/cr-r5/wt`, `cargo run --offline`, target `/tmp/cr-r5/target-source-trust`). The worktree was not modified.
- Did not run `test:pipeline`/`gate`; other reviewers own lane evidence.

```citations
[
 {"id":"DOM-ST-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":341,"snippet":"    #[error(\"untrusted archive redirect to {location}\")]","symbols":["GitHubError::UntrustedRedirect"]},
 {"id":"DOM-ST-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":969,"snippet":"                    return Err(GitHubError::UntrustedRedirect { location });","symbols":["GitHubClient::archive"]},
 {"id":"DOM-ST-2","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":94,"snippet":"    fn parse(raw: &str) -> Result<ObjectKey, InvalidIdentity> {","symbols":["ObjectKey::parse"]},
 {"id":"DOM-ST-2","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":108,"snippet":"    /// Deserializes through segment re-validation so serde cannot mint a key","symbols":["ObjectKey::deserialize"]},
 {"id":"DOM-ST-2","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":44,"snippet":"/// `/`-separated segment is key-safe, so a persisted key is re-validated on","symbols":["ObjectKey"]},
 {"id":"DOM-ST-3","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":378,"snippet":"    pub fn archive_host(&self) -> &str {","symbols":["GitHubHost::archive_host"]},
 {"id":"DOM-ST-3","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":741,"snippet":"fn is_trusted_redirect(location: &str, archive_host: &str) -> bool {","symbols":["is_trusted_redirect"]}
]
```
