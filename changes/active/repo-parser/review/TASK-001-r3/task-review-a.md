# TASK-001 task review r3 — sample A

## Subject

- Base: `9715440d6969dd317a62bd7c2a4ff30082f6082e`
- Candidate (cumulative HEAD): `1fbcc4af81de98baf070a8538341e2142a1ef01c`
- Spec: `changes/active/repo-parser/spec.md` revision 10 (REQ-001..003, INV-001/002/004, AC-001)
- Task: `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md`
- Remediations applied: TASK-001-R1 (`932f6a5`), TASK-001-R2 (`cbf517a`)
- Diff reviewed: `git diff 9715440d..1fbcc4a -- . ':!changes/active/repo-parser/review'` (14 files, +1120/-59)
- Conclusions formed from source, diff, and the verification runs listed below. No other reviewer report or verdict was read.

## Acceptance matrix

| Requirement / criterion / constraint | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001 export byte length + declaration facts per path | `SymbolIndex::export` (`crates/bohmian-code-review-index/src/lib.rs:127-142`) emits `IndexedFileFacts { path, extraction_version, byte_len, declarations }`; `SymbolicContext::export` (`crates/bohmian-code-review/src/lib.rs:177-179`) stamps `EXTRACTION_VERSION` | journey `exported_facts_reload_without_source_and_reject_invalid_records` asserts paths, `byte_len == USER.len()`, declaration names/lines; index `export_round_trips_through_load_and_rejects_invalid_records` | PASS |
| REQ-001 load into a fresh index without parsing | `SymbolIndex::load` (`index/src/lib.rs:156-165`) validates then `upsert`s rebuilt nodes; `SymbolicContext::load` (`facade/src/lib.rs:192-204`) | journey loads from JSON into `SymbolicContext::new()` and asserts `file_count`, `byte_len`, `lookup` | PASS |
| REQ-001 projection contains only lookup/ranking/span fields, no source fragments | `DeclarationFact` (`core/src/facts.rs:18-35`) carries id, kind, name, qualified_name, language, start/end line, start_column only; `into_node` zeroes every optional. Select consumes only `node.id/start_line/end_line` (`select/src/lib.rs:221-224`); index posts on `name`/`qualified_name`/`start_line`/`start_column`/`id` (`index/src/lib.rs:62-76`) | core `projection_is_source_free_and_rebuilds_a_node`; journey asserts JSON lacks `console.log`, `return { id }`, `(id: string)`, `docstring`, `signature` | PASS |
| REQ-001 loaded facts give same ranking and candidate context as fresh parse | Ranking depends only on posted names + id + lines, all retained; posting order key `(path, start_line, start_column, id, slot)` preserved because export filters in extractor order | journey asserts `reloaded == fresh` ranking and equal `build_context` results | PASS |
| REQ-001 invalid paths fail explicitly | `RelativePath` custom `Deserialize` routes through `RelativePath::new` (`core/src/relative_path.rs:83-96`); `IndexedFileFacts.path: RelativePath` cannot hold a non-canonical path | journey asserts `serde_json::from_str` of `../user.ts` record errors | PASS |
| REQ-001 malformed facts fail explicitly without corrupting other files | Empty identity and non-ascending/zero span rejected before index mutation (`facts.rs:88-99`, `index/src/lib.rs:157`); version checked before load (`facade/src/lib.rs:193`). Gap: a fact whose `kind` is `File`/`Import`/`Export`/`Parameter` passes `validate` and is silently dropped by `upsert` (`index/src/lib.rs:62`) — see TASK-A-1 | journey + index tests cover span/identity/version rejection and untouched siblings; no test covers a non-declaration `kind` | FAIL (TASK-A-1) |
| REQ-002 extraction version exposed, stamped on facts, mismatch refuses load | `EXTRACTION_VERSION` (`extract/src/lib.rs:113-119`), re-exported by facade; `Error::ExtractionVersionMismatch` returned before any mutation | journey asserts mismatch error with path/found and unchanged `file_count`/lookup afterwards | PASS |
| REQ-002 version "changes when extracted facts or grammar behavior changes" | Maintained constant naming kernel SHA, tree-sitter pin, and `facts-1`; the task file records this as a limit (no mechanical coupling) | N/A — no automated check exists; the constant is a convention. Not a defect of this task: the spec does not require automation, and the pinned-grammar fixtures are the change detector | PASS (noted limit) |
| REQ-002 path-only eligibility; ambiguous routes may use content at parse time | `Extractor::path_language` (`extract/src/extractor.rs:66-72`) = `router.detect(path, None)` then `CompositeKind::from_discovered(...).is_some() || (native && enabled)`; this mirrors the dispatch in `CompositeExtractor::extract` (`composite/mod.rs:273-302`) so a `Some` path is exactly one `extract_from_source` would attempt. Facade `path_language` validates the path first | extract `path_language_reports_eligibility_without_source`; journey `path_language_identifies_eligible_paths_before_download` (`.h` → C, `.md`/`Makefile` → None, `/etc/passwd` → `InvalidPath`) | PASS |
| REQ-003 union of changed-line names across files | `ChangedFile::query_names` + `rank_files` chains all files' names into one `resolve` call (`select/src/lib.rs:151-216`); `resolve` dedups via `BTreeSet` | select `multi_file_ranking_unions_names_and_excludes_every_changed_path`; journey `multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only` | PASS |
| REQ-003 every changed path excluded | `excluded: BTreeSet<&RelativePath>` filter on every hit (`select/src/lib.rs:210-213`) | journey asserts no `src/report.ts` in ranking; select test excludes both paths | PASS |
| REQ-003 score once per distinct matched name | `score: names.len()` over a `BTreeSet<String>` (`select/src/lib.rs:232-234`) | journey: `lookupUser` referenced by both changed files scores `src/user.ts` at 2 (`lookupUser`, `saveUser`), not 3 | PASS |
| REQ-003 sort by descending score then canonical path | `sort_by(score desc, path asc)` (`select/src/lib.rs:239-243`); order-independence of input files proven | select test asserts `rank_files([other, handler]) == rank_files([handler, other])` | PASS |
| REQ-003 candidate-only packing without a changed-head blob, counts only selected candidates, explicit omissions | `pack_candidates` starts from `Context::default()` (files = 0) and shares `pack_candidates_into` with `pack` (`select/src/lib.rs:395-472`); facade `build_candidate_context` | select `candidate_packing_counts_only_selected_candidates` (files/tokens, `TokenLimit`/`FileLimit` omissions, `max_files = 0` all-omitted); journey asserts spans, `(files, tokens) == (1, 11)`, omissions, determinism | PASS |
| REQ-003 existing single-file ranking and changed-hunk packing remain compatible | `rank` wraps one `ChangedFile` and calls `rank_files`; `pack` retains `files: 1`, `NoFileBudget`, hunk-first behavior and delegates the candidate loop | pre-existing journey tests (3) and select tests unchanged and green; select test asserts `rank_files([handler]) == rank(...).files`; journey asserts `single.files[0] == ranked[0]` | PASS |
| INV-001 six crates keep no-IO boundary and dependency direction | Diff adds no IO, network, provider, or storage code; only `serde_json` as facade dev-dependency (`crates/bohmian-code-review/Cargo.toml`, `Cargo.lock` +1 line); dependency edges unchanged (core → index/extract → select → facade) | `cargo tree` not needed: `Cargo.lock` diff shows only the facade's `serde_json` edge; `mise run lints` green | PASS |
| INV-002 honest coverage | No code in the diff claims repository completeness; `Context.omissions` reports every dropped candidate; docs (AGENTS.md, design) restate "never report a partial caller-supplied index as complete" | journey asserts omission lists exhaustively | PASS |
| INV-004 determinism, name ambiguity preserved | `BTreeMap`/`BTreeSet` throughout; `resolve` returns all hits; `rank_files` output independent of input order | select and journey assert repeated `build_candidate_context` calls and swapped input order give identical results | PASS |
| AC-001 journey extended (export/load equivalence, two-file ranking, exclusion, distinct-name scoring, candidate-only packing); single-file green; `mise run gate` passes | Three new journey tests in `crates/bohmian-code-review/tests/symbolic_context_journey.rs`; three pre-existing journey tests untouched | `mise run test:symbolic-context:journey`: 6 passed; `mise run gate`: exit 0 | PASS |
| Task: authorities updated before implementation | `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md` distinguish six symbolic crates from the outer pipeline crate; commit `a59e51d` precedes the code commits | `mise run check:skills-sync` exit 0 | PASS |
| Task: prohibited — no GitHub/storage/DB/model code in the six crates; no grammar pin or golden change | Diff touches no `grammars/`, no fixtures, no `[dependencies]` besides the dev-dep | `mise run test:extract` 140 passed (corpus fixtures intact) | PASS |
| Task: write set (six crates, nearest tests, journey, architecture docs, manifest/lockfile as needed) | 14 files, all inside that set plus the task document's evidence section | `git diff --stat` | PASS |
| Task: REFACTOR — share name collection/scoring with single-file path | `rank` delegates to `rank_files`; `pack` delegates to `pack_candidates_into` | source read | PASS |
| Task: rustdoc on every new/modified item, `# Errors` on fallible fns, no unchecked `unwrap` | All new items documented; `unwrap()`/`#[allow]`/`#[ignore]` absent from added lines | `grep` of diff; `mise run lints` (clippy `-D warnings` + test-placement script) exit 0 | PASS |
| AC-R1-1 task status is `review` | `tasks/TASK-001-symbolic-library-contract.md:4` | `sed -n 4p` prints `status: review` | PASS |
| AC-R1-2 no `status: implemented` sibling | — | `rg -n '^status: implemented' changes/` exit 1 | PASS |
| AC-R1-3 no collateral change | — | `git diff --stat 3e71501..932f6a5`: 1 file, +1/-1 | PASS |
| AC-R2-1 every named test has a focused-command row (>= 9) | Task file "Focused commands" table, 9 rows covering all 8 named tests plus both `facts::tests` | `rg -c 'mise exec -- cargo test --locked' <task file>` prints `9` | PASS |
| AC-R2-2 each command exits 0 with `1 passed` | — | All nine rerun sequentially; each exit 0, `1 passed; 0 failed` (details below) | PASS |
| AC-R2-3 no collateral change | — | `git diff --stat 1108731..cbf517a -- . ':!changes/active/repo-parser/review'`: 1 file, +14/-0 | PASS |
| Git rules: local identity, no AI co-author trailers | — | `git log 9715440d..1fbcc4a`: all 12 commits by `Thorrester <sjforrester32@gmail.com>`, no trailers | PASS |

