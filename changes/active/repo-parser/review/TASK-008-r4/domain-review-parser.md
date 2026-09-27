# Parser provenance and extraction boundary

**Subject:** `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c` → `f204d3383c62c6137a603437668103187a4fd19c`; approved `SPEC-repo-parser` revision 14, original TASK-008, and R3 remediation. **Result: PASS.** No material parser-domain finding.

## Boundary and authority

Reviewed the native walker parse entry points, `Extractor::extract`, composite Svelte dispatch, bounded parse-pool behavior, corpus fixture expectations, extraction-version stamp, facade projection, and the pipeline's `Ingest::ingest`/`settle` handoff. The governing obligations are spec REQ-002, REQ-005–REQ-006, INV-002–INV-003; TASK-008 scenarios 1–3 and prohibited changes; `AGENTS.md` and `architecture/agent-rules.md` for grammar provenance, source-free facts, and file-local failure; and `architecture/bohmian-code-review-design.md` plus `architecture/references/domain/symbolic-context.md` for extractor/index ownership. The r1 maintainer decision explicitly invalidates FIND-008-1: declarations recovered inside tree-sitter `ERROR` regions must remain eligible. I applied that decision and did not treat those declarations as defects.

## Source and verification coverage

- The base-to-candidate diff removes all 24 `has_error()` whole-file rejection sites from the native walkers. `Extractor::extract` now maps actual walker errors to `ExtractionFailed`; UTF-8 and admission errors remain typed. `parse_pool.rs` still returns per-file outcomes, so a failed file does not abort adjacent parses.
- The 21 formerly deferred manifest entries are `recover`; the corpus compares unchanged clean `match` fixtures to their goldens and asserts recovered declarations for malformed fixtures. Focused Rust and TypeScript reproductions exercise the named grammar-gap and NUL cases. No grammar C, pin, checked-in fixture source, or golden changed in this range.
- `EXTRACTION_VERSION` changes from `…facts-1` to `…recovered-tree.facts-2`. The facade exports declaration facts stamped with that version. The pipeline treats a successful extraction with no exported declarations as a blob-identified `NoFacts` outcome; actual extractor errors remain `ParseFailed`, which is not a benign complete-snapshot skip. R3's indexed outcome carries a nonempty exported declaration set.
- I ran `mise run test:extract` at the candidate: exit 0; 27 core and 142 extractor tests passed, including malformed corpus and parser-pool cases. The recorded live Wyrd evidence identifies commit `2855eac00a15581cede6e29ec5a0dde0b8bc4d48`, 1,681 eligible, 1,538 indexed, 143 `NoFacts` skips, and a published outcome with neither named syntax-error file skipped.

## Limits

The checked-in live run predates the R3 snapshot-format bump from 1 to 2; it proves parser recovery on that repository but does not itself prove a live format-2 publication. Snapshot storage and pointer correctness belong to the separate snapshot-domain review. I did not rerun the credentialed live test. The focused malformed-source tests assert recovered declarations around the broken construct; the approved maintainer decision controls whether declarations inside `ERROR` regions are retained.

## Proposed findings

None.

```citations
[]
```
