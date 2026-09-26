---
id: TASK-004
kind: implementation
status: proposed
spec: SPEC-repo-parser@11
depends_on: [TASK-003]
maps: [REQ-007, REQ-013, INV-002, INV-003, INV-004]
---

## Outcome and Value

The update facade advances a repository pointer from one complete immutable snapshot to another after default-branch changes, while a review already using the old object remains valid.

## Owners, Scope, Consumers, and Prohibited Changes

The outer library compares tree path/blob identities, replaces changed facts, removes deleted facts, and publishes validated results. Callers decide when to invoke it and supply its deadline; it accepts only an object storage and the snapshot store. No queue or worker is part of this crate. No stale facts may survive a failed replacement; incomplete updates cannot move the pointer.

## Approach

1. Compare the current pinned branch tree with the loaded snapshot, including eligibility and version changes.
2. Reuse unchanged facts, fetch and verify changed blobs, and remove deleted/ineligible facts.
3. Publish a new object and pointer only after complete validation; report a whole-build retry reason otherwise.

## Ordered Implementation Scenarios

### Scenario 1 — Change, delete, and rename

**Behavior.** A push changing, deleting, and renaming files yields facts equivalent to a fresh parse of the new tree; old snapshot bytes remain unchanged.

**RED.** Add a fake-provider update journey with a from-scratch oracle; the missing update facade or stale facts fail it.

**GREEN.** Implement tree comparison and immutable publication; rerun the oracle and cold-build journey.

**REFACTOR.** Share the eligibility and verified blob path with cold ingest.

### Scenario 2 — Moving head and incomplete update

**Behavior.** A force-push or changed branch head is detected from tree identity; failed changed blobs leave the old pointer intact, and retrying a whole build can publish a complete result. Repeated calls converge on the current tree.

**RED.** Add paused/racing branch responses and changed-blob failure to the update journey; assert pointer identity and explicit coverage.

**GREEN.** Use conditional pointer publication and return a retry state when another invocation has advanced the pointer; do not hold a snapshot-store transaction across provider or storage IO.

**REFACTOR.** Keep version and pointer checks at the publication boundary rather than duplicating them in callers.

## Acceptance Criteria

S3–S4, S8–S9, S16–S17 pass at the facade boundary: changed/deleted/renamed facts match a fresh parse, old snapshots are immutable, incomplete attempts do not move the pointer, and a moving branch head requests another update. A review-time stale-pointer check returns a typed result without blocking the current review.

## Expected Write Set and Consumer Closure

Outer crate update facade, focused fake-provider tests and caller-facing result contract. No consumer code is required.

## Verification and Evidence

Run the outer crate's scoped lane, `mise run fmt:check`, `mise run lints`, and `mise run gate`; record old/new snapshot bytes, pointer state, coverage and oracle equality.

## Material Stop Conditions

Stop if update publication can overwrite an immutable snapshot or expose a pointer before its object is validated.

## Authority Links

[Revision 11 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md).
