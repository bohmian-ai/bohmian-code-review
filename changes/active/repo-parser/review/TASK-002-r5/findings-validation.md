# TASK-002 r5 — Wave 2 findings validation (ponytail-rev)

> The harness blocked ponytail-rev from writing this file. The orchestrator copied it verbatim from the agent's hand-back and changed nothing of substance.

Candidate `6899a43` (base `1842eaf`). All source was read from `/tmp/cr-r5/wt` or with `git show 6899a43:`.

The reproductions use a scratch binary crate at `/tmp/cr-r5/scratch-ponytail/repro`. It depends on the candidate pipeline crate by path, uses only the public API with a scripted `HttpTransport`, and builds with `CARGO_TARGET_DIR=/tmp/cr-r5/target-ponytail`. Its output:

```
C1a secondary 403 -> Forbidden disposition=Terminal
C1b bare 429 -> rate limited; retry after 60s disposition=Retry { after: Some(60s) }
C2 "x" -> Ok("x")
C2 "a/b/c" -> Ok("a/b/c")
C2 "tenants/a/installations/0/repositories/7/contexts/c" -> Ok(...)
C2 "tenants/a/installations/1/repositories/7/snapshots/zz/1/nothex" -> Ok(...)
C2 "foo/bar" -> Ok("foo/bar")
C3 -> untrusted archive redirect to https://evil.example/archive?token=SECRET-SIGNED
C4 -> Some(Terminal)
```

The scenarios behind those lines:

- **C1a:** a 403 with `x-ratelimit-remaining: 4990`, no `retry-after`, and GitHub's secondary-limit body ("You have exceeded a secondary rate limit ...").
- **C1b:** a 429 with no headers under `RequestLimits::default()`. The 60s is `max_retry_wait`, not a value the provider sent.
- **C4:** a GHES host `https://ghe.example.com/api/v3` answering with a 302 to `https://codeload.ghe.example.com/...`.

## Merge

- **C4, C5 and C6 → FIND-TASK-002-11.** All three come from one owner, the `GitHubHost` shape at `identity.rs:305-388`. C5 and C6 are DRIFT that the correction deletes.
- **C1 → FIND-TASK-002-9.** Both halves, the header-less 403 and the header-less 429, have one root cause: the 403/429 arm of `send_with_policy`.
- **C3 → FIND-TASK-002-10.** It is linked to C4, because the GHES codeload redirect is the realistic way the token leaks. It stays a separate finding because it has its own owner, the error variant's payload.

## Per-claim disposition

