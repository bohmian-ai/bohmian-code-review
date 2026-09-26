# TASK-001 task review, round 2, sample B

Subject: cumulative candidate `1108731565fc8801b47f286bd04351a3c467cf90` (base `9715440d6969dd317a62bd7c2a4ff30082f6082e`), read-only. Question: does the candidate satisfy TASK-001 exactly, including remediation TASK-001-R1?

## Lanes run (observed)

| Lane | Exit | Result line |
|---|---|---|
| `mise run test:symbolic-context:journey` | 0 | 6 passed (4 pre-existing + `exported_facts_reload_without_source_and_reject_invalid_records`, `path_language_identifies_eligible_paths_before_download`, `multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only`; `pr_diff_selects_user_context_deterministically` still green) |
| `mise run test:index` | 0 | 4 passed |
| `mise run test:select` | 0 | 10 passed |
| `mise run test:extract` | 0 | 27 + 140 passed |
| `mise run fmt:check` | 0 | — |
| `mise run lints` | 0 | — |
| `mise run gate` | 0 | all lanes + skills-sync |
| `git status --porcelain` | — | clean before and after |

## Acceptance matrix

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001 export byte length + declaration facts; load without parsing | `SymbolIndex::export`/`load` (`crates/bohmian-code-review-index/src/lib.rs:120-165`), `SymbolicContext::export`/`load` (`crates/bohmian-code-review/src/lib.rs:169-204`), `IndexedFileFacts.byte_len` | Journey asserts `byte_len == USER.len()` after reload and `file_count() == 4` with no `upsert`; index test `export_round_trips_through_load_and_rejects_invalid_records` | PASS |
| REQ-001 projection excludes source fragments; only lookup/ranking/span fields | `DeclarationFact` keeps id, kind, name, qualified_name, language, start/end line, start_column (`core/src/facts.rs:17-35`); `into_node` sets docstring/signature/etc. to `None` | Journey serializes export and asserts no `console.log`, `return { id }`, `(id: string)`, `docstring`, `signature` substrings; core test `projection_is_source_free_and_rebuilds_a_node` | PASS |
| REQ-001 loaded facts give same ranking and candidate context as fresh parse | Index postings use only name/qualified_name/start_line/start_column/id (index `upsert` :59-80); `MatchReason` = name/node_id/lines; `candidate_spans` keys on lines/node_id | Journey `assert_eq!(reloaded, fresh)` on `Ranking` and `build_context(...) == build_context(...)`; index test asserts lookup keys equal and `loaded.export("v1") == exported` | PASS |
| REQ-001 invalid paths / malformed facts fail explicitly, no corruption | `RelativePath` `Deserialize` routes through `RelativePath::new` (core `relative_path.rs:83`); `IndexedFileFacts::validate` runs before `upsert` (index :158-164); facade version check before touching index | Journey: `../user.ts` JSON fails to deserialize; `start_line = 0` and empty name return `Error::InvalidFacts` with path; after both, `file_count()==4` and `lookup("lookupUser")` unchanged. Index test: invalid record leaves `keys` and `file_count` unchanged | PASS |
| REQ-002 extraction version exposed; version change cannot silently reuse facts | `EXTRACTION_VERSION` (`extract/src/lib.rs:119`) re-exported from facade; `export` stamps it; `load` rejects mismatch with `Error::ExtractionVersionMismatch` before index access | Journey `stale.extraction_version = "other-extractor"` → `ExtractionVersionMismatch { path == "src/user.ts", found == "other-extractor" }`; constant embeds `tree-sitter-0.25.10`, matching `Cargo.toml:35` `tree-sitter = "=0.25.10"` | PASS (limit: constant is hand-maintained; see Verification limits) |
| REQ-002 path-only eligibility query; ambiguous routes may still use content | `Extractor::path_language` (`extract/src/extractor.rs:62-75`) = `router.detect(path, None)` ∩ (composite kind or enabled native); facade `path_language` validates path first | Extract test `path_language_reports_eligibility_without_source` (native, `.h`→C, svelte, play routes, README/Makefile→None, disabled language→None); journey covers same plus `/etc/passwd` → `InvalidPath` | PASS |
| REQ-003 union of changed-line names across blobs; every changed path excluded; one point per distinct name; sort score desc then path | `ChangedFile::query_names`, `rank_files` with `excluded: BTreeSet` and `names: BTreeSet<String>` per file (`select/src/lib.rs:139-244`); `resolve` collapses repeated names (resolve :47-50) | Journey: `lookupUser` referenced in both changed files scores 2 for `src/user.ts` (not 3), `src/report.ts` absent from candidates, `format.ts`/`unrelated.ts` tie at 1 ordered by path; select test: reversed input order yields identical ranking, empty input → empty | PASS |
| REQ-003 candidate-only packing without changed-head blob; counts only selected candidates; explicit omissions under limits | `pack_candidates` + shared `pack_candidates_into` (`select/src/lib.rs:383-472`); facade `build_candidate_context` | Journey: `max_files: 1` yields one candidate span, `files == 1`, `TokenLimit` and two `FileLimit` omissions, all spans `SpanKind::Candidate`; token-starved case yields 0 spans, 4 `TokenLimit` omissions; select test covers `max_files: 0` → all omitted, `MissingBlob` error | PASS |
| REQ-003 existing single-file ranking and changed-hunk packing remain compatible | `rank` delegates to `rank_files(&[changed])`; `pack` retains `NoFileBudget`, changed-hunk-first, then `pack_candidates_into`; no public signature changed | Select test asserts `rank_files(&[handler]) == rank(...).files`; pre-existing journey `pr_diff_selects_user_context_deterministically` and error journey unchanged and green | PASS |
| INV-001 six crates keep no-IO boundary and dependency direction; no stage duplicated | Diff adds no IO, no new runtime dependency (`Cargo.lock` +`serde_json` dev-dep on facade only; core already depended on serde/serde_json); facade delegates to index/select | Diff inspection; `lints` exit 0; scan of `+` lines for `unwrap()`/`#[allow]`/`#[ignore]` → none | PASS |
| INV-002 honest coverage (absent candidate ≠ absence) | Facade docs retain "index covers only files the caller supplied"; omissions always reported; `path_language` returns `Option` not a coverage claim | Design doc paragraph added at `architecture/bohmian-code-review-design.md:34`; no API reports completeness | PASS (library-side portion only; snapshot/overlay coverage is outer-crate scope) |
| INV-004 determinism; name ambiguity preserved | `BTreeMap`/`BTreeSet` throughout; stable sort by (score desc, path); `lookup` still returns all hits | Journey deterministic-context test green; select test order-independence; `resolve` keeps all hits | PASS |
| AC-001 journey extends to export/load, two-file ranking, exclusion, distinct-name scoring, candidate-only packing; single-file green; gate passes | Three new journey tests in `crates/bohmian-code-review/tests/symbolic_context_journey.rs:386-634` using only `bohmian_code_review::*` | Journey exit 0 (6/6); `gate` exit 0 | PASS |
| Task AC: authorities updated before implementation (AGENTS.md, agent-rules.md, design.md) | Commit `a59e51d` (first in range) edits exactly those three files | `git log` order; diff hunks describe six-crate vs outer-crate boundary and export/version/eligibility semantics | PASS |
| Task AC: "unsupported paths are identified before download" | `path_language` takes only `&str` | Journey `README.md`/`Makefile` → `Ok(None)` with no source | PASS |
| Task AC: "extraction changes invalidate old facts" | `load` version gate | Journey stale-version assertion | PASS |
| Task AC: "existing APIs retain their behavior" | No removed/changed public signatures in diff; re-exports only added | Pre-existing 3 journey tests + owner tests unchanged and green | PASS |
| Constraint: no GitHub/storage/database/model code in six crates | Diff touches only facts/index/select/extract/facade modules | Diff inspection; `Cargo.lock` diff is one dev-dep line | PASS |
| Constraint: no grammar pin or golden/fixture change | No change under `grammars/`, `corpus_tests`, fixtures, or workspace pins | `git diff --stat` lists none; extract lane 140 corpus tests green | PASS |
| Constraint: write set = six crates, nearest tests, journey, architecture docs, manifest/lockfile as needed | 14 files, all in that set | `git diff --stat` | PASS |
| Non-goal: outer pipeline crate, GitHub client, storage traits not added | No new crate in workspace | `git diff --stat`; `Cargo.toml` unchanged | PASS |
| Scenario RED/GREEN discipline recorded | Task evidence lists compile-failure RED on missing symbols | Not independently reproducible from the immutable candidate; GREEN reproduced | PASS (RED accepted as recorded) |
| AC-R1-1 `sed -n 4p` prints `status: review` | Commit `932f6a5` | Observed `status: review` | PASS |
| AC-R1-2 `rg -n 'status: implemented' changes/` no matches | — | Anchored `^status: implemented` exit 1; unanchored hits only in `review/TASK-001-r1/*` prose quoting the finding, none in task frontmatter | PASS |
| AC-R1-3 no collateral change | — | `git diff --stat 3e71501..932f6a5` → task file only, 1 insertion, 1 deletion; `git diff --stat 34f4ae7..1108731 -- . ':!review'` → same single line | PASS |

