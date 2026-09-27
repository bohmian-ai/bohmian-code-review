# TASK-008 Wave 1 task review A

Subject: `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c` → `bdca195f6f08fda8f847efdd7c0f1d2d102a0a15`. Authority: approved repo-parser spec revision 14, original TASK-008, repository rules, and the maintainer decision withdrawing FIND-008-1. This review inspects the cumulative code diff; declarations recovered inside tree-sitter error regions are required by the amended REQ-002.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Syntax errors retain recovered declarations in every former deferring walker (REQ-002; TASK-008 scenario 1) | The complete extractor diff removes every former `root_node().has_error()` whole-file rejection and removes `ExtractionError::Deferred`; `Extractor::extract` decodes successful recovered walks. Rust and TypeScript focused tests assert declarations around the named grammar gaps. | `git grep` finds 24 former rejection sites at the base and `rg` finds none at the candidate; `mise run test:extract` passed (27 core, 142 extractor). | PASS |
| Malformed corpus checks recovery; valid fixtures and goldens retain their contract | Manifest changes only `defer` rows to `recover`; `check_entry` requires successful extraction and the file node; `recover_fixtures_keep_valid_regions_per_fixture` checks recovered names for fixtures with valid regions. | `test:extract` passed; diff lists no changed `match` fixtures, goldens, grammar pins, or vendored grammar sources. | PASS |
| Extraction identity changes; prior facts cannot load (REQ-002, INV-003) | `EXTRACTION_VERSION` changes from `…facts-1` to `…recovered-tree.facts-2`; the public journey loads a prior-version record and expects `ExtractionVersionMismatch`. Snapshot scope keys include the extraction version. | `mise run test:symbolic-context:journey` passed all 6 tests. | PASS |
| Verified files with no declaration facts become blob-identified benign skips; fetch, verification, and extractor failures remain incomplete (REQ-005, REQ-006, INV-002; TASK-008 scenario 3) | `Ingest::ingest` classifies an upsert containing only file nodes as `NoFacts`, removes its empty index entry, and records path and blob. `SkipReason::is_benign` adds only `NoFacts`; other failure variants remain outside that set. Cold build, update, and PR overlay use this ingest owner. | `mise run test:pipeline` passed. The new published-build test checks skip reason, blob, and complete coverage; existing incomplete-path assertions remain. | PASS |
| Live cold build indexes the two named Wyrd blockers and publishes complete coverage | Saved S20 evidence at `changes/active/repo-parser/evidence/TASK-008/s20-wyrd-pr1.json` reports commit `2855eac00a15581cede6e29ec5a0dde0b8bc4d48`, `Published`, 1,681 eligible, 1,538 indexed, and 143 `NoFacts` skips. Neither named blocker appears in the skip list; the Rust path also appears in the selected spans. | The saved run reports `full_coverage: true` and `read_back_equal: true`. A repeat live GitHub query was unavailable because this sandbox could not connect to `api.github.com`; the evidence does not enumerate every indexed path individually. | PASS |
| Scope and repository constraints | The diff adds no dependency, grammar change, source persistence, model call, or altered golden. The facade reexports existing `NodeKind` solely for pipeline classification. The prior FIND-008-1 was withdrawn by the maintainer and is not an applicable defect. | `mise run fmt:check`, `mise run lints`, and `git diff --check cea6100 bdca195` passed. | PASS |

## Findings

None. The live artifact's aggregate coverage and skip list support the named-file outcome, with the direct repeat-run limit noted above. No blocking implementation gap or scope drift was reproduced.

## Overall result

**PASS**.

```citations
[]
```
