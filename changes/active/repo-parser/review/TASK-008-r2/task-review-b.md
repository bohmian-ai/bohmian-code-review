# TASK-008 task review B

Subject: `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c` → `bdca195f6f08fda8f847efdd7c0f1d2d102a0a15`. The approved revision 14 REQ-002 and the maintainer decision govern error-region declarations; the older wording in TASK-008 does not override them. I inspected the cumulative source diff, its consumers, the checked-in live report, and the applicable repository authorities. Candidate HEAD matched the stated commit; `git diff --check` passed.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-002: extract tree-sitter's recovered declarations, including those inside error regions, without whole-file deferral in every ported walker | All `has_error()` rejection branches are removed from the native walkers; `Extractor::extract` dispatches their results directly. The `Deferred` variant and string-prefix routing are removed. The corpus `recover` cases exercise malformed files, including expected declarations from their recovered trees. | `mise run test:extract` passed (142 extractor and 27 core tests). Rust attribute and TypeScript NUL focused tests passed; the 21 malformed corpus entries use native recovery. | PASS |
| Preserve valid-file fixture output and grammar provenance | The actual diff changes no `match` fixture, golden, grammar pin, or vendored grammar C file. | The corpus differential and grammar-provenance tests passed in `test:extract`. | PASS |
| INV-003: change extraction identity and reject facts from the prior version | `EXTRACTION_VERSION` is now `codegraph-kernel-572d22bf.tree-sitter-0.25.10.recovered-tree.facts-2`; the public journey loads a record stamped with the prior `facts-1` version and requires `ExtractionVersionMismatch`. Snapshot scope includes the version. | `mise run test:symbolic-context:journey` passed (6 tests). | PASS |
| REQ-005, REQ-006, INV-002: verified no-declaration files are source-free, blob-identified content skips that permit publication; fetch, verification, and extractor failures remain incomplete | `Ingest::ingest` removes a file-node-only upsert and records `SkipReason::NoFacts` with path and blob. The single `SkipReason::is_benign` definition adds that variant; `ParseFailed`, `FetchFailed`, and `NotFetched` remain non-benign. Cold build, update, and PR overlay use this ingest owner. | `no_facts_file_is_a_recorded_skip_in_a_published_build` and the amended incomplete-attempt case passed in `mise run test:pipeline`. Existing incomplete fetch/verification cases passed in the same lane. | PASS |
| Live cold build indexes both named `bohmian-ai/wyrd` files and publishes a reusable snapshot | Checked-in S20 report records commit `2855eac00a15581cede6e29ec5a0dde0b8bc4d48`, `Published`, 1,681 eligible, 1,538 indexed, and 143 blob-identified `NoFacts` skips. Neither named path appears among skips; both are supported Rust/TypeScript paths. | Recorded live invocation reports `full_coverage: true`, 50 spans, and `read_back_equal: true`. The report does not enumerate indexed paths, so its path-level proof rests on complete eligible coverage and the skip list. | PASS |
| Prohibited behavior: do not turn failed source access, identity verification, or extractor errors into benign skips; no source persistence or symbolic-crate IO | The new benign branch follows a successful verified-byte upsert only. Error paths still record `ParseFailed` or fetch skips; no changed symbolic crate adds IO or source retention. | Source-path trace, `test:pipeline`, `test:symbolic-context:journey`; `mise run fmt:check` and `mise run lints` passed. | PASS |

## Findings

None. The old TASK-008 prose and two test comments still say an error region drops declarations; revised REQ-002 and the maintainer decision control, and the implementation does not enforce that stale statement. This is non-blocking wording, not a behavioral finding.

## Overall result

**PASS.** No missing, incorrect, drifting, violating, or regressing behavior was found in the candidate-owned diff. The live run was assessed from its checked-in report rather than rerun with provider credentials.

```citations
[]
```
