# TASK-001 task review r2 — verdict

## Immutable subject

- Repository: `bohmian-code-review`, branch `review-helper-setup`
- Base: `9715440d6969dd317a62bd7c2a4ff30082f6082e`
- Candidate (cumulative): `1108731565fc8801b47f286bd04351a3c467cf90` — unchanged for the whole review (checked before Wave 1, before the citation check, before Wave 2, and before writing this file)
- Approved spec: `changes/active/repo-parser/spec.md` (revision 10)
- Original task: `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md`
- Prior review: `changes/active/repo-parser/review/TASK-001-r1/verdict.md` (FIX_REQUIRED, FIND-TASK-001-1)
- Remediation reviewed: `changes/active/repo-parser/review/TASK-001-r1/TASK-001-R1-task-status.md` (commit `932f6a5`)
- Diff reviewed: complete `9715440d..1108731` excluding the r1 review directory (14 files, code + authorities + task documents)

## Verdict

**FIX_REQUIRED** — one confirmed, docs-only repository-standard finding (`FIND-TASK-001-2`). Every code obligation of REQ-001, REQ-002, REQ-003, INV-001, INV-002, INV-004, and AC-001 passes; the prior finding FIND-TASK-001-1 is closed. Remediation task: `TASK-001-R2-focused-test-commands.md` in this directory.

## Acceptance matrix

Consolidated from `task-review-a.md` and `task-review-b.md` (independent samples, identical conclusions) with orchestrator-observed verification.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001 source-free facts export/load; loaded facts reproduce lookup, ranking, packing; invalid facts fail without corrupting other files | `core::facts` (`DeclarationFact`, `IndexedFileFacts`, `InvalidFact`), `SymbolIndex::export`/`load` (validate before `upsert`), `SymbolicContext::export`/`load` | journey `exported_facts_reload_without_source_and_reject_invalid_records` (equality on `rank` and `build_context`); index round-trip test; core `facts::tests`; facts domain scratch run (Debug/JSON output carries no source) | PASS |
| REQ-002 extraction identity; version mismatch refused; path-only eligibility | `extract::EXTRACTION_VERSION` (matches `tree-sitter = "=0.25.10"`), `Extractor::path_language` sharing the router/composite classifier with `extract_from_source`, `Error::ExtractionVersionMismatch` checked before index touch | journey `path_language_identifies_eligible_paths_before_download`; extract `path_language_reports_eligibility_without_source`; mismatch assertion in the facts journey; facts domain sweep across 11 paths | PASS |
| REQ-003 multi-file ranking: union of changed-line names, one point per distinct name, every changed path excluded, deterministic ties, candidate-only packing with explicit omissions, single-file path preserved | `select::ChangedFile::query_names`, `rank_files`, `pack_candidates_into` shared by `pack`/`pack_candidates`; facade `rank_files`, `build_candidate_context`; `rank`/`pack` delegate | journey `multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only`; select tests; budget domain scratch grid (tokens ≤ budget, files ≤ limit, total tie order, no reachable panic) | PASS |
| INV-001 no IO, no stage duplication, dependency direction | No runtime dependency added; `serde_json` facade dev-dep only; facade delegates only | `Cargo.lock` diff (one edge, no new package); repo-rev boundary audit | PASS |
| INV-002 honest coverage | Omissions always reported; no completeness claim added | journey starved-budget case | PASS |
| INV-004 determinism, ambiguity preserved | `BTreeMap`/`BTreeSet` throughout; `lookup` unchanged | repeated-call equality in journey and select tests | PASS |
| AC-001 journey extended; single-file flow green; gate passes | three new journey tests; pre-existing tests unchanged | `mise run test:symbolic-context:journey` 6/6; `mise run gate` exit 0 | PASS |
| Task AC: unsupported paths identified before download; extraction changes invalidate old facts; existing APIs retain behavior | rows above; no public signature changed | owner lanes + journey | PASS |
| Authorities updated to distinguish six IO-free crates from the outer IO crate | `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md` (commit `a59e51d`, before implementation) | `mise run check:skills-sync` exit 0 | PASS |
| Prohibited: no GitHub/storage/database/model code in the six crates; no grammar pin or golden change | none in diff; pins and fixtures untouched | `git diff --stat` file set | PASS |
| Write set bounded | six crates, nearest tests, journey, authorities, facade manifest + lockfile, task evidence | `git diff --stat` | PASS |
| Ponytail / DRIFT | no deletable item; each new item maps to a REQ or a task REFACTOR step | both task-rev samples | PASS |
| Rust rules (rustdoc incl. private items and tests, `# Errors`, no unchecked unwrap, thiserror, imports at top, clone as ownership) | audited per item by repo-rev | `mise run fmt:check`, `mise run lints` exit 0 | PASS |
| Task document contract: frontmatter status in-set (AC-R1-1..3) | line 4 `status: review`; commit `932f6a5` single line | `sed -n 4p` → `status: review`; `rg '^status: implemented' changes/` exit 1 | PASS |
| Task document contract: every named test carries its exact focused command (spec-driven-development.md:183-184, 196-200; testing-workflows.md:17-18) | Implementation Evidence names eight tests; line 78 lists only `mise run` lanes | `rg -n 'mise exec' <task file>` → no match | FAIL (FIND-TASK-001-2) |

