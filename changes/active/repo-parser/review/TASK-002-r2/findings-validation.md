# TASK-002 task review r2 — findings validation (Wave 2, ponytail-rev)

Candidate `93b2542da4ccb26ff9468c22b35605ce0091443a`, base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`. Conclusions reached from source and independent reproduction; Wave 1 reasoning not read. Budget honoured (15 min); nothing left unfinished except that I did not re-run `mise run gate` (orchestrator lane).

## (a) Merge decisions

- Claim 1 (TASK-A-1 + TASK-B-1) — one root cause: the r1 re-validator (`ObjectKey::parse`) uses a validator whose value domain (`validate_name`, ≤100 bytes) is narrower than what the constructors emit (hex of a free-text version; tenant ≤128). Both symptoms (real `EXTRACTION_VERSION`; 101–128-byte tenant) are the same defect at the same owner. This is the seam FIND-TASK-002-1 corrected and it violates that remediation's AC-R1-2 → **reopen FIND-TASK-002-1 as CLASS_INCOMPLETE**. No new ID.
- Claim 2 (TASK-B-2) — same root cause as FIND-TASK-002-2 (closure sweep treated `-> Result<` as the only fallible spelling; `io::Result` missed) → **reopen FIND-TASK-002-2 as CLASS_INCOMPLETE**. No new ID.
- Claim 3 (TRUST-1) — same *defect class* as FIND-TASK-002-1 (validated value with a public construction path that skips its invariant) but a different owner and seam (provider host at the GitHub trust boundary, not a key-bearing value at the durable seam); r1's root cause statement and sibling sweep were explicitly scoped to key-bearing durable values. Not the same root cause → **new ID FIND-TASK-002-4**.

## (b) Per-claim validation

### Claim 1 — CLASS_INCOMPLETE (reopens FIND-TASK-002-1)

- Gate: (2) violates remediation AC-R1-2 ("a constructor-built key round-trips through serde equal to itself", TASK-002-R1-validated-seam-values.md:81); also (1) once TASK-003 persists a `SnapshotPointer` built with the real `EXTRACTION_VERSION`, every pointer reload fails, so no snapshot can ever be reused (REQ-006/REQ-015 exact-key lookup, spec.md:63, :78).
- Reproduction (scratch crate `/tmp/t002r2-pony`, path deps on the pipeline and facade crates):
  ```
  EXTRACTION_VERSION len=53 key_safe=true
  C1a real-version reload: Err("object key segment must be 1..=100 bytes of [A-Za-z0-9._-] and not `.`/`..`")
  C1b 128-byte tenant reload: Err("object key segment must be 1..=100 bytes of [A-Za-z0-9._-] and not `.`/`..`")
  C1c v1 reload: Ok("ok")
  ```
  The in-tree tests (storage.rs:220, persistence.rs:263) only use `"v1"` (hex = 4 bytes) and a 1-byte tenant, which is why they pass.
- Caller trace: `ObjectKey::snapshot` (storage.rs:64) → production callers: none yet (tests only); it is the task's deliverable constructor for REQ-006 keys and TASK-003 must call it with `EXTRACTION_VERSION` (facade lib.rs:178, :193 is the only source of that value). `ObjectKey::parse` (storage.rs:92) ← `Deserialize for ObjectKey` (storage.rs:116) ← `SnapshotPointer.key` (persistence.rs:51), `ContextRecord.key` (persistence.rs:110) ← `Persistence::snapshot_pointer` / context read-back (persistence.rs:137, trait). `validate_name` (identity.rs:140) ← `RepositoryIdentity::new`, `ContextId::new`, `ObjectKey::parse`.
- Ladder: delete the re-validation? No — it is FIND-TASK-002-1's correction (traversal defence). Drop the hex encoding and validate the version in `snapshot` instead? Rejected: makes a public constructor fallible (API change) for no gain — hex already guarantees key-safety for any input; the real `EXTRACTION_VERSION` happens to be key-safe but the constructor contract says free text (storage.rs:63). Raise `MAX_NAME_BYTES`? Rejected: widens GitHub owner/name validation. Minimum correction: the length cap is the only mismatch; key segments need the alphabet/empty/dot rules, not the GitHub-name length rule. Fix at the one owner (`parse`) — both siblings close together.

### Claim 2 — CLASS_INCOMPLETE (reopens FIND-TASK-002-2)

- Gate: (3) AGENTS.md:32 ("Fallible functions need `# Errors`"); architecture/references/languages/rust-core.md:7.
- Reproduction: github.rs:565-566 — `/// Reads at most the remaining budget; a further byte is an error.` then `fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {`; no `# Errors`. Extended sweep (`-> (io::)?Result<`, 14-line lookback) over the pipeline crate prints exactly one line: `MISS crates/bohmian-code-review-pipeline/src/github.rs:566`. No other sibling.
- Caller trace: `BoundedReader` ← `GitHubClient::archive` stream bound (task evidence row REQ-014); production path, required by the task.
- Ladder: doc-only correction; no code change.

