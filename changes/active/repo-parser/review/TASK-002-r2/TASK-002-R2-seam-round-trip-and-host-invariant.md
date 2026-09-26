---
id: TASK-002-R2
kind: remediation
status: ready
spec: SPEC-repo-parser@10
remediates: TASK-002
findings: [FIND-TASK-002-1, FIND-TASK-002-2, FIND-TASK-002-4]
---

## Subject

- Approved spec: `changes/active/repo-parser/spec.md` (revision 10)
- Original task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`
- Prior remediation: `changes/active/repo-parser/review/TASK-002-r1/TASK-002-R1-validated-seam-values.md`
- Reviewed candidate: `93b2542da4ccb26ff9468c22b35605ce0091443a` (base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`)
- Review: `changes/active/repo-parser/review/TASK-002-r2/verdict.md`; validated ledger `findings-validation.md`

## Issue diagnosis — FIND-TASK-002-1, CLASS_INCOMPLETE (re-validator refuses constructor-built keys)

- **Root cause:** the r1 fix made `ObjectKey::parse` (`crates/bohmian-code-review-pipeline/src/storage.rs:92-97`) validate every `/` segment through `validate_name` (`identity.rs:140-150`), whose `MAX_NAME_BYTES = 100` cap (`identity.rs:16`) was designed for GitHub owner/repository names. The constructors emit segments under different bounds: `ObjectKey::snapshot` writes `hex(extraction_version)` (`storage.rs:73`), which is 106 bytes for the workspace's own `EXTRACTION_VERSION` (`crates/bohmian-code-review-extract/src/lib.rs:119`, 53 bytes), and `TenantId` admits up to `MAX_TENANT_BYTES = 128` (`identity.rs:13`). The first owner of the wrong decision is `parse` choosing a validator whose length rule does not match what `snapshot` and `prefix` emit.
- **Defect class:** constructor/parser invariant mismatch at the durable seam (round-trip broken).
- **Sibling locations (one finding):** `storage.rs:92-97` (`parse`, owner); `storage.rs:73` (hex segment, producer); `storage.rs:52-59` (`prefix` tenant segment ≤128 vs ≤100); `identity.rs:140-150` (`validate_name` cap reused for the wrong domain); `persistence.rs:51` (`SnapshotPointer.key` reload) and `:110` (`ContextRecord.key` reload); tests `storage.rs:220` and `persistence.rs:263` (only `"v1"` and a one-byte tenant, so the mismatch is untested).
- **Violated obligation:** AC-R1-2 (`TASK-002-R1-validated-seam-values.md:81`, "a constructor-built key round-trips through serde equal to itself"); spec REQ-015 `spec.md:78` (exact-key lookup) and REQ-006 `spec.md:63` (snapshot reuse); INV-003 `spec.md:87`; the crate's own invariant `storage.rs:44-45`.
- **Current behavior / reproduction:** scratch crate with a path dependency on the candidate crate and the facade (three independent reproductions: `/tmp/t002r2a`, `/tmp/t002r2b`, `/tmp/t002r2-pony`):
  ```
  EXTRACTION_VERSION len=53 hex_len=106
  snapshot key round-trip:      Err("object key segment must be 1..=100 bytes of [A-Za-z0-9._-] and not `.`/`..`")
  128-byte tenant key round-trip: Err("object key segment must be 1..=100 bytes of [A-Za-z0-9._-] and not `.`/`..`")
  "v1" round-trip:              Ok
  ```
- **Observable consequence:** with the only real extraction version, `Persistence::snapshot_pointer` can never return a pointer a serde-backed implementation persisted; the cold facade (TASK-003) would rebuild every snapshot and the update facade (TASK-004) could never find its predecessor. Long-but-valid tenant ids hit the same wall for context records. The r1 fix turned silent admission into a hard reload failure for the only legitimate input.
- **Why the candidate falls short:** the r1 round-trip tests use `"v1"` and single-letter tenants, so they never exercise a segment longer than the cap.

## Issue diagnosis — FIND-TASK-002-2, CLASS_INCOMPLETE (one fallible item still lacks `# Errors`)

- **Root cause:** the r1 closure sweep matched only `-> Result<`; `BoundedReader::read` (`github.rs:565-566`) returns `io::Result<usize>`, mints its own `io::Error::other` when the budget is exhausted, and carries a one-line summary only.
- **Defect class:** missing mandatory `# Errors` on a new fallible item.
- **Sibling locations:** `github.rs:565-566` only (Wave 1 and Wave 2 both swept `-> (io::)?Result<` across the crate; this is the sole MISS).
- **Violated obligation:** `AGENTS.md:32`; `architecture/agent-rules.md:11`; `architecture/references/languages/rust-core.md:7`.
- **Evidence / reproduction:** the sweep under "Focused proof" prints `MISS crates/bohmian-code-review-pipeline/src/github.rs:566` at the candidate.
- **Observable consequence:** the only place archive over-limit becomes an error to a caller reading the stream is undocumented, while `archive`'s own doc defers to it.

