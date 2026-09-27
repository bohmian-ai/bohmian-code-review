---
id: TASK-007
kind: implementation
status: proposed
spec: SPEC-repo-parser@13
depends_on: [TASK-001, TASK-002, TASK-003, TASK-004, TASK-005]
maps: [REQ-004, REQ-008, REQ-009, REQ-010, REQ-011, REQ-012, REQ-013, REQ-017, REQ-018, INV-001, INV-002, INV-003, INV-004, INV-005, INV-006, INV-007, AC-002, AC-003, AC-004, AC-006, AC-007, AC-008, AC-010, AC-011]
---

## Outcome and Value

The complete pipeline is proven from this repository through separate cold-build, update, PR-context and read facades, including a configurable run against an existing PR in a real large GitHub repository.

## Owners, Scope, Consumers, and Prohibited Changes

All code, test commands and evidence for this task live in this repository. Normal CI uses fake GitHub and injected test dependencies. The real-provider contract and PR runs are explicitly invoked, credentialed, read-only and excluded from `mise run gate`. No consumer service, schema, route, job or worker is a prerequisite. Final task and change reviews happen after task completion under the repository lifecycle.

## Approach

1. Call each public facade separately, then compose a cold PR, update, warm PR and read-back with the same injected capabilities.
2. Complete S1–S19 and from-scratch/property oracles, including negative coverage, rate, identity and crash cases.
3. Add a dedicated configurable S20 command, defaulting to the existing `bohmian-ai/wyrd` PR #91, that calls this library directly with real GitHub and injected in-memory object storage, snapshot store and context store.
4. Run the controlled provider contract check and fixed-commit 1k/10k/50k benchmarks; record results and any needed remediation.

## Ordered Implementation Scenarios

### Scenario 1 — Composed public library journey

**Behavior.** Separate calls build a complete snapshot, advance it for a new default-branch tree, build pinned PR context and read identical selected spans by ID. Cold and warm calls use different source budgets, and incomplete input never claims full coverage. A cold build dropped by the caller's `tokio::time::timeout` leaves no pointer. The next PR-context call then uses scoped fallback without consuming the review's own timeout (AC-002). A retried cold build converges and publishes.

**RED.** Add a public outer-crate async integration journey over fake GitHub and the injected test capabilities. It fails before composition because there is no valid pointer, context ID, or read-back equivalence.

**GREEN.** Close only cross-stage seams and missing behavior found by the journey while retaining the stage owners from earlier tasks.

**REFACTOR.** Keep one identity and coverage result through the facades. Do not add an all-in-one production method, a detached task, or a library deadline.

### Scenario 2 — Real existing PR

**Behavior.** With configured GitHub credentials, an on-demand command reads an existing PR in the real `bohmian-ai/wyrd` repository, invokes the library facades directly, analyzes a supported changed file, publishes nonempty selected context and reads it back by ID. Missing credentials or an inaccessible PR fail visibly; GitHub is never mutated. The command bounds each facade call with its own `tokio::time::timeout`.

**RED.** Add the explicitly invoked S20 integration test; it initially cannot produce a verified context ID and read-back through real GitHub.

**GREEN.** Use the same production facades with injected in-memory object storage, snapshot store and context store, recording PR URL/base/head, coverage, omissions, spans, object/metadata identity, calls, elapsed time and memory.

**REFACTOR.** Keep the command a thin configurable entrypoint to existing facades, not a second implementation of the pipeline.

## Proof Strategy

The controlled provider contract and benchmarks measure existing behavior, so no manufactured RED applies. They remain explicitly invoked evidence runs outside normal CI. Failing results return to the owning task or a reviewed remediation task.

## Acceptance Criteria

S1–S19, the complete-coverage from-scratch oracle, independent facade calls and composed journey pass in this repository. S20 passes at least once against an existing real PR with nonempty selected spans and read-back. The provider contract and reproducible benchmark reports are recorded. No consuming repository change or hosted shadow run is required. Final reviews remain separate lifecycle gates.

## Expected Write Set and Consumer Closure

Outer-crate public integration tests, fake-provider/injected-adapter support, scoped `mise` lanes, on-demand real-PR command and recorded evidence. No files outside this repository are required.

## Verification and Evidence

Run `mise run gate`, the new focused pipeline journey, and the explicitly invoked provider contract, S20 and benchmark commands; record exact commands and reports. S20 must not be added to normal CI.

## Material Stop Conditions

Stop if the four public facades cannot compose under the approved injected contracts, if real GitHub invalidates a pinned-source assumption, or if large-repository bounds require a material change to coverage or bounds.

## Authority Links

[Revision 12 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md), [library design](../../../../architecture/bohmian-code-review-design.md).
