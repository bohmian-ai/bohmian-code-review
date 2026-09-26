# TASK-001 task review — sample A (task-rev)

**Subject:** TASK-001 symbolic library contract, base `9715440d` → candidate `34f4ae71` (HEAD, `review-helper-setup`). Spec `changes/active/repo-parser/spec.md` rev 10, obligations REQ-001, REQ-002, REQ-003, INV-001, INV-002, INV-004, AC-001. Reviewed read-only through the public `bohmian-code-review` facade and the owner crates it composes.

## Acceptance matrix

| Requirement / criterion / constraint / non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001 export is source-free (no docstring/signature/return type/decorators) | `crates/bohmian-code-review-core/src/facts.rs:17-34` `DeclarationFact` has only id/kind/name/qualified_name/language/lines/start_column; `into_node` (`:56-78`) fills every source-derived field with `None` | core `projection_is_source_free_and_rebuilds_a_node`; journey `exported_facts_reload_without_source_and_reject_invalid_records` asserts serialized JSON contains no `console.log`, `return { id }`, `(id: string)`, `docstring`, `signature` | PASS |
| REQ-001 byte length exported per path | `index/src/lib.rs:127-142` `export` copies `file.byte_len`; `IndexedFileFacts.byte_len` (`facts.rs:112`) | journey asserts `user.byte_len == USER.len()` and `loaded.byte_len(user) == Some(USER.len())` | PASS |
| REQ-001 load into a fresh index without parsing | `index/src/lib.rs:156-165` `load` → `into_node` → `upsert`; facade `load` (`bohmian-code-review/src/lib.rs:192-204`) never calls the extractor | journey loads 4 files from JSON into `SymbolicContext::new()`; `test:index` `export_round_trips_through_load_and_rejects_invalid_records` | PASS |
| REQ-001 same ranking and candidate context as fresh parse | Ranking reads only postings (`resolve` → `SymbolIndex::lookup`) and `hit.node.{id,start_line,end_line}` (`select/src/lib.rs:215-227`); all of these survive the projection. `candidate_spans` groups by `(lines, node_id)` only | journey: `assert_eq!(reloaded, fresh)` on `Ranking` and equal `build_context` results | PASS |
| REQ-001 invalid path fails explicitly | `RelativePath` `Deserialize` revalidates via `RelativePath::new` (`core/src/relative_path.rs:83-99`, pre-existing); facade `relative_path` for string paths | journey asserts `../user.ts` JSON fails to deserialize; `path_language("/etc/passwd")` → `InvalidPath` | PASS |
| REQ-001 malformed facts fail explicitly without corrupting other files | `facts.rs:88-101` `validate` (empty identity, zero/reversed span); `IndexedFileFacts::validate` runs before `upsert` (`index/src/lib.rs:157`), so a rejected record leaves `facts.path` and other files untouched | journey: stale-version, zero start_line, empty-name records all rejected; `file_count()==4` and `lookupUser` still resolves afterward. Index test asserts postings unchanged after rejection. Gap: a fact whose `kind` is File/Import/Export/Parameter is accepted silently — see TASK-A-1 | PASS (with TASK-A-1, non-blocking) |
| REQ-002 extraction version exposed | `extract/src/lib.rs:119` `EXTRACTION_VERSION = "codegraph-kernel-572d22bf.tree-sitter-0.25.10.facts-1"`; re-exported by facade `lib.rs:23` | Value cross-checked against `Cargo.toml:35` (`tree-sitter = "=0.25.10"`) and kernel commit `572d22bf` in `THIRD_PARTY_NOTICES.md:5`; journey asserts non-empty and stamped on export | PASS |
| REQ-002 version changes on fact/grammar change | Maintained constant with rustdoc naming the bump triggers (`extract/src/lib.rs:113-118`); no automatic derivation | N/A (manual discipline; task evidence states this limit honestly). Not a defect: a build-time hash would be speculative machinery beyond the task | PASS (limit noted) |
| REQ-002 path-only eligibility query | `extract/src/extractor.rs:70-75` `path_language` routes with `source = None`, admits composite kinds or enabled native languages; facade `path_language` (`lib.rs:215-218`) | extract `path_language_reports_eligibility_without_source`; journey `path_language_identifies_eligible_paths_before_download` (`.ts`, `.svelte`, `.h`→C, `README.md`→None, `Makefile`→None) | PASS |
| REQ-002 version mismatch cannot silently reuse | Facade `load` compares `facts.extraction_version != EXTRACTION_VERSION` before touching the index (`lib.rs:193-199`) | journey asserts `Error::ExtractionVersionMismatch { path: "src/user.ts", found: "other-extractor" }` | PASS |
| REQ-003 union of names across changed files | `select/src/lib.rs:211` `changed.iter().flat_map(ChangedFile::query_names)` feeds one `resolve` call | select `multi_file_ranking_unions_names_and_excludes_every_changed_path`; journey two-file case | PASS |
| REQ-003 every changed path excluded | `select/src/lib.rs:210,216` `excluded` set of all changed paths filters every hit | journey asserts no `src/report.ts` in ranking; `src/handler.ts` absent (scores list) | PASS |
| REQ-003 score once per distinct matched name | `resolve` dedupes query names in a `BTreeSet` (`resolve/src/lib.rs:47-49`), so `lookupUser` referenced in both files yields one `MatchReason`; per-file `BTreeSet<String>` sets score (`select:213-234`) | journey: `user.ts` scores 2 with `lookupUser` in both changed files; `single.files[0] == ranked[0]` | PASS |
| REQ-003 sort desc score then canonical path | `select/src/lib.rs:238-243`; `RelativePath` derives `Ord` on its canonical string (`relative_path.rs:21`) | journey: `format.ts` before `unrelated.ts` at equal score; select test asserts order-independence of input | PASS |
| REQ-003 candidate-only packing without changed head blob | `pack_candidates` (`select:397-406`) starts from `Context::default()` (files=0, no hunk); facade `build_candidate_context` (`lib.rs:330-342`) | journey asserts all spans are `SpanKind::Candidate` | PASS |
| REQ-003 counts only selected candidate files | `pack_candidates_into` increments `files` only when a span was selected (`select:470`); no initial `files: 1` | journey `(files, tokens) == (1, 11)`; select test `(2, 7)` and zero `max_files` → `(0, 0)` | PASS |
| REQ-003 explicit omissions under file/token limits | `select:428-437` FileLimit, `:448-455` TokenLimit; packing continues instead of truncating | journey asserts 3 omissions with reasons; starved case 4 TokenLimit omissions, no error | PASS |
| REQ-003 single-file ranking and changed-hunk packing compatible | `rank` wraps one `ChangedFile` and delegates to `rank_files` (`select:182-200`); `pack` keeps `files: 1` and hunk loop then delegates (`select:341-381`). No public signature changed | Pre-existing journey `pr_diff_selects_user_context_deterministically` and error test pass unchanged; select 10 tests pass | PASS |
| INV-001 six crates keep no-IO boundary, one-way deps | Diff adds no runtime dependency (only facade dev-dep `serde_json`); no fs/net code in any crate | `Cargo.lock` diff: one dev-dep line; `git diff --stat` shows only the six crates, journey, docs, manifests | PASS |
| INV-002 honest coverage (omissions and errors distinguishable) | Omissions typed; `file_count` documented as not a coverage claim; load failures typed per path | journey omission assertions | PASS |
| INV-004 determinism and ambiguity preserved | `BTreeMap`/`BTreeSet` throughout `rank_files`; `resolve` keeps all hits; `lookup` canonical order | journey reruns equal; select `rank_files(&[other, handler]) == ranked` | PASS |
| AC-001 journey extended (export/load equivalence, two-file ranking, exclusion, distinct-name, candidate-only) and single-file green | `crates/bohmian-code-review/tests/symbolic_context_journey.rs` +264 lines, 3 new tests | `mise run test:symbolic-context:journey` exit 0, 6 passed | PASS |
| Authority docs updated before implementation | `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md` distinguish six symbolic crates from outer pipeline crate | Diff read; content matches spec boundary (outer crate consumes facade only, sole IO owner) | PASS |
| Prohibited: no GitHub/storage/db/model code in six crates | None present | Diff read in full | PASS |
| Prohibited: no grammar pin or golden/fixture change | No `Cargo.toml` pin change; no corpus/fixture files in diff | `git diff --name-only` filtered for fixture/golden/corpus/toml: only facade `Cargo.toml` | PASS |
| Write set limited to owners, tests, journey, docs, manifests | 14 files, all within the permitted set (plus task-file evidence section) | Diff stat | PASS |
| Ponytail: `serde_json` dev-dependency | Workspace-installed dep, dev-only, used by the journey to prove the wire form carries no source and that path revalidation fires on deserialize | Rung 5 (already-installed dependency); the source-free check on the serialized form is not reproducible without it. Keep | PASS |
| Ponytail: `ChangedFile`, `rank_files`, `pack_candidates_into` | `rank` and `pack` now delegate; no duplicated selector logic; no trait/generic layer added | Diff read | PASS |

