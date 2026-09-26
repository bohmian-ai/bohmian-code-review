# TASK-001 r3 domain review: source trust, parser provenance, fact persistence

Subject: base `9715440d`, candidate `1fbcc4af` (immutable). Reviewer: `domain-rev`, read-only.

## Reviewed boundary

- `crates/bohmian-code-review-core/src/facts.rs`: `DeclarationFact`, `IndexedFileFacts`, `validate`, `from_node`, `into_node`.
- `crates/bohmian-code-review-core/src/relative_path.rs`: `Deserialize for RelativePath`.
- `crates/bohmian-code-review-index/src/lib.rs`: `export`, `load`, `upsert`, `remove`, postings.
- `crates/bohmian-code-review-extract/src/lib.rs` (`EXTRACTION_VERSION`), `src/extractor.rs` (`path_language`, `extract_from_source`, `extract`), `src/routing.rs` (`detect`), `src/composite/mod.rs` (`classify`, `from_discovered`, `CompositeExtractor::extract`), `src/ids.rs`.
- `crates/bohmian-code-review/src/lib.rs`: `SymbolicContext::export/load/path_language`, `Error`.
- `crates/bohmian-code-review/tests/symbolic_context_journey.rs`: `exported_facts_reload_without_source_and_reject_invalid_records`, `path_language_identifies_eligible_paths_before_download`.
- Fields consumed downstream: `crates/bohmian-code-review-resolve/src/lib.rs` (`index.lookup`), `crates/bohmian-code-review-select/src/lib.rs` (`hit.node.id`, `start_line`, `end_line`).

## Authority and source coverage

- Spec REQ-001 (line 55), REQ-002 (line 56), INV-001 (85), INV-002 (86), INV-004 (88).
- `architecture/agent-rules.md` lines 8-9: malformed file must not corrupt other entries; no source retention; facts stamped with extraction version; load from another version fails.
- `AGENTS.md` ownership boundaries (core/extract/index/facade).
- Prior review reports were not read.

## Questions answered with source evidence

1. **Source fragments in exported facts.** `DeclarationFact` (facts.rs:18-35) carries only `id`, `kind`, `name`, `qualified_name`, `language`, `start_line`, `end_line`, `start_column`. `into_node` (facts.rs:56-80) sets `docstring`, `signature`, `decorators`, `return_type`, `extra`, etc. to `None`. Node IDs are `kind:<md5-hex>` (`ids.rs:17`), not source. Probe export of a C++ header confirmed: payload contained only names/qualified names/coordinates. `name`/`qualified_name` are identifiers, which REQ-001 permits. No fragment leak.
2. **RelativePath revalidation at the serde boundary.** `relative_path.rs:94-100` deserializes via `RelativePath::new`, so `../user.ts` fails at parse time; the journey test asserts this (journey.rs:476-482). Inner field is private (`pub struct RelativePath(` line 23) so callers cannot bypass `new`.
3. **Partial mutation before failure.** Facade `load` (lib.rs:192-204) compares `extraction_version` before touching the index; index `load` (index lib.rs:156-165) runs `facts.validate()` before `upsert`; `upsert` is only reached with a fully validated record. Probe: loading a record for `src/b.ts` with one bad declaration left `src/a.ts` intact, `file_count == 1`, and no `src/b.ts` entry. Journey test covers stale version, malformed span, and renamed+malformed cases with count and lookup unchanged afterwards.
4. **Load reproduces lookup/ranking.** Resolve consumes only `index.lookup` (resolve lib.rs:52). Select consumes `hit.node.id`, `start_line`, `end_line` (select lib.rs:221-224) and `hit.path`. Postings are keyed on `name`/`qualified_name` and ordered by `(path, start_line, start_column, id, slot)`; all are preserved by the projection. Journey test asserts `reloaded == fresh` for `rank` and equal `build_context` output; index test asserts `loaded.export("v1") == exported`. Probe on a `.h` file routed to C++ confirmed equal lookups for `Foo` and `ns::Baz` fresh vs loaded.
5. **`path_language` vs parse-time routing.** Both call the same `LanguageRouter::detect` (extractor.rs:71 with `None`, :207 with `Some(source)`). Source only influences the `.h` C/C++/Objective-C heuristic (routing.rs:381-396); all three are native and enabled under `all_native`, so eligibility agrees and the ambiguity is documented (extractor.rs:63-67, journey test asserts `Some(C)` for `include/api.h`). Composite classification `from_discovered` vs `classify(.., Some(text))` differs only in Xml vs MyBatis, both `Some`. Unknown extensions: `path_language` returns `None` and `CompositeExtractor::extract` returns `UnsupportedLanguage`. Disabled native language: `None` vs `DisabledLanguage`. Consistent for every route reachable through the facade.
6. **Version stamp ordering.** Compared at lib.rs:193 before `self.index.load` at :201. Index-crate `load` itself has no version concept; the version stamp is owned by the facade, which is the only entry the outer pipeline crate may use (AGENTS.md).
7. **Grammar pins and fixtures.** `git diff --stat 9715440d..1fbcc4a` touches no `Cargo.toml` under `crates/bohmian-code-review-extract`, no fixtures, no corpus goldens, no `grammars/`. `Cargo.lock` change is a `serde_json` dev-dependency on the facade only. `tree-sitter` stays `0.25.10`, matching the `EXTRACTION_VERSION` string.

