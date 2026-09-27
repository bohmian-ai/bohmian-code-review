---
id: TASK-008-R2
kind: remediation
status: ready
spec: SPEC-repo-parser@14
parent_task: TASK-008
remediates: [FIND-008-2, FIND-008-3]
---

# Align no-facts coverage with persisted facts and revision 14 authority

## Authority and subject

Approved spec: `changes/active/repo-parser/spec.md` revision 14 as amended at `bdca195f6f08fda8f847efdd7c0f1d2d102a0a15`, especially REQ-001, REQ-002, the Complete snapshot and Coverage terms, REQ-005–REQ-006, and INV-002–INV-003. Original task: `changes/active/repo-parser/tasks/TASK-008-recovered-tree-extraction.md`. Reviewed base: `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c`; cumulative candidate: `bdca195f6f08fda8f847efdd7c0f1d2d102a0a15`. Validated findings: `FIND-008-2` and `FIND-008-3` in `changes/active/repo-parser/review/TASK-008-r2/findings-validation.md`. The maintainer decision in `TASK-008-r1/maintainer-decision-error-regions.md` invalidates `FIND-008-1` and withdraws its remediation.

## Diagnosis and required outcome

### FIND-008-2 — no-declaration source is counted indexed

The shared `Ingest::ingest` at `crates/bohmian-code-review-pipeline/src/snapshot.rs:1005-1015` calls a successful upsert `NoFacts` only when all raw extractor nodes are `File`. That is the wrong shape for the persisted snapshot: `SymbolIndex::export` at `crates/bohmian-code-review-index/src/lib.rs:123-140` retains only declaration facts and drops `File`, `Import`, `Export`, and `Parameter` nodes. `SymbolicContext::upsert` at `crates/bohmian-code-review/src/lib.rs:148-157` exposes the unfiltered nodes, allowing an `Indexed` outcome with zero persisted declarations.

A public-facade probe of `src/only.ts` containing `import { X } from './x';` produced `(File, Import)` nodes and an exported `IndexedFileFacts` with empty `declarations`. The candidate takes `snapshot.rs:1012-1015`, increments `indexed`, and records no skip. The cold build can publish complete coverage even though this eligible file has no persisted symbol facts or blob-identified `NoFacts` reason. The new `no_facts_file_is_a_recorded_skip_in_a_published_build` test covers only file-node-only output, so it does not catch the case. The defect class is a raw-node classifier disagreeing with the durable fact projection, not a missing error check.

All sinks share this owner: verified archive ingest (`snapshot.rs:900-905`), verified blob fallback used by cold build/update/overlay (`:966-985`), and PR head ingestion (`crates/bohmian-code-review-pipeline/src/pull_request.rs:432`). `Ingest::reuse` at `snapshot.rs:944-952` also loads a prior empty-declaration record as indexed, so an update can preserve the false status. Publication writes `context.export()` alongside coverage at `snapshot.rs:648-692`.

**Outcome:** A successfully extracted eligible file with no persisted declaration facts is a recorded `NoFacts` content skip with its verified path/blob and no indexed fact record. An indexed outcome carries at least one persisted declaration. Cold build, update/reuse, and PR overlay agree on that classification. Fetch, verification, and extraction failures remain non-benign and cannot become apparent success.

**Recommendation:** Reuse the facade/index declaration projection as the ingest result's source of truth and represent the two successful outcomes by shape: `NoFacts` or `Indexed` containing nonempty persistable declarations. Reconcile prior empty-declaration records through the same outcome when reused, because candidate snapshots may share this extraction version. Replace the pipeline's raw `NodeKind` classification and remove its unnecessary re-export if no consumer remains. This closes every listed sink at the common owner without adding downstream guards, duplicating the declaration-kind rule, introducing a dependency, or changing approved behavior. Keep path/blob verification and source-free serialization at their current owners.

### FIND-008-3 — documentation states the withdrawn rule

The amended spec at `changes/active/repo-parser/spec.md:56,149` requires declarations recovered inside error regions. The active task still says “outside the error region” at `tasks/TASK-008-recovered-tree-extraction.md:12,32` and makes inside-region facts a stop condition at `:77`. Changed `crates/bohmian-code-review-extract/src/corpus_tests.rs:8-11,1648-1653` says the region's declarations are dropped; nearby focused-test rustdoc in `extractor.rs:393-411` describes the same obsolete rule where it speaks generally. The task's stop condition conflicts directly with the higher-authority approved spec and maintainer decision, and the changed rustdoc states a false invariant. Existing recovery assertions prove known names survive; they do not exclude names inside error regions.

**Outcome and recommendation:** Delete the superseded exclusion/stop wording and align the task and changed recovery documentation with amended REQ-002. Keep fixture-specific statements factual, the valid-fixture golden rule, and the prohibition on inventing facts absent from tree-sitter's recovered tree. No parser logic, fixture expectation, or grammar change is needed for this finding.

## Constraints and non-goals

Do not add guards at pipeline callers or per-language walkers. Do not generate an interface declaration for Kotlin `fun interface` when the pinned grammar recovered none; `PARSER-008-1` was rejected. Do not change grammar pins, vendored C, clean fixtures/goldens, provider admission, blob verification, source lifetime, or typed failure classification. Do not reopen withdrawn `FIND-008-1`. Preserve complete publication for genuinely benign content skips and incomplete coverage for fetch, verification, and extractor failures.

The correction write set includes the public facade/index projection seam as needed, shared snapshot ingest and reuse, their focused tests and public journey where behavior changes, and the active task plus changed recovery documentation. Keep the actual diff to the minimum needed at those owners.

## Acceptance and proof

1. `FIND-008-2`: A fake-provider cold build with one normal declaration file and one valid import-only TypeScript file publishes complete coverage with one indexed file. The import-only file is absent from exported fact records and present once in `coverage.skipped` as `NoFacts` with its exact blob.
2. `FIND-008-2` siblings: An update/reuse from a source-free empty-declaration record converges to the same skip, and a PR head overlay of an import-only file reports the same classification. The normal declaration, malformed no-declaration, fetch-failure, and verification-failure paths retain their existing outcomes; no stale posting survives replacement.
3. `FIND-008-3`: The task outcome, scenario description, stop condition, corpus module/rustdoc, and relevant focused-test rustdoc no longer state that recovered declarations inside error regions are dropped. They agree with spec REQ-002 and preserve the ban on invented facts.
4. Run the focused pipeline and facade proof, then `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run test:extract` if extractor docs/tests are touched, `mise run fmt:check`, `mise run lints`, and `git diff --check`. The public journey and existing gate must remain green. Route this task directly to `$cr-implement` and review the next cumulative candidate against the original TASK-008.
