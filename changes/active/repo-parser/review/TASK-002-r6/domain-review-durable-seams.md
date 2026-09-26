# TASK-002 r6 domain review: durable seams

Reviewer: domain-rev (durable seams). Candidate `7983907` (detached at `/tmp/cr-r6/wt`), base `1842eaf`.

## Reviewed boundary

- `crates/bohmian-code-review-pipeline/src/persistence.rs`: the Revision 11 split into `SnapshotStore` and `ContextStore`.
- `crates/bohmian-code-review-pipeline/src/storage.rs`: the `ObjectKey` constructors, `parse`/`canonical`, `Deserialize`, and the invariant docs (R4 FIND-TASK-002-1).
- Only as needed for the above: the `identity.rs` validators that `canonical` reuses (`TenantId::new`, `InstallationId::new`, `ObjectSha::new`, `validate_name`); `lib.rs` re-exports; and the trait-rename edits in `testing.rs` (`MemorySnapshotStore`) and `snapshot.rs` (`&dyn SnapshotStore`, `MemorySnapshotStore`).

## Authority and source coverage

- Spec rev 11: REQ-006, REQ-014, REQ-015, INV-007, and AC-009.
- TASK-002's "Revision 11 delta".
- R4 remediation `TASK-002-R4-key-layout-rate-limits-and-host-shape.md`: the FIND-TASK-002-1 diagnosis, recommendation 1, AC-R4-1, and AC-R4-2.
- The R2 source-trust domain review, read only for what it recorded about dot segments.
- Read in full: `persistence.rs` and `storage.rs`. Also read: the relevant parts of `identity.rs` and `testing.rs`, and the `git diff 1f6d538 b83d286` hunks for `snapshot.rs`, `lib.rs`, and `eligibility.rs`.

### Revision 11 split: PASS

- **Method set.** The trait methods correspond one-to-one with the old `Persistence` (`git show 1f6d538`). `SnapshotStore` has `snapshot_pointer`, `publish_snapshot_pointer`, and `remove_repository`. `ContextStore` has `publish_context`, `context`, `expired_contexts`, `delete_context`, and `remove_repository`. Nothing is missing and nothing extra. The old combined `remove_repository` is split, and each half's doc and return value covers only its own records.
- **Identity, visibility, and coupling.**
  - Every method carries `Identity`. For `expired_contexts`, the identity comes back in the result.
  - Both traits restate the object-before-row and no-transaction-across-IO rules.
  - The two traits share only `PersistenceError`, which the delta allows.
  - Neither trait takes a pool, SDK, or consumer type.
- **`trait Persistence` sweep.** `rg 'trait Persistence\b'` finds nothing. `MemorySnapshotStore` implements only `SnapshotStore`.
- **Conditional publication.** The signature is unchanged: compare-and-set on the current pointer's tree (`Option<&ObjectSha>`). The in-memory implementation compares `current != expected` under one lock. This is sound for tree-addressed keys.
- **Expired-snapshot cleanup.** REQ-015 and REQ-006 call for expired-snapshot cleanup. The delta deliberately leaves it out of TASK-002, and TASK-003 (line 48) owns it, so it is not a finding here.
- **Consumer edits.**
  - `snapshot.rs` changes only `Persistence` to `SnapshotStore` and `MemoryPersistence` to `MemorySnapshotStore`, plus rustfmt reflow.
  - `lib.rs` re-exports both new names.
  - Compile-only closure is correct.

### R4 FIND-TASK-002-1: partial

- **What holds.**
  - `validate_key_segment` and `KeySegment` are gone. `rg` returns nothing, so AC-R4-2 holds.
  - `canonical` matches exactly the two layouts and re-formats. The byte-for-byte comparison refuses `01`, `+1`, extra and empty segments, and the AC-R4-1 hostile set.
  - Constructor keys with a 128-byte tenant and the real `EXTRACTION_VERSION` round-trip.
- **What fails.** The parser is still not an exact inverse of the constructors. See DOM-DS-1 and DOM-DS-2.

## Verification limits

- The probes ran in a scratch copy, `/tmp/cr-r6/scratch-ds`. `mise` refused the untrusted scratch config, and I did not run `mise trust`. Instead I invoked the toolchain `cargo` resolved by `mise exec -- which cargo` in the candidate, using `--all-features`.
- The orchestrator lanes in `/tmp/cr-r6/lanes.txt` (fmt:check, lints, test:pipeline) all report exit 0. I did not re-run them.
- Commit `b83d286` also changes `eligibility.rs` glob semantics (trailing `**`) and a `snapshot.rs` tar fixture. Those are TASK-003 behavior, outside this boundary, and I did not review them.
- I did not assess binding a reloaded key to the caller identity. R4 declares it a non-goal and defers it to TASK-003..005.

## Findings

### DOM-DS-1: CONFIRMED. A dot-only tenant id yields a traversing object key that is built and reloaded as valid

- **Root cause.** `TenantId::new` (`identity.rs:73-74`) accepts any 1..=128-byte `[A-Za-z0-9._-]` string, including `.` and `..`.
  - `validate_name`, used for owners, names, and context ids, refuses `.` and `..`. `TenantId` does not.
  - `ObjectKey::prefix` embeds the tenant verbatim.
  - The R4 `canonical` validates the tenant segment with `TenantId::new` (`storage.rs:122`), so it accepts the same dot segment.
  - Before R4, the per-segment parser refused `.`/`..` segments (see the r2 source-trust review, line 30). That refusal was the only guard, and the rewrite removed it.
