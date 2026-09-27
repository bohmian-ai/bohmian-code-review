# TASK-008 round-two verdict

**Verdict: FIX_REQUIRED.** Blocking findings: `FIND-008-2`, `FIND-008-3`. Remediation: [TASK-008-R2-no-facts-projection-and-authority.md](TASK-008-R2-no-facts-projection-and-authority.md).

## Immutable subject and authority

- Base `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c`; candidate `bdca195f6f08fda8f847efdd7c0f1d2d102a0a15`. HEAD stayed at the candidate through review. The complete cumulative diff was reviewed.
- Approved [spec](../../spec.md) revision 14 as amended at HEAD; original [TASK-008](../../tasks/TASK-008-recovered-tree-extraction.md); [maintainer decision](../TASK-008-r1/maintainer-decision-error-regions.md). The maintainer invalidated prior `FIND-008-1` and withdrew its remediation. Declarations recovered inside tree-sitter `ERROR` regions are required.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-002: retain tree-sitter recovered declarations, including inside error regions, without whole-file deferral | `has_error()` rejection removed from ported walkers; `Deferred` path removed; recovery manifest and corpus assertions changed. | Both task samples and parser domain reviewed the walker diff; `mise run gate` passed. Kotlin claim was independently rejected because the pinned tree has no interface declaration. | PASS |
| Clean fixtures, goldens, grammar pins, and vendored C unchanged | Diff changes malformed `defer` expectations only; no clean fixture/golden or grammar source changed. | Extractor lane and full gate passed. | PASS |
| Changed extraction version rejects old facts | `EXTRACTION_VERSION` bumped; public journey loads `facts-1` and requires mismatch. | Public journey and full gate passed. | PASS |
| Verified files with no persistable declaration facts become blob-identified `NoFacts` skips; fetch, verification, and extractor failures remain incomplete | Shared ingest recognizes file-node-only output, but import-only output has `Import` and exports no declarations. Reuse carries the misclassified fact. Failure skip reasons remain non-benign. | Public-facade probe produced `(File, Import)` and an empty declaration export; source trace reaches the indexed branch. Existing pipeline tests cover only the file-node-only case. | **FAIL — FIND-008-2** |
| Live Wyrd cold build indexes both named files and publishes | Checked-in S20 artifact records commit `2855eac00a15581cede6e29ec5a0dde0b8bc4d48`, 1,681 eligible, 1,538 indexed, 143 `NoFacts` skips, `Published`, full coverage and equal read-back; neither named file is skipped. | Artifact reviewed; credentialed run was not repeated. | PASS within recorded-run limit |
| Written task and rustdoc follow approved revision 14 | Task outcome/stop condition and changed corpus documentation still state the revoked outside-only rule. | Direct comparison to spec REQ-002 and maintainer decision. | **FAIL — FIND-008-3** |
| Scope and repository boundaries | No new runtime dependency, symbolic IO, source persistence, altered golden, or weakened gate in the code diff. | Standards review; `git diff --check`; `mise run gate` passed on the immutable candidate. | PASS apart from documentation conflict |

## Independent review and validation

Wave 1: task-review A **PASS**, task-review B **PASS**, standards-review **FAIL** (`STD-008-R2-1`), parser domain **FAIL** (`PARSER-008-1`), snapshot domain **FAIL** (`SNAPSHOT-008-R2-1`). Citation verification exited zero with every final citation `OK`; rejected citations: none. An initial snapshot citation-symbol formatting error was corrected before final verification. Wave 2 independently reproduced the no-facts path, confirmed the written-rule conflict, and rejected the Kotlin claim. The deduplicated ledger, source paths, sibling sweep, and corrections are in [findings-validation.md](findings-validation.md).

`FIND-008-2` is the shared ingest outcome mismatch: raw non-declaration nodes can produce an `Indexed` status even though the durable declaration projection is empty. Cold archive ingest, verified fallback, PR head overlay, and later reuse are affected. Correct the candidate-owned outcome shape using the existing declaration projection so `Indexed` carries nonempty persisted facts and `NoFacts` carries the verified path/blob; no downstream guard is prescribed.

`FIND-008-3` is documentation authority drift. Align the active task and changed recovery rustdoc with amended REQ-002; preserve existing fixture assertions and parser behavior.

Prior-finding closure: `FIND-008-1` remains **invalid and withdrawn** by the maintainer decision; it is neither reopened nor a remediation target. Notes: none. Rejected claim: `PARSER-008-1`, because the pinned Kotlin grammar does not recover an interface declaration from the tested `fun interface` input. Verification limits: the import-only pipeline outcome was traced from a public-facade probe rather than run through a new fake-provider test; the checked-in live GitHub artifact was reviewed rather than repeated. These are closure proofs for implementation, not reasons to waive the reproduced contract defect.