## Verification run

All via `mise exec -- cargo test --locked --all-features`:

- `-p bohmian-code-review-core facts`: 2 passed (`projection_is_source_free_and_rebuilds_a_node`, `validation_rejects_empty_identity_and_bad_spans`).
- `-p bohmian-code-review-index export_round_trips`: 1 passed.
- `-p bohmian-code-review-extract path_language_reports`: 1 passed.
- `-p bohmian-code-review --test symbolic_context_journey`: 6 passed, including the export/load and `path_language` journeys.
- `/tmp/facts-probe` (path-dependent crate, `CARGO_TARGET_DIR` = workspace target, nothing written in repo): confirmed C++-routed `.h` export payload is source-free, fresh vs loaded lookups equal, malformed load leaves unrelated file and count untouched, and `python`-only `Extractor::path_language("a.svelte") == Some(Svelte)` while `extract_from_source` returns `DisabledLanguage { Typescript }`.

## Verification limits

- No full `gate` run; the extract corpus differential was not re-run because no extract fixture, walker, or grammar pin changed in the diff.
- `EXTRACTION_VERSION` is a hand-maintained constant. Nothing mechanically ties `codegraph-kernel-572d22bf` to the grammar manifest; the doc comment states the bump rule. This is within what REQ-002 asks and is not raised as a finding.

## Findings

None material.

Observation (not a finding, unreachable through the approved facade surface): `Extractor::path_language` in the extract crate reports `Some(Svelte)`/`Some(Vue)`/etc. for composite routes regardless of whether the embedded native language is enabled, while extraction then fails with `DisabledLanguage`. The rustdoc ("or is disabled") overstates the guarantee for restricted `ExtractionConfig`s. `SymbolicContext` always uses `ExtractionConfig::all_native()`, so REQ-002 as exposed by the facade is unaffected. If a later task exposes configurable languages through the facade, `path_language` should also gate composite kinds on their host native language.

Overall: PASS

```citations
[
  {"id": "C1", "path": "crates/bohmian-code-review/src/lib.rs", "line": 193, "snippet": "        if facts.extraction_version != EXTRACTION_VERSION {", "symbols": ["SymbolicContext::load", "EXTRACTION_VERSION"]},
  {"id": "C2", "path": "crates/bohmian-code-review-index/src/lib.rs", "line": 157, "snippet": "        facts.validate()?;", "symbols": ["SymbolIndex::load", "IndexedFileFacts::validate"]},
  {"id": "C3", "path": "crates/bohmian-code-review-core/src/relative_path.rs", "line": 99, "snippet": "        RelativePath::new(&raw).map_err(serde::de::Error::custom)", "symbols": ["RelativePath", "Deserialize"]},
  {"id": "C4", "path": "crates/bohmian-code-review-core/src/facts.rs", "line": 68, "snippet": "            docstring: None,", "symbols": ["DeclarationFact::into_node"]},
  {"id": "C5", "path": "crates/bohmian-code-review-extract/src/extractor.rs", "line": 71, "snippet": "        let language = self.router.detect(file_path, None);", "symbols": ["Extractor::path_language", "LanguageRouter::detect"]},
  {"id": "C6", "path": "crates/bohmian-code-review-extract/src/extractor.rs", "line": 207, "snippet": "        let language = self.router.detect(file_path, Some(source));", "symbols": ["Extractor::extract_from_source", "LanguageRouter::detect"]},
  {"id": "C7", "path": "crates/bohmian-code-review-extract/src/lib.rs", "line": 119, "snippet": "pub const EXTRACTION_VERSION: &str = \"codegraph-kernel-572d22bf.tree-sitter-0.25.10.facts-1\";", "symbols": ["EXTRACTION_VERSION"]}
]
```
