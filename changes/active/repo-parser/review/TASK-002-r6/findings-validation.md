# TASK-002 r6 findings validation (Wave 2, ponytail-rev)

> The harness refused the `ponytail-rev` agent's write to this file (subagents may not write report files). The orchestrator saved the ledger the agent returned verbatim; the agent's conclusions are unmodified.

- **Subject:** candidate 7983907, base 1842eaf.
- **Input:** `claim-ledger.md` (C1, C2, C3).
- **Reproduction setup:** scratch copy at `/tmp/cr-r6/scratch-pony` (a `git archive` of 7983907), run with `cargo test -p bohmian-code-review-pipeline --all-features`.

**Status summary:** C1 is CONFIRMED as a new blocking finding, FIND-TASK-002-12. C2 is a NOTE in FIND-1's class and does not meet the blocking gate. C3 is a NOTE: a struct field is not a signature, so FIND-8 stays closed. The drift audit is clean.

## Merge decisions

- **C1 is not FIND-TASK-002-1.**
  - FIND-1's class is "reload accepts a key that no constructor produces."
  - The R4 layout parser closed that for the tenant segment: `canonical` (`storage.rs:122`) checks the tenant with `TenantId::new`, the same validator the constructors use.
  - A `..` tenant key *is* produced by `ObjectKey::context` and `ObjectKey::snapshot`. The wrong decision happens earlier, in `TenantId::new` (`identity.rs:73-77`), and both the constructors and reload inherit it.
  - That is a different root cause, so it gets a new ID: **FIND-TASK-002-12**.
- **C2 is in FIND-1's class.** Reload is not the exact inverse of the constructor's encoding for the version segment. It does not meet the blocking gate, so it is a NOTE and FIND-1 is not reopened.
- **C3 does not reopen FIND-TASK-002-8.** The reasons are in the notes.

## Retained blocking findings

### FIND-TASK-002-12: tenant id accepts `.` and `..`, so constructed and reloaded keys leave their scope

- **Wave 1 source IDs:** C1 (DOM-DS-1)
- **Status:** CONFIRMED. This is a new root cause, not a recurrence of FIND-1.
- **Gate:**
  - (2) INV-007 (`spec.md:91`): library methods "do not accept unscoped object keys".
  - (4) It breaks the storage-key durability and trust boundary, even though the path is rare.
  - The owner's written invariants are also false:
    - `storage.rs:44-46` says a key "can never be unscoped or traverse out of its scope".
    - The `identity.rs` module doc says "Construction rejects anything the library would otherwise have to sanitize later".
- **Classification:** a validation gap at a trust boundary (caller-supplied identity).
- **Root cause:**
  - `TenantId::new` (`identity.rs:73-77`) refuses empty values, values over the length limit, and characters outside `[A-Za-z0-9._-]`, but it accepts `.` and `..`.
  - Its sibling validator `validate_name` (`identity.rs:141-151`) refuses both.
  - The tenant is the only free-text, consumer-supplied key segment that is not refused when it is dot-only.
- **Defect class:** a key-segment validator accepts dot-only (traversal) segments.
- **Sibling locations** (all go through the one owner):
  - `identity.rs:73-77`: `TenantId::new`, the owner.
  - `identity.rs:92-96`: `TenantId` `Deserialize`, which delegates to `new`.
  - `storage.rs:53-60`: `ObjectKey::prefix`, which embeds the tenant verbatim and is used by both constructors.
  - `storage.rs:122`: `canonical`, where reload delegates to `TenantId::new`.
  - `storage.rs:44-46`: the invariant doc, which is currently false.
  - `identity.rs:21-23`: the `InvalidIdentity::Tenant` doc and message, which should state the `.`/`..` rule the way `Name` does.
  - `storage.rs:231-257`: the `hostile_keys` fixture, which lacks this case.
  - Every other segment is already closed:
    - owner, repository name and context id go through `validate_name`;
    - installation id, repository id and format version are numbers;
    - the tree id is 40 hex characters;
    - the version is hex.
- **Violated obligation:** INV-007, and the R1 intent recorded for FIND-1 ("a foreign or traversing key is refused").
- **Evidence:** `identity.rs:74` reads `if raw.is_empty() || raw.len() > MAX_TENANT_BYTES || !is_key_safe(raw) {`, and `is_key_safe` allows `.`.
- **Reproduction:** a probe test in the scratch copy builds an `Identity` with the tenant `.` and then `..`, calls `ObjectKey::context`, then calls `ObjectKey::parse`.
  - Command: `cargo test -q -p bohmian-code-review-pipeline --all-features pony_probe -- --nocapture`
  - Output:
    ```
    TenantId::new(".") = Ok(TenantId("."))
    constructed tenants/./installations/1/repositories/7/contexts/c; reload = Ok(ObjectKey("tenants/./installations/1/repositories/7/contexts/c"))
    TenantId::new("..") = Ok(TenantId(".."))
    constructed tenants/../installations/1/repositories/7/contexts/c; reload = Ok(ObjectKey("tenants/../installations/1/repositories/7/contexts/c"))
    ```