### Ponytail ladder on changed surfaces

- `core::facts` (`DeclarationFact`, `IndexedFileFacts`, `InvalidFact`): required; no existing type is source-free and serializable with a validating path. Keep.
- `SymbolIndex::export/load`: reuses `upsert`/`is_declaration`; no duplicate posting logic. Keep.
- `EXTRACTION_VERSION` constant: minimum mechanism for REQ-002. Keep.
- `Extractor::path_language`: no existing source-less eligibility query; it re-states the `CompositeExtractor::extract` dispatch rule in one expression rather than duplicating the match. Keep.
- `select::ChangedFile`, `rank_files`, `pack_candidates`, `pack_candidates_into`: `rank`/`pack` now delegate, so there is one owner of name collection, scoring, and candidate packing. Keep.
- Facade `ChangedBlob`, `rank_files`, `build_candidate_context`, private `prepare_changed`/`extract`: `upsert` and `rank` now share `extract`; no duplicated stage logic. Keep.
- `serde_json` facade dev-dependency: already a workspace dependency; the journey's JSON round trip proves the persisted projection revalidates paths, which a `Debug` string could not. Keep.
- No DRIFT found.

## Findings

### TASK-A-1 — INCORRECT

- **Root cause:** `DeclarationFact::validate` is documented as checking "the invariants lookup and span selection rely on", but it does not check the one invariant the index actually gates posting on: `is_declaration(kind)`. The first owner producing the wrong decision is `DeclarationFact::validate` (`crates/bohmian-code-review-core/src/facts.rs:88`).
- **Defect class:** incomplete invariant validation at a persistence trust boundary (silent drop instead of explicit rejection).
- **Locations (one finding):**
  - `crates/bohmian-code-review-core/src/facts.rs:88-99` — `validate` checks identity (`:89`) and span (`:94`) only.
  - `crates/bohmian-code-review-index/src/lib.rs:157` — `load` trusts `validate`, then `upsert` skips non-declaration kinds at `:62` without reporting.
  - `crates/bohmian-code-review/src/lib.rs:193-204` — facade `load` adds only the version check; inherits the gap.
