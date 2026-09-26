---
id: TASK-002
kind: implementation
status: proposed
spec: SPEC-repo-parser@10
depends_on: [TASK-001]
maps: [REQ-004, REQ-013, REQ-015, REQ-016, INV-001, INV-003, INV-006]
---

## Outcome and Value

The new outer library can obtain and verify GitHub/GHES repository, commit, tree, blob, and PR identities through provider APIs. It defines narrow storage and persistence traits that later stage facades and any consuming crate can implement.

## Owners, Scope, Consumers, and Prohibited Changes

The new library crate owns provider IO, trust boundaries and dependency-injection traits. Callers supply credentials, trait implementations, deadlines and cancellation. The crate imports no consumer type and runs no server or worker loop. Never clone, check out, or retain source on disk. Provider limits and credential handling must fail visibly.

## Approach

1. Add the outer crate to the workspace with domain values for verified host/installation/repository and pinned Git identities.
2. Define narrow injected storage and persistence capabilities needed by later stage facades.
3. Implement bounded GitHub/GHES source calls, authentication, response validation, and rate/error classification without duplicating symbolic algorithms.

## Ordered Implementation Scenarios

### Scenario 1 — Verified provider identities

**Behavior.** A controlled fake provider returns pinned branch/tree/blob/PR data; mismatched repository, path, SHA, or untrusted archive redirect is rejected before symbolic parsing.

**RED.** Add an outer-crate integration test that supplies valid and mismatched provider responses; it fails before verified source access exists.

**GREEN.** Implement only the source operations and identity checks needed by cold, update, and PR facades; rerun the scenario.

**REFACTOR.** Centralize the common validated identities while keeping provider-specific response handling explicit.

### Scenario 2 — Bounded provider failure

**Behavior.** Rate exhaustion, retry-after, cancellation, and deadline yield typed retry or terminal outcomes; no facade can present partial input as complete.

**RED.** Add fake-provider cases for 403/429, malformed payload, and cancellation; each initially fails its expected outcome.

**GREEN.** Add bounded request accounting and error classification; rerun the first scenario and these failures.

**REFACTOR.** Keep one provider request policy for all stages without introducing a general provider framework.

## Acceptance Criteria

Both GitHub hosts are supported through configuration; verified identities and blob bytes cross the trust boundary, and provider retry/deadline states remain explicit. TASK-003–TASK-005 implement and prove the individual facades; the integrated journey later proves their composition.

## Expected Write Set and Consumer Closure

New outer crate, workspace manifest/lockfile, provider contract tests, and a scoped `mise` lane. Later cold/update/PR tasks consume the same source and capability contracts.

## Verification and Evidence

Run the new crate's focused test lane, `mise run fmt:check`, `mise run lints`, and `mise run gate`. Record fake-provider requests and failures. The controlled real-GitHub contract run remains an integrated acceptance gate outside normal CI.

## Material Stop Conditions

Stop if GitHub/GHES cannot provide the required pinned identity or if the dependency contracts require importing a consumer crate or raw SQL pools.

## Authority Links

[Revision 10 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md), [library design](../../../../architecture/bohmian-code-review-design.md).
