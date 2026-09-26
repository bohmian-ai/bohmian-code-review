# TASK-002 task review r7, reviewer B (acceptance audit)

Subject: detached worktree `/tmp/cr-r7/wt`, HEAD `9fab199dfb282d07186e9f66ec3b3686328421e7` (verified at start and end). Base `1842eaf`. Focus: R5 (`6c0e386`) and everything it touches, plus the cumulative TASK-002 and Revision 11 obligations.

**Overall: PASS.** No findings.

## Verification run (scratch copy `/tmp/cr-r7/scratch-b`; mise refused the untrusted config, so the lane commands from `mise.toml` were run directly)

| Command | Result |
|---|---|
| `cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` | 23 passed, 0 failed |
| `cargo test --locked -p bohmian-code-review-pipeline --doc --all-features` | 0 doctests, ok |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | Finished, no warnings |
| `bash scripts/check-test-placement.sh` | exit 0 |
| `cargo doc --no-deps -p bohmian-code-review-pipeline --all-features` | no pipeline warnings |
| Mutation: remove the `raw != "."` and `raw != ".."` lines from `is_key_segment` | 4 tests fail: `identities_validate_key_safe_spellings` (identity.rs:408), `context_ids_are_validated_on_construction_and_deserialization`, `keys_are_revalidated_on_deserialization`, `records_revalidate_their_keys_on_deserialization`. The new assertions discriminate. Reverted afterwards. |
| `grep -rnE 'trait Persistence\b' crates` | no match (exit 1) |
| `git diff --stat 7983907 6c0e386` | code files touched: only `identity.rs` and `storage.rs` |

## Matrix

| Obligation | Implementation evidence | Verification evidence | PASS/FAIL |
|---|---|---|---|
| AC-R5-1: `TenantId::new(".")`/`("..")` return `Err(Tenant)`; serde fails too | `identity.rs:57-65` `is_key_segment` refuses `.`/`..`; `TenantId::new` (:82-87) routes through it; `Deserialize` (:102-105) delegates to `new` | `identities_validate_key_safe_spellings` loop over `[".", ".."]` asserts both; the mutation makes it fail | PASS |
| AC-R5-2: dot-only tenant keys in `hostile_keys`, refused as `ObjectKey`, `SnapshotPointer.key`, `ContextRecord.key` | `storage.rs:254-255`; `canonical` (:122) calls `TenantId::new` | `keys_are_revalidated_on_deserialization`, `records_revalidate_their_keys_on_deserialization` pass, and fail under the mutation | PASS |
| AC-R5-3: docs and message state the dot-only rule; the storage invariant (:44-46) is true | `identity.rs:21-23`, `:80-81`. `prefix` gets its tenant only from a validated `TenantId`, and reload re-validates through `new` | Read; `cargo doc` clean | PASS |
| AC-R5-4: every existing test passes, including `long_constructor_segments_round_trip` | 128-byte tenant is still inside `MAX_TENANT_BYTES` | 23/23 pass | PASS |
| R5 constraint: key layout, constructors, `canonical`, `Serialize`, R4 behavior unchanged; TASK-003 files untouched | Diff is limited to `identity.rs` (+validator, docs, tests) and `storage.rs` (+2 fixtures) | `git diff --stat 7983907 6c0e386` | PASS |
| R5 constraint "keep `is_key_safe`" | Replaced by `is_key_segment`, which keeps the same alphabet and folds in the empty, length and dot checks. Evidence cites a superseding user instruction (AGENTS.md: the current user instruction takes precedence) | `validate_name` behavior is identical: the old inline checks equal the new predicate. Deleting the duplicated checks is the ladder-preferred shape | PASS (documented deviation, no behavior drift) |
| Sibling sweep: every free-text key segment refuses dot-only values | tenant → `is_key_segment`; owner/name/context id → `validate_name` → `is_key_segment`; extraction version → hex digits only; format version → `u32`; tree → `ObjectSha`; installation/repository → nonzero integers | Read of `storage.rs:106-145` | PASS |
| TASK-002: both hosts through configuration (REQ-004) | `GitHubHost::public()`/`enterprise()` | `hosts_expose_api_base_and_archive_host`, archive redirect test | PASS |
| TASK-002 S1: verified identities and blob bytes; mismatches and untrusted redirects rejected | `github.rs` identity comparisons, `git_blob_sha`, `trusts_archive_host` | `verified_provider_identities_cross_and_mismatches_are_rejected`, `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` | PASS |
| TASK-002 S2: rate, retry-after, cancellation and deadline are typed | `send_with_policy`, `GitHubError::disposition` | `rate_limits_retry_within_bounds_or_report_typed_outcomes`, `malformed_payloads_and_interruptions_are_explicit`, `check_reports_cancellation_then_deadline` | PASS |
| Rev 11: `Persistence` split into `SnapshotStore` (lookup, conditional publish, own removal) and `ContextStore` (publish, exact lookup, expired listing, delete, own removal); shared `PersistenceError`; no pool or consumer type; `ObjectStorage` unchanged | `persistence.rs:134-227`; `lib.rs:37-40` | grep `trait Persistence\b` → none; single `impl SnapshotStore for MemorySnapshotStore`, no type implements both | PASS |
| Rev 11 consumer closure (compile-only edits to TASK-003 files) | `snapshot.rs` `ColdBuild` takes `&dyn SnapshotStore`; `testing.rs` `MemorySnapshotStore` | snapshot tests compile and pass | PASS |
| INV-007: no unscoped or traversing keys | Constructors plus `parse`/`canonical` byte-for-byte check | 20 hostile strings refused | PASS |
| Rustdoc and `# Errors` on changed items | `is_key_segment` documented; `TenantId::new` `# Errors` updated | Read; `cargo doc` clean | PASS |
| DRIFT check: prior-round code made redundant by R5 | `is_key_safe` and the duplicated inline checks in `validate_name` were removed, not left beside the new rule. No remaining duplicate of the segment rule (grep: only `TenantId::new` and `validate_name` call `is_key_segment`) | clippy `-D warnings` shows no dead code | PASS |

## Proposed findings

None. I tried to falsify each criterion through reachable paths: construction, serde of `TenantId`/`ObjectKey`/`SnapshotPointer`/`ContextRecord`, the context-id route, and the other key segments. Every path refuses dot-only segments, and constructor output still round-trips. The one deviation from the letter of R5 (replacing `is_key_safe`) keeps behavior the same, removes duplication, and is recorded against a user instruction.

The r6 non-goals C2 (version-segment decode round-trip) and C3 (`ureq::Agent` field path) are still out of scope and not reported.

```citations
[]
```
