---
id: TASK-003-R1
kind: remediation
status: proposed
spec: SPEC-cochange-eval
spec_revision: 2
requirements: [REQ-003, AC-001]
depends_on: [TASK-003]
parent_task: TASK-003
remediates: [FIND-TASK-003-1]
---

# Make the reported rerun command executable

## Subject and diagnosis

Approved spec: `changes/active/cochange-eval/spec.md` revision 2. Original task: `changes/active/cochange-eval/tasks/TASK-003-reports.md`. Review base: `61d6df683b0cbe7aee34b819012e9c10c9f99802`; candidate: `b75079fa2c32710924a230c224eda8b55aeb456e`.

`FIND-TASK-003-1` is an output-contract defect. The report producer at `crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs:574` calls `GITHUB_TOKEN=<read-only token> mise run live:context-relevance` the exact rerun command. Its test at `:918` asserts the same string, and all three reports at `changes/active/cochange-eval/reports/{pydantic-ai,tokio,zod}.json:8` contain it. In `zsh`, `<read-only` is input redirection: `zsh -c 'GITHUB_TOKEN=<read-only token> /usr/bin/true'` exits 1 before executing the program. A reader therefore cannot run the recorded command, violating REQ-003 and AC-001. The current assertion checks the malformed value rather than executable behavior.

## Correction outcome and boundary

Record the existing executable `mise run live:context-relevance` command. The caller supplies `GITHUB_TOKEN` in the environment, as `live()` already requires; credentials must not enter reports. Correct the single report producer, its report-facing assertion, and the `command` field in each of the three checked-in reports. This existing `mise` task is the invocation mechanism; no new API, wrapper, configuration, or credential handling is needed.

Preserve all sample, trial, effect, cost, decision, and validity data; keep the command opt-in, read-only, and outside `gate`. Do not change production ranking, packing, persistence, public APIs, provider behavior, or historical evaluation reports. The write set is `crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs` and `changes/active/cochange-eval/reports/{pydantic-ai,tokio,zod}.json`.

## Acceptance and proof

- `FIND-TASK-003-1` closes when the report producer, the no-network report test, and all three checked-in JSON artifacts agree on `mise run live:context-relevance` and no credential placeholder remains in a `command` field.
- With an inherited test token or with no token, a shell invocation of the recorded command parses and reaches the existing task; without a token, the example must fail at its existing credential check before provider access.
- Run the exact focused report test through `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features tests::run_report_is_complete_and_source_free -- --exact`, verifying it selects and passes one test. Then run `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check`. Check the report diff to confirm only the command metadata changed.

Route this task to `$cr-implement`. A later review reassesses the original TASK-003 against the cumulative candidate.
