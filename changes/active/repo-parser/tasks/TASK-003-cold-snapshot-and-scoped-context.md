---
id: TASK-003
kind: implementation
status: proposed
spec: SPEC-repo-parser@12
depends_on: [TASK-001, TASK-002]
maps: [REQ-005, REQ-006, REQ-012, REQ-013, REQ-015, REQ-016, INV-002, INV-003, INV-005, INV-007, AC-005, AC-009]
---

## Outcome and Value

The first call for a repository builds a verified, source-free default-branch snapshot from a streamed archive. Publication is three idempotent steps, so a caller can drop the build future at any `.await`. After a drop, the pointer still names a complete snapshot or nothing, and a retry converges. An incomplete or oversized build returns an explicit fallback result for the PR-context facade.

## Owners, Scope, Consumers, and Prohibited Changes

The outer library owns eligibility, bounded archive/blob ingest, snapshot serialization, publication, cleanup, and fallback signaling. The async cold-build facade accepts only caller-implemented object storage and a snapshot store. It takes no cancel token or deadline. It never detaches a task: any concurrent fetches run in a `JoinSet` owned by the call. This task first consumes the snapshot store and the object storage write/delete semantics, so it fixes their method contracts and proves them with in-memory implementations.

Non-goals: storage backends, scoped PR indexing (TASK-005), incremental update (TASK-004), persisting source, archives, or raw blobs, scheduling, and any library deadline maximum.

This task is reimplemented on the async design, not patched. The synchronous candidate on this branch (`741c049`..`d953463`, reviews `TASK-003-r1`..`r3`) is prior art, not a baseline to remediate. Its eligibility, serialization, and verification logic may be kept where it meets these criteria. `TASK-003-R3` is superseded. Scenario 3 replaces its FIND-2, and Scenario 5 replaces its FIND-7.

## Approach

1. Apply supported-path, mode, size, root `.gitattributes`, and LFS eligibility to the pinned tree.
2. Stream the commit-pinned archive, verify eligible entries against their blob SHAs, and fall back to bounded verified blob fetches where needed.
3. Publish a complete snapshot in order: upsert its record, which refreshes `touched_at`; create the object if absent; compare-and-swap the pointer. Otherwise return incomplete coverage and a fallback reason.
4. Expose `collect(identity, older_than)` and repository namespace removal.

## Ordered Implementation Scenarios

### Scenario 1 — Complete cold snapshot

**Behavior.** A fake GitHub repository yields a verified, source-free snapshot. A later call with a matching pointer reloads its facts without any archive, tree, or blob fetch (REQ-005, INV-003, INV-005). The same tree always serializes to identical bytes.

**RED.** Add an async cold-build journey over in-memory object storage and snapshot store. It fails because no snapshot, record, or pointer exists yet.

**GREEN.** Build the eligible index, serialize facts and coverage, then publish through the three steps.

**REFACTOR.** Reuse the public symbolic facade and the TASK-002 provider identities.

### Scenario 2 — Incomplete build

**Behavior.** Each of these leaves the pointer unmoved and returns exact coverage plus a reason the PR-context facade can use for scoped fallback: a truncated tree, an archive mismatch past the fallback bound, a parse failure, an exhausted archive or blob bound, or a pointer whose object is missing. An LFS pointer or a verified oversize blob is a reasoned skip that keeps its path and blob identity (REQ-005, INV-002, S10–S11).

**RED.** Add fake-provider cases for each input. They fail while the facade publishes an incomplete build or hides its coverage.

**GREEN.** Enforce the bounds and return the incomplete result.

**REFACTOR.** Keep eligibility and coverage accounting reusable by TASK-005's scoped path.

### Scenario 3 — Drop-safe, idempotent publication

**Behavior.** The build future is dropped at any publication step: after the record upsert, after the object create, or before the compare-and-swap. The pointer remains on the prior complete snapshot, or stays absent. Every written object has a record. Retrying the same tree converges on the same key and advances the pointer. An existing object under the key is adopted only after identity and byte validation. A compare-and-swap that loses to another writer returns a typed retry state (REQ-006, REQ-012, REQ-013).

