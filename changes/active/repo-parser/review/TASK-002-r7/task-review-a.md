# TASK-002 task review r7, reviewer A

Subject: candidate `9fab199dfb282d07186e9f66ec3b3686328421e7` (verified at start and end), base `1842eaf`. Scope: TASK-002 with its Revision 11 delta, plus remediation TASK-002-R5 (FIND-TASK-002-12, commit `6c0e386`).

## Overall result: PASS

No findings.

## Obligation matrix

| Obligation | Implementation evidence | Verification evidence | PASS/FAIL |
|---|---|---|---|
| AC-R5-1: `TenantId::new(".")` and `TenantId::new("..")` return `Err(Tenant)`, and serde refuses both | `identity.rs:82-86` `TenantId::new` → `is_key_segment` (`:57-65`) refuses `.` and `..`. `Deserialize` (`:102-105`) delegates to `new`. `TenantId(` is only constructed at `:86` | `identity::tests::identities_validate_key_safe_spellings` passes. A mutation re-admitting the dot tenants makes it fail at `identity.rs:410` | PASS |
| AC-R5-2: both dot-only tenant keys are in `hostile_keys`, and `ObjectKey`, `SnapshotPointer` and `ContextRecord` refuse them | `storage.rs` `hostile_keys` gains both keys. `canonical` validates the tenant through `TenantId::new` (`storage.rs:122`). `prefix` is only reached from an already-validated `Identity` | Under the same mutation, `storage::tests::keys_are_revalidated_on_deserialization` fails (`storage.rs:306`) and so does `persistence::tests::records_revalidate_their_keys_on_deserialization` (`persistence.rs:302`). Both pass at the candidate. The two new keys differ from valid keys only in the tenant, so they test the tenant rule and nothing else | PASS |
| AC-R5-3: the docs and the error message state the dot-only rule, and the `storage.rs:44-46` invariant holds | `identity.rs:21-23` covers the doc and `#[error]`, and `:78-81` covers `# Errors`. Every key segment (tenant, context id) and every non-segment part (numbers, hex version, sha) is validated, so the invariant holds as written | `cargo doc --no-deps -p bohmian-code-review-pipeline --all-features` produced no pipeline warnings | PASS |
| AC-R5-4: all existing tests pass, including `long_constructor_segments_round_trip` | No other behavior changed. Code diff `7983907..9fab199`: `identity.rs` and `storage.rs` only | `cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` → 23 passed. The `--doc` step ran 0 tests and exited 0 | PASS |
| R5 constraints: key layout, constructors, `canonical`, `Serialize` and the R4 host, rate-limit and redirect behavior unchanged, TASK-003 files untouched | The diff is limited to the validator, docs and fixtures. `is_key_safe` was folded into `is_key_segment` with the same alphabet. This departs from "keep `is_key_safe`", but the change is recorded as a user instruction and deletes a duplicated rule (`validate_name` no longer carries its own dot check), so it counts as a simplification, not drift | `git diff --stat 7983907..9fab199 -- crates mise.toml Cargo.*` → 2 files | PASS |
| Rev 11: `Persistence` is split into `SnapshotStore` and `ContextStore` with exactly the listed methods, `PersistenceError` is shared, and neither trait takes a pool or consumer type | `persistence.rs:134-168` (`snapshot_pointer`, `publish_snapshot_pointer`, `remove_repository`) and `:178-227` (`publish_context`, `context`, `expired_contexts`, `delete_context`, `remove_repository`) | `rg -n 'trait Persistence\b' crates` → no match. `rg -n 'impl (SnapshotStore\|ContextStore) for' crates` → one impl (`testing.rs:166`, snapshot only) | PASS |
| Rev 11: `ObjectStorage` is unchanged, and the consumer closure (`testing.rs`, `snapshot.rs`, `lib.rs`) is done | `storage.rs` trait. `lib.rs` re-exports the persistence names | The 4 `snapshot::tests` compile and pass | PASS |
| TASK-002 AC: both hosts, verified identities, blob bytes, trusted redirect, retry and deadline states, credential redaction (REQ-004/013/014, INV-003/006) | `identity.rs` `GitHubHost`, and `github.rs` client and policy (reviewed r1-r6, unchanged since R4) | The `github::tests::*` (6), `identity::tests::*` (4) and `execution::tests::*` (1) tests pass | PASS |
| INV-007: no unscoped key accepted | `ObjectKey::parse`/`canonical` round-trip byte for byte. The tenant hole is now closed | `hostile_keys` has 20 cases, all refused | PASS |
| Repository rules: fmt, lints, test placement, rustdoc on new items | `is_key_segment` has rustdoc | `cargo fmt --all -- --check` exit 0. `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` exit 0. `scripts/check-test-placement.sh` exit 0 | PASS |
| DRIFT check: no code from earlier remediation rounds has become redundant | `is_key_safe` was removed, not left as dead code. `validate_name` still serves owner, name and context id. `ContextId`'s dot test still exercises its own owner | Compiles with `-D warnings`, so there is no dead code | PASS |

## Verification notes

- mise refused the untrusted scratch config, so the lane commands from `mise.toml` (`test:pipeline`, `fmt:check`, `lints`) were run directly with cargo in `/tmp/cr-r7/scratch-a`, all with `--all-features`.
- RED reproduction was done as a mutation: `TenantId::new` was edited to re-admit `.` and `..`. Exactly the three R5 assertions failed (20 passed, 3 failed). The file was then restored.
- `mise run gate` was not run. The code change is confined to the pipeline crate, and the workspace-wide clippy run passed.

## Proposed findings

None.

```citations
[]
```
