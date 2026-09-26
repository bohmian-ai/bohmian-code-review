---
id: TASK-001
kind: implementation
status: proposed
spec: SPEC-repo-parser@10
depends_on: []
maps: [REQ-001, REQ-002, REQ-003, INV-001, INV-002, INV-004, AC-001]
---

## Outcome and Value

The existing public library can export and reload source-free symbol facts, identify extraction compatibility, rank a multi-file PR, and pack candidate-only spans. The existing single-file journey still works.

## Owners, Scope, Consumers, and Prohibited Changes

The six current crates retain their owners and IO-free boundaries; the public `bohmian-code-review` facade is the consumer contract for the outer pipeline. Update `AGENTS.md`, `architecture/agent-rules.md`, and `architecture/bohmian-code-review-design.md` to distinguish those six crates from the newly approved outer IO-capable library before implementation. Do not add GitHub, storage, database, or model code to the six crates; do not change grammar pins or goldens to obtain a pass.

## Approach

1. Record the approved outer-crate boundary in the repository authorities.
2. Add the minimum persistable facts, extraction identity, and path eligibility behavior.
3. Extend ranking and packing through the existing owners, then exercise both public journeys.

## Ordered Implementation Scenarios

### Scenario 1 — Source-free facts round trip

**Behavior.** Exported facts contain no source fragments; loading them in a fresh index reproduces lookup and ranking, while invalid facts fail explicitly.

**RED.** Extend the public facade journey with export/load equivalence and invalid-input assertions; `mise run test:symbolic-context:journey` fails before the capability exists.

**GREEN.** Add the required projection and load path through core/index/facade; rerun the same journey.

**REFACTOR.** Reuse existing path validation and postings replacement while preserving the journey result.

### Scenario 2 — Multi-file candidate context

**Behavior.** Changed-line names from multiple files contribute once per distinct name, every changed path is excluded, ties are deterministic, and candidate-only packing obeys file/token limits without changed hunks.

**RED.** Extend `crates/bohmian-code-review/tests/symbolic_context_journey.rs` with a two-file case and a budget failure; `mise run test:symbolic-context:journey` fails on missing behavior.

**GREEN.** Add path eligibility/version exposure, multi-file ranking, and candidate-only packing through the existing owners; keep the single-file flow green.

**REFACTOR.** Share name collection and scoring with the single-file path instead of duplicating selector logic.

## Acceptance Criteria

REQ-001–REQ-003 and the public journey pass; loaded facts reproduce results, unsupported paths are identified before download, extraction changes invalidate old facts, and existing APIs retain their behavior.

## Expected Write Set and Consumer Closure

Current six crate owners, their nearest tests, the public journey, repository architecture documents, workspace manifest and lockfile only as needed. The outer pipeline consumes only the public facade.

## Verification and Evidence

Run `mise run test:extract`, `mise run test:index`, `mise run test:select`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `mise run gate`; record RED/GREEN and the public journey result.

## Material Stop Conditions

Stop if source-free facts cannot reproduce required selection without persisting source text, or if a grammar behavior change requires altering pinned fixtures.

## Authority Links

[Revision 10 spec](../spec.md), [library design](../../../../architecture/bohmian-code-review-design.md), [agent rules](../../../../architecture/agent-rules.md).
