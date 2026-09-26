---
id: TASK-001-R1
kind: remediation
status: proposed
spec: SPEC-repo-parser@10
remediates: TASK-001
findings: [FIND-TASK-001-1]
---

## Subject

- Approved spec: `changes/active/repo-parser/spec.md` (revision 10)
- Original task: `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md`
- Reviewed candidate: `34f4ae71a45f4e6d150c26aab680484519708f07` (base `9715440d6969dd317a62bd7c2a4ff30082f6082e`)
- Review: `changes/active/repo-parser/review/TASK-001-r1/verdict.md`

## Issue diagnosis — FIND-TASK-001-1

- **Root cause:** the implementer recorded a task lifecycle status (`implemented`) that the spec-driven document contract does not define.
- **Defect class:** enumerated-value violation in a spec-driven document.
- **Sibling locations:** none. `rg -n 'status: implemented' changes/` returns only the one hit below; every other task in `changes/` uses an in-set value.
- **Violated obligation:** `architecture/references/languages/spec-driven-development.md:171-173` — task status moves through `proposed`, `ready`, `in_progress`, `review`, `approved`, with `superseded` for invalidated tasks.
- **Current behavior:** `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md:4` reads `status: implemented`.
- **Evidence:** diff hunk in `git diff 9715440d..34f4ae71 -- changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` changes `status: proposed` to `status: implemented`.
- **Reproduction:** `sed -n 4p changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` prints `status: implemented`.
- **Observable consequence:** the task cannot be placed in a defined lifecycle phase; the next transition (`approved` on PASS) has no defined predecessor, and any tooling or reader following the reference cannot interpret the document.
- **Why the candidate falls short:** the candidate's own evidence section does not address document status; `mise run gate` does not validate task frontmatter, so a green gate does not prove this contract.

## Intended outcome

TASK-001's frontmatter carries an in-set lifecycle status that reflects its real state: implementation complete and the immutable candidate under independent task review.

## Decision-complete recommendation

Change line 4 of `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` from `status: implemented` to `status: review`. `review` is the enumerated phase for a completed implementation awaiting a verdict (spec-driven-development.md:171), and the cr-implement lifecycle routes the candidate to task review at exactly this point. Rejected alternatives: `in_progress` misstates that implementation is complete; `approved` is the reviewer's transition, not the implementer's; adding `implemented` to the reference would change an approved authority to fit a document, which is out of scope. No other line of the task file and no code changes.

## Constraints, preserved behavior, non-goals

- Docs-only change; the six crates, tests, manifests, lockfile, grammar pins, and fixtures are untouched.
- Do not edit the "Implementation Evidence" section or any other frontmatter key.
- Non-goals: do not address the review notes (`EXTRACTION_VERSION` coupling, `DeclarationFact::validate` kind check) or the pre-existing `status: draft` on `changes/active/repo-parser/spec.md`; those are recorded in the verdict and are not remediation.

## Acceptance criteria

- AC-R1-1 (FIND-TASK-001-1): `sed -n 4p changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` prints exactly `status: review`.
- AC-R1-2 (sibling sweep): `rg -n 'status: implemented' changes/` exits 1 with no matches.
- AC-R1-3 (no collateral change): `git diff --stat <candidate>..HEAD` lists only the task file with a one-line change.

## Proof

- Focused: the two commands in AC-R1-1 and AC-R1-2, with observed output recorded in the task's evidence section.
- Broader: `mise run gate` exit 0 and `mise run check:skills-sync` exit 0 (docs-only change must not disturb either).

Route to `$cr-implement`. The next task review reassesses the cumulative candidate against the original task.
