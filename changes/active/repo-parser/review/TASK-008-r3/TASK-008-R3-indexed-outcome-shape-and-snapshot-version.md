---
id: TASK-008-R3
kind: remediation
status: ready
spec: SPEC-repo-parser@14
parent_task: TASK-008
remediates: [FIND-008-2]
---

# Make indexed snapshot outcomes carry nonempty declarations

## Authority and subject

Approved spec: `changes/active/repo-parser/spec.md` revision 14, especially the Complete snapshot term, REQ-001, REQ-005–REQ-006, and INV-002–INV-003. Original task: `changes/active/repo-parser/tasks/TASK-008-recovered-tree-extraction.md`. Prior remediation: `changes/active/repo-parser/review/TASK-008-r2/TASK-008-R2-no-facts-projection-and-authority.md`. Reviewed base: `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c`; cumulative candidate: `f56ec0e3091f1458e80c7addb3286577527f638f`. Validated `FIND-008-2` is `CLASS_INCOMPLETE` in this directory's `findings-validation.md`. The r1 maintainer decision invalidates `FIND-008-1`; `FIND-008-3` is closed.

## Diagnosis

The prior candidate could publish a verified import-only TypeScript file as indexed because raw `File` and `Import` nodes counted as successful extraction, while `SymbolIndex::export` persisted only declarations. It produced `IndexedFileFacts { declarations: [] }`, `coverage.indexed = 1`, and no blob-identified `NoFacts` skip. R2 now handles fresh ingest and changed-tree reuse by querying `SymbolicContext::has_declarations` after upsert/load and then choosing indexed or skip. That boolean decision fixes the tested examples but leaves the durable result representable as parallel, contradictory `Snapshot.files`, `Snapshot.facts`, and `Snapshot.coverage` fields (`snapshot.rs:175-179`), with unrestricted empty `IndexedFileFacts.declarations` (`core/src/facts.rs:107-115`). The owner defect class is a persisted indexed/no-facts outcome whose shape permits indexed with no persistable fact.

The prior and current candidates use the same extraction version and snapshot format version 1. A valid prior object and pointer therefore pass digest, identity, tree, version, and numeric completeness checks in `Snapshots::read`/`Snapshot::decode` (`snapshot.rs:264-281,190-219`). At the same default-branch tree, `ColdBuild::run` → `ColdBuild::build` returns `Reused` with the old indexed coverage (`snapshot.rs:511-515,543-555`) before `Ingest::reuse` runs. `Update::run` delegates its same-tree case to `build` (`update.rs:70-76`); `PrContext::snapshot` reads the old object before its private overlay can normalize it (`pull_request.rs:371-384`). Fresh archive ingestion (`snapshot.rs:896-905`), verified blob fallback (`:964-992`), changed-tree reuse (`:935-957`), and PR head ingestion (`pull_request.rs:432`) share the current settlement path, but do not repair that published same-tree pointer. `ColdBuild::complete` still constructs fact records and coverage independently (`snapshot.rs:648-692`). These are siblings of one representational root cause.

The observable result is a public cold build or same-tree update reporting complete coverage with an import-only path indexed, no persisted declaration, and no `NoFacts` skip carrying its verified blob. The current direct `Ingest::reuse` test bypasses the existing-pointer branch. The source trace in `findings-validation.md` reproduces the path; a persisted-object regression is still needed.

## Required correction

Use the existing facade/index declaration projection as the indexed fact source. At the candidate-owned snapshot outcome boundary, represent each eligible file as one mutually exclusive durable outcome: **indexed with nonempty persistable declarations**, or **skipped with verified path, blob, and reason**. Derive indexed/skipped coverage from those outcomes. Replace the separate boolean classification and parallel state that allow an indexed-empty, unskipped eligible file to be serialized as complete. This is one shape correction at the common owner, including fresh ingest and reuse; do not add guards to `ColdBuild::build`, update, PR overlay, or individual language walkers.

Retire the old permissive snapshot namespace using the existing snapshot format/versioned scope mechanism, so a prior same-tree pointer cannot be read as a current complete snapshot. The ordinary cold path builds and publishes the new source-free outcome under its new namespace. Keep immutable object identity, record-before-object publication, pointer compare-and-swap, verified blob identity, and the current extractor version semantics. This is a candidate-owned persisted-shape change within approved behavior; no new product or consumer decision is needed.

## Preserved behavior and non-goals

Keep recovered declarations inside tree-sitter `ERROR` regions. Keep clean fixtures/goldens, grammar pins, source lifetime, provider admission and blob verification, benign LFS/oversize/no-facts skips, and nonbenign fetch, verification, and extraction failures. Preserve changed-tree reuse, private PR overlays, source-free snapshots, and prior-pointer safety on incomplete attempts. Do not reopen withdrawn `FIND-008-1`, synthesize declarations absent from the recovered tree, introduce a dependency, or add caller-specific guards.

## Acceptance and proof

1. `FIND-008-2` same-tree cold-build sibling: seed a fake store with a prior-format, valid-digest complete snapshot and pointer whose verified import-only file has empty declarations and indexed coverage. Calling the public cold-build facade at the same tree must not return that pointer as reusable complete coverage. The current-format result records the exact blob once as `NoFacts`, contains no indexed-empty fact record, and preserves a normal declaration file as indexed.
2. `FIND-008-2` same-tree update/read siblings: the update facade at that tree reaches the same current-format result, and a current-scope read cannot report the old indexed-empty object as complete. Changed-tree reuse and PR head overlay classify import-only source consistently; no stale posting survives replacement.
3. The current-format serialized outcome cannot encode an indexed eligible file without at least one persisted declaration. Coverage counts and skips come from the same outcomes. Normal declaration and benign/no-facts cases round-trip; fetch, verification, and extractor failures remain visibly incomplete.
4. Record the exact focused command for the new persisted-object regression and run it. Run `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `git diff --check`; run `mise run test:extract` if extractor code or tests change. Keep the existing public journey and gate green.

The correction write set includes the pipeline snapshot representation, ingest and reuse, publication/read/version scope, and focused tests, plus the facade/index projection seam if needed to make the indexed outcome carry the actual persisted declarations. Route directly to `$cr-implement`; the next review reassesses the cumulative candidate against original TASK-008.