- **Violated obligation:** `changes/active/repo-parser/spec.md:55` (REQ-001: "Invalid paths or malformed facts fail explicitly"); `architecture/agent-rules.md:6` ("Do not turn an omitted file or budget failure into apparent success").
- **Evidence:** `IndexedFileFacts { declarations: [DeclarationFact { kind: NodeKind::Import, name: "foo", id: "x", qualified_name: "foo", start_line: 1, end_line: 1, .. }], .. }` passes `validate` (no branch inspects `kind`), so `SymbolIndex::load` returns `Ok(())`; `upsert` hits `if !is_declaration(node.kind) { continue; }` and posts nothing; `lookup("foo")` is empty and a subsequent `export` omits the record. Reachable only through caller-constructed or externally altered facts, never through `export` → `load` of this crate's own output (export filters to declarations at `index/src/lib.rs:137`). No sibling: `validate` is the only fact validator and every load path routes through it.
- **Observable consequence:** a fact record the loader claims to have accepted is invisible to lookup, ranking, and packing, and the caller receives success. Other files are not corrupted, so only the "fail explicitly" clause of REQ-001 is missed.
- **Testable correction:** add an `InvalidFact::NotDeclaration { id, kind }` arm and reject in `DeclarationFact::validate` when `!is_declaration(self.kind)` (move or mirror the four-kind predicate into core, since `is_declaration` currently lives in `index`); extend `validation_rejects_empty_identity_and_bad_spans` (core) and `export_round_trips_through_load_and_rejects_invalid_records` (index) with a `NodeKind::File` fact asserting `Err(InvalidFact::NotDeclaration { .. })` and unchanged `file_count`.

