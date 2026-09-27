---
id: TASK-005
kind: implementation
status: proposed
spec: SPEC-repo-parser@12
depends_on: [TASK-001, TASK-002, TASK-003]
maps: [REQ-008, REQ-009, REQ-010, REQ-011, REQ-012, REQ-013, REQ-014, REQ-015, REQ-016, REQ-018, INV-002, INV-004, INV-005, INV-006, INV-007, AC-005, AC-009]
---

## Outcome and Value

One PR invocation overlays its pinned head on a private index, ranks related files from changed lines, stores selected source spans once, and returns a context ID that authorized consumers can read for 24 hours.

## Owners, Scope, Consumers, and Prohibited Changes

The outer library owns PR source/diff reconciliation, ranking orchestration, budgeted packing, immutable context publication, and read-time integrity/expiry checks. The six symbolic crates own extraction and scoring. Callers supply verified identity and implementations of the traits each facade uses, and they bound each call by dropping its future. The library takes no cancel token or deadline and never detaches a task; concurrent blob fetches use a `JoinSet` owned by the call. The traits used by each facade are: the PR-context facade takes the snapshot store, the context store, and object storage for snapshots and contexts; the read facade takes only the context store and context object storage. This task first consumes the context store, so it fixes that trait's method contract and proves it with an in-memory implementation. The library does not invoke a model or publish comments.

## Approach

1. Load a compatible snapshot into an invocation-private index and reconcile it with the PR head tree, or build a bounded scoped index when the cold build reports fallback.
2. Guard paginated PR files with before/after base/head reads and head blob checks; classify missing or malformed patches per file.
3. Rank and fetch only selected candidate blobs, and enforce the candidate budget.
4. Publish in order: upsert the context record with `expires_at`, then create the object if absent. Read one immutable context object by ID.

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

**Behavior.** Before expiry, repeated reads by ID return the same selected spans without GitHub calls. These reads fail: at or after expiry, from another tenant or installation, for a corrupt object, and with a missing identity. A read ignores a record whose object is absent. Dropping the PR-context future after the record upsert or after the object create leaves no readable half context and no unrecorded object; a retry returns a readable context. Cleanup handles contexts past `expires_at` in the same order as snapshot collection: delete the object, where a missing object is success, then delete the record. A failed delete keeps its record for retry and is reported as lag (REQ-012, REQ-014, REQ-015).

**RED.** Add paused-clock reads for S14–S15 over the in-memory context store with two identities. Include a `tokio::time::timeout` drop at each publication step, a record whose object is absent, and a failing delete. These cases fail until the expiry, integrity, isolation, and cleanup checks exist.

**GREEN.** Read the exact published record and object under the supplied identity and check expiry and integrity. Never rebuild from the current branch. Expose expired-context cleanup in object-then-record order, reusing TASK-003's cleanup order.

**REFACTOR.** Keep source-bearing storage confined to the context object and drop transient source after use.

## Acceptance Criteria

S5–S7, S10, S14–S15 and S18–S19 pass with injected test capabilities, the context store exercised separately from the snapshot store (AC-009): pinned identity, private/scoped indexing, selected text, coverage, omissions, expiry and budget. The diff remains separate. The read facade enforces identity and expiry. A dropped PR-context call leaves no readable partial context, and a retry converges. A downstream model caller counts its complete request separately.

## Expected Write Set and Consumer Closure

Outer crate PR and read facades, focused fake-provider/storage tests and the public library journey. No consumer implementation is required.

## Verification and Evidence

Run the outer crate's scoped lane, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `mise run gate`; record stored object, metadata, read result, provider calls and paused-clock expiry.

## Material Stop Conditions

Stop if provider data cannot be tied to one recorded base/head pair, or if context can be read by ID alone without tenant and installation checks.

## Authority Links

[Revision 12 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md).
