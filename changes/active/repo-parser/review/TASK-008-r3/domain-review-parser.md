# Parser provenance and facade projection domain review

**Subject:** `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c` → `f56ec0e3091f1458e80c7addb3286577527f638f` (HEAD), spec revision 14. **Result: FAIL.**

## Boundary and coverage

| Boundary | Authority and inspected source | Result |
|---|---|---|
| Native recovery in every ported language, including composite fragments | Spec REQ-002; original TASK-008 Scenario 1; maintainer decision invalidating FIND-008-1; full extraction diff, `Extractor::run_walker`, composite Svelte routing | PASS: whole-file `has_error()` rejection is gone; recovered trees reach the existing walkers. No error-region exclusion is required or proposed. |
| Grammar provenance and corpus | AGENTS.md grammar/fixture rules; TASK-008 prohibitions; grammar manifests, `corpus_tests.rs`, changed fixture manifest and diff paths | PASS: only malformed fixture expectations changed from `defer` to `recover`; clean fixture/golden and grammar files are unchanged. Native malformed fixtures assert recovery while clean fixtures retain golden comparison. |
| Error and version contract | TASK-008 Scenarios 1–2; `error.rs`, `extractor.rs`, `lib.rs`, public journey | PASS: dead `Deferred` route removed; other typed extraction failures remain; version differs from `…facts-1` and journey rejects old facts. |
| Persisted declaration projection | Spec Complete snapshot, INV-002–003; TASK-008-R2 FIND-008-2 outcome and recommendation; user instruction to make errors unrepresentable by shape; facade `upsert`/`has_declarations`, index `upsert`/`export`, pipeline `ingest`/`reuse`/`settle` | FAIL: `Indexed` with zero persistable declarations remains representable until a later boolean check removes it. |
| Recovery authority text | Spec REQ-002; maintainer decision; task outcome/stop condition and changed recovery rustdoc | PASS: wording now permits declarations inside error regions and keeps the ban on invented facts. FIND-008-3 is closed in this boundary. |

## Verification and limits

`mise run test:extract` passed (27 core and 142 extract tests). `mise run test:symbolic-context:journey` passed (6 tests). The exact base-to-candidate diff contains no grammar pin, vendored C, clean fixture, or golden changes. The recorded live `bohmian-ai/wyrd` run in task evidence was read but not independently repeated; live provider behavior remains a verification limit for this domain. Snapshot publication and provider verification are owned by the snapshot domain reviewer.

## Proposed finding

### PARSER-R3-1 — post-hoc declaration predicate leaves an empty indexed state representable

- **Classification / prior root:** VIOLATION; `FIND-008-2` class incomplete if validated.
- **Root cause and defect class:** The facade and index represent successful extraction as an unconditional indexed file of raw nodes. The new facade projection is a separate `bool`, so the type returned by `upsert` and the index's file map permit a successful `IndexedFileFacts` record with empty declarations. This is a shape defect at the facade/index projection seam, not a missing guard at an ingest caller.
- **Sibling locations:** `SymbolicContext::upsert` unconditionally calls `SymbolIndex::upsert`; `SymbolIndex::upsert` stores a file even when no node posts; `SymbolIndex::export` serializes that file with an empty declaration vector; `SymbolicContext::has_declarations` computes a later predicate; shared `Ingest::settle` uses that predicate after fresh ingest and reuse to remove the empty record. Archive ingest, blob fallback, and PR overlay all route through this shared path.
- **Violated obligation:** TASK-008-R2 says the two successful outcomes should be represented by shape as `NoFacts` or `Indexed` with nonempty persistable declarations. The current user explicitly requires errors made unrepresentable by shape, not added guards. The Complete snapshot term and INV-002 require honest persisted coverage.
- **Evidence and reachable consequence:** An import-only TypeScript file successfully extracts `File` and `Import` nodes. The index filters both from persisted declarations but still stores and exports a record; `has_declarations` returns false, and pipeline `settle` then removes it. Covered pipeline paths appear to yield the correct `NoFacts` skip, but the invalid indexed state remains reachable through the public facade and can be misclassified by any path that fails to call `settle`. The correction depends on a post-hoc branch instead of the required outcome shape.
- **Testable correction outcome:** At the shared facade/index projection owner, successful extraction and load/reuse yield either `NoFacts` or an indexed result that carries at least one persistable declaration. The pipeline consumes that outcome directly for cold build, update/reuse, and PR overlay; no downstream or per-walker guards. A focused import-only public-facade proof must show the empty indexed state is unavailable, while ordinary declaration, malformed recovery, and typed failure paths retain their behavior.

```citations
[{"id":"PARSER-R3-1","path":"crates/bohmian-code-review/src/lib.rs","line":151,"snippet":"        self.index.upsert(path.clone(), source.len(), result.nodes);","symbols":["SymbolicContext::upsert","SymbolIndex::upsert"]},{"id":"PARSER-R3-1","path":"crates/bohmian-code-review-index/src/lib.rs","line":79,"snippet":"        self.files.insert(path, IndexedFile { byte_len, nodes });","symbols":["SymbolIndex::upsert"]},{"id":"PARSER-R3-1","path":"crates/bohmian-code-review-index/src/lib.rs","line":134,"snippet":"                declarations: file","symbols":["SymbolIndex::export"]},{"id":"PARSER-R3-1","path":"crates/bohmian-code-review/src/lib.rs","line":230,"snippet":"    pub fn has_declarations(&self, path: &RelativePath) -> bool {","symbols":["SymbolicContext::has_declarations"]},{"id":"PARSER-R3-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":1025,"snippet":"        if self.context.has_declarations(&path) {","symbols":["Ingest::settle","SymbolicContext::has_declarations"]}]
```
