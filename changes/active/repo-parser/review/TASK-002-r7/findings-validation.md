# TASK-002 review r7: findings validation (Wave 2, ponytail-rev)

> The harness denied ponytail-rev's write to the review directory. The orchestrator saved the ledger it returned verbatim.

Subject: `/tmp/cr-r7/wt` at HEAD `9fab199dfb282d07186e9f66ec3b3686328421e7`. HEAD matched at the start and at the end, and the worktree is clean.

## Result

**Validated empty blocking ledger.** Wave 1 proposed no claims, and this independent source pass found no blocking defect. FIND-TASK-002-12 is closed across its whole class. The DRIFT audit found nothing to delete.

## Final blocking ledger

None.

## Claim validation

Wave 1 left the ledger empty, so there are no claims to classify.

### Independent check: FIND-TASK-002-12 class closure

Defect class: a string used verbatim as a storage-key segment accepts `.` or `..`, which lets a key leave its scope. The owner is now a single predicate, `is_key_segment` (`crates/bohmian-code-review-pipeline/src/identity.rs:57`). It refuses empty, over-long, `.`, `..` and non-`[A-Za-z0-9._-]` input. Every verbatim segment source goes through it:

| Sibling | Path | Status |
|---|---|---|
| `TenantId::new` | identity.rs:82-83, calls `is_key_segment(raw, MAX_TENANT_BYTES)` | closed |
| `TenantId` serde | identity.rs:102-104, calls `TenantId::new` | closed |
| owner and repository name | identity.rs:150-151 `validate_name`, calls `is_key_segment`; `RepositoryIdentity::new` at :207-208 | closed |
| `RepositoryIdentity` serde | identity.rs:163 `try_from = "RawRepository"`, then `RepositoryIdentity::new` | closed |
| `Identity` serde | identity.rs:234, derived; each field uses one of the validating deserializers above | closed |
| `ContextId::new` and serde | persistence.rs:76-77 and :93-95, call `validate_name` | closed |
| `ObjectKey::snapshot` / `ObjectKey::context` | storage.rs:65-84. Segments come only from `TenantId`, numeric ids, hex-encoded extraction version, `ObjectSha` and `ContextId`, so no raw string reaches a key | closed |
| `ObjectKey` parse and reload (serde) | storage.rs:97-147. `TenantId::new`, `InstallationId::new`, `NonZeroU64`, the hex check, `ObjectSha::new` and `ContextId::new` are applied per segment, and the key is accepted only if its canonical re-format matches byte for byte (storage.rs:98-99) | closed |

No other code builds a key. A sweep of `crates/` for `tenants/` and `ObjectKey(` finds only storage.rs:55, :71, :80 and :121, plus tests. The byte filter at `github.rs:703` does URL percent-encoding, not key building, so it is outside this class.

## DRIFT audit (R1-R5)

Nothing is redundant, so nothing needs deleting.

- `validate_name` is now a thin wrapper, but it still maps failures to `InvalidIdentity::Name(what)`, which carries the field label. It has 3 callers.
- The canonical re-format check in `ObjectKey::parse` (storage.rs:98-99) is not covered by the per-part validators. It is the only thing that rejects non-canonical numbers such as `01`, and I reproduced that below.
- The hex check on `version` (storage.rs:125-131) is not covered by `is_key_segment`. It enforces the encoding that `ObjectKey::snapshot` produces.
- R5 removed the old `is_key_safe` and the duplicate `.`/`..` checks in `validate_name`, and left no leftovers.

## Notes (non-blocking)

- NOTE: `ContextId::new` rustdoc (persistence.rs:74-75) repeats the segment rule in prose instead of linking the shared rule. This is documentation only and does not affect behaviour.

## Prior FIND-* closure status

| ID | Status |
|---|---|
| FIND-TASK-002-1 .. -11 | Closed in r6 (r6 verdict.md, findings-validation.md). No regression seen in the R5 diff or in the retouched paths. |
| FIND-TASK-002-12 | Closed. R5 (commit 6c0e386) routes `TenantId` through `is_key_segment`, and every constructor, parse and serde sibling is covered (table above). |

## Reproduction

The worktree is untrusted by mise, so I ran cargo directly in the scratch copy `/tmp/cr-r7/scratch-ponytail` (`cp -R /tmp/cr-r7/wt /tmp/cr-r7/scratch-ponytail`).

1. I appended a temporary test `storage::ponytail_r7::dot_segments_refused_everywhere`. It asserts that serde rejects:
   - `ObjectKey` strings with tenant `.` or `..`
   - `ObjectKey` strings with context `.` or `..`
   - an `ObjectKey` string with installation `01`
   - `Identity` JSON with tenant `..`, owner `..` or name `.`

   It also asserts that valid `Identity` and `ObjectKey` values are accepted.

   `cargo test -p bohmian-code-review-pipeline --all-features`: probe passed, 24 passed, 0 failed, and doc-tests passed.
2. I removed the probe (`git checkout` of storage.rs in the scratch copy) and ran:
   - `cargo fmt --all --check`: passed.
   - `cargo clippy -p bohmian-code-review-pipeline --all-features --all-targets -- -D warnings`: clean.

## Verification limits

- The mise lanes (`test:pipeline`, `fmt:check`, `lints`) were not run because the worktree is untrusted. I ran the cargo equivalents above instead.
- I did not run the workspace `gate` or `test:symbolic-context:journey`. R5 touches only the pipeline crate, and no other crate depends on it.
- `snapshot.rs`, `eligibility.rs` and the TASK-003 parts of `testing.rs` are out of scope. They were covered only by the fact that the crate compiles and its tests pass.
- The review took about 12 minutes. The DRIFT audit covered identity.rs, storage.rs and persistence.rs, which R5 and the earlier storage rounds touched. It did not re-audit github.rs in depth.

```citations
[]
```
