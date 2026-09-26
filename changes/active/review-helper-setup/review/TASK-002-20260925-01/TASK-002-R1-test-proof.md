---
id: TASK-002-R1
kind: remediation
status: proposed
spec: SPEC-review-helper-setup
spec_revision: 2
parent_task: CR-SETUP-T02
remediates: [FIND-TASK-002-1, FIND-TASK-002-2]
---

# TASK-002-R1: Place owner tests and record focused proof

## Subject and diagnosis

Approved spec: `changes/active/review-helper-setup/spec.md` revision 2. Original task: `changes/active/review-helper-setup/tasks/TASK-002-index-resolver.md` at base `6c7f9d3aef831f66510bc8c9c6ad553b277f7b2d`. Reviewed candidate: `03664569e52baa96e86c71026fdfb68a5f9502b1`. Review verdict: `changes/active/review-helper-setup/review/TASK-002-20260925-01/verdict.md`. Remediation reviews the cumulative candidate, not only this task's eventual diff.

- **FIND-TASK-002-1:** `architecture/agent-rules.md` requires owner tests inline by default; an external test earns its place by crossing crate boundaries through the public facade. `crates/bohmian-code-review-index/tests/index.rs` and `crates/bohmian-code-review-resolve/tests/resolve.rs` contain the four TASK-002 owner checks, but neither calls the facade. The index tests call index/core; the resolver test calls resolver and the real index. Their location violates the repository's test layout despite passing owner lanes.
- **FIND-TASK-002-2:** `architecture/references/languages/testing-workflows.md` and `spec-driven-development.md` require exact `mise exec -- cargo test --locked` commands with package, target, and exact selector for every named Rust test, plus observed selection. `TASK-002-index-resolver.md` names four tests at lines 90–106 but records only broad owner-lane commands and abbreviated results. Those lanes prove collective passage; they do not record one-test focused proof. The historical RED attempts cannot be replayed or upgraded into an unobserved claim.

## Required outcome and correction

Move the existing complete index and resolver owner checks, including helpers and every assertion, into their respective owning `src/lib.rs` inline test modules. Preserve the resolver's real index boundary and all production API behavior. Existing Rust test machinery and the `test:index` and `test:resolve` mise lanes are the owners to reuse. The public facade journey belongs to TASK-003; it is not a substitute for relocating these owner tests.

After the move, run each of the four named tests with `mise exec -- cargo test --locked -p <owning package> --lib <actual module path>::<test name> -- --exact`. Record the four actual commands and observed one-test selection/pass results in the TASK-002 evidence record, labelled as current GREEN closure. Determine the final selector paths from the moved source and test listing. Retain the existing historical RED account as historical; do not claim those exact commands were used for RED.

This correction uses the existing crates, Cargo targets, and task evidence. It requires no new dependency, test harness, public API, architecture, or product decision.

## Constraints and non-goals

Preserve exact lookup, ambiguity, replacement/removal, fact-only retention, test assertions, crate ownership, and source-free runtime behavior. Do not change extractor fixtures, generated artifacts, grammar pins, or the separate `IMPLEMENTATION_PLAN.md` working-tree edit. Do not add the public journey or facade extraction-failure transaction here; TASK-003 owns them. Do not rewrite historical evidence as though a new focused RED run occurred.

## Acceptance and proof

| Finding | Acceptance criterion | Focused proof |
|---|---|---|
| FIND-TASK-002-1 | Both owner test files are replaced by inline tests in their owning libraries, with all four test bodies and assertions preserved. | Inspect the relocated tests and absence of the external owner targets; run `mise run test:index` and `mise run test:resolve`. |
| FIND-TASK-002-2 | The TASK-002 record contains four exact final `--lib` commands and observed `1 passed; 0 filtered out` results, clearly marked as current GREEN proof. | Execute each recorded command and confirm it selects exactly its named test. |

Run `mise run fmt:check`, `mise run lints`, and `git diff --check` after the correction. Preserve the existing owner-lane results as broader verification, and submit the complete new cumulative candidate for another `$cr-task-review`.
