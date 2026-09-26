---
id: TASK-002-R5
kind: remediation
status: ready
spec: SPEC-repo-parser@11
remediates: TASK-002
findings: [FIND-TASK-002-12]
---

## Subject

- Approved spec: `changes/active/repo-parser/spec.md` (revision 11).
- Original task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`, including its Revision 11 delta.
- Prior remediations: `../TASK-002-r1/TASK-002-R1-validated-seam-values.md`, `../TASK-002-r2/TASK-002-R2-seam-round-trip-and-host-invariant.md`, `../TASK-002-r3/TASK-002-R3-host-payload-seal-and-lane-doctests.md`, `../TASK-002-r5/TASK-002-R4-key-layout-rate-limits-and-host-shape.md`.
- Reviewed candidate: `7983907a6aa0b01a942fb964362da73bc9e7dba6`, base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`.
- Review record: `verdict.md` and `findings-validation.md` in this directory.
- Owning crate: `crates/bohmian-code-review-pipeline`. Write set: `src/identity.rs` and `src/storage.rs` only.

## Issue diagnosis: FIND-TASK-002-12, CONFIRMED (a tenant id of `.` or `..` yields a traversing object key)

- **Root cause.** `TenantId::new` (`identity.rs:73-77`) refuses empty, over-long, and non-`[A-Za-z0-9._-]` values, but accepts `.` and `..`. Its sibling validator `validate_name` (`identity.rs:141-151`) refuses both. The tenant is the only free-text key segment without that rule.
- **Defect class.** A key-segment validator accepts dot-only (traversal) segments.
- **Sibling locations** (all route through the one owner):
  - `identity.rs:73-77`: `TenantId::new`, the owner, and its `# Errors` doc at `:70-72`.
  - `identity.rs:92-96`: the `TenantId` `Deserialize` implementation, which delegates to `new`.
  - `identity.rs:21-24`: the `InvalidIdentity::Tenant` doc and `#[error]` text.
  - `storage.rs:53-60`: `ObjectKey::prefix`, which embeds the tenant verbatim for both constructors.
  - `storage.rs:122`: `canonical`, where reload delegates the tenant check to `TenantId::new`.
  - `storage.rs:44-46`: the invariant doc, "can never be unscoped or traverse out of its scope", which is currently false.
  - `storage.rs:237`: the `hostile_keys` fixture, which lacks the case. It drives `keys_are_revalidated_on_deserialization` (`storage.rs:294`) and `persistence::tests::records_revalidate_their_keys_on_deserialization` (`persistence.rs:260`).
- **Violated obligation.**
  - INV-007 (`spec.md:91`): "do not accept unscoped object keys".
  - The R1 intent: "a foreign or traversing key is refused".
  - The module's own contract (`identity.rs:6`): construction rejects anything the library would otherwise have to sanitize later.
- **Current behavior.** `TenantId::new("..")` returns `Ok`. `ObjectKey::context` then builds `tenants/../installations/1/repositories/7/contexts/c`, and `ObjectKey::parse` reloads it as `Ok`. The same happens with `.`.
- **Reproduction.** Wave 2 ran a probe test on a scratch copy of the candidate, using `cargo test -q -p bohmian-code-review-pipeline --all-features pony_probe -- --nocapture`. For both tenants the output showed `TenantId::new` returning `Ok`, the traversing key being constructed, and `reload = Ok(...)`.
- **Consequence.** A consumer `ObjectStorage` that resolves paths, such as a filesystem or a normalizing gateway, maps `tenants/../installations/...` outside the `tenants/` scope the library promises. The library issues, and reloads, an effectively unscoped key.
- **Why the existing proof falls short.** The tenant owner test (`identity.rs:402-404`) checks only empty and `/`. `hostile_keys` has no dot-only tenant.

## Intended outcome

No `TenantId`, and therefore no constructed or reloaded `ObjectKey`, contains a `.` or `..` tenant segment. Every existing producer output still round-trips.

## Recommendation

- Add the dot-only rule to `TenantId::new` itself: refuse `raw == "." || raw == ".."`, returning `InvalidIdentity::Tenant`.
  - This is the same rule `validate_name` already applies. No new helper, variant or type is needed.
  - Fixing the owner closes every sibling: the constructors, the `Deserialize` path and `canonical` all route through `new`.
- Update the `InvalidIdentity::Tenant` doc and `#[error]` text, and the `# Errors` doc of `TenantId::new`, to state the rule the way `Name` does.
- Extend the existing tests:
  - Add `tenants/./installations/1/repositories/7/contexts/c` and `tenants/../installations/1/repositories/7/contexts/c` to `hostile_keys`.
  - Assert `TenantId::new(".")` and `TenantId::new("..")` return `Err(InvalidIdentity::Tenant)` in the existing tenant owner test. Assert that deserializing `"."` and `".."` as `TenantId` fails.

## Constraints and non-goals

- Keep everything else as is:
  - the key layout, the constructors, `canonical`, `Serialize` and the serialized form;
  - `is_key_safe`;
  - the R4 host, rate-limit and redirect behavior.
- Do not touch `snapshot.rs`, `eligibility.rs` or `testing.rs`, which hold TASK-003 work in progress.
- Non-goals: the version-segment decode round-trip (r6 note C2) and the `ureq::Agent` field path (r6 note C3). Neither blocks, and neither is part of this correction.

## Acceptance criteria

- **AC-R5-1:** `TenantId::new(".")` and `TenantId::new("..")` return `Err(InvalidIdentity::Tenant)`, and deserializing either value as `TenantId` fails. Covers the owner and the `Deserialize` sibling.
- **AC-R5-2:** both dot-only tenant keys are in `hostile_keys`. They fail to deserialize as `ObjectKey` and as the `key` field of `SnapshotPointer` and `ContextRecord`. Covers the `canonical`, `prefix` and fixture siblings.
- **AC-R5-3:** the `InvalidIdentity::Tenant` doc and message and the `TenantId::new` `# Errors` doc state the dot-only rule. The `storage.rs:44-46` invariant is then true as written.
- **AC-R5-4:** every existing test still passes, including `long_constructor_segments_round_trip`.

## Proof

- **Focused:** `mise run test:pipeline`. The new assertions fail at `7983907` and pass after the fix.
- **Broader:** `mise run fmt:check`, `mise run lints`, and `mise exec -- cargo doc --no-deps -p bohmian-code-review-pipeline --all-features` with no pipeline warnings.
