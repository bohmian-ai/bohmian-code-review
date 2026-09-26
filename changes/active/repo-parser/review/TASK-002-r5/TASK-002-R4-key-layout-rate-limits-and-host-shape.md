---
id: TASK-002-R4
kind: remediation
status: ready
spec: SPEC-repo-parser@10
remediates: TASK-002
findings: [FIND-TASK-002-1, FIND-TASK-002-8, FIND-TASK-002-9, FIND-TASK-002-10, FIND-TASK-002-11]
---

## Subject

- Approved spec: `changes/active/repo-parser/spec.md`.
- Original task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`.
- Prior remediations: `../TASK-002-r1/TASK-002-R1-validated-seam-values.md`, `../TASK-002-r2/TASK-002-R2-seam-round-trip-and-host-invariant.md`, `../TASK-002-r3/TASK-002-R3-host-payload-seal-and-lane-doctests.md`.
- Reviewed candidate: `6899a4323b486a1b99eaeaf1fe3af67ea8c9e429`, base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`. Its code is identical to `801d957`.
- Review record: `changes/active/repo-parser/review/TASK-002-r5/verdict.md`, with the validated ledger in `findings-validation.md`.
- Owning crate: `crates/bohmian-code-review-pipeline`. This task changes only `identity.rs`, `storage.rs`, `persistence.rs`, `github.rs`, and `lib.rs`, and changes `lib.rs` only if a re-export changes.

This review applies the revised review rules. `GitHubHost`, `ObjectKey`'s parser, and `GitHubError`'s payloads are surfaces the candidate owns; the spec does not name them. Change their shape wherever the shape is the root cause. Delete earlier-round guards that the new shape makes redundant.

## Issue diagnosis: FIND-TASK-002-1, CLASS_INCOMPLETE (reloaded object keys are not checked against the constructor layout)

- **Root cause.** `ObjectKey::parse` (`storage.rs:94-99`) splits on `/` and checks each segment with `validate_key_segment`, then accepts the raw string. Only two constructors produce keys, `ObjectKey::snapshot` (`:64-75`) and `ObjectKey::context` (`:78-83`), and each emits exactly one layout:
  - `tenants/{tenant}/installations/{installation}/repositories/{repository}/snapshots/{hex version}/{format version}/{tree sha}`
  - `tenants/{tenant}/installations/{installation}/repositories/{repository}/contexts/{context id}`

  Any string made of key-safe segments deserializes, whatever its layout.
- **Why the earlier fixes let it back in.** R1 validated each segment with `validate_name`. R2 found that `validate_name`'s length cap broke round-trips, so it switched to a looser per-segment rule. Both rounds adjusted a per-segment rule, and a per-segment rule is always looser than the layout. Replace that rule; do not tune it again.
- **Defect class.** Reload validation checks the segment alphabet, not the key shape.
- **Sibling locations** (all one finding):
  - `storage.rs:94-99` (`parse`, the owner)
  - `storage.rs:107-122` (`Deserialize`; its doc at `:108-109` claims "serde cannot mint a key the constructors would never produce")
  - `storage.rs:3-5` and `:39-45` (the module and type invariants), and `:85-93` (the `parse` doc)
  - `persistence.rs:52` (`SnapshotPointer.key`), `:111` (`ContextRecord.key`), and `:206` (`remove_repository` returns `Vec<ObjectKey>`)
  - `identity.rs:35-38` (`InvalidIdentity::KeySegment`) and `:138-154` (`validate_key_segment`), which exist only to serve this parser
  - the tests `storage.rs:221-244` and `persistence::tests::records_revalidate_their_keys_on_deserialization`
- **Violated obligations.**
  - INV-007 (`spec.md:91`): "do not accept unscoped object keys".
  - The R1 intent (`TASK-002-R1-validated-seam-values.md:63,65`): "a foreign or traversing key is refused".
  - The crate's own documented invariant.
- **Reproduction.** A scratch crate using only the public API deserialized each of these as `Ok`:
  - `"x"`
  - `"a/b/c"`
  - `"foo/bar"`
  - `tenants/a/installations/0/repositories/7/contexts/c`
  - `tenants/a/installations/1/repositories/7/snapshots/zz/1/nothex`

  A `ContextRecord` with `key: "x"` also loads.
- **Consequence.** A `Persistence` implementation can hand back a record whose key has no scope. The library then treats it as a validated key and makes an exact-key read of an object it never wrote.
- **Why the existing proof falls short.** `keys_are_revalidated_on_deserialization` covers only traversal and empty segments. It never tests layout.
- **Out of scope.** Binding a well-formed key to the caller's identity remains deferred to TASK-003..005, as r3 recorded.