**RED.** Add cases that use a test store to stall the chosen step and a paused-clock `tokio::time::timeout` to drop the future. Each asserts the pointer and records, then retries. Also cover a pre-existing object with mismatched bytes and a lost compare-and-swap. They fail before the three-step order exists.

**GREEN.** Implement record upsert with `touched_at`, create-if-absent objects, and pointer compare-and-swap. Rerun Scenarios 1–2.

**REFACTOR.** Keep publication as one path shared with TASK-004.

### Scenario 4 — Isolation and cleanup

**Behavior.** For each record in its identity that the pointer does not reference and that was not touched since `older_than`, `collect(identity, older_than)` deletes the object, then the record. A missing object counts as success. It never touches the pointed-to snapshot, a record touched after `older_than`, or another identity's records. Namespace removal returns only that repository's keys. A second tenant or installation cannot read, advance, or collect the first identity's pointer or snapshots. A missing identity fails closed (REQ-006, REQ-015, REQ-016, AC-005, AC-009).

**RED.** Add cases with two identities, a referenced and an unreferenced record, a record re-touched by an in-flight build, a record whose object is already gone, and a failing object delete. The failing delete must leave its record for the next collect. They fail before `unreferenced` and `collect` exist.

**GREEN.** Add `unreferenced(identity, older_than)` and record deletion to the snapshot store. Add create-if-absent, plus delete where a missing object is success, to object storage. Implement `collect`.

**REFACTOR.** Share the object-then-record order with TASK-005's context cleanup when it arrives, not before.

### Scenario 5 — Root `.gitattributes` resets

**Behavior.** For `linguist-generated` and `linguist-vendored`, follow Git: `attr` and `attr=true` set it; `-attr` and `attr=false` are explicit false and override the built-in vendor-segment and minified-output exclusions; `!attr` returns the attribute to unspecified, so the built-in exclusions apply again. The last matching rule wins.

**RED.** Add `eligibility::tests::attribute_resets_read_as_unset`. It covers `vendor/a.ts` and `app.min.js`: `-attr` and `attr=false` make them eligible; `!attr` after a prior `-attr` makes them excluded again. Command: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib eligibility::tests::attribute_resets_read_as_unset -- --exact`. It fails while `!attr` reads as false.

**GREEN.** Carry set, false, and unspecified as distinct states through the attribute resolver.

**REFACTOR.** None beyond keeping existing attribute tests green.

## Acceptance Criteria

- The cold-build portions of S1 and S11 pass: a complete source-free snapshot, verified archive and fallback blobs, and reload without source fetches.
- Incomplete builds leave the pointer unmoved and report exact coverage and a fallback reason.
- Dropping the facade future at any publication step leaves the pointer on a complete snapshot or absent, and leaves every object recorded. A retry converges.
- `collect` deletes the object and then the record for unreferenced records not touched since `older_than`. It treats a missing object as success and never collects the pointed-to or a re-touched snapshot.
- The snapshot-store portion of S14 and AC-009 passes against an in-memory snapshot store that is separate from any context store.
- `.gitattributes` `!attr` reads as unspecified and `-attr` as false, per Scenario 5.
- The crate calls no `tokio::spawn`.

## Expected Write Set and Consumer Closure

Outer crate eligibility, snapshot, persistence, storage, and in-memory test modules; `lib.rs` exports; lockfile only if streaming needs a dependency. TASK-004 reuses the publication path. TASK-005 consumes snapshots, `Coverage`, and the fallback reason.

## Verification and Evidence

Run the scenario tests with `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib <module::test> -- --exact`. Then run `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `mise run gate`. Record stored objects, records, and pointer state after each drop point. Confirm `rg -n 'tokio::spawn' crates/bohmian-code-review-pipeline` returns nothing.

## Material Stop Conditions

- Stop if create-if-absent or pointer compare-and-swap cannot be expressed so that an in-memory implementation and a typical object store or SQL implementation share the same semantics.
- Stop if streaming the archive asynchronously requires persisting it.
- Stop if coverage cannot distinguish excluded files from missing eligible files.

## Authority Links

[Revision 12 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md), [TASK-002](TASK-002-github-source-and-stage-contracts.md).
