# Domain review: source-free facts export/load, extraction version identity, path-only eligibility

Reviewer: `domain-rev` (finding prefix `DOM-FACTS`). Subject: candidate `1108731565fc8801b47f286bd04351a3c467cf90` vs base `9715440d6969dd317a62bd7c2a4ff30082f6082e`, read-only.

## Reviewed boundary

- `crates/bohmian-code-review-core/src/facts.rs` — `DeclarationFact`, `IndexedFileFacts`, `InvalidFact`, `from_node`/`into_node`, `validate`.
- `crates/bohmian-code-review-core/src/relative_path.rs` — `RelativePath::new` and the validating `Deserialize` impl.
- `crates/bohmian-code-review-index/src/lib.rs` — `upsert`, `remove`, `lookup`, `export`, `load`, `is_declaration`, `names`.
- `crates/bohmian-code-review-extract/src/lib.rs` (`EXTRACTION_VERSION`), `src/extractor.rs` (`path_language`, `extract`, `extract_from_source`, `extract_from_source_with_language`), `src/routing.rs` (`LanguageRouter::detect`), `src/composite/mod.rs` (`CompositeKind::from_discovered`/`classify`).
- `crates/bohmian-code-review/src/lib.rs` — `SymbolicContext::export`/`load`/`path_language`/`upsert`/`remove`, `Error::InvalidFacts`, `Error::ExtractionVersionMismatch`, `relative_path`.
- Consumers of loaded nodes: `crates/bohmian-code-review-select/src/lib.rs` (reads only `kind`, `id`, `start_line`, `end_line` of index hits), `crates/bohmian-code-review-resolve/src/lib.rs` (reads `name`, `qualified_name`, `start_line`).

## Authority and source coverage

Authorities read: `changes/active/repo-parser/spec.md` REQ-001, REQ-002, INV-001, INV-002, INV-004; `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` Scenario 1 and acceptance criteria; `architecture/agent-rules.md:8-9`; `architecture/bohmian-code-review-design.md:5,26,34`; `architecture/references/domain/symbolic-context.md`; `architecture/references/languages/rust-core.md` (via router).

### Question-by-question results

1. **Do exported facts carry source text?** No. `DeclarationFact` holds `id`, `kind`, `name`, `qualified_name`, `language`, `start_line`, `end_line`, `start_column` only (`facts.rs:18-34`). `into_node` sets every optional source-derived `Node` field (`docstring`, `signature`, `decorators`, `return_type`, `type_parameters`, `extra`, ...) to `None` and `end_column` to `0`. `id` is `kind:md5hex` (`ids.rs:17-32`), not a signature. `name`/`qualified_name` are the identifiers postings are keyed on — required by REQ-001 for exact lookup and explicitly permitted ("only fields needed to reproduce exact lookup"). Observed `Debug` and serde JSON output of a real export in a scratch project (below): no fragment beyond identifiers, path, version, byte length, kind, language, coordinates. Journey asserts JSON contains none of `console.log`, `return { id }`, `(id: string)`, `docstring`, `signature`.

2. **Does load reproduce lookup and ranking identically, including replacement and removal?** Yes. `SymbolIndex::load` validates then delegates to `upsert`, which calls `remove(&path)` first, so a second load of the same path fully replaces postings (`index/src/lib.rs:58-77, 158-167`). Journey `exported_facts_reload_without_source_and_reject_invalid_records` asserts `rank` output and `build_context` output are `==` between fresh-parse and loaded contexts. Scratch run confirmed: loading `src/a.ts` a second time with a subset of declarations dropped the stale `alpha` posting for that path and left `src/b.ts`'s posting; `remove` after load empties postings and decrements `file_count`. Non-declaration nodes (File/Import/Export/Parameter) are dropped on export; they never post and `select::rank` filters index-side nodes through the same `is_declaration`, so ranking parity holds. `byte_len` is exported and restored.

