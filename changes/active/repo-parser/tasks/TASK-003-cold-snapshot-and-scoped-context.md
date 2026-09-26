---
id: TASK-003
kind: implementation
status: proposed
spec: SPEC-repo-parser@11
depends_on: [TASK-001, TASK-002]
maps: [REQ-005, REQ-006, REQ-015, REQ-016, INV-002, INV-003, INV-005, INV-007, AC-005, AC-009]
---

## Outcome and Value

A first repository invocation builds a verified, source-free default-branch snapshot from a streamed archive. Only a complete snapshot is published; an incomplete or oversized build returns an explicit fallback result for the PR-context facade.

## Owners, Scope, Consumers, and Prohibited Changes

The outer library owns eligibility, bounded archive/blob ingest, snapshot serialization, publication order, and fallback signaling. The cold-build facade accepts only an object storage and a snapshot store, both caller-implemented, plus a caller-supplied deadline and cancellation; the library ships no storage backend and imposes no deadline maximum. This task first consumes the snapshot store, so it proves that trait's contract with an in-memory implementation. No source archive, raw blob cache, or checkout is persisted. Caller job scheduling is outside this crate.

## Approach

1. Apply supported-path, mode, size, root attributes, and LFS eligibility to the pinned tree.
2. Stream and verify eligible archive entries, falling back to bounded verified blob calls where needed.
3. Publish only validated complete snapshots before advancing the pointer; return incomplete coverage and a scoped-fallback reason otherwise.

## Ordered Implementation Scenarios

### Scenario 1 — Complete cold snapshot

**Behavior.** A fake GitHub repository produces a verified source-free snapshot; its pointer appears only after the object is complete, and a later load reproduces symbol facts without source fetches.

**RED.** Add an outer-crate cold-build journey over injected in-memory object storage and snapshot store; the absent snapshot/pointer flow fails.

**GREEN.** Build the eligible index, serialize facts and coverage, write/validate the object, then publish metadata and pointer.

**REFACTOR.** Reuse the public symbolic facade and provider identities; keep publication as one observable boundary.

### Scenario 2 — Incomplete build outcome

**Behavior.** Truncated tree, archive mismatch, LFS pointer, parse failure, or exhausted bound never advances a complete pointer; the build result carries exact omissions and a reason for the PR-context facade to use scoped fallback.

**RED.** Add negative fake-provider cases matching S10–S11 and an object-write crash case; the initial result wrongly publishes or hides coverage.

**GREEN.** Enforce bounds and incomplete-attempt results; preserve an existing pointer and permit whole-build retry.

**REFACTOR.** Keep eligibility and coverage accounting reusable by the later scoped PR path without treating incomplete input as full repository coverage.

### Scenario 3 — Snapshot-store isolation and cleanup

**Behavior.** With two tenant/installation identities, one identity can neither read nor advance the other's pointer; a missing identity fails closed; interruption after object write is retry-safe. Collection removes expired unreferenced snapshots and never a pointed-to one; repository namespace removal returns its object keys for deletion.

**RED.** Add cold-build and cleanup cases over the in-memory snapshot store for cross-identity access, interruption after write, and a referenced versus unreferenced expired snapshot; they fail before isolation and cleanup exist.

**GREEN.** Enforce identity and object-before-pointer through the snapshot store and expose snapshot cleanup; rerun Scenarios 1–2.

**REFACTOR.** Keep publication and cleanup checks at the one publication boundary.

## Acceptance Criteria

The cold-build portions of S1 and S11 pass: complete source-free snapshot, verified archive/fallback blobs, immutable object and safe pointer publication. Incomplete attempts and crashes leave the prior pointer intact with exact coverage and retry/fallback reasons. The snapshot-store portion of S14 and AC-009 passes against an in-memory implementation separate from any context store. The later library journey proves the composed first-PR, scoped-review and independent-clock outcomes.

## Expected Write Set and Consumer Closure

Outer crate ingest/snapshot behavior and focused tests, injected test capabilities, scoped lane and relevant lockfile. TASK-004 and TASK-005 consume immutable snapshots.

## Verification and Evidence

Run the outer crate's scoped lane, `mise run test:symbolic-context:journey`, `mise run fmt:check`, and `mise run lints`; record stored object/row inspection and paused-clock behavior.

## Material Stop Conditions

Stop if the chosen storage operation cannot make incomplete objects invisible or if coverage cannot distinguish excluded from missing eligible files.

## Authority Links

[Revision 11 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md).