### Claim 3 — CONFIRMED as FIND-TASK-002-4

- Gate: (4) security on a reachable path — a consumer writing `GitHubHost::Enterprise { api_base: "http://…" }` (the field is public and obviously named; `enterprise()` is discoverable only by reading docs) sends `Authorization: Bearer <token>` in cleartext (github.rs:1115 builds the URL from `api_base()`, :1133 attaches the token whenever `authenticated`); (3) rust-core.md:7 "possible panics are documented or removed": `archive_host` (identity.rs:340) slices `api_base["https://".len()..]` unchecked — an undocumented panic in a public method; AGENTS.md:29 "Use domain values … at trust boundaries" — a domain value whose invariant can be bypassed is not one; AC-R1-3 already established the crate rule "the field is not publicly constructible" for validated seam values.
- Reproduction (same scratch crate):
  ```
  C3a bypass api_base=http://ghe.example.com/api/v3 archive_host=he.example.com
  thread 'main' panicked at .../identity.rs:340:25: start byte index 8 is out of bounds of `x`
  C3b short api_base archive_host panicked=true
  ```
- Caller trace: `GitHubHost` ← `GitHubClient` (host field) → `request` (github.rs:1115) and archive redirect trust `is_trusted_redirect(&location, self.host.archive_host())` (github.rs:962, :735). Public re-export lib.rs:25. Task evidence row REQ-004 names `GitHubHost::{Public, enterprise}`, `api_base`, `archive_host` as the contract; the variant's *field shape* is not named by spec or task.
- Ladder: delete? No (REQ-004 GHES). Existing repo pattern? Newtype with private field (`TenantId`, `ContextId`) — works but adds a type. Language feature? `#[non_exhaustive]` on the variant: external crates cannot construct it by struct expression (E0639) but can still match with `..`; verified in `/tmp/t002r2-ne` (two-crate scratch: `error[E0639]: cannot create non-exhaustive variant using struct expression`). One attribute, no new type, no spec-named API changes → not SPEC_REVISION_REQUIRED. The panic is removed with the existing `strip_prefix` pattern already used at identity.rs:309 and github.rs:737.

## (c) Final deduplicated ledger

### FIND-TASK-002-1 — CLASS_INCOMPLETE (Wave 1: TASK-A-1, TASK-B-1)

