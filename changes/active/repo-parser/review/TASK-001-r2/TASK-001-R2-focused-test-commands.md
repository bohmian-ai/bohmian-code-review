---
id: TASK-001-R2
kind: remediation
status: review
spec: SPEC-repo-parser@10
remediates: TASK-001
findings: [FIND-TASK-001-2]
---

## Subject

- Approved spec: `changes/active/repo-parser/spec.md` (revision 10)
- Original task: `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md`
- Reviewed candidate: `1108731565fc8801b47f286bd04351a3c467cf90` (base `9715440d6969dd317a62bd7c2a4ff30082f6082e`), cumulative over TASK-001 and TASK-001-R1
- Review: `changes/active/repo-parser/review/TASK-001-r2/verdict.md`

## Issue diagnosis — FIND-TASK-001-2

- **Root cause:** the implementer recorded TASK-001's verification at `mise run` lane granularity while naming eight specific Rust tests as the acceptance evidence for REQ-001, REQ-002, and REQ-003.
- **Defect class:** missing required field in a spec-driven document (a named test without its exact focused command and observed selection).
- **Sibling locations:** `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md:73`, `:74`, `:75` (the evidence rows naming the tests) and `:78` (the lane-only "Commands run in this session" line). No other site: `TASK-001-R1-task-status.md` names no Rust test, and `rg -n 'mise exec' changes/active/repo-parser/tasks/` returns nothing.
- **Violated obligation:** `architecture/references/languages/spec-driven-development.md:183-184` ("Every named test in a task must also carry its exact focused command") and `:196-200` ("the implementer then records the exact focused command. A named test cannot be replaced by only a package-wide `mise run` lane"); `architecture/references/languages/testing-workflows.md:17-18`; `architecture/references/languages/implementation-execution.md:241-242`. The exception at `:197-198` (a new task may name only its owner lane) ended when the Implementation Evidence named the tests.
- **Current behavior:** line 78 lists `mise run test:extract`, `test:index`, `test:select`, `test:resolve`, `test:symbolic-context:journey`, `fmt:check`, `lints`, `git diff --check`, `mise run gate`; no `mise exec -- cargo test --locked … -- --exact` command appears anywhere in the file.
- **Evidence:** the diff hunk in `git diff 9715440d..1108731 -- changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` adds the whole Implementation Evidence section, including lines 73-78.
- **Reproduction:** `rg -n 'mise exec' changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` prints nothing and exits 1.
- **Observable consequence:** the record cannot show that any named test was individually selected and passed. A reader or tooling following the reference has no replayable per-test command, and a later regression in one named test could hide behind a green package-wide lane. The identical gap was already treated as blocking in this repository (FIND-TASK-002-2, `changes/active/review-helper-setup/review/TASK-002-20260925-01/`).
- **Why the candidate falls short:** `mise run gate` does not validate task evidence, so a green gate proves nothing about this contract. The code is correct: the review derived every selector from source and ran each one; all nine passed. The defect is confined to the evidence record.

## Intended outcome

TASK-001's Implementation Evidence carries, for every Rust test it names, the exact `mise exec -- cargo test --locked -p <crate> --<target> <selector> -- --exact` command and the observed result, so each per-criterion proof is replayable from the document alone.

## Decision-complete recommendation

Append a "Focused commands" table to the Implementation Evidence section of `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md`, directly after the "Commands run in this session" line (line 78 at the candidate). Use exactly these nine rows; package names come from each crate's `Cargo.toml`, module paths and test names from source, and every selector was verified by the review to select exactly one test:

