# TASK-001 task review r3 — sample B

## Subject

- Base: `9715440d6969dd317a62bd7c2a4ff30082f6082e`
- Candidate (cumulative HEAD): `1fbcc4af81de98baf070a8538341e2142a1ef01c`
- Spec: `changes/active/repo-parser/spec.md` revision 10 (REQ-001..003, INV-001/002/004, AC-001)
- Task: `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md`
- Remediations already applied: TASK-001-R1 (task status), TASK-001-R2 (focused test commands)
- Diff reviewed: `git diff 9715440d..1fbcc4a -- . ':!changes/active/repo-parser/review'` (14 files, +1120/-59)
- Method: read every non-review hunk; read the owning source around each new item (`facts.rs`, `SymbolIndex::upsert/export/load`, `Extractor::extract`/`extract_from_source`, `CompositeExtractor::extract`, `rank`/`rank_files`/`pack`/`pack_candidates_into`, facade); ran lanes and the nine focused commands myself.

## Acceptance matrix

| Requirement / criterion / constraint | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001 export each indexed path's byte length and declaration facts | `SymbolIndex::export` (`crates/bohmian-code-review-index/src/lib.rs:127`) emits `IndexedFileFacts { path, extraction_version, byte_len, declarations }` per file in `BTreeMap` path order; facade `SymbolicContext::export` (`crates/bohmian-code-review/src/lib.rs:177`) stamps `EXTRACTION_VERSION` | journey `exported_facts_reload_without_source_and_reject_invalid_records` asserts paths in canonical order, `byte_len == USER.len()`, declaration names/lines; index `export_round_trips_through_load_and_rejects_invalid_records` | PASS |
| REQ-001 load into a new index without parsing source | `SymbolIndex::load` (`index/src/lib.rs:156`) rebuilds nodes via `DeclarationFact::into_node` and calls the existing `upsert`; facade `load` (`bohmian-code-review/src/lib.rs:192`) does no extraction | journey loads from JSON into `SymbolicContext::new()`; `file_count == 4`, `lookup("lookupUser")` hits `src/user.ts` | PASS |
| REQ-001 projection contains only fields needed for lookup, ranking reasons, spans; no verbatim source fragments | `DeclarationFact` (`core/src/facts.rs:18`) carries id, kind, name, qualified_name, language, start/end line, start_column only; `into_node` leaves docstring/signature/decorators/return_type `None`. Source read: resolve/select/index read none of the dropped fields outside tests (`rg` over the four crates' `src`) | journey asserts JSON lacks `console.log`, `return { id }`, `(id: string)`, `docstring`, `signature`; core `projection_is_source_free_and_rebuilds_a_node` | PASS |
| REQ-001 loaded facts give same ranking and candidate context as fresh parse | Index posts only `is_declaration` nodes (`upsert` line 62); export filters the same predicate, so postings are identical. `candidate_spans` uses line ranges only | journey asserts `reloaded == fresh` for `rank` and `build_context` equality on the same limits/blobs | PASS |
| REQ-001 invalid paths or malformed facts fail explicitly without corrupting other files | `RelativePath` custom `Deserialize` (`core/src/relative_path.rs:83`) revalidates; `IndexedFileFacts::validate` (`facts.rs:124`) runs before `upsert` touches the index; facade version check precedes `index.load` | journey: stale version, zero start_line, empty name under a new path all `Err`, then `file_count == 4` and lookup intact; `../user.ts` JSON rejected; index test asserts postings unchanged after rejected record | PASS |
| REQ-002 extraction version exposed; changes when facts/grammar change | `EXTRACTION_VERSION` (`extract/src/lib.rs:119`) re-exported by facade; maintained constant with bump rule in rustdoc | journey asserts stamped value equals `EXTRACTION_VERSION`; no mechanical link to grammar pins (task file records this limit; spec wording "changes when" is a maintenance contract, not an automatic one) | PASS |
| REQ-002 version changes cannot silently reuse incompatible snapshots | `SymbolicContext::load` returns `Error::ExtractionVersionMismatch { path, found, expected }` before touching the index | journey asserts the mismatch error with path/found | PASS |
| REQ-002 path-only language/eligibility query for pre-download filtering; ambiguous routes may still use content | `Extractor::path_language` (`extract/src/extractor.rs:70`) = `router.detect(path, None)` admitted by the same predicate `CompositeExtractor::extract`/`Extractor::extract` use (composite kind, or native and enabled); facade `path_language` (`bohmian-code-review/src/lib.rs:215`) validates the path first | extract `path_language_reports_eligibility_without_source` (native, `.h` → C, Svelte, `conf/routes` → Yaml, `README.md`/`Makefile` → None, enabled-set honoured); journey `path_language_identifies_eligible_paths_before_download` incl. `InvalidPath` for `/etc/passwd` | PASS |
| REQ-003 union of overlapping declaration names and references on changed lines across multiple changed blobs with validated nonempty ranges | `ChangedFile::query_names` (`select/src/lib.rs:141` onward) is the single collector shared by `rank` and `rank_files`; facade `rank_files` (`bohmian-code-review/src/lib.rs:269`) runs `prepare_changed` per blob, which validates via `ChangedLines::new` | select `multi_file_ranking_unions_names_and_excludes_every_changed_path`; journey asserts `DeletionOnly` for an empty range in `rank_files` | PASS |
| REQ-003 every changed path excluded | `rank_files` (`select/src/lib.rs:209`) builds `excluded: BTreeSet<&RelativePath>` from all changed paths and filters hits | journey asserts `src/report.ts` absent and `src/handler.ts` never indexed; select test fixture excludes `src/d.ts` | PASS |
| REQ-003 score once per distinct matched name; sort by descending score then canonical path | `matched: BTreeMap<&RelativePath,(BTreeSet<String>, Vec<MatchReason>)>`, score = `names.len()`; `sort_by` score desc then path asc (unchanged code, now shared) | journey: `lookupUser` referenced in both changed files scores `src/user.ts` 2 not 3; `format.ts`/`unrelated.ts` tie ordered by path; select test asserts order independence `rank_files(&[other, handler]) == ranked` | PASS |
| REQ-003 candidate-only packing without a changed head blob, counts only selected candidate files, explicit omissions under limits | `pack_candidates` (`select/src/lib.rs:397`) → `pack_candidates_into` (`:419`) starting from `Context::default()`; `files` increments only on selected candidate; facade `build_candidate_context` (`bohmian-code-review/src/lib.rs:330`) | select `candidate_packing_counts_only_selected_candidates` (TokenLimit + FileLimit omissions, `max_files == 0` all-omitted, MissingBlob error); journey asserts spans all `Candidate`, `(files,tokens) == (1,11)`, three omissions, fully starved case | PASS |
| REQ-003 existing single-file ranking and changed-hunk packing remain compatible | `rank` now wraps `rank_files(&[changed])` and rebuilds the same `Ranking`; `pack` keeps changed-hunk logic then calls `pack_candidates_into`; no public signature changed | pre-existing journey tests (3) and select tests unchanged and green; select test asserts `rank_files(&[handler]) == rank(...).files`; journey asserts `single.files[0] == ranked[0]` | PASS |
| INV-001 six crates retain no-IO boundary and dependency direction; no stage logic duplicated | No new deps except `serde_json` dev-dep on facade (`Cargo.lock` +1 line); core already depended on serde. Facade delegates every stage (`index.export/load`, `extractor.path_language`, `select::rank_files/pack_candidates`) | `rg` for `std::fs`, `reqwest`, `git` in the diff: none. `mise run lints` exit 0 | PASS |
| INV-002 honest coverage (missing facts/partial index remain visible) | Library unchanged in scope: index still describes only supplied files; `MissingBlob` remains an error in candidate packing; a file-limited candidate is an explicit `FileLimit` omission | select and journey omission assertions above | PASS |
| INV-004 determinism and preserved ambiguity | All new collections are `BTreeMap`/`BTreeSet`; `lookup` unchanged returns all hits; journey shows byte-equal `Context` on repeated `build_candidate_context` | journey `build_candidate_context(...) == Ok(packed)` second call; select order-independence assertion | PASS |
| AC-001 journey extends to export/load equivalence, two-file ranking, exclusion, distinct-name scoring, candidate-only packing; existing single-file behavior green; gate | Three new journey tests in `crates/bohmian-code-review/tests/symbolic_context_journey.rs:389,488,516` use only public facade APIs | `mise run test:symbolic-context:journey` 6 passed, exit 0; `fmt:check`, `lints` exit 0; `gate` not rerun here (see limits) | PASS |
| Task: authorities updated to distinguish six crates from outer crate before implementation | `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md` hunks name the outer crate, its IO monopoly, facade-only dependency, source-free export | Read hunks; statements match spec INV-001/INV-005 language | PASS |
| Task prohibited: no GitHub/storage/DB/model code in six crates; no grammar pins/goldens changed | `git diff --stat` on `extract/grammars`, `extract/fixtures`, `extract/Cargo.toml`: no changes; only `crates/bohmian-code-review/Cargo.toml` (+3 dev-dep lines) among manifests | Observed empty stat | PASS |
| Task write set (six crates, nearest tests, journey, architecture docs, manifest/lockfile as needed) | 14 changed files all inside that set | `git diff --stat` | PASS |
| Task scenario REFACTOR: shared name collection/scoring, reuse of path validation and postings replacement | `ChangedFile::query_names` + `rank_files` shared by `rank`; `load` reuses `upsert`; `RelativePath` deserializer reuses `RelativePath::new`; facade `extract`/`prepare_changed` helpers de-duplicate `upsert`/`rank` bodies | Source read | PASS |
| Task RED/GREEN evidence with focused commands (R2) | Task file "Focused commands" table, nine rows | reproduced below, all `1 passed`, exit 0 | PASS |
| AC-R1-1 `sed -n 4p` prints `status: review` | task file line 4 | observed `status: review` | PASS |
| AC-R1-2 `rg -n 'status: implemented' changes/` no matches | — | anchored form `^status: implemented` exits 1; unanchored form matches only quoted prose in review records (accepted in R1 evidence) | PASS |
| AC-R1-3 only the task file, one-line change | — | `git diff --stat 34f4ae7..932f6a5 -- . ':!.../review'`: 1 file, +1/-1 | PASS |
| AC-R2-1 every named test has a focused-command row; count >= 9 | task file table | `rg -c` prints 9; the eight named tests plus both `facts::tests` tests are each present | PASS |
| AC-R2-2 nine commands exit 0 with `1 passed` | — | each run sequentially, observed `1 passed; 0 failed`, exit 0 (log below) | PASS |
| AC-R2-3 only the task file, Implementation Evidence section | — | `git diff --stat 1108731..1fbcc4a -- . ':!.../review'`: 1 file, +14/-0; hunk sits after "Commands run in this session" | PASS |

## Ponytail ladder on changed surfaces

- `DeclarationFact`/`IndexedFileFacts` (new): required; no existing source-free type exists (`Node` carries fragments). Serde derives reuse core's existing `serde` dependency. Keep.
- `InvalidFact` (new error enum): needed for explicit failure without corrupting; `thiserror` per rules. Keep.
- `ChangedFile<'a>` (select) and `ChangedBlob<'a>` (facade): different layers (extracted facts vs raw bytes + unvalidated ranges); collapsing them would push extraction into select or bytes into the facade signature of the selector. Keep.
- `pack_candidates` + private `pack_candidates_into`: the `_into` helper is what lets `pack` append after changed hunks; `pack_candidates` is the one-line public entry. Keep.
- `serde_json` dev-dependency on facade: needed for the journey's JSON round trip; already a workspace dependency, lockfile grows by one line. Keep.
- `EXTRACTION_VERSION` constant: minimal mechanism; no existing constant covers it. Keep.
- Docs: three authority edits requested by the task. Keep.

No DRIFT identified.

## Findings

None. No MISSING, INCORRECT, DRIFT, VIOLATION, or REGRESSION finding survived source reading and verification.

Non-findings considered and rejected (recorded so the reasoning is auditable, not as findings):
- `DeclarationFact::validate` does not check `kind` is a declaration. A loaded non-declaration fact is not posted because `SymbolIndex::upsert` filters with `is_declaration` (`index/src/lib.rs:62`); export never emits one. No reachable incorrect lookup or ranking.
- `path_language` reports `Some(Svelte)` even when the embedded languages are disabled; `CompositeExtractor::extract` admits composites the same way (`composite/mod.rs:273`), so path-only eligibility and parse-time admission agree, and the spec permits content-time refinement.
- `EXTRACTION_VERSION` is a maintained constant; REQ-002 requires it to change when behavior changes, and the task file records the maintenance limit. No automatic check is demanded by the spec.

## Verification I ran (observed)

| Command | Observed |
|---|---|
| `mise run fmt:check` | exit 0 |
| `mise run lints` | exit 0 (only pre-existing grammar C warnings) |
| `mise run test:extract` | 27 passed + 140 passed, exit 0 |
| `mise run test:index` | 4 passed, exit 0 |
| `mise run test:select` | 10 passed, exit 0 |
| `mise run test:resolve` | 1 passed, exit 0 |
| `mise run test:symbolic-context:journey` | 6 passed, exit 0 |
| nine `mise exec -- cargo test --locked -p ... -- --exact` commands from the task file | each `1 passed; 0 failed`, exit 0 |
| `git diff --check 9715440d..1fbcc4a` | exit 0 |
| `sed -n 4p <task>` / `rg -n '^status: implemented' changes/` / `rg -c 'mise exec -- cargo test --locked' <task>` | `status: review` / exit 1 / `9` |
| `git diff --stat 34f4ae7..932f6a5` and `1108731..1fbcc4a` (excluding review dir) | 1 file +1/-1; 1 file +14/-0 |
| `git status --short` | clean before and after review |

## Verification limits

- `mise run gate` was not rerun within the time cap; its constituent lanes (`fmt:check`, `lints`, owner tests, journey) were run individually and passed. `check:skills-sync` was not run.
- Cross-language equivalence of loaded vs fresh ranking was verified on the TypeScript journey fixture and by reading that no dropped `Node` field is consumed by index/resolve/select; no per-grammar probe was run.

Overall: PASS

```citations
[]
```
