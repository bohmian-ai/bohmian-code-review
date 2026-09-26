# TASK-001 task review r3 — verdict

## Immutable subject

- Repository: `bohmian-code-review`, branch `review-helper-setup`
- Base: `9715440d6969dd317a62bd7c2a4ff30082f6082e`
- Candidate (cumulative): `1fbcc4af81de98baf070a8538341e2142a1ef01c` — unchanged for the whole review (`git rev-parse HEAD` checked before Wave 1, before the citation check, before Wave 2, and before writing this file; working tree carried only this untracked review directory)
- Approved spec: `changes/active/repo-parser/spec.md` (SPEC-repo-parser revision 10)
- Original task: `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md`
- Prior reviews: `review/TASK-001-r1/verdict.md` (FIX_REQUIRED, FIND-TASK-001-1), `review/TASK-001-r2/verdict.md` (FIX_REQUIRED, FIND-TASK-001-2)
- Remediations reviewed: `review/TASK-001-r1/TASK-001-R1-task-status.md` (commit `932f6a5`), `review/TASK-001-r2/TASK-001-R2-focused-test-commands.md` (commit `cbf517a`)
- Diff reviewed: complete `9715440d..1fbcc4a` excluding `changes/active/repo-parser/review/` (14 files, +1120/−59: three authorities, `Cargo.lock`, facade manifest, six-crate sources, journey test, task document)

## Verdict

**PASS** — the Wave 2 validation completed with an empty blocking ledger. Every obligation of REQ-001, REQ-002, REQ-003, INV-001, INV-002, INV-004, AC-001, the task's own acceptance criteria and prohibited changes, and AC-R1-1..3 / AC-R2-1..3 passes on repository, diff, and verification evidence. FIND-TASK-001-1 remains closed; FIND-TASK-001-2 is closed by the cumulative candidate. Three non-blocking notes are recorded below; one needs a maintainer decision before `cr-complete`.

## Acceptance matrix