## Proposed findings

### TASK-A-1 — `load` silently accepts a non-declaration `kind` in persisted facts

- **Classification:** INCORRECT (minor, non-blocking)
- **Root cause:** `DeclarationFact::validate` (`crates/bohmian-code-review-core/src/facts.rs:88`) checks only empty identity and line span. `SymbolIndex::load` (`crates/bohmian-code-review-index/src/lib.rs:157`) trusts that and hands the nodes to `upsert`, whose `is_declaration` filter (`index/src/lib.rs:62`) drops File/Import/Export/Parameter kinds without error.
- **Defect class:** incomplete input validation at a persistence trust boundary.
- **Siblings:** none; `validate` is the single owner and both facade and index `load` route through it.
- **Violated obligation:** REQ-001 "malformed facts fail explicitly" (`changes/active/repo-parser/spec.md:55`); `IndexedFileFacts.declarations` is documented as "Declarations in extractor order" (`facts.rs:114`) and `export` documents "Only declarations are exported" (`index/src/lib.rs:123`).
- **Evidence:** code trace above. Reachable via the public facade: `IndexedFileFacts` fields are `pub` and `Deserialize` accepts any `NodeKind`, so a corrupted snapshot record with `kind: "import"` loads with `Ok(())`. The journey exercises the analogous hand-corruptions (zero line, empty name) and expects rejection.
- **Observable consequence:** the record is accepted, the offending fact is retained in `nodes` but never posted, and a re-export drops it. No lookup, ranking, or span output is wrong and no other file is affected, which is why this does not block acceptance.
- **Testable correction:** in `DeclarationFact::validate`, add an `InvalidFact::NotDeclaration { id, kind }` arm when `!is_declaration(kind)` (move the kind predicate to core or duplicate the four-variant match there so core stays free of the index crate); extend the index round-trip test with a `NodeKind::Import` fact and assert `Err(InvalidFact::NotDeclaration { .. })` with postings unchanged.

