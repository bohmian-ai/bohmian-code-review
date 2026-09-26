# TASK-001 task review — sample B

Subject: TASK-001 symbolic library contract, base `9715440d` → candidate `34f4ae71` (HEAD), spec SPEC-repo-parser@10. Reviewed as a fresh implementation reviewer against the task contract above "## Implementation Evidence"; the evidence section was treated as an unverified claim.

## Acceptance matrix

| Requirement / criterion / constraint | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001 export contains byte length + declaration facts, no source fragments | `crates/bohmian-code-review-core/src/facts.rs:18-35` (`DeclarationFact` has only id/kind/names/language/lines/start_column), `:107-116` (`IndexedFileFacts.byte_len`), `crates/bohmian-code-review-index/src/lib.rs:127-142` (`export`) | journey `exported_facts_reload_without_source_and_reject_invalid_records` asserts JSON lacks `console.log`, `return { id }`, `(id: string)`, `docstring`, `signature`; core `projection_is_source_free_and_rebuilds_a_node` | PASS |
| REQ-001 load into fresh index without parsing | `index/src/lib.rs:156-165` (`load` → `validate` → `into_node` → `upsert`), facade `src/lib.rs:192-204` | journey loads from JSON into `SymbolicContext::new()` and asserts `file_count`, `byte_len`, `lookup`; probe (below) confirmed `export(loaded) == export(fresh)` | PASS |
| REQ-001 same ranking and candidate context as fresh parse | same `upsert` path as parse; only non-posting fields dropped (`facts.rs:56-80`) | journey compares `rank` + `build_context` fresh vs loaded; probe compared `rank_files` + `build_candidate_context` fresh vs loaded → equal | PASS |
| REQ-001 invalid path / malformed facts fail explicitly without corrupting other files | `facts.rs:88-102` (`validate`), `index/src/lib.rs:157` (validate before `upsert`), `RelativePath` deserialize revalidates (`core/src/relative_path.rs:96-99`) | journey: version mismatch, zero start_line, empty name under a new path, `../user.ts` JSON all rejected; `file_count`/lookup unchanged after. Index owner test `export_round_trips_through_load_and_rejects_invalid_records`. Probe: bad `end_line=0` on an already-loaded path leaves that path's postings and byte_len intact | PASS |
| REQ-002 extraction version exposed | `crates/bohmian-code-review-extract/src/lib.rs:119`, re-exported `crates/bohmian-code-review/src/lib.rs:23` | journey asserts `EXTRACTION_VERSION` stamped on export and non-empty | PASS |
| REQ-002 version changes when facts/grammar behavior change | rustdoc convention only (`extract/src/lib.rs:113-118`); no check ties the constant to the `tree-sitter*` pins in `Cargo.toml:35-50` or to the corpus goldens | N/A — nothing fails when a pin bumps and the constant does not | FAIL (TASK-B-1) |
| REQ-002 path-only eligibility query | `extract/src/extractor.rs:69-75` (`path_language`), facade `src/lib.rs:215-218` | extract `path_language_reports_eligibility_without_source`, journey `path_language_identifies_eligible_paths_before_download`; probe: `Some(Svelte)`/`Some(Yaml)`/`Some(C)`/`None` each agreed with the outcome of a real `upsert` of that path (svelte, `conf/routes`, `.h` with C++ content, `README.md` → `UnsupportedLanguage`) | PASS |
| REQ-002 version mismatch cannot silently reuse | facade `src/lib.rs:193-199` returns `ExtractionVersionMismatch` before touching the index | journey `stale.extraction_version = "other-extractor"` rejected | PASS |
| REQ-003 union of names across changed files | `select/src/lib.rs:156-171` (`query_names`), `:211` (`flat_map`) | select `multi_file_ranking_unions_names_and_excludes_every_changed_path`; journey multi-file test | PASS |
| REQ-003 every changed path excluded | `select/src/lib.rs:210,217` (`excluded` set) | select test excludes `src/changed.ts` and `src/d.ts`; journey asserts no `src/report.ts`; probe with duplicated changed path equals single-file ranking | PASS |
| REQ-003 score once per distinct matched name | `select/src/lib.rs:219-220` (`BTreeSet<String>` per candidate, `score: names.len()`) | journey: `lookupUser` referenced in both changed files scores 1 for `src/user.ts` (total 2 with `saveUser`) | PASS |
| REQ-003 sort desc score then canonical path | `select/src/lib.rs:238-243` | journey tie `src/format.ts` before `src/unrelated.ts`; select test `rank_files(&[other, handler]) == ranked` proves order independence of input | PASS |
| REQ-003 candidate-only packing without changed head blob | `select/src/lib.rs:397-406` (`pack_candidates`), facade `build_candidate_context` `src/lib.rs:330-341` | select `candidate_packing_counts_only_selected_candidates`; journey asserts all spans are `Candidate` | PASS |
| REQ-003 counts only selected candidate files | `pack_candidates` starts from `Context::default()` (`files: 0`); `pack` still starts at `files: 1` (`select/src/lib.rs:350-353`) | select test `(files, tokens) == (2, 7)` with `max_files: 2`; journey `(1, 11)` with `max_files: 1` | PASS |
| REQ-003 explicit omissions under file/token limits | `pack_candidates_into` `select/src/lib.rs:418-470` | select and journey assert `TokenLimit`/`FileLimit` omissions; `max_files: 0` → all omitted, no read | PASS |
| REQ-003 single-file ranking and changed-hunk packing remain compatible | `rank` delegates to `rank_files` (`select/src/lib.rs:182-199`), `pack` delegates to `pack_candidates_into`; no public signature changed | pre-existing select/journey tests unchanged and green; select test `rank_files(&[handler]) == rank(...).files` | PASS |
| INV-001 six crates keep no-IO boundary and dependency direction | diff adds no IO/network/provider dependency; `Cargo.lock` delta is `serde_json` dev-dep only; grep of the crate diff for reqwest/octocrab/opendal/sqlx/tokio/std::fs/std::net → none | `mise run lints` exit 0 | PASS |
| INV-002 honest coverage (omissions, not silent drops) | `Omission` list emitted for every non-selected candidate | journey `starved.omissions.len() == 4` | PASS |
| INV-004 determinism and ambiguity preserved | `BTreeMap`/`BTreeSet` throughout `rank_files`; `resolve` returns every hit | probe `rank_files` fresh == loaded; select order-independence test; index lookup returns all hits (`lookupUser` 2 hits after Svelte declares it too) | PASS |
| AC-001 journey extended; single-file green; gate | `crates/bohmian-code-review/tests/symbolic_context_journey.rs:384-634` | `mise run test:symbolic-context:journey` exit 0; `gate` not run here (orchestrator) | PASS |
| Authority docs updated before implementation | commit `a59e51d` (docs) precedes all code commits; `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md` | read diff | PASS |
| No GitHub/storage/db/model code in the six crates | diff grep (above) | N/A | PASS |
| No grammar pin or golden changes | `Cargo.toml` workspace pins untouched; no `grammars/`, `tests/fixtures/`, or corpus file in the diff stat | N/A | PASS |
| Write set limited | 14 files: 3 authority docs, task file, `Cargo.lock`, facade `Cargo.toml`, 6 crate sources, journey | N/A | PASS |
| Ponytail ladder: `serde_json` dev-dep on facade | already a workspace dep (`Cargo.toml:28`, used by core/extract); only route to exercise the invalid-path branch of REQ-001 since `IndexedFileFacts.path` is already a `RelativePath` in memory | rung 5 (installed dependency) | PASS |
| Ponytail ladder: `ChangedFile` (select) / `ChangedBlob` (facade) | both carry exactly what the union/exclusion needs; `rank` reuses `ChangedFile` so no duplicated selector logic | N/A | PASS |
| Ponytail ladder: `DeclarationFact` field set | every field is consumed: `kind` (upsert filter), `id`/`start_line`/`start_column` (posting order, span key), `name`/`qualified_name` (postings), `end_line` (span), `language` (required by `Node`) | N/A | PASS |