Consolidated from `task-review-a.md` and `task-review-b.md` (independent samples; identical rows except A's REQ-001 malformed-facts row, which Wave 2 classified as a note) with orchestrator-observed verification.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001 export byte length + declaration facts per path; load into a fresh index without parsing | `SymbolIndex::export`/`load` (`crates/bohmian-code-review-index/src/lib.rs:127-165`); `SymbolicContext::export`/`load` (`crates/bohmian-code-review/src/lib.rs:177-204`) | journey `exported_facts_reload_without_source_and_reject_invalid_records`; index `export_round_trips_through_load_and_rejects_invalid_records` | PASS |
| REQ-001 projection is source-free; loaded facts reproduce lookup, ranking, candidate context | `DeclarationFact` (`crates/bohmian-code-review-core/src/facts.rs:18-35`) carries id/kind/name/qualified_name/language/lines/start_column only; select/index consume only those fields | core `projection_is_source_free_and_rebuilds_a_node`; journey asserts JSON carries no source text and `reloaded == fresh` for rank and context | PASS |
| REQ-001 invalid paths / malformed facts fail explicitly without corrupting other files | `RelativePath` `Deserialize` revalidates; `IndexedFileFacts::validate` before `upsert`; version check before index touch | journey: `../user.ts`, stale version, zero start_line, empty name rejected; other files intact; index test asserts postings unchanged | PASS (note TASK-A-1: non-declaration `kind` accepted and dropped; unreachable via facade export→load) |
| REQ-002 extraction version exposed, stamped, mismatch refused before mutation | `EXTRACTION_VERSION` (`crates/bohmian-code-review-extract/src/lib.rs:113-119`); `Error::ExtractionVersionMismatch` | journey mismatch assertion with unchanged `file_count`/lookup afterwards | PASS (note: hand-maintained constant, same as r1/r2) |
| REQ-002 path-only eligibility consistent with parse-time routing | `Extractor::path_language` (`crates/bohmian-code-review-extract/src/extractor.rs:66-72`) uses the same router/composite predicate as extraction; facade validates the path first | extract `path_language_reports_eligibility_without_source`; journey `path_language_identifies_eligible_paths_before_download`; facts domain sweep | PASS |
| REQ-003 union of changed-line names; every changed path excluded; one point per distinct name; desc score then canonical path; order-independent | `ChangedFile::query_names`, `rank_files` (`crates/bohmian-code-review-select/src/lib.rs:141-243`) | journey `multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only`; select `multi_file_ranking_unions_names_and_excludes_every_changed_path`; budget domain probe grid | PASS |
| REQ-003 candidate-only packing counts only selected candidates, explicit omissions, no changed-hunk blob | `pack_candidates` → shared `pack_candidates_into` (`select/src/lib.rs:395-472`); facade `build_candidate_context` | select `candidate_packing_counts_only_selected_candidates`; journey `(files, tokens) == (1, 11)`, omissions, starved-budget case; budget probe: tokens ≤ budget, files ≤ limit, no panic | PASS |
| REQ-003 existing single-file `rank`/`pack` compatible | `rank` wraps one `ChangedFile` → `rank_files`; `pack` keeps hunk-first logic then delegates; no public signature changed | pre-existing journey (3) and select tests unchanged and green; `rank_files([handler]) == rank(...).files` | PASS |
| INV-001 no IO in six crates; dependency direction; no stage duplication | no runtime dependency added; `serde_json` facade dev-dep only (`Cargo.lock` +1 line); facade delegates every stage | diff read by repo-rev and both task-rev samples; `mise run lints` exit 0 | PASS |
| INV-002 honest coverage | every dropped candidate is an explicit omission; no completeness claim added | journey omission assertions | PASS |
| INV-004 determinism; ambiguity preserved | `BTreeMap`/`BTreeSet` throughout; `lookup` unchanged | repeated-call and swapped-order equality in journey and select tests | PASS |
| AC-001 journey extended; single-file flow green; gate passes | three new journey tests; three pre-existing untouched | `mise run test:symbolic-context:journey` 6/6; `mise run gate` exit 0 (orchestrator) | PASS |
| Task: authorities updated before implementation to distinguish six crates from outer crate | `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md` in commit `a59e51d`, first in the range | `mise run check:skills-sync` exit 0 | PASS (note REPO-2 on spec status) |
| Task: prohibited — no GitHub/storage/DB/model code in six crates; no grammar pin, golden, or fixture change | none in diff; `extract/grammars`, fixtures, extract manifest untouched | `git diff --stat`; `mise run test:extract` 27+140 passed | PASS |
| Task: write set bounded | 14 files, all inside six crates, journey, authorities, facade manifest + lockfile, task evidence | `git diff --stat` | PASS |
| Task: REFACTOR steps (shared name collection/scoring; reuse path validation and postings replacement) | `rank` → `rank_files`; `pack` → `pack_candidates_into`; `load` → `upsert`; `RelativePath::new` reused by deserializer | source read | PASS |
| Ponytail / DRIFT | no deletable item; every new item maps to a REQ or task REFACTOR step | both task-rev samples | PASS |
| Rust rules (rustdoc incl. private items/tests, `# Errors`, no unchecked unwrap, thiserror, imports at top, clone as ownership, concrete owners) | audited per item by repo-rev; all pass rows | `mise run fmt:check`, `mise run lints` exit 0 | PASS |
| Git rules (local identity, no AI co-author trailers) | 12 commits by `Thorrester <sjforrester32@gmail.com>`, no trailers | `git log 9715440d..1fbcc4a` | PASS |
| AC-R1-1..3 (status `review`; no `status: implemented`; one-line change) | task line 4 | `sed -n 4p` → `status: review`; `rg '^status: implemented' changes/` exit 1; `git diff --stat 3e71501..932f6a5` 1 file +1/−1 | PASS |
| AC-R2-1 every named test has a focused-command row (≥ 9) | task lines 80-92 (nine rows) | `rg -c 'mise exec -- cargo test --locked' <task file>` → `9` | PASS |
| AC-R2-2 each command exits 0 with `1 passed` | — | nine commands re-run sequentially by task-rev A, task-rev B, repo-rev, and ponytail-rev: each `1 passed; 0 failed`, exit 0 | PASS |
| AC-R2-3 no collateral change | — | `git diff --stat 1108731..1fbcc4a -- . ':!changes/active/repo-parser/review'` → 1 file, +14/−0; `git diff --check` exit 0 | PASS |
| Task-document contract: every named test carries its exact focused command (spec-driven-development.md:183-184, 196-200) | task lines 80-92 | as AC-R2-1/2 | PASS (closes FIND-TASK-001-2) |
| Task-document contract: RED/GREEN recorded (spec-driven-development.md:206-222) | task line 68 records RED as the compile failure naming the nine new public symbols; GREEN per lane and per focused command | Wave 2: compile failure on brand-new public API is the expected missing behavior; task lines 30/40 define RED at lane level; R2 remediation kept this wording | PASS (REPO-1a rejected) |

## Wave 1 results

| Reviewer | Report | Result | Findings |
|---|---|---|---|
| task-rev A | `task-review-a.md` | FAIL | TASK-A-1 |
| task-rev B | `task-review-b.md` | PASS | none |
| repo-rev | `standards-review.md` | FAIL | REPO-1, REPO-2 |
| domain-rev facts (source trust / provenance / eligibility) | `domain-review-facts.md` | PASS | none (one non-material observation) |
| domain-rev budget (ranking determinism / budget safety) | `domain-review-budget.md` | PASS | none |

## Citation check

`mise run review:verify-citations changes/active/repo-parser/review/TASK-001-r3 1fbcc4af81de98baf070a8538341e2142a1ef01c` exit 0; output preserved in `citations.txt` (57 entries).

Rejected citations: five, all pass-evidence entries rather than findings — `C2`, `C4`, `C5`, `C6` (domain-review-facts) and `PASS-facade` (task-review-b) used `Type::method` symbol form the checker does not resolve. No finding lost a citation; every REPO-1, REPO-2, and TASK-A-1 citation was `OK`. Correction to the record: `standards-review.md` refers to the focused-command table as task lines 112-124; the table is at lines 80-92 (line 112 does not exist in the 94-line file). Wave 2 and this verdict use the source-verified numbers.

## Validated blocking ledger (Wave 2, `findings-validation.md`)

Empty — explicitly validated. Each surviving claim was reproduced or traced from source; none meets gate (a) production path, (b) task contract, (c) repository standard as applied to the diff, or (d) absolute boundary.

## Rejected claims

- **REPO-1a** (per-scenario RED): spec-driven-development.md:206-208 forbids a RED that fails for *unrelated* compilation; task line 68 records a compile failure naming exactly the nine public symbols the task introduces, which is the expected missing behavior for new public API. The approved task defines RED at lane granularity (lines 30, 40) and the R2 remediation instructed that this RED wording stay as written. No cited sentence requires one RED row per named test.

## Notes (non-blocking)

- **REPO-1b**: the Implementation Evidence records `git diff --check` and a tracked-diff observation but no explicit untracked-file inspection sentence (testing-workflows.md:19-20 checklist item). State verified clean by Wave 2 (`git status --short` shows only this review directory). Recording checklist item on an immutable commit range; no consequence.
- **REPO-2**: `AGENTS.md:23` and `architecture/bohmian-code-review-design.md:5` describe the outer crate as "approved" (the wording the task at line 16 instructed) while `changes/active/repo-parser/spec.md:4` still reads `status: draft` and line 9 says revision 10 "awaits approval". Pre-existing at base, outside the task write set, unchanged by the diff, and already noted in r1 and r2. **Maintainer action before `cr-complete`:** record revision-10 approval in `spec.md` (status and line 9), or re-derive the two authority sentences from the actual status. Only a human may move draft → approved (spec-driven-development.md:128-129); it is not an implementer decision and does not block TASK-001.
- **TASK-A-1**: `DeclarationFact::validate` (`core/src/facts.rs:88-99`) does not reject non-declaration `kind`s; a hand-built Import fact loads `Ok`, is not posted, and is dropped on re-export (reproduced by Wave 2 in a `/tmp` crate). Unreachable through the facade's own export→load (export filters to declarations, `index/src/lib.rs:137`); behavior mirrors `upsert`'s documented handling of extractor nodes. Same observation as the r1/r2 note. Minimum correction if a later spec wants it: one branch in `validate` plus moving `is_declaration` into core.
- Carried from r1/r2, unchanged: `EXTRACTION_VERSION` is a hand-maintained constant; `prepare_changed` decodes UTF-8 twice; `pack_candidates` treats `max_files == 0` as all-omitted while `pack` errors (documented on both).
- domain-rev facts observation: `Extractor::path_language` reports composite routes eligible under a restricted `ExtractionConfig` where embedded native extraction would return `DisabledLanguage`; unreachable via the facade, which always uses `all_native()`.
- domain-rev budget observation: `rank_files(&[])` returns `Ok(vec![])`; per-file deletion-only input is already rejected, so an empty set is the caller's explicit choice.

## Verification (orchestrator)

| Command | Result |
|---|---|
| `mise run gate` at candidate | exit 0 (log `/tmp/task001-r3-gate.log`: check:default, test:rust 27/140/4/1/10/6, test:review-scripts, check:skills-sync) |
| `mise run review:verify-citations …` | exit 0; 57 citations, 52 OK, 5 REJECT (pass-evidence symbol form only) |
| `rg -c 'mise exec -- cargo test --locked' <task file>` | `9` |
| `sed -n 4p <task file>` | `status: review` |
| `git diff --stat 1108731..1fbcc4a -- . ':!changes/active/repo-parser/review'` | 1 file, +14 |
| `git diff --check 9715440d..1fbcc4a -- . ':!changes/active/repo-parser/review'` | exit 0 |
| Owner lanes, `fmt:check`, `lints`, `check:skills-sync`, nine focused `-- --exact` runs | exit 0 / `1 passed` each, run independently by Wave 1 and Wave 2 reviewers (see reports) |

## Verification limits

- No sub-reviewer timed out. Wave 1 ≈ 7 min (five reports), citation check < 1 min, Wave 2 ≈ 4 min; review total ≈ 15 min of the 25–35 min budget.
- RED at base was not reproduced by any reviewer; it is a compile failure by construction (the journey references symbols absent at base).
- TASK-A-1 was reproduced through the index crate directly; the facade path was traced, not executed (facade `load` only adds the version check before delegating).
- Restricted-`ExtractionConfig` eligibility for composite routes was not exercised through the facade, which cannot construct one.
- `mise run gate` ran concurrently with reviewer cargo invocations in a warm cache; runtime is not representative.
- The wyrd-diff MCP server failed to connect this session; it was not needed and no reviewer depended on it.

## Prior-finding closure

- **FIND-TASK-001-1** — remains CLOSED. `sed -n 4p` of the task file prints `status: review` (in the enumerated set, spec-driven-development.md:171); no diff since r2 touches line 4.
- **FIND-TASK-001-2** — CLOSED by the cumulative candidate. Task lines 80-92 carry nine `mise exec -- cargo test --locked -p <crate> --<target> <selector> -- --exact` rows covering all eight named tests plus both `facts::tests` tests, labelled as current GREEN closure per the R2 recommendation. Every row was re-run by four independent reviewers: each exit 0, `1 passed; 0 failed`. Commit `cbf517a` touched only that table (+14 lines). No sibling site names a test without its command (`rg -n 'mise exec' changes/active/repo-parser/tasks/` matches only this table). Not reopened.

No remediation task is produced. TASK-001 has reached its completion gate; the REPO-2 note is routed to the maintainer for the spec-status decision ahead of `cr-complete`.
