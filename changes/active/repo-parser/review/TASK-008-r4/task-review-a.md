# TASK-008 r4 independent task review A

**Subject:** base `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c`, candidate/HEAD `f204d3383c62c6137a603437668103187a4fd19c`; approved `changes/active/repo-parser/spec.md` revision 14, original TASK-008, R3 remediation, and the r1 maintainer decision. I reviewed the cumulative diff, not only R3. `FIND-008-1` is invalid: declarations recovered inside tree-sitter `ERROR` regions are required.

| Requirement, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-002: extract declarations from recovered syntax-error trees, including error regions, across ported walkers | Whole-file `has_error()` deferrals removed from language walkers; extractor dispatch no longer maps `defer:` to `Deferred`; corpus malformed cases use `recover` | `recovered_tree_*` tests, corpus recovery test, recorded `test:extract` pass | PASS |
| Preserve valid-file fixtures, goldens, grammar pins, and facts provenance | Cumulative diff changes the fixture manifest's malformed-file expectations; no clean golden or grammar pin changed | Recorded `test:extract` pass; diff inspection | PASS |
| REQ-002/INV-003: extraction identity changes and old facts cannot load | `EXTRACTION_VERSION` now ends in `recovered-tree.facts-2`; facade journey checks the previous value is rejected | Recorded public journey pass | PASS |
| Complete snapshot: each verified eligible file with no persisted declarations is a blob-identified `NoFacts` content skip | `Ingest::settle` uses the facade's `export_file` projection to construct `IndexedFile` only with nonempty declarations; otherwise removes the context entry and records `NoFacts` | `import_only_file_is_no_facts_on_ingest_and_reuse`, `no_facts_file_is_a_recorded_skip_in_a_published_build`; recorded pipeline pass | PASS |
| R3 FIND-008-2: same-tree cold build/update cannot reuse a prior-format indexed-empty snapshot; current read rejects its old shape | `SNAPSHOT_FORMAT_VERSION = 2` changes scope/key; `Snapshot` stores one `FileOutcome` per eligible file and derives coverage from outcomes; `IndexedFile` validates nonempty declarations on construction and decode | I ran exact `snapshot::tests::prior_format_indexed_empty_snapshot_is_not_reused` under `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib ... -- --exact`: 1 passed; `indexed_outcome_without_declarations_does_not_decode` recorded passing | PASS |
| R3 changed-tree reuse and PR head overlay keep classification and no stale postings | `Ingest::reuse` carries the indexed projection or skipped outcome through shared `settle`; PR overlay uses `Ingest::ingest`/`reuse`, then `finish` and derived coverage | Import-only reuse test and recorded pipeline/PR tests | PASS |
| Fetch, verification, and extractor failures stay incomplete; publication and source trust remain intact | `SkipReason::is_benign` includes only LFS, oversize, and NoFacts; `ColdBuild::complete` checks derived coverage before publication; verified archive/fallback path retained | Existing pipeline failure tests and recorded gate pass | PASS |
| Live Wyrd build indexes both named paths | Checked-in S20 report gives `Published`, 1,681 eligible, 1,538 indexed, 143 NoFacts; neither named path is skipped | Recorded `mise run live:pr` evidence | PASS |
| No new dependencies, source persistence, grammar changes, or unrelated scope | Only candidate-owned projection/outcome shape and extraction recovery changed; manifest and evidence are task-related | Cumulative diff and workspace manifest inspection | PASS |

**Prior findings:** `FIND-008-1` remains invalid and withdrawn by maintainer authority. `FIND-008-2` is closed by the indexed outcome representation and snapshot format change. `FIND-008-3` remains closed: revised task and extractor documentation agree with REQ-002.

**Ponytail check:** The new `FileOutcome` and `IndexedFile` carry the minimum durable distinction needed to make an indexed-empty snapshot unrepresentable. The facade's per-file export reuses the index's existing declaration projection. No new dependency, parallel coverage state, or caller guard is present.

**Verification limit:** I ran the exact persisted-object regression and inspected source and recorded task evidence. I did not repeat the live GitHub run or full gate. The R3 same-tree update assertion follows a cold rebuild; source tracing shows `Update::run` uses the same current versioned scope and cold-build path when started independently from the old-format pointer.

**Result: PASS.** No proposed findings.

```citations
[]
```