## Proposed findings

### TASK-B-1 — MISSING: extraction version is a comment-enforced constant, so a grammar pin bump does not change it and old facts load silently

- Classification: Important (REQ-002 obligation satisfied by convention only).
- Root cause: `EXTRACTION_VERSION` (`crates/bohmian-code-review-extract/src/lib.rs:119`) hard-codes `tree-sitter-0.25.10` and the kernel commit as a literal. Nothing in the crate relates that literal to the exact `tree-sitter*` pins in the workspace manifest (`Cargo.toml:35-50`) or to any extraction fixture, so the property REQ-002 requires — the version "changes when extracted facts or grammar behavior changes" — holds only if a maintainer remembers the rustdoc at `lib.rs:116-118`.
- Defect class: uncoupled identity constant (invariant documented, not checked).
- Siblings: none; the single owner is the constant. Consumer of the wrong value: `crates/bohmian-code-review/src/lib.rs:193` (`load` compares against it).
- Violated obligation: `changes/active/repo-parser/spec.md` REQ-002 ("exposes an extraction version that changes when extracted facts or grammar behavior changes ... Version changes cannot silently reuse incompatible snapshots"); task acceptance "extraction changes invalidate old facts"; `architecture/agent-rules.md` line 9 ("loading facts from another version fails").
- Evidence: `rg 572d22bf|0\.25\.10` shows the literal has no other checked source; `grammar_manifest_matches_vendored_sources` (`corpus_tests.rs:948`) verifies vendored grammar identity but never touches `EXTRACTION_VERSION`. The task's own evidence section acknowledges: "`EXTRACTION_VERSION` is a maintained constant; ... not an automatic check."
- Observable consequence (reachable path): bump `tree-sitter-typescript = "=0.23.2"` → corpus goldens fail → maintainer regenerates goldens per the rustdoc's own advice → constant untouched → a consumer snapshot stamped `codegraph-kernel-572d22bf.tree-sitter-0.25.10.facts-1` passes `SymbolicContext::load` and ranks a PR against facts the new grammar would not produce.
- Testable correction (smallest): one `#[test]` in `bohmian-code-review-extract` that reads the workspace `Cargo.toml` (the `repository_root()` helper and `sha2` dev-dep already exist in `corpus_tests.rs`), hashes the `tree-sitter*` pin lines, and asserts `EXTRACTION_VERSION` contains that digest (or the exact pinned version list). Then a pin bump fails the lane until the constant is bumped. Walker-logic changes remain manual and should stay documented as such; the correction closes the mechanical dependency path, not the human one.

