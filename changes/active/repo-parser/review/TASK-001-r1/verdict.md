# TASK-001 task review r1 — verdict

## Immutable subject

| Item | Value |
|---|---|
| Task | `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` |
| Spec | `changes/active/repo-parser/spec.md` (SPEC-repo-parser revision 10) |
| Base | `9715440d6969dd317a62bd7c2a4ff30082f6082e` |
| Candidate | `34f4ae71a45f4e6d150c26aab680484519708f07` (unchanged through both waves; verified by `git rev-parse HEAD` before Wave 1, before the citation check, and before this verdict) |
| Diff | `git diff 9715440d..34f4ae71`: 14 files, +1106/-59 |
| Review directory | `changes/active/repo-parser/review/TASK-001-r1/` |

## Verdict

**FIX_REQUIRED** — one confirmed, docs-only repository-standard finding (`FIND-TASK-001-1`). All code obligations of REQ-001, REQ-002, REQ-003, INV-001, INV-002, INV-004, and AC-001 pass. Remediation task: `TASK-001-R1-task-status.md` in this directory.

## Acceptance matrix

Consolidated from the two independent `task-rev` samples (both reached identical rows; A passed every row, B failed only the REQ-002 "version changes" row, which Wave 2 downgraded to a note).

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001 export is source-free | `core/src/facts.rs` `DeclarationFact` carries id/kind/name/qualified_name/language/lines/start_column only; `into_node` fills every source-derived field with `None` | core `projection_is_source_free_and_rebuilds_a_node`; journey `exported_facts_reload_without_source_and_reject_invalid_records` asserts the JSON contains no source text | PASS |
| REQ-001 byte length exported per path | `index/src/lib.rs` `export` copies `byte_len` | journey asserts `byte_len == USER.len()` fresh and reloaded | PASS |
| REQ-001 load without parsing | `SymbolIndex::load` → `into_node` → `upsert`; facade `load` never calls the extractor | journey loads 4 files into a fresh `SymbolicContext`; index round-trip test | PASS |
| REQ-001 loaded facts reproduce ranking and candidate context | ranking reads only postings and node id/start_line/end_line, all preserved | journey `assert_eq!(reloaded, fresh)` on ranking and context; task-rev B probe `export(loaded) == export(fresh)` | PASS |
| REQ-001 invalid path fails explicitly | `RelativePath` `Deserialize` revalidates | journey: `../user.ts` rejected; `/etc/passwd` → `InvalidPath` | PASS |
| REQ-001 malformed facts fail without corrupting other files | `IndexedFileFacts::validate` runs before `upsert`; upsert has no fallible step | journey: stale-version, zero start_line, empty-name records rejected; other files still resolve; index test asserts postings unchanged | PASS (note TASK-A-1) |
| REQ-002 extraction version exposed and checked on load | `extract/src/lib.rs:119` `EXTRACTION_VERSION`; facade `load` compares before touching the index (`lib.rs:193`) | journey `ExtractionVersionMismatch` assertion; stamp matches `Cargo.toml:35` pin | PASS |
| REQ-002 version changes on fact/grammar change | maintained constant with rustdoc bump rule; task declares the limit | N/A (convention) | PASS (note TASK-B-1/DOM-FACTS-1) |
| REQ-002 path-only eligibility query | `Extractor::path_language`, facade `path_language` | extract and journey tests; task-rev B probe agrees with real `upsert` outcomes for `.svelte`, `conf/routes`, `.h`, `README.md` | PASS |
| REQ-003 union of names across changed files | `select` `rank_files` flat-maps `ChangedFile::query_names` into one `resolve` call | select `multi_file_ranking_unions_names_and_excludes_every_changed_path`; journey two-file case | PASS |
| REQ-003 every changed path excluded | `excluded` BTreeSet of all changed paths | journey and select assertions; probe with duplicated changed path equals single-file ranking | PASS |
| REQ-003 score once per distinct matched name | `resolve` dedupes query names; per-file `BTreeSet<String>` sets score | journey: `lookupUser` referenced in both changed files scores once | PASS |
| REQ-003 sort desc score then canonical path | `RelativePath` `Ord` on canonical string | journey tie `format.ts` before `unrelated.ts`; input-order-independence test | PASS |
| REQ-003 candidate-only packing without changed head blob | `pack_candidates` starts from `Context::default()` | journey asserts all spans `Candidate` | PASS |
| REQ-003 counts only selected candidate files; explicit omissions | files incremented only when a span is selected; `FileLimit`/`TokenLimit` omissions, never mid-span cuts | select and journey omission assertions; `max_files: 0` → all omitted | PASS |
| REQ-003 single-file `rank`/`pack` compatible | `rank`/`pack` delegate; no public signature changed | pre-existing journey and select tests unchanged and green | PASS |
| INV-001 no IO in six crates, downward deps | no runtime dependency added; `Cargo.lock` delta is one facade dev-dep | diff read; `mise run lints` 0 | PASS |
| INV-002 honest coverage | typed omissions and per-path load errors | journey omission assertions | PASS |
| INV-004 determinism, ambiguity preserved | BTree collections throughout; `resolve` keeps all hits | journey reruns equal; order-independence test | PASS |
| AC-001 journey extended, single-file green, gate passes | journey +264 lines, 3 new tests | `mise run test:symbolic-context:journey` 0 (6 passed); `mise run gate` 0 (orchestrator) | PASS |
| Authority docs updated before implementation | `AGENTS.md`, `agent-rules.md`, design doc distinguish six symbolic crates from outer crate; commit `a59e51d` precedes code commits | diff read | PASS |
| Prohibited: no GitHub/storage/db/model code; no grammar pin/golden change | none present; no fixture or pin file in diff | diff `--name-only` filtered | PASS |
| Write set limited | 14 files within permitted set plus task evidence section | diff stat | PASS |
| Task document contract (spec-driven-development.md:171-173 status enumeration) | `status: implemented` at task line 4 | `rg -n '^status:' changes/` | FAIL (FIND-TASK-001-1) |
| Ponytail: `serde_json` dev-dep, `ChangedFile`/`rank_files`/`pack_candidates_into` | dev-only, already a workspace dep, needed to prove the wire form is source-free; no trait or generic layer; `rank`/`pack` delegate without duplication | reviewers A and B both kept it at rung 5 | PASS |