| Command | Observed |
|---|---|
| `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey exported_facts_reload_without_source_and_reject_invalid_records -- --exact` | 1 passed |
| `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey path_language_identifies_eligible_paths_before_download -- --exact` | 1 passed |
| `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only -- --exact` | 1 passed |
| `mise exec -- cargo test --locked -p bohmian-code-review-index --lib tests::export_round_trips_through_load_and_rejects_invalid_records -- --exact` | 1 passed |
| `mise exec -- cargo test --locked -p bohmian-code-review-extract --lib extractor::tests::path_language_reports_eligibility_without_source -- --exact` | 1 passed |
| `mise exec -- cargo test --locked -p bohmian-code-review-select --lib tests::multi_file_ranking_unions_names_and_excludes_every_changed_path -- --exact` | 1 passed |
| `mise exec -- cargo test --locked -p bohmian-code-review-select --lib tests::candidate_packing_counts_only_selected_candidates -- --exact` | 1 passed |
| `mise exec -- cargo test --locked -p bohmian-code-review-core --lib facts::tests::projection_is_source_free_and_rebuilds_a_node -- --exact` | 1 passed |
| `mise exec -- cargo test --locked -p bohmian-code-review-core --lib facts::tests::validation_rejects_empty_identity_and_bad_spans -- --exact` | 1 passed |

Re-run each command sequentially before recording it and record what was observed. Label the table as current GREEN closure; do not retroactively claim these commands were used for RED (the recorded RED, a compile failure of the journey target, is accurate and stays as written). Line 73's `facts::tests` is a module name; recording both of its tests is the conservative closure.

Rejected alternatives: replacing the named tests with lane names only would remove the per-criterion proof the reference requires; adding an evidence validator to `mise run gate` is new tooling outside this task; editing the reference to permit lane-only evidence would change an approved authority to fit a document.

## Constraints, preserved behavior, non-goals

- Docs-only change to the Implementation Evidence section of the task file. The six crates, tests, manifests, lockfile, grammar pins, and fixtures are untouched.
- Do not change frontmatter (status stays `review`), the acceptance table rows, the RED/GREEN narrative, or any other section.
- Non-goals: do not address the review notes (`EXTRACTION_VERSION` maintenance, `DeclarationFact::validate` kind check, double UTF-8 decode, `max_files == 0` asymmetry) or the pre-existing `status: draft` on `changes/active/repo-parser/spec.md`.

## Acceptance criteria

- AC-R2-1 (FIND-TASK-001-2, lines 73-75): every test named in the evidence table has a matching row in the new table; `rg -c 'mise exec -- cargo test --locked' changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` prints at least 9.
- AC-R2-2 (FIND-TASK-001-2, line 78): each of the nine commands, run sequentially, exits 0 and reports `1 passed`.
- AC-R2-3 (no collateral change): `git diff --stat 1108731..HEAD -- . ':!changes/active/repo-parser/review'` lists only the task file, and the diff touches only the Implementation Evidence section.

## Proof

- Focused: the commands in AC-R2-1 and AC-R2-2, with observed output recorded in this task's evidence section.
- Broader: `mise run gate` exit 0 and `mise run check:skills-sync` exit 0 (a docs-only change must not disturb either); `git diff --check` clean.

Route to `$cr-implement`. The next task review reassesses the cumulative candidate against the original task.

## Implementation Evidence

Commit `cbf517a` on `review-helper-setup` inserts a "Focused commands" table (14 lines) after the "Commands run in this session" line of the task's Implementation Evidence; no other line changed.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| AC-R2-1 every named test has a focused-command row | `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md`, Implementation Evidence, "Focused commands" table (nine rows covering all eight named tests plus both `facts::tests` tests) | `rg -c 'mise exec -- cargo test --locked' <task file>` prints `9` | PASS |
| AC-R2-2 each command exits 0 with `1 passed` | the nine rows as recorded | each `mise exec -- cargo test --locked … -- --exact` run sequentially in this session before recording: `1 passed; 0 failed`, exit 0 | PASS |
| AC-R2-3 no collateral change | frontmatter, acceptance rows, RED/GREEN narrative untouched | `git diff --stat 1108731..cbf517a -- . ':!changes/active/repo-parser/review'` lists only the task file, 14 insertions, 0 deletions | PASS |

Commands run in this session, each exit 0: the nine focused commands above, `mise run gate`, `mise run check:skills-sync`, `git diff --check`.

Non-goals stayed excluded: no crate, test, manifest, lockfile, grammar pin, or fixture changed; the review notes and `spec.md` status were not touched.