- Gate: (2) AC-R1-2; (1) REQ-006/REQ-015 pointer reload.
- Classification: blocking. Root cause: `ObjectKey::parse` re-validates with `validate_name` (GitHub-name domain: ≤100 bytes) while `ObjectKey::snapshot` emits an unbounded hex segment and `Identity.tenant` admits ≤128 bytes; constructor and re-validator disagree on the segment domain.
- Defect class: constructor/parser invariant mismatch at the durable seam (round-trip broken).
- Siblings: storage.rs:92-97 (owner, `parse`), storage.rs:73 (hex segment, producer), storage.rs:52-59 (tenant segment ≤128), persistence.rs:51 and :110 (reload sites), identity.rs:140-150 (`validate_name` cap reused for the wrong domain), tests storage.rs:220 / persistence.rs:263 (only `"v1"`, 1-byte tenant).
- Violated obligation: TASK-002-R1-validated-seam-values.md:81 (AC-R1-2); spec.md:63 REQ-006, :78 REQ-015; storage.rs:44-45 own invariant.
- Evidence/reproduction: §(b) Claim 1 output.
- Observable consequence: every real snapshot pointer / context record whose key carries the real extraction version (106-byte hex) or a >100-byte tenant fails deserialization; no snapshot can be reused after publication.
- Correction (owner `identity.rs` + `storage.rs:92-97`): add `pub(crate) fn validate_key_segment(raw: &str) -> Result<(), InvalidIdentity>` in identity.rs next to `validate_name`, refusing empty, `.`, `..`, and `!is_key_safe(raw)`; have `validate_name` keep its length check and delegate the rest to it; `ObjectKey::parse` calls `validate_key_segment` instead of `validate_name`. Add `InvalidIdentity::KeySegment` (`"object key segment must be nonempty [A-Za-z0-9._-] and not `.`/`..`"`) so the error text stops claiming a 100-byte cap; `# Errors` on every touched fallible item. Preserve: hex encoding, key format, all seven hostile strings still refused, `ContextId`/`RepositoryIdentity` caps unchanged. Do not: make `snapshot` fallible, raise `MAX_NAME_BYTES`, hash/truncate the version segment, or add a key-length cap (speculative).
- Closure proof: extend `storage::tests::keys_are_revalidated_on_deserialization` to round-trip `ObjectKey::snapshot(&identity(&"t".repeat(128)), bohmian_code_review::EXTRACTION_VERSION, 1, &tree)` (crate already depends on the facade, Cargo.toml:12) and keep the hostile list; extend `persistence::tests::records_revalidate_their_keys_on_deserialization` to build its key with `EXTRACTION_VERSION`. `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib storage::tests persistence::tests`, `fmt:check`, `lints`.

### FIND-TASK-002-2 — CLASS_INCOMPLETE (Wave 1: TASK-B-2)

- Gate: (3) AGENTS.md:32; rust-core.md:7.
- Root cause: closure sweep matched `-> Result<` only; `io::Result` spelling escaped it. Defect class: missing mandatory `# Errors` on a new fallible item.
- Siblings: github.rs:565-566 only.
- Consequence: rule violation; no runtime effect.
- Correction: add `# Errors` to `BoundedReader::read` stating that inner read errors propagate and that `io::Error::other` is returned once the budget is exhausted and the inner reader yields another byte. Doc only.
- Closure proof: `for f in crates/bohmian-code-review-pipeline/src/*.rs; do rg -n '^\s*(pub(\([a-z]+\))? )?fn \w+[^;]*-> (io::)?Result<' "$f" | while IFS=: read ln rest; do s=$((ln-14)); [ $s -lt 1 ] && s=1; sed -n "${s},${ln}p" "$f" | grep -q '# Errors' || echo "MISS $f:$ln"; done; done` prints nothing (this pattern supersedes the r1 sweep); `cargo doc --no-deps -p bohmian-code-review-pipeline` 0 warnings.

### FIND-TASK-002-4 — CONFIRMED (Wave 1: TRUST-1)

- Gate: (4) bearer token over cleartext via public bypass; (3) rust-core.md:7 undocumented panic; AGENTS.md:29.
- Root cause: `GitHubHost::Enterprise { api_base: String }` (identity.rs:294-297) is a public struct variant, so the `enterprise()` invariant (https, host present, no userinfo/query/fragment/trailing slash) is bypassable; `archive_host` (identity.rs:340) then assumes the invariant with an unchecked slice.
- Defect class: validated domain value with a second, unvalidated public construction path (same class as FIND-TASK-002-1; different owner).
- Siblings: identity.rs:294-297 (variant), identity.rs:340 (unchecked slice/panic), github.rs:1115 (URL from bypassed base), github.rs:1133 (token attached regardless of scheme), github.rs:962 (redirect trust keyed on mis-sliced host).
- Violated obligation: AGENTS.md:29; rust-core.md:7; identity.rs:3-6 module contract; spec.md:77 REQ-014 (credentials never exposed), :61 REQ-004.
- Evidence/reproduction: §(b) Claim 3 output.
- Consequence: a consumer can mint a host that sends the installation token over `http://`, mis-computes the trusted archive host, or panics inside `archive_host`.
- Correction (owner identity.rs): put `#[non_exhaustive]` on the `Enterprise` variant (language feature; external struct-expression construction becomes E0639, matching still works with `..`); in `archive_host`, replace the slice with `api_base.strip_prefix("https://").and_then(|rest| rest.split('/').next()).unwrap_or("")` (the pattern already used at identity.rs:309 and github.rs:737) so no panic path remains. Preserve: `Public`, `enterprise()`, `api_base()`, `archive_host()` names and behaviour; internal matches unchanged. Do not: add a newtype, make `GitHubHost` a struct, or add scheme checks in `GitHubClient` (the invariant belongs to the value).
- Closure proof: a `compile_fail` doctest on `GitHubHost` (`let _ = bohmian_code_review_pipeline::GitHubHost::Enterprise { api_base: "http://x".to_owned() };`) — doctests compile as an external crate, so this is the exact bypass proof; `identity::tests::hosts_expose_api_base_and_archive_host` unchanged; `cargo test --locked -p bohmian-code-review-pipeline --all-features --doc`.