## Issue diagnosis — FIND-TASK-002-4, CONFIRMED (public host variant bypasses validation)

- **Root cause:** `GitHubHost::Enterprise { api_base: String }` (`identity.rs:294-297`) is a public struct variant with a public field. Every other identity value in the module is a private-field newtype whose only admission path is its validating constructor; `GitHubHost` alone exposes a second path that skips `GitHubHost::enterprise` (`identity.rs:308-325`: https scheme, host present, no userinfo/query/fragment/trailing slash). `archive_host` (`identity.rs:340`) then assumes the invariant with an unchecked slice `api_base["https://".len()..]`.
- **Defect class:** validated domain value with a second, unvalidated public construction path (same class as FIND-TASK-002-1; different owner, so a new ID).
- **Sibling locations (one finding):** `identity.rs:294-297` (variant, admission point); `identity.rs:340` (unchecked slice, undocumented panic); `github.rs:1115` (URL built from the bypassed base); `github.rs:1132-1134` (bearer token attached regardless of scheme); `github.rs:962` (redirect trust keyed on the mis-sliced host).
- **Violated obligation:** `AGENTS.md:29` (domain values at trust boundaries); `AGENTS.md:30` and `rust-core.md:7` (no undocumented panic on input); module contract `identity.rs:3-6`; spec REQ-014 `spec.md:77` (credentials never forwarded/exposed) and REQ-004 `spec.md:61`.
- **Current behavior / reproduction:** scratch crate (`/tmp/t002r2-trust`, `/tmp/t002r2-pony`):
  ```
  api_base=http://ghe.example.com/api/v3  archive_host="he.example.com"
  sent=[("http://ghe.example.com/api/v3/repos/o/r", true)]   # true = Authorization header present
  thread 'main' panicked at .../identity.rs:340:25: start byte index 8 is out of bounds of `x`
  ```
  In the same crate `GitHubHost::enterprise("http://ghe.example.com")` is `Err(InvalidIdentity::ApiBase)`.
- **Observable consequence:** a consumer who writes the variant directly (the field is public and obviously named) gets a client that sends the installation token over cleartext HTTP, computes a wrong archive trust host, and can panic inside `archive`.
- **Why the candidate falls short:** `hosts_expose_api_base_and_archive_host` only exercises the constructor; nothing proves the variant cannot be built outside the crate.

## Intended correction outcome

Every key the constructors build round-trips through serde unchanged, including keys carrying the real `EXTRACTION_VERSION` and a maximum-length tenant, while every hostile string the r1 tests refuse is still refused. Every fallible item in the crate documents `# Errors`. `GitHubHost` can only be obtained through `Public` or `enterprise()`, and `archive_host` has no panic path. Key layout, `Serialize` output, trait method sets, provider client behaviour, request policy, and every existing test expectation are unchanged.

## Decision-complete recommendation

**FIND-TASK-002-1** — owner `identity.rs` validators, consumed by `storage.rs:92-97`. Add `pub(crate) fn validate_key_segment(raw: &str) -> Result<(), InvalidIdentity>` in `identity.rs` next to `validate_name`, refusing empty, `.`, `..`, and any byte failing the existing `is_key_safe`; keep `validate_name` as the length-capped name rule and have it delegate the non-length checks to the new function so there is one alphabet rule. `ObjectKey::parse` calls `validate_key_segment` instead of `validate_name`. Add `InvalidIdentity::KeySegment` with message `object key segment must be nonempty [A-Za-z0-9._-] and not `.`/`..`` so the error stops claiming a 100-byte cap; put `# Errors` on every touched fallible item. Why this closes the class: the three constructors (`prefix`/`snapshot`/`context`) already emit only key-safe bytes (validated identity, hex, decimal, `ContextId`), so segment-safety without a name-length cap is exactly the domain they produce; the hostile set (`../x`, `./x`, empty segment, leading/trailing `/`, space, empty) is still refused by the same function. Rejected alternatives: making `snapshot` fallible (moves the decision to every caller); raising `MAX_NAME_BYTES` (weakens the GitHub-name rule for a different domain); hashing or truncating the version segment or adding a whole-key length cap (persisted-format decisions the spec does not require).

**FIND-TASK-002-2** — add `# Errors` to `BoundedReader::read` stating that inner read errors propagate and that `io::Error::other` is returned once the budget is exhausted and the inner reader yields another byte. Documentation only.

