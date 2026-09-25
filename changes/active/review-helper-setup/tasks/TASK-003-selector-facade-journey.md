---
id: CR-SETUP-T03
kind: implementation
status: proposed
spec: SPEC-review-helper-setup
spec_revision: 2
requirements: [REQ-003, REQ-004, REQ-005, REQ-006, REQ-007, REQ-010, INV-001, INV-002, INV-003, INV-004, INV-005, AC-003, AC-004, AC-005]
depends_on: [CR-SETUP-T01, CR-SETUP-T02]
---

# Ranked, bounded context through the public facade

## Outcome and Value

A consumer uses only `bohmian-code-review`'s synchronous public API to upsert supplied source, inspect a changed head blob, rank related files, and build context under caller-supplied limits. An external in-memory TypeScript journey exercises every real stage. Required execution skill: `$cr-implement`.

## Current Repository Facts

Tasks 01–02 supply extractor, index, and resolver owners. Selector, facade, `test:select`, and the distinct public journey lane do not exist. The selector's ranking/packing policy and facade composition share one observable output, so the public journey closes them together.

## Owners, Scope, Consumers, and Prohibited Changes

`bohmian-code-review-select` owns changed-line name collection, distinct-name scores, deterministic ranking, and budgeted packing. The facade owns stateful orchestration/public errors only; the future service supplies verified repository/commit/blob identity, paths, bytes, ranges, selected candidate blobs, limits, and tokenizer. Upsert extracts before index replacement. Preserve ambiguity, partial-coverage honesty, changed-hunk priority, whole spans, exact callback-counted tokens, and explicit omissions/errors. No duplicated stage logic, source fetch/store, hidden checkout, fuzzy match, or proven-definition claim.

## Approach

1. Establish package-scoped `test:select` and external facade `test:symbolic-context:journey` lanes.
2. Implement changed-line collection and distinct-name ranking in selector.
3. Implement changed-hunk-first context packing, limits, omissions, and typed failures in selector.
4. Compose the real owners in the facade, then prove the full public journey and negative paths.

## Ordered Implementation Scenarios

### Scenario 1 — Changed-line ranking

**Behavior.** Parse the full head blob; overlapping declarations and references on validated one-based changed ranges become names. Each distinct exact name scores another file once; positive candidates sort by descending score then canonical path, with matched reasons. Covers REQ-005, INV-002, INV-004, AC-004.

**RED.** Add a focused selector check with repeated names, off-range references, and a score tie; run `mise run test:select` and record a missing/wrong score, exclusion, or order.

**GREEN.** Implement collection/ranking in selector and rerun the focused check.

**REFACTOR.** Keep one deterministic name set and one ranking owner; avoid facade scoring.

### Scenario 2 — Budgeted context and explicit failures

**Behavior.** Changed hunks precede ranked spans; distinct files and callback-counted selected text stay within limits. Output reports selected text/ranges/reasons/tokens and omissions without mid-span cuts. Invalid path/ranges, deletion-only input, unfittable changed hunk, and missing selected blob fail explicitly. Covers REQ-006, INV-003, AC-004.

**RED.** Add focused selector checks for a one-secondary-span budget and required invalid/missing cases; run `mise run test:select` and record the missing selection, count, or typed failure.

**GREEN.** Implement packing/validation in selector; rerun Scenario 1 and focused negatives.

**REFACTOR.** Keep one accounting path for token and file limits; reuse core validation where appropriate.

### Scenario 3 — Transactional facade upsert

**Behavior.** One supplied file yields extracted symbols/references/diagnostics and byte length without retained source. A malformed/unsupported replacement preserves old complete facts and unrelated files; remove is idempotent. Covers REQ-003, REQ-010, INV-001, INV-003, AC-003.

**RED.** Add an external public-API check for successful upsert, rejected replacement, and repeated removal; run `mise run test:symbolic-context:journey` and record missing composition after the target exists.

**GREEN.** Compose extractor and index so failure precedes mutation; rerun owner lanes and earlier scenarios.

**REFACTOR.** Leave extraction, postings, and candidate algorithms with their owners.

### Scenario 4 — Complete public two-pass journey

**Behavior.** In-memory `src/handler.ts`, `src/user.ts`, `src/format.ts`, and `src/unrelated.ts` produce user score 2, format score 1, unrelated absent; a one-secondary-span budget selects the user span with correct text/range/reasons/tokens. Repeated ranking/context are identical; unfittable budget and missing blob fail explicitly. Covers REQ-007, INV-004, AC-005.

**RED.** Add the external test through only public facade APIs and real extractor/index/resolver/selector; run `mise run test:symbolic-context:journey` and record the missing/wrong public result.

**GREEN.** Complete facade composition and public output/error propagation; rerun all earlier scenarios and owner lanes.

**REFACTOR.** Remove policy duplication from the facade while keeping the public test green.

## Acceptance Criteria

- Changed-line filtering, distinct-symbol scores, canonical ties, matched reasons, limits, omissions, and typed negative outcomes match the approved spec.
- The facade exposes one-file upsert/remove/lookup/rank/context without source retention, hidden IO, or partial-coverage claims; failed extraction preserves old index state.
- A real external public-API journey proves the exact four-file TypeScript ranking, selected content/token count, repeat determinism, and budget/missing-blob errors without mocks or internal shortcuts.

## Expected Write Set and Consumer Closure

Likely surfaces: `crates/bohmian-code-review-select/**`, `crates/bohmian-code-review/**` including its external journey target, their manifests, and `test:select`/`test:symbolic-context:journey` in `mise.toml`. Lower-owner corrections require demonstrated public integration defects and updated owner proof. No Git adapter, shared store, provider SDK, or review model belongs here.

## Verification and Evidence

Run `mise run test:select`, `mise run test:symbolic-context:journey`, all upstream owner lanes, `mise run fmt:check`, `mise run lints`, and `git diff --check`; inspect tracked/untracked diff. Record each RED/GREEN cycle, exact public outputs/errors, and acceptance-to-evidence closure. The journey is mandatory; component tests cannot substitute.

## Material Stop Conditions

Return `SPEC_REVISION_REQUIRED` if correctness requires changing no-clone trust, crate ownership, completeness/resolution claims, scoring, file/token limits, or public error semantics. Do not decide these in the facade.

## Authority Links

`changes/active/review-helper-setup/spec.md`; `AGENTS.md`; `architecture/agent-rules.md`; `architecture/bohmian-code-review-design.md`; `architecture/references/domain/symbolic-context.md`; `architecture/references/languages/spec-driven-development.md`; `architecture/references/languages/implementation-execution.md`; `architecture/references/languages/testing-workflows.md`.