- **Defect class.** A key-segment validator admits path dot-segments. The owner's shape permits it: `TenantId` is key-embedded but uses a looser rule than its key-embedded siblings.
- **Sibling locations.**
  - `identity.rs:73-74` (`TenantId::new`, the owner). Its `Deserialize` inherits the rule.
  - `storage.rs:122` (`canonical` tenant check).
  - `storage.rs:44-46` (the invariant doc claims a reloaded key "can never be unscoped or traverse out of its scope").
  - `storage.rs:237` (`hostile_keys` has no dot-tenant case).
- **Violated obligation.**
  - INV-007: "do not accept unscoped object keys".
  - REQ-014: "Tenant ... identities scope snapshot/context keys".
  - The R1 intent "a foreign or traversing key is refused".
  - The crate's own documented invariant at `storage.rs:44-46`.
- **Evidence.** The scratch test `ds_probe::probe_tenant_dotdot_key_accepted` fails as follows:
  - `ObjectKey::snapshot` with tenant `..` builds `tenants/../installations/1/repositories/7/snapshots/7631/1/bbbb…`.
  - Reloading that key through `serde_json` returns `Ok`.
  - Tenant `.` gives `tenants/./installations/1/repositories/7/contexts/c`, which also reloads as `Ok`.
- **Observable consequence.** A path-normalizing `ObjectStorage` (filesystem-backed, or any backend that resolves `..`) places tenant `..` objects at `installations/1/...`, outside the `tenants/` namespace. The library treats those keys as scoped and validated, both on write and on reload.
- **Testable correction.**
  - Make `TenantId::new` (and therefore its `Deserialize`) refuse `.` and `..`, as `validate_name` does.
  - Add dot-tenant keys to `hostile_keys` and a `TenantId` rejection case.
  - Keep the storage invariant doc true.

### DOM-DS-2: CONFIRMED. The snapshot version segment rule is not the inverse of the constructor's encoding

- **Root cause.** `ObjectKey::snapshot` (`storage.rs:71-74`) hex-encodes any `&str`, including `""`. `canonical` (`storage.rs:128-132`) accepts any nonempty, even-length, lowercase-hex segment. The parser checks the hex alphabet instead of decoding the segment back to the constructor's input domain (UTF-8 text) and re-encoding it.
- **Defect class.** The reload rule checks the version's alphabet, not its round-trip through the encoder. This is the FIND-TASK-002-1 class (shape looser or stricter than the constructor), still present in one variable part.
- **Sibling locations** (both one finding):
  - `storage.rs:128-132`: accepts `ff` and other non-UTF-8 byte sequences that no `&str` produces.
  - `storage.rs:71-74`: an empty `extraction_version` yields `snapshots//{fv}/{tree}`, which the parser refuses.
- **Violated obligation.**
  - R4 intended outcome: "Only the two constructor layouts round-trip through `ObjectKey`. Everything else is refused at load."
  - The `Deserialize` doc (`storage.rs:157-158`): "every constructor-built key round-trips byte-for-byte".
- **Evidence.** Two scratch tests fail:
  - `ds_probe::probe_non_utf8_version_parses`: `tenants/a/installations/1/repositories/7/snapshots/ff/1/<tree>` deserializes as `Ok`.
  - `ds_probe::probe_empty_version_roundtrip`: `ObjectKey::snapshot(&id, "", 1, &tree)` produces `…/snapshots//1/…`, and its own serialization fails to deserialize.
- **Observable consequence.**
  - The public constructor (re-exported via `lib.rs:46`) can mint a key that makes any `SnapshotPointer` holding it permanently unloadable.
  - Reload accepts keys that no extraction version could have produced.
  - Current in-crate callers pass only `EXTRACTION_VERSION`, so the consequence depends on public-API use. It is still a live contract mismatch in the exact surface R4 was meant to close.
- **Testable correction.** Make the parser's version rule the exact inverse of the encoder. One way is to hex-decode the segment, require `String::from_utf8`, and re-encode with the same `hex` helper. The alternative is to make an empty version unrepresentable at the constructor, or to accept it at the parser. Either way, the round-trip property must hold for every constructor input. Add both probe cases as owner tests.

## Overall: FAIL

The store split is correct. The ObjectKey reload is still not exact: DOM-DS-1 (traversing tenant keys) and DOM-DS-2 (version segment encode/decode mismatch).

```citations
[
  {"id":"DOM-DS-1","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":73,"snippet":"    pub fn new(raw: &str) -> Result<TenantId, InvalidIdentity> {\n        if raw.is_empty() || raw.len() > MAX_TENANT_BYTES || !is_key_safe(raw) {","symbols":["TenantId","new","MAX_TENANT_BYTES","is_key_safe","InvalidIdentity"]},
  {"id":"DOM-DS-1","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":122,"snippet":"            TenantId::new(tenant).ok()?.as_str(),","symbols":["TenantId","new","as_str","canonical"]},
  {"id":"DOM-DS-1","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":44,"snippet":"/// tenants can never share an object even for identical source. A persisted\n/// key is re-parsed against those two layouts on reload, so it can never be\n/// unscoped or traverse out of its scope.","symbols":["ObjectKey"]},
  {"id":"DOM-DS-1","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":237,"snippet":"    pub(crate) fn hostile_keys() -> Vec<String> {","symbols":["hostile_keys"]},
  {"id":"DOM-DS-2","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":128,"snippet":"                let version_is_hex = !version.is_empty()\n                    && version.len() % 2 == 0\n                    && version\n                        .bytes()\n                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));","symbols":["version_is_hex","canonical"]},
  {"id":"DOM-DS-2","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":71,"snippet":"        ObjectKey(format!(\n            \"{}/snapshots/{}/{format_version}/{tree}\",\n            ObjectKey::prefix(identity),\n            hex(extraction_version.as_bytes())","symbols":["ObjectKey","prefix","hex","snapshot"]}
]
```
