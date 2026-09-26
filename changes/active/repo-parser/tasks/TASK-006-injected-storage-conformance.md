---
id: TASK-006
kind: implementation
status: proposed
spec: SPEC-repo-parser@10
depends_on: [TASK-002, TASK-003, TASK-005]
maps: [REQ-014, REQ-015, REQ-016, INV-005, INV-007, AC-005, AC-009]
---

## Outcome and Value

The library's storage and persistence traits have testable identity, publication, read and cleanup semantics. Its OpenDAL storage adapter works with local, GCS, S3 and Azure backends without changing pipeline behavior.

## Owners, Scope, Consumers, and Prohibited Changes

This repository owns the traits, OpenDAL adapter and contract tests. A consuming crate may implement the same traits with its own handles. Do not add a production PostgreSQL schema, raw pool injection, server, job queue or dependency on a consumer crate. Source text may exist at rest only in selected-context objects.

## Approach

1. Prove the persistence trait with two tenant/installation identities, conditional pointer publication, exact context lookup and expiry/cleanup behavior using an injected test implementation.
2. Implement the OpenDAL storage adapter with validated tenant/installation/repository keys and identical create/read/delete behavior across four backends.
3. Exercise local and emulator-backed storage using this repository's `mise` lanes and Docker Compose fixtures.

## Ordered Implementation Scenarios

### Scenario 1 — Trait-bound identity and publication

**Behavior.** A pointer or context ID becomes visible only after its object is complete; wrong or missing tenant/installation identity cannot read or publish another identity's data; interrupted publication is retry-safe.

**RED.** Add injected-adapter contract cases for two identities, missing binding and interruption after object write; they fail before the trait implementation and publication rules exist.

**GREEN.** Enforce validated identity and object-before-metadata behavior through the narrow traits and test implementation.

**REFACTOR.** Keep publication checks in one boundary rather than repeating them in every facade.

### Scenario 2 — Four storage backends and expiry

**Behavior.** Local, GCS, S3 and Azure adapters preserve identical immutable-write, exact-read and cleanup semantics. Reads at 24 hours fail even when physical deletion is retried.

**RED.** Add backend conformance and paused-clock expiry cases; the initial adapter or read path fails them.

**GREEN.** Wire the same object contract through OpenDAL for all four configured backends and expose retryable deletion outcomes.

**REFACTOR.** Reuse the same path validation and conformance assertions across backends.

## Acceptance Criteria

S14–S15 pass through library-defined traits and injected implementations. Source text is absent from snapshot objects, metadata, logs and metrics. The four storage backends pass the same object contract. A consumer PostgreSQL implementation remains outside this library change and must preserve tenant-bound RLS and separate privileged operations.

## Expected Write Set and Consumer Closure

Outer crate traits and OpenDAL adapter, in-memory persistence test implementation, local/emulator test support, Docker Compose and scoped `mise` lanes in this repository. No files in a consuming repository are needed.

## Verification and Evidence

Run the new focused trait/backend lanes, `mise run fmt:check`, `mise run lints` and `mise run gate`; record two-identity denials, object/metadata publication order and four-backend results. The test lanes must be present before claiming a pass.

## Material Stop Conditions

Stop if an adapter cannot enforce exact-key identity and immutable publication, or if source text must be placed in metadata to satisfy the read contract.

## Authority Links

[Revision 10 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md), [library design](../../../../architecture/bohmian-code-review-design.md).
