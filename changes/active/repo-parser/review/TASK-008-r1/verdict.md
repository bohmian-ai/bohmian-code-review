# TASK-008 review verdict

**Verdict: FIX_REQUIRED.** One bounded implementation finding remains: `FIND-008-1`.

## Immutable subject

- Base: `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c`
- Candidate: `caad6d05711e9e98a992d712331bc18a1fdb78ca`
- Authority: approved `changes/active/repo-parser/spec.md` revision 14
- Original task: `changes/active/repo-parser/tasks/TASK-008-recovered-tree-extraction.md`
- Reviewed range: the complete 39-file base-to-candidate diff. `HEAD` remained at the candidate throughout review.

## Acceptance matrix

| Obligation | Candidate evidence | Verification | Result |
|---|---|---|---|
| REQ-002: retain declarations outside an error region and drop those inside it | Whole-file deferral removed across the ported walkers; raw-node traversal still dispatches declarations below `ERROR` | Independent Rust and TypeScript probes reproduced emitted declarations within `ERROR` | **FAIL** |
| Recovered extraction across formerly deferred files; clean fixtures and goldens unchanged | 21 malformed manifest rows changed to `recover`; no clean fixture, golden, grammar pin, or vendored C changed | `mise run gate` passed; focused recovery and corpus tests passed | PASS for tested behavior |
| Changed extraction identity and rejection of old facts | `EXTRACTION_VERSION` bumped; public journey attempts to load `facts-1` | Public journey passed within `mise run gate` | PASS |
| Verified no-facts files record path/blob skip and permit complete publication; fetch and extractor failures remain incomplete | Shared `Ingest::ingest` emits `NoFacts`; `is_benign` includes only content skips | Focused no-facts test and pipeline lane passed | PASS |
| Cold build, update, and PR overlay preserve blob identity and remove stale facts | All three use shared ingest; unchanged facts are reused only for matching path/blob | Pipeline tests passed; source-trust review traced the paths | PASS |
| Live Wyrd cold build indexes both named files | Checked-in S20 artifact records commit `2855eac00a15581cede6e29ec5a0dde0b8bc4d48`, 1,681 eligible, 1,538 indexed, 143 `NoFacts` skips, publication, and neither named path skipped | Artifact reports full coverage, 50 spans, and equal read-back | PASS, subject to recorded-run limit |
| Prohibited scope and repository rules | No new dependency, source persistence, grammar change, fixture rewrite, or symbolic-crate IO | Standards review passed; `git diff --check` and `mise run gate` passed | PASS |

## Independent review and validation

Wave 1 results: task review A **FAIL** (`TASK-A-1`); task review B **PASS**; repository standards **PASS**; parser provenance domain **FAIL** (`DOMAIN-PARSER-1`); snapshot/source-trust domain **PASS**. Both failing reports identify the same root cause. The required citation check exited zero and marked all eight cited locations `OK`; rejected citations: none. Wave 2 independently reproduced the failure in TypeScript, merged the two claims, and confirmed `FIND-008-1` under the named REQ-002 task-contract gate and the public extraction path. Its full diagnosis is in `findings-validation.md`.

**Blocking finding `FIND-008-1`:** A recovered tree's raw `Node` reaches declaration dispatch without retaining whether an `ERROR` node is an ancestor. The candidate emits declarations inside parser error regions, so those names can enter snapshot facts, candidate lookup, and ranking. The correction belongs at the candidate-owned declaration traversal boundary. Its declaration input must represent only syntax outside error regions, while preserving recovered siblings and separate non-declaration recovery behavior. The self-contained correction task is `TASK-008-R1-error-region-provenance.md`.

Notes: none. Prior findings: none; this is the first review of TASK-008. Verification limits: the live GitHub run was assessed from its checked-in artifact, not repeated with credentials. Rust and TypeScript error-region emissions were reproduced; other language walkers were traced as sibling surfaces, not individually reproduced with malformed files. The green gate lacks an assertion that rejects declarations inside an `ERROR` subtree.