## Verification commands run

| Command | Observed |
|---|---|
| `mise run test:index` | exit 0 — 4 passed |
| `mise run test:select` | exit 0 — 10 passed |
| `mise run test:extract` | exit 0 — 27 passed (plus corpus lanes in same run) |
| `mise run test:resolve` | exit 0 — 1 passed |
| `mise run test:symbolic-context:journey` | exit 0 — 6 passed |
| `mise run fmt:check` | exit 0 |
| `mise run lints` | exit 0 (only pre-existing clang `-u tf-8` build-script warnings) |
| `git diff 9715440d..34f4ae71 --stat` / `--name-only` | 14 files; no fixture, golden, corpus, or grammar-pin file touched |
| `grep` of `EXTRACTION_VERSION` components vs `Cargo.toml:35` and `THIRD_PARTY_NOTICES.md:5` | tree-sitter `=0.25.10` and kernel `572d22bf` match the constant |

## Verification limits

- `mise run gate` not run (orchestrator owns it); task evidence claims exit 0.
- TASK-A-1 was established by code trace, not an executed test, because writing a scratch test into the read-only tree is prohibited and a `/tmp` crate would require a full grammar rebuild.
- `Extractor::path_language` on a restricted `ExtractionConfig` reports composite kinds (e.g. `.svelte`) whose embedded native fragments would be `DisabledLanguage` at parse time. Unreachable through the facade (`SymbolicContext::new` uses `all_native()`), and the spec allows content-time refinement, so not recorded as a finding.
- `EXTRACTION_VERSION` is a maintained constant; correctness of future bumps is a review discipline, not a check.

Overall: PASS

```citations
[
  {"id": "TASK-A-1", "path": "crates/bohmian-code-review-core/src/facts.rs", "line": 88, "snippet": "pub fn validate(&self) -> Result<(), InvalidFact> {\n        if self.id.is_empty() || self.name.is_empty() || self.qualified_name.is_empty() {", "symbols": ["validate", "InvalidFact"]},
  {"id": "TASK-A-1", "path": "crates/bohmian-code-review-core/src/facts.rs", "line": 94, "snippet": "if self.start_line == 0 || self.start_line > self.end_line {", "symbols": ["start_line", "end_line"]},
  {"id": "TASK-A-1", "path": "crates/bohmian-code-review-core/src/facts.rs", "line": 114, "snippet": "/// Declarations in extractor order.\n    pub declarations: Vec<DeclarationFact>,", "symbols": ["declarations", "DeclarationFact"]},
  {"id": "TASK-A-1", "path": "crates/bohmian-code-review-index/src/lib.rs", "line": 157, "snippet": "facts.validate()?;", "symbols": ["validate"]},
  {"id": "TASK-A-1", "path": "crates/bohmian-code-review-index/src/lib.rs", "line": 62, "snippet": "if !is_declaration(node.kind) {", "symbols": ["is_declaration"]},
  {"id": "TASK-A-1", "path": "crates/bohmian-code-review-index/src/lib.rs", "line": 123, "snippet": "/// Only declarations are exported; file, import, export, and parameter", "symbols": ["export"]}
]
```
