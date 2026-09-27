# TASK-008 r4 Wave 2 findings validation

**Immutable subject:** `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c..f204d3383c62c6137a603437668103187a4fd19c` (`HEAD` during validation). Authority: `spec.md` revision 14, original TASK-008, R3 remediation, and the r1 maintainer decision. I inspected the cumulative source diff and candidate code independently. The citation-checked claim ledger is empty; no Wave 1 report, matrix, or evidence prose informed this validation.

## Validated ledger

**EMPTY.** There are no surviving claims to merge, reproduce, confirm, revise, or reject. My source inspection found no new blocking finding under the production-path, task-contract, written-standard, or absolute-boundary gates. No notes or remediation recommendations.

## Prior finding closure

- **FIND-008-1 — invalid and withdrawn.** The r1 maintainer decision and revision 14 expressly require declarations recovered inside tree-sitter `ERROR` regions. The cumulative diff removes whole-file `has_error()` deferrals; the current extractor has no such rejection. No error-region exclusion should be added.
- **FIND-008-2 — closed.** `SymbolicContext::export_file` delegates to the index's existing declaration projection (`bohmian-code-review/src/lib.rs:227-232`; `index/src/lib.rs:145-162`). The shared `Ingest::settle` records `Indexed` only with that nonempty projection and otherwise removes the context entry and records a blob-identified `NoFacts` skip (`pipeline/src/snapshot.rs:1140-1156`). Fresh archive and verified fallback ingestion, changed-tree reuse, and PR head ingestion reach this owner (`snapshot.rs:1020-1023,1054-1074,1093-1107,1115-1129`; `pull_request.rs:433-435`). `FileOutcome` is mutually exclusive; `IndexedFile` has private fields and validates deserialization, while coverage derives from outcomes (`snapshot.rs:136-157,182-258,309-312`). The format version is 2 (`snapshot.rs:36`); `Snapshots::read` checks the scoped key and `Snapshot::decode` checks the format (`snapshot.rs:281-306,360-377`). A prior-format same-tree pointer therefore cannot become current coverage. The focused `prior_format_indexed_empty_snapshot_is_not_reused` test passed in this validation (`cargo test --locked --all-features -p bohmian-code-review-pipeline --lib snapshot::tests::prior_format_indexed_empty_snapshot_is_not_reused`: 1 passed). The correction changes the durable outcome shape rather than adding caller guards. No later correction made its projection or decode validation redundant: the former decides fresh outcomes; the latter protects persisted input.
- **FIND-008-3 — closed.** The active task and changed corpus documentation now require recovered-tree declarations inside error regions (`tasks/TASK-008-recovered-tree-extraction.md:9-18`; `extract/src/corpus_tests.rs:1-12,1649-1654`). They agree with revision 14.

## Independent checks and limits

`git diff --check cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c f204d3383c62c6137a603437668103187a4fd19c` exited zero. The cumulative diff changes extractor deferral logic, recovery fixtures, the facade/index projection, snapshot representation and consumers, and task evidence; no grammar pin, checked-in golden, or `match` fixture changed. I did not rerun the full `mise` gate or the live Wyrd cold build. Their reported results require separate orchestration verification. The focused test and source trace establish closure of the prior indexed-empty same-tree path; they do not independently remeasure live provider behavior.

**Independent decision: PASS; validated blocking ledger empty.**