## (d) Notes (no remediation)

- Wave 1 obs — well-formed foreign-scope keys still deserialize; scope binding is at facade lookup under `Identity` (r1 accepted).
- repo-rev — `file.patch.clone()` github.rs:1355 avoidable; preference, not a rule.
- Wave 1 obs — no RED record for the three r1 remediation tests; process note.
- Wave 1 obs — ureq `TransportError::Failed` may echo a codeload `?token=` URI; unreproduced, outside this diff's control path.

## (e) Rejected claims

None.

## (f) Prior-finding closure

- FIND-TASK-002-1: CLASS_INCOMPLETE — remaining siblings storage.rs:92-97, :73, :52-59; persistence.rs:51, :110; identity.rs:140-150 (see ledger).
- FIND-TASK-002-2: CLASS_INCOMPLETE — remaining sibling github.rs:566.
- FIND-TASK-002-3: CLOSED — `rg -n 'ObjectKind|fn kind' crates` → no output.

## (g) Verification commands and results

- `git rev-parse HEAD` → `93b2542da4ccb26ff9468c22b35605ce0091443a`.
- `/tmp/t002r2-pony` `cargo run -q` → output quoted in §(b) (C1a/C1b Err, C1c Ok, C3a http base + `he.example.com`, C3b panic at identity.rs:340:25).
- Extended `# Errors` sweep (§FIND-2 closure command) → `MISS crates/bohmian-code-review-pipeline/src/github.rs:566` only.
- `/tmp/t002r2-ne` two-crate build → `error[E0639]: cannot create non-exhaustive variant using struct expression` (proves the FIND-4 correction; matching with `..` in the same file compiled past that point).
- `rg -n 'ObjectKind|fn kind' crates` → none.
- `rg -n 'ObjectKey::snapshot|extraction_version|GitHubHost::Enterprise|api_base|archive_host' crates/bohmian-code-review-pipeline/src` → callers as traced in §(b).

```citations
[{"id": "FIND-TASK-002-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 94, "snippet": "            validate_name(segment, \"object key segment\")?;", "symbols": ["ObjectKey::parse", "validate_name"]},
 {"id": "FIND-TASK-002-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 73, "snippet": "            hex(extraction_version.as_bytes())", "symbols": ["ObjectKey::snapshot", "hex"]},
 {"id": "FIND-TASK-002-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 142, "snippet": "        || raw.len() > MAX_NAME_BYTES", "symbols": ["validate_name", "MAX_NAME_BYTES"]},
 {"id": "FIND-TASK-002-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 13, "snippet": "const MAX_TENANT_BYTES: usize = 128;", "symbols": ["TenantId", "MAX_TENANT_BYTES"]},
 {"id": "FIND-TASK-002-1", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 51, "snippet": "    pub key: ObjectKey,", "symbols": ["SnapshotPointer"]},
 {"id": "FIND-TASK-002-2", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 566, "snippet": "    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {", "symbols": ["BoundedReader"]},
 {"id": "FIND-TASK-002-4", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 294, "snippet": "    Enterprise {", "symbols": ["GitHubHost"]},
 {"id": "FIND-TASK-002-4", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 340, "snippet": "                api_base[\"https://\".len()..].split('/').next().unwrap_or(\"\")", "symbols": ["GitHubHost::archive_host"]},
 {"id": "FIND-TASK-002-4", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1115, "snippet": "        let url = format!(\"{}{resource}\", self.host.api_base());", "symbols": ["GitHubClient::request", "GitHubHost::api_base"]},
 {"id": "FIND-TASK-002-4", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1133, "snippet": "            headers.push((\"Authorization\", format!(\"Bearer {}\", self.token.expose())));", "symbols": ["GitHubClient::build", "InstallationToken::expose"]}]
```