## Issue diagnosis: FIND-TASK-002-8, CLASS_INCOMPLETE (crate-qualified types in signatures)

- **Root cause.** The R3 closure searched for one path, `std::fmt::`, instead of applying the rule. The rule is `AGENTS.md:31`: "Keep imports at module top and use imported bare types in signatures."
- **Sites.** `identity.rs:94`, `:131`, `:299`, `persistence.rs:92`, and `storage.rs:118` all use `D: serde::Deserializer<'de>`. `github.rs:1308` uses `&serde_json::Error`.
- **Evidence.** `rg -n 'serde::Deserializer|serde_json::Error' crates/bohmian-code-review-pipeline/src` returns exactly these six lines.
- **Consequence.** A written repository rule is violated in code the diff introduced.

## Issue diagnosis: FIND-TASK-002-9, CONFIRMED (a secondary rate limit without headers is reported as terminal `Forbidden`)

- **Root cause.** In `send_with_policy` (`github.rs:1182-1191`), a 403 or 429 is classified only through `rate_limit_wait` (`:717-738`), and that function reads only the `retry-after` and `x-ratelimit-remaining: 0` headers.
  - GitHub documents that a secondary-limit response may carry neither header. The body names the limit, and clients must wait at least one minute.
  - A 403 with neither header therefore returns `Forbidden`, which is terminal.
  - A 429 with neither header returns `RateLimited { retry_after: self.limits.max_retry_wait }` immediately. That reports a caller setting as the wait GitHub requested, although `:284` documents the field as "Wait the provider requested".
- **Defect class.** Rate-limit classification ignores the provider's documented signal when the headers are absent.
- **Sibling locations.**
  - `github.rs:1185-1190`
  - `github.rs:717-738` (`rate_limit_wait`)
  - `github.rs:281-285` (the `RateLimited` doc)
  - `github.rs:1158-1160` (`# Errors`)
  - the tests at `github.rs:1808-1811` and `:1821-1829`, which assert the current behaviour
- **Violated obligations.**
  - REQ-013 (`spec.md:76`): "honor installation-specific primary/secondary limits, retry-after/reset information".
  - TASK-002 Scenario 2: typed retry or terminal outcomes.
- **Reproduction.**
  - A 403 with `x-ratelimit-remaining: 4990`, no `retry-after`, and the body "You have exceeded a secondary rate limit" returned `Forbidden` with disposition `Terminal`.
  - A 429 with no headers returned the `max_retry_wait` value as `retry_after`.
- **Consequence.** A cold, update, or PR build that hits a secondary limit fails permanently instead of retrying.

## Issue diagnosis: FIND-TASK-002-10, CONFIRMED (the redirect error leaks a signed archive token)

- **Root cause.** `GitHubError::UntrustedRedirect { location: String }` (`github.rs:339-345`, constructed at `:969`) stores the raw `Location` header, and its `#[error("untrusted archive redirect to {location}")]` prints it. Archive redirect URLs carry a signed download token in the query string.
- **Defect class.** A URL that carries a credential is echoed into error text.
- **Sibling locations.** `github.rs:339-345`, `:969`, `:951-952` (`# Errors`), and the test at `:1704-1706`.
- **Violated obligation.** REQ-014 (`spec.md:77`): credentials never appear in logs.
- **Reproduction.** For a redirect to `https://evil.example/archive?token=SECRET-SIGNED`, `err.to_string()` contains `SECRET-SIGNED`.
- **Consequence.** A consumer that logs the error writes a live archive token to its logs. Because of FIND-11, every GHES codeload redirect currently takes this path.

## Issue diagnosis: FIND-TASK-002-11, CONFIRMED (GHES codeload archives are rejected; the host seal is layered guards on the wrong shape)

- **Root cause.** `GitHubHost` is an enum. Its `Enterprise { api_base: ApiBase }` variant exposes a single archive host through `archive_host()` (`identity.rs:377-387`), which re-parses the API host from the already-validated base and falls back to an unreachable `unwrap_or("")`.
  - GHES with subdomain isolation, GitHub's recommended configuration, serves archives from `codeload.<host>`, so `is_trusted_redirect` (`github.rs:740-746`) rejects every archive redirect such an instance sends.
  - The same enum-with-payload shape is why sealing the host took three layered guards, and why a validated value is parsed a second time:
    - R2's `#[non_exhaustive]` (`identity.rs:328`), now redundant.
    - R3's private `ApiBase` newtype (`:335-340`).
    - Two `compile_fail` doctests (`:310-322`) that fail with E0308 (`String` vs `ApiBase`) whether or not the field is private, so they prove nothing about the seal.