## Wave 1 results

| Reviewer | Report | Result | Findings |
|---|---|---|---|
| task-rev A | `task-review-a.md` | PASS | TASK-A-1 (minor) |
| task-rev B | `task-review-b.md` | FAIL | TASK-B-1 (important) |
| repo-rev | `standards-review.md` | FAIL | REPO-1 |
| domain-rev facts/source-trust | `domain-review-facts.md` | PASS | DOM-FACTS-1 (medium, non-blocking) |
| domain-rev budget/packing | `domain-review-budget.md` | PASS | none |

## Citation check

`mise run review:verify-citations changes/active/repo-parser/review/TASK-001-r1 34f4ae71…` exit 0; 15 citations, all `OK`. Rejected citations: none. Output preserved in `citations.txt`.

## Validated blocking ledger (Wave 2, `findings-validation.md`)

### FIND-TASK-001-1 — CONFIRMED, gate (c) repository standard
- Source: REPO-1.
- Root cause: the task document uses a lifecycle status value outside the enumerated set.
- Location: `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md:4` (`status: implemented`); no siblings (`rg -n 'status: implemented' changes/` → one hit).
- Violated rule: `architecture/references/languages/spec-driven-development.md:171-173` (`proposed`, `ready`, `in_progress`, `review`, `approved`, `superseded`).
- Consequence: the task cannot be placed in a defined lifecycle phase; the next transition has no defined predecessor.
- Correction: set line 4 to `status: review`. Docs-only.

## Notes (non-blocking)

- **TASK-B-1 / DOM-FACTS-1 (merged):** `EXTRACTION_VERSION` is a maintained constant; nothing mechanically couples it to the grammar pins. Current value matches the pins, mismatch is refused on load, and the task declares this limit. A mechanical stamp would be a new identity decision (SPEC_REVISION_REQUIRED if pursued), not a task defect.
- **TASK-A-1:** `DeclarationFact::validate` accepts a non-declaration `NodeKind`; `load` then treats it exactly as `upsert` treats a parsed non-declaration node (retained, not posted, dropped on re-export). Unreachable through the public export→load round trip; correcting it would move `is_declaration` ownership into core.
- **Pre-existing, out of scope:** `changes/active/repo-parser/spec.md:4` is `status: draft` while the task cites approved revision 10; unchanged from base.

## Verification (orchestrator)

| Command | Result |
|---|---|
| `mise run gate` | exit 0 |
| `mise run review:verify-citations …` | exit 0, 15 OK |
| Owner lanes, `fmt:check`, `lints` | exit 0 (run independently by each Wave 1 reviewer; see reports) |

## Verification limits

- No sub-reviewer timed out; all five Wave 1 reports and the Wave 2 report arrived within budget (Wave 1 ≈ 6 min, Wave 2 ≈ 3 min).
- Wave 2 reproduced TASK-A-1 and FIND-TASK-001-1 directly; TASK-B-1 was traced from source (no live pin bump performed against the read-only tree).
- `mise run gate` ran concurrently with reviewer cargo invocations; it exited 0 in a warm cache, so its runtime is not representative.

## Prior-finding closure

None; this is the first review of TASK-001.
