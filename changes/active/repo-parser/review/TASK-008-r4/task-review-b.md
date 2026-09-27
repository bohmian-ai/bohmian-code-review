# TASK-008 r4 independent task review B

**Subject:** `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c..f204d3383c62c6137a603437668103187a4fd19c` (HEAD checked at `f204d3383c62c6137a603437668103187a4fd19c`). Authority: `spec.md` revision 14, original TASK-008, R3 remediation, and the r1 maintainer decision. I inspected the cumulative code and fixture diff and traced fresh ingest, changed-tree reuse, same-tree read, and publication. `FIND-008-1` is invalid under the maintainer decision; I did not treat declarations inside `ERROR` nodes as a defect.

## Acceptance matrix

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Syntax-error files extract declarations in the recovered tree, including inside `ERROR` nodes; no whole-file deferral | The 25 ported walkers remove `has_error()` exits; `extractor.rs` maps remaining walker failures to `ExtractionFailed`; `Deferred` is removed from `error.rs`. | `recovered_tree_keeps_rust_declarations_around_a_grammar_gap`, `recovered_tree_keeps_typescript_declarations_around_a_nul_byte`, 21 `recover` corpus entries; `mise run test:extract` passed in this review. | PASS |
| Valid-file fixtures and goldens retain their contract; grammar pins and vendored C remain untouched | Manifest changes only `defer` to `recover`; no changed grammar pin, vendored C, `match` fixture, or golden appears in the cumulative diff. | Full extractor corpus and differential lane passed (`mise run test:extract`). | PASS |
| Version changes reject old facts and old snapshot scope | `extract/src/lib.rs` changes `EXTRACTION_VERSION` from `…facts-1` to `…recovered-tree.facts-2`; `snapshot.rs` changes `SNAPSHOT_FORMAT_VERSION` from 1 to 2, includes both versions in `snapshot_scope`, and `Snapshot::decode` checks both. | Public symbolic journey asserts old fact rejection; `prior_format_indexed_empty_snapshot_is_not_reused` exercises the old pointer and current-scope read; pipeline lane passed. | PASS |
| A verified no-declaration file is a blob-identified content skip while fetch, verification, and extractor failures remain incomplete | `Ingest::settle` classifies the facade/index declaration export, and `FileOutcome::Indexed` carries `IndexedFile` with nonempty declarations. `Coverage::of` derives indexed and skipped counts from outcomes; `SkipReason::is_benign` includes only LFS, oversize, and no-facts. `finish` records pending files as `NotFetched`. | `no_facts_file_is_a_recorded_skip_in_a_published_build`, `import_only_file_is_no_facts_on_ingest_and_reuse`, `indexed_outcome_without_declarations_does_not_decode`, and existing incomplete tests; `mise run test:pipeline` passed in this review. | PASS |
| R3: a prior-format indexed-empty same-tree object is rebuilt, while current-format serialization cannot encode indexed-empty facts | Versioned scope excludes prior pointer. `Snapshot.files: Vec<FileOutcome>` replaces independent fact and coverage fields; `IndexedFile::new` and deserialization require a declaration; `ColdBuild::complete` publishes those outcomes. | Persisted-object regression `prior_format_indexed_empty_snapshot_is_not_reused` covers cold build, subsequent update, and old bytes planted at the current key. `indexed_outcome_without_declarations_does_not_decode` covers round trip and rejection. | PASS |
| Changed-tree reuse and private PR overlay retain no stale posting after no-facts replacement | `Ingest::reuse` matches path and blob, loads indexed facts, then settles via the same export projection; fresh archive, fallback, and PR head ingestion use `Ingest::ingest`. `NoFacts` removes the context file. | Ingest/reuse test and existing update and PR tests in the passing pipeline lane. | PASS |
| Live Wyrd cold build indexes the two named files | Checked-in `s20-wyrd-pr1.json` reports a complete published build at commit `2855eac…`, 1,681 eligible, 1,538 indexed, 143 `NoFacts` skips; the Rust boot path is in the PR trace and neither named path occurs in the skip list. | The checked-in read-only live run was made before R3's format bump. R3 changed snapshot representation, not extraction; focused R3 persistence tests cover the new format. | PASS |
| Preserve source-free facts, verified blobs, three-step publication, and incomplete-attempt pointer safety; avoid caller guards or extra dependencies | Indexed outcomes contain `IndexedFileFacts` plus verified blob; `ColdBuild::publish` retains record → object → pointer ordering. No new dependency or caller-specific guard is in the diff. | Pipeline lane and `git diff --check` passed. | PASS |

## Proposed findings

None. The R3 representation closes the prior `FIND-008-2` class: an indexed outcome carries nonempty persisted declarations, and the old format is outside the current pointer namespace. The r1 decision keeps `FIND-008-1` withdrawn; the task and corpus wording close `FIND-008-3`.

**Verification limit:** The checked-in live Wyrd run used snapshot format 1. No second live provider run under format 2 is in the candidate. The focused current-format tests and unchanged extractor cover the relevant R3 change.

**Overall result: PASS.**

```citations
[]
```