- **Defect class.** The archive trust set omits the GHES codeload subdomain, and host facts are re-derived from an already-validated value instead of computed once.
- **Sibling locations.**
  - `identity.rs:305-388`, covering the doctests, the enum, `ApiBase`, `enterprise`, `api_base`, and `archive_host`
  - `github.rs:740-746` and `:968`
  - the test at `identity.rs:484-488`
  - the `GitHubHost::Public` uses at `github.rs:1486`, `:1662`, `:1753`, `:1839`, and `:1923-1934`
- **Violated obligations.**
  - REQ-004 (`spec.md:61`) and AC-006 (`spec.md:120`).
  - The TASK-002 outcome: both GitHub hosts are supported.
  - `AGENTS.md:27`: no future-proofing. This applies to the redundant attribute.
- **Reproduction.** With GHES host `https://ghe.example.com/api/v3`, a 302 to `https://codeload.ghe.example.com/...` returned `UntrustedRedirect` with disposition `Terminal`.
- **Consequence.** `archive` always fails on a subdomain-isolated GHES instance, and each failure leaks the token through FIND-10.

## Intended outcome

- Only the two constructor layouts round-trip through `ObjectKey`. Everything else is refused at load.
- Signatures use imported bare types.
- A secondary limit without headers is a retryable `RateLimited` outcome with an honest wait, and a plain 403 stays `Forbidden`.
- No error text contains a redirect URL.
- `GitHubHost` is a sealed struct whose trusted archive hosts are computed once and include GHES codeload.
- The R2 and R3 seal scaffolding is gone.

## Recommendation (decision-complete)

1. **FIND-1: parse the layout in `storage.rs`.**
   - Rewrite `ObjectKey::parse` to match exactly the snapshot and context layouts, and validate every variable part with the crate's existing validators:
     - `TenantId::new`
     - `InstallationId` over the parsed decimal
     - repository id as a nonzero `u64`
     - version as nonempty, even-length, lowercase hex
     - format version as `u32`
     - `ObjectSha::new`
     - `ContextId::new`
   - Accept the key only if re-formatting the parsed parts reproduces the input byte for byte. This rejects non-canonical numbers such as `01`.
   - Replace `InvalidIdentity::KeySegment` with one variant that names the layout mismatch.
   - Delete `validate_key_segment`, folding its character rule into `validate_name`, its only other caller. Remove its import and the "every segment is key-safe" wording.
   - Keep the key layout, the `Serialize` output, both constructors, and the traits unchanged.
   - Do not drop the stored `key` fields. REQ-011 (`spec.md:71`) requires persistence to publish the exact object key.
2. **FIND-8: imports.**
   - Import `Deserializer` at the top of `identity.rs`, `persistence.rs`, and `storage.rs`.
   - Import the `serde_json` error type in `github.rs` under an alias, because it collides with `thiserror::Error`.
   - Use the bare names in all six signatures. Body-level paths such as `serde::de::Error::custom` are not signatures and stay as they are.
3. **FIND-9: rate-limit classification in `send_with_policy`.**
   - Add one named constant for GitHub's documented one-minute minimum.
   - When `rate_limit_wait` returns `None`:
     - For a 429, use the constant.
     - For a 403, read at most a few KiB of the body, ignoring read errors. If the text contains "rate limit" (ASCII case-insensitive), use the constant; otherwise return `Forbidden`.
   - Route both through the existing `(wait, RateLimited { retry_after: wait })` path, so `max_retry_wait`, the deadline, and the attempt bounds apply unchanged.
   - Delete the early `retry_after: self.limits.max_retry_wait` return.
   - Update the `RateLimited` doc and `# Errors` to match.
   - Change no types, and never copy body text into an error.
4. **FIND-10: redirect error payload.** Make `UntrustedRedirect` a unit variant whose message does not include the URL, and update the construction site and the test. Do not add a host field.
5. **FIND-11: reshape `GitHubHost` in `identity.rs` and `github.rs`.**
   - Replace the enum with a struct whose private fields hold the validated API base and the trusted archive hosts, both computed once by the constructors.
   - `GitHubHost::public()` trusts `codeload.github.com`.
   - `GitHubHost::enterprise(&str)` keeps its current validation and error, and trusts `<host>` (which covers `/_codeload/`) and `codeload.<host>`.
   - Keep `api_base()`.
   - Replace `archive_host() -> &str` with a host-trust predicate that compares ASCII case-insensitively, and have `is_trusted_redirect` use it.
   - Delete the `Enterprise` variant, `#[non_exhaustive]`, `ApiBase`, both `compile_fail` doctests, and the `unwrap_or("")` re-parse.
   - Private fields are the crate's standard seal, the same one `TenantId` and `ObjectSha` use, so no doctest is needed to prove it.
   - Update the `GitHubHost::Public` call sites to `public()`.
   - Keep the `--doc` step in `mise.toml`, so FIND-5 stays closed.

