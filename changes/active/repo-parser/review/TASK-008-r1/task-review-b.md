# TASK-008 independent implementation review B

Subject: `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c..caad6d05711e9e98a992d712331bc18a1fdb78ca`, approved `SPEC-repo-parser` revision 14. I inspected the complete diff and the source paths it changes. The task's implementation-evidence prose was not treated as proof.

| Requirement, criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-002: extract recovered declarations outside syntax errors in every ported language | Whole-tree `has_error()` rejection removed from each affected walker; `Extractor::extract` no longer maps the removed `defer:` signal; native walkers still traverse tree-sitter's returned root | Focused Rust and TypeScript grammar-gap tests pass; malformed corpus entries assert extraction and per-fixture surviving names | PASS |
| Valid-file fixtures and goldens remain unchanged | Diff changes only `defer` to `recover` manifest entries; no `match` fixture or golden changes | Existing differential corpus is in `test:extract`; exact focused recovered tests passed locally | PASS |
| Old extraction facts and snapshots cannot be reused | `EXTRACTION_VERSION` changed to `…recovered-tree.facts-2`; facade load compares stamped version; snapshot decode and scope use the same constant | Public journey compares against `…facts-1` and tests version mismatch | PASS |
| A verified file with no declaration facts is a blob-identified content skip; complete cold builds publish | `Ingest::ingest` removes a file-node-only upsert and records `SkippedFile { path, blob, NoFacts }`; `NoFacts` is benign in the single completeness predicate | `no_facts_file_is_a_recorded_skip_in_a_published_build` passed locally; Scenario 2's malformed file now expects publication | PASS |
| Update and PR overlay use the same no-facts policy | `Update::run` feeds changed files through `Ingest::fallback`; `PrContext::changed_head` invokes `Ingest::ingest` for pending changed files | Shared-path source trace; no separate focused update/overlay no-facts assertion | PASS |
| Fetch, verification, and extractor failures remain incomplete | `NotFetched`, `FetchFailed`, and `ParseFailed` remain outside `SkipReason::is_benign`; failed archive entries stay pending for bounded fallback | Existing incomplete-attempt and update-failure tests remain; no changed assertion weakens those cases | PASS |
| Live Wyrd cold build publishes and indexes the two named files | Recorded cold build is `Published`, eligible 1,681, indexed 1,538, skipped 143; neither named path appears among skips; Rust `boot/mod.rs` also appears as a candidate omission, proving indexed facts | `s20-wyrd-pr1.json` records commit `2855eac00a15581cede6e29ec5a0dde0b8bc4d48`, identity, coverage, and PR read-back | PASS |
| No grammar, source, storage, or golden scope drift | No grammar pin, vendored C, clean fixture/golden, or dependency changed; source-free facts and pipeline IO ownership remain | Base-to-candidate changed-file list and diff | PASS |

## Findings

None proposed. The no-facts classification deliberately uses declaration nodes because the snapshot exports only declarations; raw references are returned by extraction but are not snapshot facts. The shared ingest owner prevents distinct cold, update, and overlay policies.

## Verification limits

The recorded live output does not list every indexed path, so the TypeScript target's inclusion is established indirectly by its supported path, complete eligible-file accounting, and absence from the recorded skip list. I did not repeat the credentialed live run. Local focused commands passed: `mise exec -- cargo test --locked -p bohmian-code-review-extract --all-features recovered_tree` and `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib no_facts`. The full `mise` lanes and live-run result are recorded in task evidence but were not independently rerun in this sample.

## Result

**PASS** for the task implementation on the inspected immutable candidate, subject to the integrated citation check and Wave 2 validation.

```citations
[]
```