3. **Do invalid facts fail without corrupting other entries; is validation before mutation?** Yes. `IndexedFileFacts::validate` runs over all declarations before `upsert` is called (`index/src/lib.rs:159`), and the facade checks the version before calling the index (`bohmian-code-review/src/lib.rs:213-219`). There is no batch entry point: both `SymbolIndex::load` and `SymbolicContext::load` are one-file-per-call, so a "batch" is the caller's loop and each element is atomic. Scratch run: an invalid record for an already-loaded path left that path's prior facts intact; a record with both a foreign version and an empty id returned `ExtractionVersionMismatch` (version checked first) with no mutation.

4. **Is the version stamped on export and refused on every load entry point?** Stamped: `SymbolIndex::export(EXTRACTION_VERSION)` via `SymbolicContext::export`. Refused: `SymbolicContext::load` is the only public load path on the facade and compares `facts.extraction_version != EXTRACTION_VERSION`. `SymbolIndex::load` deliberately does not check (documented at `index/src/lib.rs:153-155`: the index crate has no extractor dependency per the design's dependency direction, so it cannot know the version). That is consistent with `architecture/bohmian-code-review-design.md:20` ("`index` consumes core facts rather than depending on a parser"); the facade is the consumer contract for the outer pipeline and is the point where the check is required. `EXTRACTION_VERSION` embeds the kernel pin `572d22bf` and `tree-sitter-0.25.10`, which matches `Cargo.lock` (`tree-sitter` 0.25.10). The value is a maintained constant, as the task evidence records; REQ-002 requires it to change when behavior changes, not that the change be automatic.

5. **Is path-only eligibility consistent with actual routing?** Yes. `Extractor::path_language` calls the same `LanguageRouter::detect(file_path, None)` that `extract_from_source` calls with `Some(source)`; the only content-dependent branch is `.h` → C/Cpp/Objc (all three native and enabled by `all_native`), and XML → `Xml`/`MyBatis` (both composite, both `Some`). Extension lowering is `to_ascii_lowercase` in both `detect` and `CompositeKind::classify`. Scratch run over `src/A.TS`, `src/x.H`, `lib/V.SVELTE`, `lib/V.vue`, `conf/routes`, `templates/x.json`, `a.app.src`, `x.xml`, `a.cfm`, `x.md`, `x`: every `Some` path upserted successfully with empty source and every `None` path failed upsert with `UnsupportedLanguage`. Facade always builds the extractor with `ExtractionConfig::all_native`, so the `enabled_languages` term in `path_language` cannot diverge from admission.

6. **Can a caller-supplied path bypass `RelativePath` validation on load?** No. `IndexedFileFacts.path` is a `RelativePath`; its `Deserialize` impl routes through `RelativePath::new` (`relative_path.rs:82-96`), and in-process construction requires `RelativePath::new` too. Scratch run: JSON with `"../x.ts"` and `"a\\b.ts"` fails to deserialize. Journey asserts the `../user.ts` substitution fails. `SymbolicContext::path_language`/`remove`/`upsert` all go through `relative_path()`.

## Verification (observed)

- `mise run test:index` — 4 passed, exit 0.
- `mise run test:symbolic-context:journey` — 6 passed, exit 0.
- `cargo test --all-features -p bohmian-code-review-core facts` — 2 passed.
- Scratch project `/tmp/facts-scratch` (path-dependency on the facade) exercising: Debug/JSON surface dump, double-load replacement, invalid-load-on-existing-path, version-before-validation ordering, remove-after-load, serde path bypass, and path_language vs upsert agreement across 11 paths. All assertions passed ("OK").

## Verification limits

- Did not rerun `lints`, `fmt:check`, `gate`, or `test:extract` (outside this domain; other reviewers own them).
- Did not test `Extractor::path_language` under a restricted `ExtractionConfig`; the facade exposes no way to construct one, so the facade contract is unaffected.
- Did not audit whether `EXTRACTION_VERSION` would actually be bumped on a future grammar change; that is a process obligation, not a code defect, and the task evidence already records it as a limit.

## Material findings

None.

```citations
[]
```

Overall: PASS