This closes each defect class at its owner:

- The layout parser makes an unscoped key unrepresentable after load.
- The host struct makes an unvalidated or re-derived host unrepresentable.
- The unit variant makes URL leakage unrepresentable.
- The rate-limit fix is the one classification point that every request goes through.

## Constraints and preserved behaviour

- No new dependency, trait, feature, or generic layer.
- No change to the key layout, `Serialize` output, request URLs, bearer attachment (API requests only), the single unauthenticated redirect hop, `max_redirects(0)`, the retry/deadline/cancellation policy, `read_bounded`, or pagination and truncation reporting.
- `GitHubHost::enterprise` keeps its validation rules: `https://` only, no trailing slash.
- `InvalidIdentity` keeps its other variants.
- Tests stay inline. Every new or materially changed item gets rustdoc, with `# Errors` on fallible items (AGENTS.md:32).
- Non-goals:
  - scope-binding reloaded keys to the caller's identity (TASK-003..005)
  - a live GitHub or GHES run
  - printing redirect hosts in errors
  - changing r4 notes N-5 to N-8

## Acceptance criteria

- **AC-R4-1 (FIND-1).** Deserializing each of the following as `ObjectKey`, and as the `key` of `SnapshotPointer` and `ContextRecord`, fails:
  - `"x"`
  - `"a/b/c"`
  - `"foo/bar"`
  - installation `0`
  - repository `0`
  - `installations/01`
  - a non-hex version
  - an odd-length version
  - an invalid tree SHA
  - a trailing extra segment
  - the existing traversal/empty set

  `long_constructor_segments_round_trip` and the persistence record round-trip tests pass unchanged, including 128-byte tenants and real `EXTRACTION_VERSION` values.
- **AC-R4-2 (FIND-1 DRIFT).** `rg -n 'validate_key_segment|KeySegment' crates` returns nothing. The `storage.rs` invariant docs describe layout parsing.
- **AC-R4-3 (FIND-8).** `rg -n '<D: serde::|&serde_json::' crates/bohmian-code-review-pipeline/src` returns nothing.
- **AC-R4-4 (FIND-9).** All of the following hold:
  - A 403 with a secondary-limit body, no rate headers, and `max_retry_wait` below 60 s yields `RateLimited { retry_after: 60s }` with a `Retry` disposition. This test fails at the candidate.
  - A bare 403 with an empty body yields `Forbidden` with `Terminal`.
  - A bare 429 yields `RateLimited { retry_after: 60s }`.
  - Existing header-driven cases are unchanged.
- **AC-R4-5 (FIND-10).** For a rejected redirect to `...?token=secret`, `err.to_string()` does not contain `secret` or the URL. This test fails at the candidate.
- **AC-R4-6 (FIND-11).**
  - `public()` trusts only `codeload.github.com`.
  - `enterprise("https://ghe.example.com/api/v3")` trusts `ghe.example.com` and `codeload.ghe.example.com`, and rejects `codeload.ghe.example.com.evil` and `evil.example.com`.
  - A GHES 302 to `https://codeload.ghe.example.com/...` is followed without an `Authorization` header. This test fails at the candidate.
  - `api_base()` values are unchanged for both hosts.
- **AC-R4-7 (FIND-11 DRIFT).** `rg -n 'ApiBase|non_exhaustive|compile_fail|unwrap_or\(""\)' crates/bohmian-code-review-pipeline/src/identity.rs` returns nothing. An external struct-literal construction of `GitHubHost` does not compile, because its fields are private.
- **AC-R4-8.** The write set is limited to the pipeline crate's `src/` files, and to `lib.rs` only for re-exports. `mise.toml` is unchanged.

## Proof

- Focused: the new and updated inline tests for AC-R4-1, AC-R4-4, AC-R4-5, and AC-R4-6, each observed failing at the candidate before the fix.
- Lanes: `mise run fmt:check`, `mise run lints`, `mise run test:pipeline` (lib tests and the `--doc` step, which may now run zero doctests), `mise exec -- cargo doc --no-deps -p bohmian-code-review-pipeline --all-features` with no pipeline warnings, and `mise run gate`.
- Sweeps: the `rg` commands in AC-R4-2, AC-R4-3, and AC-R4-7.