| Claim | Source IDs | Disposition | Gate | Evidence | Correction boundary |
|---|---|---|---|---|---|
| C1 | TASK-A-2, DOM-PB-1 | CONFIRMED → FIND-TASK-002-9 | 1 (production path); 2 (REQ-013 `spec.md:76`, "honor installation-specific primary/secondary limits, retry-after/reset"; TASK-002 S2, "Rate exhaustion, retry-after ... yield typed retry or terminal outcomes") | scratch C1a/C1b; `github.rs:1185-1190` | 403/429 arm of `send_with_policy`, `RateLimited` doc, two test expectations |
| C2 | TASK-B-1, DOM-ST-2 | CLASS_INCOMPLETE → FIND-TASK-002-1 | 2 (R1 intent "a foreign or traversing key is refused", `TASK-002-R1-validated-seam-values.md:63,65`; INV-007 `spec.md:91`, "do not accept unscoped object keys"; the crate's own `storage.rs:3-5` and `:108-109`) | scratch C2: `"x"`, `"foo/bar"`, installation `0`, non-hex version all deserialize | `ObjectKey::parse`; delete `validate_key_segment` and `KeySegment` |
| C3 | DOM-ST-1 | CONFIRMED → FIND-TASK-002-10 | 4 (credential exposure; REQ-014 `spec.md:77`, "Credentials ... never appear in logs") | scratch C3: the signed `token=` query appears in `Display` | payload of `GitHubError::UntrustedRedirect` (`github.rs:339-345`, `:969`) |
| C4 | DOM-ST-3 | CONFIRMED → FIND-TASK-002-11 | 1 and 2 (REQ-004 `spec.md:61`; AC-006 `spec.md:120`; TASK-002 outcome "Both GitHub hosts are supported") | scratch C4: the GHES codeload redirect ends in `UntrustedRedirect` with a Terminal disposition. With subdomain isolation (GitHub's recommended configuration), GHES serves archives from `codeload.<hostname>`; without it, from `<hostname>/_codeload/` | `GitHubHost` (`identity.rs:305-388`), `is_trusted_redirect` (`github.rs:741`) |
| C5 | TASK-B-2 | REVISED → DRIFT inside FIND-11 | none | Both doctests assign a `String` to an `ApiBase` field, which fails with E0308 whether or not `ApiBase` is private. They therefore catch only a revert to a `String` payload. AC-R3-1 explicitly allowed "assigning from a String", so R3 is met as written. The seal itself (private `ApiBase.0`) holds, and FIND-4 is not reopened | deleted by the FIND-11 reshape |
| C6 | TASK-A-1, TASK-B-3, REPO-2 | REVISED → DRIFT inside FIND-11 | none | `#[non_exhaustive]` at `:328` is redundant now that R3 made `ApiBase` private. `archive_host` re-parses the already-validated base and has an unreachable `unwrap_or("")` at `:381-385`. The claim that `ApiBase` is unnameable is wrong: it is reachable as `bohmian_code_review_pipeline::identity::ApiBase` because `lib.rs:14` declares `pub mod identity` | deleted by the FIND-11 reshape |
| C7 | REPO-1 | CLASS_INCOMPLETE → FIND-TASK-002-8 | 3 (`AGENTS.md:31`, "Keep imports at module top and use imported bare types in signatures") | six signature sites | imports in four source files |

## Blocking ledger

### FIND-TASK-002-1 — CLASS_INCOMPLETE (reopened; Wave 1: TASK-B-1, DOM-ST-2)

- **Gate:** 2.
- **Classification:** INCORRECT.
- **Root cause:** `ObjectKey::parse` (`storage.rs:94-99`) checks that each segment uses the allowed characters. It never checks the key layout. Only two producers exist, `ObjectKey::snapshot` (`:64-75`) and `ObjectKey::context` (`:78-83`), and each emits exactly one layout. The parser still accepts any string made of key-safe segments.
- **Why the earlier fixes let it back in:** R1 routed every segment through `validate_name`. R2 found that the length cap broke round-trips and swapped in `validate_key_segment`. Neither round checked the parser against what the constructors actually produce, and a rule applied to each segment separately is always looser than the layout. Replace that rule; do not tune it a third time.
- **Defect class:** reload validation checks the segment alphabet, not the key shape.
- **Sibling locations:**
  - `storage.rs:94-99` (`parse`, the owner).
  - `storage.rs:107-122` (`Deserialize`; its doc at `:108-109` is false).
  - `storage.rs:39-45` (type doc) and `:3-5` (module invariant).
  - `persistence.rs:52` (`SnapshotPointer.key`) and `:111` (`ContextRecord.key`), which inherit the defect.
  - `persistence.rs:206` (`remove_repository` returns `Vec<ObjectKey>`).
  - `identity.rs:138-154` (`validate_key_segment`) and `:35-38` (`InvalidIdentity::KeySegment`), which exist only to serve this parser.
  - Tests `storage.rs:221-244` and `persistence::tests::records_revalidate_their_keys_on_deserialization`.
- **Violated obligation:** R1 `:63`, `:65`; INV-007; `storage.rs:108-109` ("serde cannot mint a key the constructors would never produce").
- **Reproduction:** scratch C2.
- **Consequence:** a `Persistence` implementation can return a record whose key is `"x"` or `"foo/bar"`. The library treats it as a validated key and does an exact-key read of an object it never wrote, even though the crate's docs promise this cannot happen.
- **Distinct from the r3 deferral:** r3 deferred binding a well-formed key to the caller's identity, which remains TASK-003..005's job. This finding is about keys that have no scope at all.
- **Correction (owner: `storage.rs`):**
  - Rewrite `parse` to accept exactly the two layouts the constructors produce:
    - `tenants/{t}/installations/{i}/repositories/{r}/snapshots/{v}/{f}/{tree}`
    - `tenants/{t}/installations/{i}/repositories/{r}/contexts/{id}`
  - Validate each part with the existing validators:
    - `TenantId::new(t)`
    - `InstallationId` over the parsed `i`
    - `r` as a nonzero `u64`
    - `v` as nonempty, even-length, lowercase hex
    - `f` as a `u32`
    - `ObjectSha::new(tree)`
    - `ContextId::new(id)`
  - Require the canonical form to round-trip byte for byte, so non-canonical numbers such as `01` are rejected.
  - Replace `InvalidIdentity::KeySegment` with a variant that names the layout mismatch.
  - Delete `validate_key_segment`; `validate_name` is its only remaining caller, so inline it there. Also remove its import at `storage.rs:9` and the "every segment is key-safe" wording at `:44-45` and `:85-93`.
  - Leave the layout, the `Serialize` output, both constructors and the trait unchanged.
- **Producer check:** `prefix` (`:52-59`), the hex version (`:73`), the decimal format version, `ObjectSha` and `ContextId` produce exactly these shapes. Tenant ids of up to 128 bytes pass `TenantId::new`, and real `EXTRACTION_VERSION` keys still round-trip.
- **Rejected alternative:** dropping the stored `key` fields. That conflicts with REQ-011 (`spec.md:71`), under which persistence "publishes a context ID and exact object key".
- **Closure proof:**
  - `keys_are_revalidated_on_deserialization` refuses `"x"`, `"a/b/c"`, `"foo/bar"`, installation `0`, repository `0`, `installations/01`, a non-hex version, an odd-length version, a bad tree, and a trailing extra segment, as well as the existing hostile set.
  - `long_constructor_segments_round_trip` and the persistence record tests pass unchanged.
  - `rg -n 'validate_key_segment|KeySegment' crates` finds no match.

### FIND-TASK-002-8 — CLASS_INCOMPLETE (reopened; Wave 1: REPO-1)

- **Gate:** 3 (`AGENTS.md:31`).
- **Classification:** VIOLATION.
- **Root cause:** R3's sweep searched only for `std::fmt::` (AC-R3-7), but the rule covers every crate-qualified type in a signature. The sweep named one path instead of applying the rule.
- **Sites:** `identity.rs:94`, `:131`, `:299`; `persistence.rs:92`; `storage.rs:118` (all `D: serde::Deserializer<'de>`); `github.rs:1308` (`&serde_json::Error`).
- **Evidence:** `rg -n 'serde::Deserializer|serde_json::Error' crates/bohmian-code-review-pipeline/src` returns exactly those six lines.
- **Correction:** import the types at module top and use the bare names.
  - `Deserializer`: add it to the existing `use serde::{...}` lines.
  - The serde_json error type: import it under an alias, because a bare `Error` collides with `thiserror::Error`.
  - `serde::de::Error::custom` stays as written, since it appears in function bodies, not signatures.
- **Closure proof:**
  - `rg -n '<D: serde::|&serde_json::' crates/bohmian-code-review-pipeline/src` finds no match.
  - `fmt:check` and `lints` both exit 0.

### FIND-TASK-002-9 — CONFIRMED (new; Wave 1: TASK-A-2, DOM-PB-1)

- **Gate:** 1 and 2 (REQ-013, TASK-002 S2).
- **Classification:** INCORRECT.
- **Root cause:** `send_with_policy` (`github.rs:1182-1191`) classifies a 403 or 429 through `rate_limit_wait` (`:717-738`), which reads headers only. GitHub documents that a secondary-limit response may carry neither `retry-after` nor `x-ratelimit-remaining: 0`. In that case the body names the limit, and the client should wait at least one minute. As a result:
  - A secondary-limit 403 with neither header becomes `Forbidden`, a Terminal outcome.
  - A 429 with neither header reports the caller's `max_retry_wait` as `RateLimited.retry_after`. The field is documented as "Wait the provider requested" (`:284`), and `disposition()` passes the value on as though the provider sent it.
- **Defect class:** rate-limit classification ignores the provider's documented signal when the headers are absent.
- **Sibling locations:**
  - `github.rs:1185`, `:1186-1190`
  - `github.rs:717` (`rate_limit_wait`)
  - `github.rs:281-285` (`RateLimited` doc)
  - `github.rs:1158-1160` (`# Errors`)
  - Tests `github.rs:1808-1811`, `:1821-1829`
- **Reproduction:** scratch C1a/C1b.
- **Consequence:**
  - A GitHub secondary limit makes cold, update and PR builds fail permanently instead of retrying.
  - A caller who tuned `max_retry_wait` gets that cap back, presented as GitHub's requested wait.
- **Correction (owner: `send_with_policy`):**
  - Define a named constant for GitHub's documented one-minute minimum.
  - When no rate-limit header is present:
    - A 429 uses that minimum.
    - A 403 reads at most a few KiB of the body and ignores read errors. If the text contains "rate limit" (ASCII case-insensitive), it uses the minimum; otherwise it stays `Forbidden`.
  - Route both through the existing `(wait, RateLimited { retry_after: wait })` path, so the `max_retry_wait`, deadline and attempt bounds apply unchanged.
  - Delete the early return of `retry_after: self.limits.max_retry_wait`.
  - Update the `RateLimited` doc to say "provider-requested wait, or GitHub's documented one-minute minimum when none was given", and update `# Errors` to match.
  - Change no types, and never put body text into an error.
- **Closure proof:**
  - A 403 with the secondary-limit body, no headers and `max_retry_wait` below 60s gives `RateLimited { 60s }` with a Retry disposition. This fails at the candidate.
  - A bare 403 with an empty body still gives `Forbidden` and Terminal.
  - A bare 429 gives `RateLimited { 60s }`, replacing the expectation at `:1821-1829`.

### FIND-TASK-002-10 — CONFIRMED (new; Wave 1: DOM-ST-1)

- **Gate:** 4 (REQ-014).
- **Classification:** INCORRECT.
- **Root cause:** `UntrustedRedirect { location: String }` (`github.rs:339-345`) stores the raw, untrusted `Location` header, and its `#[error]` message prints it. Archive redirect URLs carry a signed download token in the query string.
- **Defect class:** a URL that carries a credential is echoed into error text.
- **Sibling locations:** `github.rs:339-345`, `:969`, `:951-952` (`# Errors`), and the test at `:1704-1706`.
- **Reproduction:** scratch C3. The realistic trigger today is FIND-11: every GHES codeload redirect lands here with its token.
- **Consequence:** any consumer that logs the error writes a live archive token to its logs.
- **Correction:** delete the payload, leaving a unit variant `UntrustedRedirect` with no URL in its message, and update the construction site and the test. Printing the host is out of scope. If operators later need it, add it with userinfo stripped.
- **Closure proof:** for `Location: ...?token=secret`, `err.to_string()` does not contain `secret`. This fails at the candidate.

### FIND-TASK-002-11 — CONFIRMED (new; Wave 1: DOM-ST-3; absorbs C5 TASK-B-2 and C6 TASK-A-1/TASK-B-3/REPO-2 as DRIFT)

- **Gate:** 1 and 2 (REQ-004, AC-006, TASK-002 outcome).
- **Classification:** INCORRECT, plus DRIFT deletions.
- **Root cause:** `GitHubHost::archive_host` (`identity.rs:378-387`) returns a single host. For Enterprise it re-parses the API host out of the stored base. A GHES instance with subdomain isolation serves archives from `codeload.<host>`, so every archive redirect it issues is rejected. The enum-with-payload shape also explains two other problems:
  - The seal needed three layered guards: R2's `#[non_exhaustive]`, R3's private `ApiBase`, and two `compile_fail` doctests that prove nothing about privacy.
  - A validated value is re-parsed, with an unreachable `unwrap_or("")`.
- **Defect class:** the archive trust set omits the GHES codeload subdomain, and host facts are re-derived from an already-validated value instead of computed once.
- **Sibling locations:**
  - `identity.rs:305-333`, `:335-340`, `:350-367`, `:369-375`, `:377-387`
  - `github.rs:740-746`, `:968`
  - Test `identity.rs:484-488`
  - Tests `github.rs:1486`, `:1662`, `:1753`, `:1839` (`GitHubHost::Public`)
  - `github.rs:1923-1934`
- **Reproduction:** scratch C4. C5 follows from the types: E0308 fires whether or not `ApiBase` is private.
- **Consequence:** on GHES with subdomain isolation (GitHub's recommended setup), `archive` always fails with a terminal error. Combined with FIND-10, each failure also leaks the token.
- **Correction:** reshape the candidate-owned surface. This is not a spec decision, because the crate has no external consumers yet.
  - Replace the enum with a `GitHubHost` struct whose private fields hold the validated API base and the trusted archive hosts, both computed once by the constructors.
  - `public()` trusts `codeload.github.com`.
  - `enterprise(&str)` keeps its current validation and trusts `<host>` and `codeload.<host>`. `<host>` covers `/_codeload/` when subdomain isolation is off.
  - Keep `api_base()`.
  - Replace `archive_host() -> &str` with a host-trust predicate that compares ASCII case-insensitively, and have `is_trusted_redirect` use it.
  - Delete the `Enterprise` variant, `#[non_exhaustive]`, `ApiBase`, both `compile_fail` doctests, and the `unwrap_or("")` re-parse. Private fields are the crate's standard seal, as with `TenantId` and `ObjectSha`.
  - Change the four `GitHubHost::Public` test sites to `public()`.
  - `mise.toml:58` keeps running `--doc`, so FIND-5 still holds with zero doctests.
  - Keep the enterprise validation rules, the `api_base()` values, and the single unauthenticated hop unchanged.
- **Closure proof:**
  - The host test checks that `public()` trusts only `codeload.github.com`, and that the GHES host trusts `ghe.example.com` and `codeload.ghe.example.com` but rejects `codeload.ghe.example.com.evil` and `evil.example.com`.
  - In the archive-redirect test, a GHES 302 to `https://codeload.ghe.example.com/...` is followed without authentication. This fails at the candidate.
  - `rg -n 'ApiBase|non_exhaustive|compile_fail|unwrap_or\(""\)' crates/bohmian-code-review-pipeline/src/identity.rs` finds no match.
  - `test:pipeline` passes.

## Notes (non-blocking)

- C6 (REPO-2): the claim that `ApiBase` is "unnameable" is wrong, because `pub mod identity` makes it reachable. This stops mattering once FIND-11 lands.
- C5 (TASK-B-2): the doctests met AC-R3-1 as written. FIND-4's seal held at the candidate. The doctests are deleted as DRIFT under FIND-11.

## Prior FIND closure

| ID | Status at 6899a43 | Evidence |
|---|---|---|
| FIND-TASK-002-1 | CLASS_INCOMPLETE (reopened) | ledger above; scratch C2 |
| FIND-TASK-002-2 | closed | `# Errors` present on the sampled fallible items; no Wave 1 claim |
| FIND-TASK-002-3 | closed | `rg 'ObjectKind\|fn kind' crates` exit 1 |
| FIND-TASK-002-4 | closed | seal holds (private `ApiBase.0`); superseded guards deleted under FIND-11 |
| FIND-TASK-002-5 | closed | `mise.toml:58` `--doc` in `test:pipeline` |
| FIND-TASK-002-6 | closed | `rg 'as u64 \+ 1'` exit 1; `execution.rs:59` `# Panics` |
| FIND-TASK-002-7 | closed | `///` above all five `mod tests` |
| FIND-TASK-002-8 | CLASS_INCOMPLETE (reopened) | `std::fmt::` gone; six `serde::`/`serde_json::` signature sites remain |
