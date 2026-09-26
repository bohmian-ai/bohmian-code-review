# TASK-002 r6 domain review: source trust

- Candidate: `7983907a6aa0b01a942fb964362da73bc9e7dba6` (read-only at `/tmp/cr-r6/wt`), base `1842eaf`.
- Reviewer role: domain-rev, SOURCE TRUST.
- Overall: **PASS**.

## Reviewed boundary

GitHub/GHES source-access trust in `crates/bohmian-code-review-pipeline/src/identity.rs` and `src/github.rs`. I traced it end to end from the public constructors:

- **Host.** `GitHubHost::public` and `GitHubHost::enterprise` build a struct whose fields are private: `api_base` and `archive_hosts`. `trusts_archive_host` checks a host against that list.
- **Client.** `GitHubClient::new` and `GitHubClient::with_transport` lead to `request`, then `build`, where the bearer is attached only when `authenticated`, then `send_with_policy`, then `UreqTransport::get`.
- **Archive.** `archive` sends an authenticated API hop. A 301-303/307/308 response then goes to `is_trusted_redirect`, which leads either to one unauthenticated `send_with_policy(..., false)` hop or to `UntrustedRedirect`. The hop result then goes to `expect_status(200)`. A second 3xx is never followed, so the response is `UnexpectedStatus` with the API resource path only.
- **Identity and blob verification.** `repository` checks the id and the case-insensitive `full_name`. `branch_head` checks the name. `commit` and `tree` check the SHA. `blob` checks the SHA-1 `git_blob_sha` against the requested `ObjectSha`. `pull_request` checks the number and the base repository id.
- **Error text.** I checked the Display and Debug output of every `GitHubError`, `TransportError` and `InvalidIdentity` variant, plus `InstallationToken` Debug. Neither file has any `tracing` or `log` call; `rg -n "tracing|log::"` over `src/` returns nothing.

## Authority and source coverage

- **Spec clauses.** `spec.md`:
  - REQ-004 (`:61`)
  - REQ-014 (`:77`: credentials are never logged, and archive redirects prevent credential forwarding)
  - INV-003 (`:87`)
  - AC-006 (`:120`: a GHES endpoint variation has a tested local contract)
- **Remediation.** `TASK-002-R4` sections FIND-10 and FIND-11, and AC-R4-5 through AC-R4-7.
- **Repository rules.** `AGENTS.md`.
- **Source read in full:**
  - `identity.rs:1-489`
  - `github.rs:40-180`, `:270-360`, `:540-620`, `:690-1425`, and the tests `:1690-1800` and `:1978-2013`
  - ureq 3.4.2 `error.rs` (the Display of every variant), `config.rs` (`max_redirects_do_error`), `run.rs`, and `util.rs` (where `BadUri` is constructed)

## Verification

**Scratch copy.** `/tmp/cr-r6/scratch-st` ran `cargo test -p bohmian-code-review-pipeline --all-features --lib scratch_source_trust`, and both tests passed.

**Test 1: `lookalike_and_authority_tricks_are_rejected`.** For GHES `https://ghe.example.com/api/v3`, each of these redirect targets was rejected:
- `https://codeload.ghe.example.com@evil.com/x`
- `https://ghe.example.com@evil.com/x`
- `https://evil.com/?h=https://ghe.example.com/x`
- `https://evilghe.example.com/x`
- `https://codeload.ghe.example.com.evil.com/x`
- `https://ghe.example.com.:443/x`
- `https://ghe.example.com\@evil.com/x`
- `https://ghe.example.com#@evil.com/x`
- `https://ghe.example.com?@evil.com/x`
- the scheme-relative `//ghe.example.com/x`
- the relative `/_codeload/x`
- an empty location

The same test showed:
- **Case.** An uppercase-host redirect is accepted, and an uppercase-configured GHES host trusts the lowercase `codeload.<host>`.
- **Ports.** A GHES host with a port trusts only that same port.

The check is exact equality on the whole authority, which is everything between `https://` and the first `/`. Userinfo, ports, query strings, fragments, backslashes and trailing dots therefore all fail closed.

**Test 2: `hop_transport_error_does_not_echo_signed_url`.** A `UreqTransport` failure on a URL carrying `token=SECRET-SIGNED` did not print the token in `TransportError`'s Display or Debug, or in the Display of `GitHubError::Transport`. The failures tried were an unresolvable host and an invalid URI character.

**ureq audit.** Only three ureq variants embed the URI:
- `BadUri` fires only when the scheme or host is missing, and `is_trusted_redirect` guarantees both are present.
- `RequireHttpsOnly` needs `https_only`, which is not set.
- `ConnectProxyFailed` is covered under limits.

`max_redirects(0)` returns the 3xx response and never raises `TooManyRedirects` (`config.rs:257-258`). The request-level `.config().timeout_global(..)` override keeps the agent's redirect setting.

**Lanes.** Taken from `/tmp/cr-r6/lanes.txt`, not re-run: `test:pipeline` and the `--doc` step are green.

**R4 acceptance, by criterion:**
- **AC-R4-5 is met.** `UntrustedRedirect` is a unit variant with the message `"untrusted archive redirect"`, and `github.rs:1732-1743` asserts that the message contains neither `secret` nor the URL.
- **AC-R4-6 is met:**
  - `public()` trusts only `codeload.github.com`.
  - GHES trusts `<host>` and `codeload.<host>`, and rejects `codeload.<host>.evil`, `evil.example.com` and `codeload.github.com`.
  - A GHES codeload 302 is followed without `Authorization` (`github.rs:1771-1781`).
  - `api_base()` is unchanged.
- **AC-R4-7 is met in substance.** The `ApiBase` newtype, `#[non_exhaustive]`, both `compile_fail` doctests and `unwrap_or("")` are gone, and the fields are private. The literal `rg` still matches `InvalidIdentity::ApiBase` (`identity.rs:46`, `:318`, `:324`, `:333`, `:475`). That error variant is the one R4 told the implementer to keep ("keeps its current validation and error"), so the hits reflect how the criterion was worded, not a defect.

## Verification limits

- I made no live GitHub or GHES call. AC-006's credentialed provider contract is outside TASK-002.
- ureq `Protocol` and `Io` error text was audited by reading the source, not exercised against every failure mode.
- Proxy-sourced errors (`ConnectProxyFailed`) apply only when a consumer configures a proxy. They are not reachable through the redirect URL.
- The tarball's contents are not checked against the tree or blob identity inside `archive`. That check belongs to ingest (TASK-003), which is out of scope.

## Notes (not findings)

- **Trust is per host, not per path.** Trust is keyed on the host alone, so any path on `<host>` or `codeload.<host>` is accepted, not just `/_codeload/`. R4 chose this explicitly ("trusts `<host>` (which covers `/_codeload/`)"). The hop carries no credentials and is never followed further, so it does not violate REQ-014.
- **Provider text in an error, outside this boundary.** `github.rs:1055` formats the provider's `state` string into `MalformedResponse.detail`. The field doc at `:324` says "never body text". That value holds neither a credential nor source text, so it is outside the source-trust boundary. It goes to the provider-bounds and repository reviewers as a doc/code consistency item.

## Findings

None.

```citations
[]
```
