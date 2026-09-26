# Domain review: durable-seam and storage-key trust boundary (TASK-002 r7)

Subject: detached worktree `/tmp/cr-r7/wt`. HEAD was `9fab199dfb282d07186e9f66ec3b3686328421e7` at both start and end, and the worktree was clean. Base is `1842eaf`. The R5 remediation under review is commit `6c0e386`.

## Reviewed boundary

The path runs from identity values to the object keys built from them, and then to reloaded persisted keys and records:

- **Identity values:** `TenantId`, `InstallationId`, `RepositoryIdentity` (owner and name), `ContextId`, and the extraction version.
- **Key construction:** `ObjectKey::prefix`, `ObjectKey::snapshot`, and `ObjectKey::context`.
- **Reload:** `ObjectKey::parse` and `canonical`, plus the serde `Deserialize` impls on `ObjectKey`, `TenantId`, `InstallationId`, `RepositoryIdentity` (via `try_from = "RawRepository"`), `ContextId`, `ObjectSha`, `SnapshotPointer`, `ContextRecord`, and `SnapshotScope`.
- **Store traits:** `SnapshotStore` and `ContextStore`, split per the Revision 11 delta.

Files: `crates/bohmian-code-review-pipeline/src/identity.rs`, `src/storage.rs`, `src/persistence.rs`. I read `src/snapshot.rs:470` only to confirm what a real producer passes as the extraction version. Its TASK-003 behavior is out of scope.

## Authority and source coverage

- **Spec (revision 11):** REQ-015 (`spec.md:78`), INV-007 (`spec.md:91`), AC-009 (`spec.md:123`).
- **Task:** the TASK-002 Revision 11 delta, and remediation TASK-002-R5 (FIND-TASK-002-12, AC-R5-1..4).
- **Diffs:** the R5 diff in full, plus the cumulative candidate state of the three owner files. All three were read end to end.

Results:

- **One shared segment rule.** `is_key_segment` (`identity.rs:57`) is now the single rule for every free-text key segment:
  - `TenantId::new` uses it directly (`identity.rs:83`).
  - Owner and name reach it through `validate_name` (`identity.rs:150-151`).
  - `ContextId::new` also reaches it through `validate_name` (`persistence.rs:77`).
  - A grep for other validators of key-derived text found none that bypass it.
- **Non-text segments.** Installation and repository ids are numeric and non-zero. The tree is 40 lowercase hex characters. The format version is a `u32`. The extraction version is hex-encoded (`storage.rs:74`), so it cannot carry `/` or dots.
- **Deserialize paths.** Every one re-validates:
  - `TenantId`, `InstallationId`, `ContextId`, and `ObjectSha` use hand-written impls that call `new`.
  - `RepositoryIdentity` uses `try_from` into `new`.
  - `ObjectKey` calls `parse`, which delegates each part to the same validator and then requires a byte-for-byte re-format.
  - `Identity`, `SnapshotScope`, `SnapshotPointer`, and `ContextRecord` are derived structs whose fields are those validated types. The exception is `SnapshotScope.extraction_version: String`, which never becomes a raw key segment because `ObjectKey::snapshot` hex-encodes it.
- **Collisions across scopes.** None are possible:
  - No segment can contain `/`.
  - The prefix position fixes each segment's meaning (tenant, installation, repository).
  - The snapshot layout has 4 trailing segments and the context layout has 2.
  - Hex encoding is injective.
  - Numbers must be canonical (`+7` and `01` are refused).
- **Store traits.** Every method on both traits takes an `Identity` (`expired_contexts` returns one per row). Neither takes a pool or consumer type, and only one test type implements a store trait.

## Verification

Commands were run in the scratch copy `/tmp/cr-r7/scratch-domain`. The source worktree was not modified.

- **`mise run test:pipeline`:** refused because the scratch config is untrusted (`mise trust` is disallowed). I ran the lane's exact commands instead:
  - `cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` gave `23 passed; 0 failed`.
  - `cargo test --locked -p bohmian-code-review-pipeline --doc --all-features` gave `0 passed; 0 failed`.
- **Focused probe:** I appended a temporary `domain_probe` test to `storage.rs`, ran `cargo test -q -p bohmian-code-review-pipeline --all-features domain_probe -- --nocapture`, then reverted the file with `git checkout`. Results:
  - **Tenant:** `.` and `..` are refused by both `new` and serde. `...`, `.a`, `a..`, and a 128-byte value are accepted. A 129-byte value is refused.
  - **ContextId:** `.` and `..` are refused by both `new` and serde. `...`, `..a`, and a 100-byte value are accepted. A 101-byte value is refused.
  - **RepositoryIdentity (serde):** owner `.` and name `..` are refused.
  - **Extreme values round-trip:** an identity with tenant `...`, installation id `u64::MAX` and repository id `u64::MAX`, and format version `u32::MAX` builds snapshot keys that reload. This holds for the real `EXTRACTION_VERSION`, for `v/../x` (hex-encoded to `762f2e2e2f78`), and for `\0`. The context key for `..a` also reloads.
  - **Hand-written keys `parse` refuses:**
    - `+1` installation, `+7` repository, `+1` and `01` format versions
    - `contexts/..` and `contexts/.`
    - `snapshots/../1/<tree>`
    - an uppercase tree
    - a context id containing `c\..\x`
- **Empty extraction version:** `ObjectKey::snapshot(.., "", ..)` builds `snapshots//…`, and `parse` refuses it. This is not a finding. The only real producer (`snapshot.rs:470`) passes the non-empty constant `EXTRACTION_VERSION` (`bohmian-code-review-extract/src/lib.rs:119`). The version-segment round-trip is also an explicit non-goal of R5 (r6 note C2).

## Verification limits

- `mise` tasks could not run under their own names: the scratch copy's config is untrusted and trusting it is disallowed. I ran the lane's cargo commands directly instead.
- I did not rerun `fmt:check` or `lints`. They were outside my time budget and belong to the general lanes, not this boundary.
- Read-side binding checks belong to later tasks and were not assessed. For example, a record's key must name the requesting identity, and `ContextRecord.id` must match its key's final segment. The read-back facade is not part of TASK-002.

## Findings

None. No constructed or reloaded key can leave its tenant, installation, repository, or context scope, and no key can collide across scopes. Every key a real producer emits round-trips. Every serde path re-validates, and no identity-derived segment bypasses `is_key_segment`. AC-R5-1..4 hold.

## Overall result

PASS

```citations
[]
```