## Wave 1 results

| Reviewer | Report | Result | Findings |
|---|---|---|---|
| task-rev A | `task-review-a.md` | PASS | none |
| task-rev B | `task-review-b.md` | PASS | none |
| repo-rev | `standards-review.md` | FAIL | REPO-1 |
| domain-rev facts (source trust / provenance / eligibility) | `domain-review-facts.md` | PASS | none |
| domain-rev budget (ranking determinism / budget safety) | `domain-review-budget.md` | PASS | none |

## Citation check

`mise run review:verify-citations changes/active/repo-parser/review/TASK-001-r2 1108731565fc8801b47f286bd04351a3c467cf90` exit 0. Output preserved in `citations.txt`.

Rejected citations: REPO-1 at task lines 73, 74, 75 — the checker requires the whole line verbatim and the reviewer quoted truncated table rows. REPO-1's citation at line 78 is `OK`, so the finding survived on that anchor; the three rejected lines were dropped as citations and passed to Wave 2 only as locations to inspect. Wave 2 re-verified them directly from source.

## Validated blocking ledger (Wave 2, `findings-validation.md`)

### FIND-TASK-001-2 — CONFIRMED, gate (c) repository standard
- Source: REPO-1.
- Classification: VIOLATION (documentation contract).
- Root cause: TASK-001's Implementation Evidence records verification at `mise run` lane granularity while naming eight Rust tests as acceptance evidence.
- Defect class: missing required field in a spec-driven document (named test without its exact focused command).
- Locations: `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md:73-75` (named tests) and `:78` (lane-only command line). No siblings: the R1 remediation task names no Rust test.
- Violated rules: `architecture/references/languages/spec-driven-development.md:183-184`, `:196-200`; `architecture/references/languages/testing-workflows.md:17-18`; `architecture/references/languages/implementation-execution.md:241-242`. The "owner lane only" exception at `:197-198` ended when the evidence named the tests.
- Reproduction: `rg -n 'mise exec' changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` → no output, exit 1.
- Precedent: FIND-TASK-002-2 in `changes/active/review-helper-setup/review/TASK-002-20260925-01/` enforced the same rule as blocking.
- Consequence: the record cannot show that any named test was individually selected and passed; a later regression in one test could hide behind a green package-wide lane.
- Correction: docs-only; append the nine focused commands with observed results to the task's Implementation Evidence. Wave 2 derived and ran all nine (`1 passed` each), so the remediation is decision-complete.

## Notes (non-blocking)

- Task line 73 cites core `facts::tests` as a module rather than a named test; the remediation records both of its tests conservatively.
- `EXTRACTION_VERSION` is a hand-maintained constant (task-rev A/B observation). Same as r1 note; unchanged.
- `DeclarationFact::validate` accepts non-declaration kinds; unreachable via the facade's own export (task-rev A/B observation). Same as r1 note; unchanged.
- `prepare_changed` decodes UTF-8 twice (task-rev A observation). No written rule; not a finding.
- `pack_candidates` treats `max_files == 0` as all-omitted while `pack` errors; documented on both and consistent with REQ-003 (budget domain observation).
- Pre-existing, out of scope: `changes/active/repo-parser/spec.md:4` is `status: draft` while tasks cite approved revision 10; unchanged from base.

## Verification (orchestrator)

| Command | Result |
|---|---|
| `mise run gate` at candidate | exit 0 (log: check:default, test:rust 27/140/4/1/10/6, test:review-scripts, check:skills-sync) |
| `mise run review:verify-citations …` | exit 0; 4 citations, 1 OK, 3 REJECT (form only) |
| Owner lanes, `fmt:check`, `lints` | exit 0, run independently by Wave 1 reviewers (see reports) |
| Nine focused `mise exec -- cargo test --locked … -- --exact` runs | `1 passed` each (Wave 2) |

## Verification limits

- No sub-reviewer timed out. Wave 1 ≈ 6 min (five reports), Wave 2 ≈ 4 min. Review total ≈ 12 min of the 25–35 min budget.
- RED at base was not reproduced by any reviewer; it is a compile failure by construction (the journey references symbols absent at base).
- `.h` eligibility under a restricted `ExtractionConfig` was not exercised; the facade cannot construct one.
- `mise run gate` ran concurrently with reviewer cargo invocations in a warm cache; runtime is not representative.
- Two reviewers report that an `rg` over `changes/` incidentally surfaced lines from r1 or Wave 1 reports; both state conclusions were formed from source first.

## Prior-finding closure

- FIND-TASK-001-1 — CLOSED. `sed -n 4p` of the task file prints `status: review` (in the enumerated set); commit `932f6a5` changes only that line; AC-R1-1..3 all observed PASS by both task-rev samples. FIND-TASK-001-2 is a different rule and defect class, so FIND-TASK-001-1 is not reopened.
