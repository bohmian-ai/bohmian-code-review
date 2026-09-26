---
id: TASK-005
kind: implementation
status: proposed
spec: SPEC-repo-parser@11
depends_on: [TASK-001, TASK-002, TASK-003]
maps: [REQ-008, REQ-009, REQ-010, REQ-011, REQ-013, REQ-014, REQ-015, REQ-016, REQ-018, INV-002, INV-004, INV-005, INV-006, INV-007, AC-005, AC-009]
---

## Outcome and Value

One PR invocation overlays its pinned head on a private index, ranks related files from changed lines, stores selected source spans once, and returns a context ID that authorized consumers can read for 24 hours.

## Owners, Scope, Consumers, and Prohibited Changes

The outer library owns PR source/diff reconciliation, ranking orchestration, budgeted packing, immutable context publication, and read-time integrity/expiry checks. The six symbolic crates own extraction and scoring. Callers supply verified identity, a deadline, and implementations of the traits each facade uses: the PR-context facade takes the snapshot store, the context store, and object storage for snapshots and contexts; the read facade takes only the context store and context object storage. This task first consumes the context store, so it proves that trait's contract with an in-memory implementation. The library does not invoke a model or publish comments.

## Approach

1. Load a compatible snapshot into an invocation-private index and reconcile it with the PR head tree, or build a bounded scoped index when the cold build reports fallback.
2. Guard paginated PR files with before/after base/head reads and head blob checks; classify missing or malformed patches per file.
3. Rank and fetch only selected candidate blobs, enforce the candidate budget, then publish and read one immutable context object.

## Ordered Implementation Scenarios

### Scenario 1 — Pinned multi-file PR

**Behavior.** Concurrent PRs and a stale base each see only their own head facts; selected candidate spans and reasons equal a from-scratch head parse when coverage is complete. A failed cold build uses changed paths for bounded scoped indexing and reports scoped coverage.

**RED.** Add a fake-provider public PR-context journey for S5–S7 and a failed-cold-build scoped case from S10; the missing overlay/fallback/ranking/publication fails it.

**GREEN.** Compose the snapshot, private overlay, PR diff and existing symbolic facade; publish one validated object and tenant-scoped ID.

**REFACTOR.** Keep overlays invocation-local and reuse the symbolic selector's deterministic ranking.

### Scenario 2 — Drift, omissions, and budget

**Behavior.** A base/head change during pagination triggers retry; bad or absent patches affect only their files; exhausted budget and incomplete overlay are reported without claiming full coverage.

**RED.** Add fake-provider S18–S19 and failed-blob cases; assert no mixed-head publication, no stale facts, and explicit reasons.

**GREEN.** Add bounded retry, file-local omission, coverage and token-estimate behavior.

**REFACTOR.** Keep one coverage/budget result shape for full and scoped reviews.

### Scenario 3 — Shared read and expiry

**Behavior.** Repeated reads by ID return the same selected spans without GitHub calls before expiry; reads at expiry or for another tenant/installation fail, as do corrupt/missing objects and a missing identity. A context ID is visible only after its object validates; interruption after object write is retry-safe. Cleanup deletes expired contexts and reports and retries deletion failures.

**RED.** Add paused-clock reads for S14–S15 over the in-memory context store with two identities, interruption after write, and a failing delete; absent expiry, integrity, isolation, or cleanup checks fail.

**GREEN.** Read exact published metadata/object under supplied identity, check expiry and integrity, never rebuild from current branch, and expose expired-context cleanup.

**REFACTOR.** Keep source-bearing storage confined to the context object and drop transient source after use.

## Acceptance Criteria

S5–S7, S10, S14–S15 and S18–S19 pass with injected test capabilities, the context store exercised separately from the snapshot store (AC-009): pinned identity, private/scoped indexing, selected text, coverage, omissions, expiry and budget. The diff remains separate. The read facade enforces identity and expiry; a downstream model caller counts its complete request separately.

## Expected Write Set and Consumer Closure

Outer crate PR and read facades, focused fake-provider/storage tests and the public library journey. No consumer implementation is required.

## Verification and Evidence

Run the outer crate's scoped lane, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `mise run gate`; record stored object, metadata, read result, provider calls and paused-clock expiry.

## Material Stop Conditions

Stop if provider data cannot be tied to one recorded base/head pair, or if context can be read by ID alone without tenant and installation checks.

## Authority Links

[Revision 11 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md).