## Ponytail ladder over changed abstractions

- `DeclarationFact` / `IndexedFileFacts` / `InvalidFact` (core): required by REQ-001 as the persistable projection; no existing type is source-free (`Node` carries docstring/signature). Keep.
- `ChangedFile<'a>` (select): the REFACTOR step in Scenario 2 explicitly requires sharing name collection between `rank` and `rank_files`; `rank` now delegates. Keep.
- `pack_candidates_into` (private): the shared loop that keeps `pack` and `pack_candidates` from diverging. Keep.
- `ChangedBlob<'a>` (facade): the public multi-blob input; `Vec<LineRange>` mirrors `rank`'s existing signature. Keep. (Derives `PartialEq, Eq` that no caller needs; trivially cheap, not reportable.)
- `SymbolicContext::extract` / `prepare_changed` (private): deduplicate decode+extract already present twice at base. Net deletion. Keep.
- `serde_json` dev-dependency on facade: the only way for the journey to prove "no source fragment" over the wire and to exercise `RelativePath` deserialization validation. Core already carries it. Keep.
- `Language` re-export from facade: needed as the `path_language` return type. Keep.
- No config surface, trait, feature flag, or compatibility shim added.

No DRIFT identified.

## Proposed findings

None. Every attempted falsification path from the public facade (stale version, malformed span, empty identity, renamed path, non-canonical path JSON, duplicate names across changed blobs, both changed paths indexed, `max_files` 0 and 1, token starvation, deletion-only ranges in `rank_files`) is covered by an executed test that I ran and observed green, or by direct source reading of the delegated pre-existing owner.

## Non-blocking observations (not findings; not required by the task)

- `DeclarationFact::validate` does not reject a hand-built fact whose `kind` is `File`/`Import`/`Export`/`Parameter`; `load` would accept it and `upsert` silently posts nothing. The export path never produces such a record, so there is no reachable path from the facade's own output. Recorded for the outer-crate task that reads facts from storage.
- `EXTRACTION_VERSION` is a hand-maintained constant. A `tree-sitter` or grammar pin bump without a constant bump would let old facts load. The spec (REQ-002) does not require automatic derivation; the implementer's evidence states the limit.

## Verification limits

- RED state (compile failure before the capability existed) was not reproduced; only GREEN was.
- `.h` eligibility when C is disabled but C++/Objective-C are enabled returns `None` from `path_language`; the facade's default `all_native()` config enables C, so this is unreachable through `SymbolicContext::new()`. Not exercised.
- I did not read the round-1 review or findings-validation files, per instruction.
- Time budget prevented a scratch crate exercising the facade from outside the workspace; the journey test is an external integration test target and stands in for that.

```citations
[]
```

Overall: PASS