## Verification commands run

| Command | Exit | Observed |
|---|---|---|
| `mise run test:index` | 0 | 4 passed |
| `mise run test:select` | 0 | 10 passed |
| `mise run test:extract` | 0 | 27 passed (unit lane) |
| `mise run test:resolve` | 0 | 1 passed |
| `mise run test:symbolic-context:journey` | 0 | all journey tests passed |
| `mise run fmt:check` | 0 | clean |
| `mise run lints` | 0 | only pre-existing clang `-u tf-8` build warnings |
| `git diff --stat 9715440d..34f4ae71` | 0 | 14 files, no grammar/fixture/pin file |
| Throwaway probe `/tmp/taskb-probe` (path dep on facade, `cargo run --offline`, shared target dir) | 0 | see below |

Probe observations (not in repo): `path_language` agreed with `upsert` for `.svelte` (3 nodes), `conf/routes` (Yaml, 0 nodes), `.h` with C++ body (C route, 2 nodes), `README.md` (`None` / `UnsupportedLanguage`); a TS file with only imports exports 0 declarations and reloads with `byte_len` intact and `file_count` equal (8 == 8); `export(loaded) == export(fresh)`; `rank_files` and `build_candidate_context` equal fresh vs loaded (2 files, 27 tokens); duplicated `ChangedBlob` path equals single-file ranking; reloading an already-loaded path replaces postings (saveUser 1 → 0 → 1); a bad `end_line = 0` record on an already-loaded path is rejected and leaves its postings and `byte_len` (179) intact; `rank_files(&[])` is `Ok(0)`; `git status` afterwards showed no tracked-file change.

## Verification limits

- `mise run gate` not run (orchestrator owns it); AC-001's gate clause is taken from the lanes above plus `fmt:check`/`lints`.
- No live grammar-pin bump was performed to demonstrate TASK-B-1 (would modify tracked files); the consequence is argued from code reading and the absence of any coupling test.
- `.h` ambiguity was probed only with both C and C++ enabled (default config).

Overall: FAIL

```citations
[
  {"id": "TASK-B-1", "path": "crates/bohmian-code-review-extract/src/lib.rs", "line": 119, "snippet": "pub const EXTRACTION_VERSION: &str = \"codegraph-kernel-572d22bf.tree-sitter-0.25.10.facts-1\";", "symbols": ["EXTRACTION_VERSION"]},
  {"id": "TASK-B-1", "path": "crates/bohmian-code-review-extract/src/lib.rs", "line": 116, "snippet": "/// Bump it whenever a grammar pin in `Cargo.toml`, a walker, the decoder, or", "symbols": ["EXTRACTION_VERSION"]},
  {"id": "TASK-B-1", "path": "crates/bohmian-code-review/src/lib.rs", "line": 193, "snippet": "if facts.extraction_version != EXTRACTION_VERSION {", "symbols": ["EXTRACTION_VERSION", "extraction_version"]},
  {"id": "TASK-B-1", "path": "Cargo.toml", "line": 35, "snippet": "tree-sitter = \"=0.25.10\"", "symbols": ["tree-sitter"]}
]
```