## Verification run (observed)

All commands run at HEAD `1fbcc4a`, clean tree, via `mise` (rust 1.94.0, `jobs = 1`).

| Command | Observed |
|---|---|
| `mise run fmt:check` | exit 0 |
| `mise run lints` | exit 0 (clippy `-D warnings` and `scripts/check-test-placement.sh`) |
| `mise run test:extract` | core 27 passed; extract 140 passed; exit 0 |
| `mise run test:index` | 4 passed; exit 0 |
| `mise run test:resolve` | 1 passed; exit 0 |
| `mise run test:select` | 10 passed; exit 0 |
| `mise run test:symbolic-context:journey` | 6 passed; exit 0 |
| `mise run test:review-scripts` | exit 0 |
| `mise run check:skills-sync` | exit 0 |
| `mise run gate` | exit 0 |
| `git diff --check` | exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey exported_facts_reload_without_source_and_reject_invalid_records -- --exact` | 1 passed; 0 failed; 5 filtered out; exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey path_language_identifies_eligible_paths_before_download -- --exact` | 1 passed; 5 filtered out; exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only -- --exact` | 1 passed; 5 filtered out; exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-index --lib tests::export_round_trips_through_load_and_rejects_invalid_records -- --exact` | 1 passed; 3 filtered out; exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-extract --lib extractor::tests::path_language_reports_eligibility_without_source -- --exact` | 1 passed; 139 filtered out; exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-select --lib tests::multi_file_ranking_unions_names_and_excludes_every_changed_path -- --exact` | 1 passed; 9 filtered out; exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-select --lib tests::candidate_packing_counts_only_selected_candidates -- --exact` | 1 passed; 9 filtered out; exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-core --lib facts::tests::projection_is_source_free_and_rebuilds_a_node -- --exact` | 1 passed; 26 filtered out; exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-core --lib facts::tests::validation_rejects_empty_identity_and_bad_spans -- --exact` | 1 passed; 26 filtered out; exit 0 |
| `sed -n 4p changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` | `status: review` |
| `rg -n '^status: implemented' changes/` | no matches, exit 1 |
| `rg -c 'mise exec -- cargo test --locked' changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` | `9` |
| `git diff --stat 3e71501..932f6a5` | task file only, +1/-1 |
| `git diff --stat 1108731..cbf517a -- . ':!changes/active/repo-parser/review'` | task file only, +14/-0 |
| `git log --format='%an <%ae>' 9715440d..1fbcc4a` | 12 commits, all `Thorrester <sjforrester32@gmail.com>`, no trailers |

## Verification limits

- TASK-A-1 was established by reading `validate`, `load`, and `upsert`; no scratch crate was built to execute the non-declaration-kind load (a separate target dir would rebuild every vendored grammar). The control flow has no branch that could change the outcome.
- The RED state (journey failing to compile before the capability) was not reproduced; the task's RED claim is a compile failure on missing public items, which is consistent with the diff adding exactly those items.
- The `EXTRACTION_VERSION` bump discipline is a convention, not a check; this review did not attempt to prove the constant would change under a grammar edit, because nothing in the spec or task requires automation.

Overall: FAIL

```citations
[
  {"id": "TASK-A-1", "path": "crates/bohmian-code-review-core/src/facts.rs", "line": 88, "snippet": "    pub fn validate(&self) -> Result<(), InvalidFact> {", "symbols": ["DeclarationFact::validate"]},
  {"id": "TASK-A-1", "path": "crates/bohmian-code-review-core/src/facts.rs", "line": 89, "snippet": "        if self.id.is_empty() || self.name.is_empty() || self.qualified_name.is_empty() {", "symbols": ["DeclarationFact::validate"]},
  {"id": "TASK-A-1", "path": "crates/bohmian-code-review-core/src/facts.rs", "line": 94, "snippet": "        if self.start_line == 0 || self.start_line > self.end_line {", "symbols": ["DeclarationFact::validate"]},
  {"id": "TASK-A-1", "path": "crates/bohmian-code-review-index/src/lib.rs", "line": 157, "snippet": "        facts.validate()?;", "symbols": ["SymbolIndex::load"]},
  {"id": "TASK-A-1", "path": "crates/bohmian-code-review-index/src/lib.rs", "line": 62, "snippet": "            if !is_declaration(node.kind) {", "symbols": ["SymbolIndex::upsert", "is_declaration"]},
  {"id": "TASK-A-1", "path": "crates/bohmian-code-review/src/lib.rs", "line": 193, "snippet": "        if facts.extraction_version != EXTRACTION_VERSION {", "symbols": ["SymbolicContext::load", "EXTRACTION_VERSION"]}
]
```