- **Consequence:**
  - A consumer's `ObjectStorage` that resolves paths, such as a filesystem or a gateway that normalizes paths, treats `tenants/../installations/1/...` as `installations/1/...`. That is outside the `tenants/` scope the library promises every key stays in. With `.`, `tenants/./...` loses its tenant segment.
  - The library hands the consumer an effectively unscoped key, and reload accepts it back.
  - It is unlikely in practice, because a consumer would have to configure the tenant `..`. It still blocks under gate (4): the library's documented contract is that building the identity makes the key scoped, so consumers are not expected to sanitize.
- **Correction (decision-complete, at the owner):**
  - In `TenantId::new` (`identity.rs:74`), also refuse `raw == "." || raw == ".."`. This is the same rule `validate_name` already applies, so no new helper is needed.
  - Change the `InvalidIdentity::Tenant` doc and its `#[error]` text to add "and not `.`/`..`", as `Name` does. Update the `# Errors` doc of `TenantId::new` to match.
  - Add `tenants/../installations/1/repositories/7/contexts/c` and `tenants/./installations/1/repositories/7/contexts/c` to `hostile_keys`.
  - Add a rejection assertion for `TenantId::new(".")` and `TenantId::new("..")` to the existing tenant owner test.
  - Leave everything else as is: the constructors, `canonical`, the key layout and the serialized form.
  - Real producers are unaffected: ordinary tenants such as `a`, 128-byte tenants and `EXTRACTION_VERSION`.
- **Closure proof:**
  - (a) I applied the one-line guard in the scratch copy and ran `cargo test -q -p bohmian-code-review-pipeline --all-features`: 24 passed, 0 failed. Every existing producer and round-trip test still passes, including `long_constructor_segments_round_trip`.
  - (b) The new `hostile_keys` entries must fail to deserialize as `ObjectKey` and as the `key` of `SnapshotPointer` and `ContextRecord`. The existing hostile-key tests already loop over the fixture, so all siblings are covered.
  - (c) `TenantId::new(".")` and `TenantId::new("..")` must return `Err(InvalidIdentity::Tenant)`, and so must the `TenantId` deserialize path.
  - (d) Sweep: `rg -n 'is_key_safe' crates/bohmian-code-review-pipeline/src` shows exactly two callers, and after the fix both refuse dot-only values.

## Notes (non-blocking)

- **C2 (DOM-DS-2): NOTE, FIND-1 class, gate not met.**
  - Reload accepts hex that does not decode to UTF-8, such as `ff`. I reproduced it: `reload ff = Ok(...)`.
  - Such a key stays fully scoped and cannot traverse. At worst it names an object that does not exist, which returns `NotFound`.
  - The empty-version key `snapshots//...` cannot be produced. Every producer (`persistence.rs:288`, `snapshot.rs:472`, `snapshot.rs:1000`) passes the nonempty constant `EXTRACTION_VERSION` (`bohmian-code-review-extract/src/lib.rs:119`).
  - The candidate's version check is exactly what the decision-complete R4 remediation prescribed ("version as nonempty, even-length, lowercase hex").
  - Optional, if someone is already editing this code: decode the hex, require it to be valid UTF-8, and rebuild the key through the constructor. Not required.
- **C3 (REPO-1): NOTE, not a rule violation. FIND-TASK-002-8 stays closed.**
  - `AGENTS.md:31` says "use imported bare types in signatures". `github.rs:107` (`agent: ureq::Agent,`) is a private struct field declaration, not a signature.
  - The R4 remediation drew the same line when it said body-level paths "are not signatures".
  - The repo already uses qualified field types elsewhere, for example `extract/src/lua.rs:94` (`_marker: std::marker::PhantomData<&'a ()>`).
  - The FIND-8 closure check (`rg -n '<D: serde::|&serde_json::'`) returns nothing, and the in-scope pipeline files have no crate-qualified types in function signatures.
  - `snapshot.rs:215` does put `bohmian_code_review::Error` in a function signature. It is in out-of-scope TASK-003 work in progress, so I have flagged it for the TASK-003 review and not ruled on it here.
- **Drift audit:** nothing left over from R1–R3.
  - `rg 'validate_key_segment|KeySegment|Sealed|sealed::|mod private'` over the pipeline sources returns nothing.
  - `is_key_safe` has exactly two live callers.