**FIND-TASK-002-4** — owner `identity.rs`. Put `#[non_exhaustive]` on the `Enterprise` variant: external struct-expression construction becomes a compile error (E0639, verified in a two-crate scratch build), in-crate construction inside `enterprise()` is unaffected, and external matching still works with `..`. In `archive_host`, replace the unchecked slice with `strip_prefix("https://")` followed by the existing `split('/').next()` (the pattern already used at `identity.rs:309` and `github.rs:737`) so no panic path remains. Keep `Public`, `enterprise()`, `api_base()`, `archive_host()` names and behaviour; internal matches unchanged. Rejected alternatives: a newtype for the payload or turning `GitHubHost` into a struct (more surface for the same guarantee); scheme checks inside `GitHubClient` (the invariant belongs to the value, and `enterprise()` already enforces it).

## Constraints, preserved behavior, and non-goals

- Do not change the key string layout, `Serialize` output, hex encoding of the version segment, trait method sets, provider client behaviour, request policy, or any existing test expectation.
- Do not add a dependency, a helper struct, a trait, or a whole-key length cap.
- Do not add facade code, an OpenDAL adapter, conformance implementations (TASK-006), a concurrency limiter, or a real-GitHub run.
- Do not alter the six symbolic crates, grammar pins, or fixtures.
- Keep tests inline in the owning module; record the expected RED failure for each new or extended test per `architecture/references/languages/spec-driven-development.md`.

## Acceptance criteria

- **AC-R2-1 (FIND-TASK-002-1, `storage.rs:92-97`, `:73`, `:52-59`, `identity.rs:140-150`):** `ObjectKey::snapshot(&identity_with_128_byte_tenant, bohmian_code_review::EXTRACTION_VERSION, 1, &tree)` serialises and deserialises to an equal key; the same holds for `ObjectKey::context` under the same identity.
- **AC-R2-2 (FIND-TASK-002-1, `persistence.rs:51`, `:110`):** a `SnapshotPointer` and a `ContextRecord` whose keys are built with `EXTRACTION_VERSION` round-trip through serde equal to themselves.
- **AC-R2-3 (FIND-TASK-002-1, regression boundary):** every hostile string in `storage::tests::keys_are_revalidated_on_deserialization` and `persistence::tests::records_revalidate_their_keys_on_deserialization` is still refused; `ContextId::new` and `validate_name` behaviour (including the 100-byte cap for names and context ids) is unchanged.
- **AC-R2-4 (FIND-TASK-002-2, `github.rs:566`):** the sweep below prints no `MISS` line for the pipeline crate; `cargo doc --no-deps -p bohmian-code-review-pipeline` emits no warning.
- **AC-R2-5 (FIND-TASK-002-4, `identity.rs:294-297`):** a `compile_fail` doctest on `GitHubHost` containing `let _ = bohmian_code_review_pipeline::GitHubHost::Enterprise { api_base: "http://x".to_owned() };` fails to compile from outside the crate; `GitHubHost::enterprise("http://ghe.example.com")` remains `Err(InvalidIdentity::ApiBase)`.
- **AC-R2-6 (FIND-TASK-002-4, `identity.rs:340`):** `archive_host` contains no indexing slice; `identity::tests::hosts_expose_api_base_and_archive_host` passes unchanged.
- **AC-R2-7:** `git diff --stat 93b2542..HEAD` touches only `identity.rs`, `storage.rs`, `persistence.rs`, `github.rs`, and the task document's evidence section.

## Focused proof and broader verification

- Focused: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib storage::tests` and `... persistence::tests` (extended round-trip tests for AC-R2-1..3); `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --doc` (AC-R2-5); `... --lib identity::tests::hosts_expose_api_base_and_archive_host -- --exact` (AC-R2-6).
- `# Errors` sweep (supersedes the r1 pattern): `for f in crates/bohmian-code-review-pipeline/src/*.rs; do rg -n '^\s*(pub(\([a-z]+\))? )?fn \w+[^;]*-> (io::)?Result<' "$f" | while IFS=: read ln rest; do s=$((ln-14)); [ $s -lt 1 ] && s=1; sed -n "${s},${ln}p" "$f" | grep -q '# Errors' || echo "MISS $f:$ln"; done; done` plus a manual read of multi-line signatures.
- Broader: `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `mise run gate`; `git diff --stat` per AC-R2-7.
- Record observed results (including the expected RED for each new or extended test) in the task document's evidence section per `architecture/references/languages/implementation-execution.md`.

Route to `$cr-implement`. A later review reassesses the complete cumulative candidate against the original TASK-002.
