# TASK-001 r1 — repository-standards review (repo-rev)

Subject: base `9715440d` → candidate `34f4ae71` (HEAD), read-only. Diff: 14 files, +1106/−59.

## 1. Authority coverage

| Changed surface | Applicable authorities |
|---|---|
| `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md` | task doc L16 (update authorities); spec-driven-development.md:81-84 (architecture stays authoritative, updates land in owning doc); references/README.md:3 (references create no second architecture) |
| `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` | spec-driven-development.md:138-175 (task contract, status set); testing-workflows.md:17-20 (record exact commands/results); implementation-execution.md:114-137, 280-340 (execution record, completion report) |
| `Cargo.lock`, `crates/bohmian-code-review/Cargo.toml` | AGENTS.md:27, 34 (no unnecessary/prohibited deps); implementation-execution.md:106 (adding a dependency is material); task doc L52 (manifest/lockfile only as needed); testing-workflows.md:15 (lanes network-free) |
| `crates/bohmian-code-review-core/src/facts.rs`, `core/src/lib.rs` | AGENTS.md:14, 21, 29-32; agent-rules.md:7, 9, 11; design.md:13, 20, 34; rust-core.md:3, 5, 7; symbolic-context.md:3 |
| `crates/bohmian-code-review-extract/src/extractor.rs`, `extract/src/lib.rs` | AGENTS.md:15, 30-33; agent-rules.md:6, 9, 11; design.md:14, 34; rust-core.md:3-7 |
| `crates/bohmian-code-review-index/src/lib.rs` | AGENTS.md:16, 21, 29-32; agent-rules.md:5, 8, 9, 11; design.md:15, 26, 34; rust-core.md:5, 7; symbolic-context.md:3; testing-workflows.md:6-7 |
| `crates/bohmian-code-review-select/src/lib.rs` | AGENTS.md:18, 27, 29-32; agent-rules.md:5, 7, 10, 11; design.md:17, 20, 28-30, 34; rust-core.md:3, 7; symbolic-context.md:5, 7; testing-workflows.md:7 |
| `crates/bohmian-code-review/src/lib.rs` | AGENTS.md:19, 21, 29-32; agent-rules.md:3, 7, 9, 11; design.md:18, 20, 34; rust-core.md:3, 7 |
| `crates/bohmian-code-review/tests/symbolic_context_journey.rs` | AGENTS.md:38; agent-rules.md:4, 5, 12; design.md:37; testing-workflows.md:5, 9-15; implementation-execution.md:224-232 |
| Untouched but rule-relevant: `mise.toml`, grammar pins, fixtures, `.claude/skills` | AGENTS.md:33, 40; agent-rules.md:6, 13; testing-workflows.md:15 |

All named authorities were read in full: AGENTS.md, agent-rules.md, bohmian-code-review-design.md, references/README.md, rust-core.md, testing-workflows.md, spec-driven-development.md, symbolic-context.md, plus implementation-execution.md (router row for "verification evidence").

## 2. Rule-by-rule results

| # | Rule (file:line) | Result | Evidence |
|---|---|---|---|
| R1 | Crate ownership, no duplicate stage logic in facade — AGENTS.md:14-19; design.md:20; agent-rules.md:3 | PASS | `core/facts.rs` holds only IO-free projection + validation; `index::export/load` reuse `upsert` (index/lib.rs:157-165); `select::rank` now delegates to `rank_files`, `pack` to `pack_candidates_into` (select/lib.rs:189-199, 379); facade `rank_files`/`build_candidate_context` build inputs and call `bohmian_code_review_select::{rank_files, pack_candidates}` (facade lib.rs:269-285, 329-341). Version comparison lives in the facade because only it can see `EXTRACTION_VERSION` without an upward dep. |
| R2 | Downward-only dependencies — design.md:20; agent-rules.md:3 | PASS | No inter-crate `[dependencies]` edge added. Facade `Cargo.toml` adds `serde_json` under `[dev-dependencies]` only. `Cargo.lock` delta is that single edge. |
| R3 | IO-free symbolic crates; no persistence of source — AGENTS.md:21, 28; agent-rules.md:9; rust-core.md:5 | PASS | `serde`/`serde_json` were already `[dependencies]` of core at base (`git show 9715440d:crates/bohmian-code-review-core/Cargo.toml`); the diff adds no crate-level dependency to core. Serde derives are in-memory serialization, not IO; no `std::fs`, `std::net`, or runtime appears in the diff. `DeclarationFact` carries id/kind/name/qualified_name/language/lines/column; `into_node` sets every source-derived Option to `None` (facts.rs:56-79). |
| R4 | No unnecessary/prohibited dependency — AGENTS.md:27, 34; implementation-execution.md:106; task L52 | PASS (noted) | Only new manifest edge is a test-only `serde_json` on an already-workspace-pinned crate, used by the journey to prove source-free serialization. Task L52 explicitly allows "workspace manifest and lockfile only as needed"; evidence records it (task L80). No Node/NAPI/SQLite/PG/PyO3/SDK/remote dep. |
| R5 | `&str`/slices/refs; `.clone()` as ownership decision — AGENTS.md:29; rust-core.md:5 | PASS | `ChangedFile<'a>` is all-borrow, `Copy` (select/lib.rs:140-150); `rank_files(index, &[ChangedFile])`, `pack_candidates(&[RankedFile], &BTreeMap<RelativePath,&[u8]>)`. Clones present: `export` clones `path` into an owned output record; facade `load` clones `path` once for the error branch after `facts` is moved; `blob.changed.clone()` (facade lib.rs:272) exists because pre-existing `ChangedLines::new(ranges: Vec<LineRange>, …)` (select/lib.rs:59) consumes a `Vec` — an owned-slice `ChangedBlob.changed` would cost the same `to_vec()`. Each is an ownership decision, not an inferred violation. |
| R6 | thiserror; no unchecked unwrap; `expect` only for named invariants — AGENTS.md:30; rust-core.md:7 | PASS | `InvalidFact` derives `thiserror::Error` (facts.rs:127); `Error::{ExtractionVersionMismatch, InvalidFacts}` added to the existing thiserror enum. `grep unwrap(` over the diff: none. `expect(` occurs only inside `#[cfg(test)]`/journey code. |
| R7 | Imports at module top; bare imported types in signatures — AGENTS.md:31; agent-rules.md:11 | PASS | All new `use` lines are at module top (facts.rs:10-14, index:9-11, facade:12-16). Signatures use bare `IndexedFileFacts`, `InvalidFact`, `ChangedFile`, `RankedFile`, `Language`, `ExtractionResult`. Path-qualified `bohmian_code_review_select::rank_files(...)` in a body mirrors the pre-existing `::rank` call style and is not a signature. |
| R8 | Rustdoc on every new/modified item incl. private + tests; `# Errors`; `# Panics` — AGENTS.md:32; agent-rules.md:11; rust-core.md:7 | PASS | Every new struct, field, fn, const, error variant, private helper (`query_names`, `pack_candidates_into`, `prepare_changed`, `extract`), test module, test helper (`node()`), test fn, and journey const (`REPORT`) carries rustdoc. Fallible fns (`validate` ×2, `SymbolIndex::load`, `SymbolicContext::load`, `path_language`, `rank_files`, `prepare_changed`, `extract`, `pack_candidates`, `pack_candidates_into`, `build_candidate_context`) have `# Errors`. No new production panic path exists, so no `# Panics` is owed. |
| R9 | Tests inline by default; external only across crate boundaries via facade — agent-rules.md:5; testing-workflows.md:5-7 | PASS | Unit tests sit in owner `mod tests` (facts, extractor, index, select). Only external test is the facade journey. `scripts/check-test-placement.sh` passed inside `mise run lints`. |
| R10 | Reviewer-output change must update the public journey — agent-rules.md:4; testing-workflows.md:9-10 | PASS | Journey gains three tests covering export/load equivalence + rejection, path eligibility, and multi-file rank + candidate-only pack including budget/deletion-only negatives (journey.rs:385-634). |
| R11 | Validate canonical paths, nonempty ranges, token counts — agent-rules.md:7; rust-core.md:7 | PASS | `path_language` and `rank_files` route through `relative_path()`; `rank_files` validates each blob's ranges via `ChangedLines::new` before extraction (facade lib.rs:295-297); `IndexedFileFacts.path` is a `RelativePath` whose `Deserialize` revalidates (relative_path.rs:83-101), proven by the `../user.ts` journey assertion; `DeclarationFact::validate` rejects empty identity and non-one-based/reversed spans. Token counting remains caller-supplied `Fn(&str)->usize` (unchanged). |
| R12 | Malformed file must not corrupt other entries; replacement all-or-nothing — agent-rules.md:8; design.md:34 | PASS | `SymbolIndex::load` runs `facts.validate()?` before `upsert` (index/lib.rs:157-165); facade checks version before delegating. Index test asserts `file_count()==2` and unchanged `keys` after a rejected record; journey asserts `file_count()==4` and lookup intact after version, span, and identity rejections. |
| R13 | Exported facts source-free, version-stamped; foreign version fails — agent-rules.md:9; design.md:34; AGENTS.md:21 | PASS | `export` stamps `extraction_version`; facade `load` returns `ExtractionVersionMismatch` on inequality (facade lib.rs:191-197). Journey asserts JSON contains none of `console.log`, `return { id }`, `(id: string)`, `docstring`, `signature`. `SymbolIndex::load` deliberately does not compare versions (documented, index/lib.rs:147-149) because index cannot depend on extract; the public contract is the facade. |
| R14 | No `#[allow]`, `#[ignore]`, weakened assertion, changed golden — agent-rules.md:12; AGENTS.md:40; implementation-execution.md:224-232 | PASS | `grep -nE '#\[allow|#\[ignore' /tmp/task001-diff.patch` → none. No existing assertion removed; pre-existing tests unchanged. |
| R15 | Grammar pins and fixtures intact — AGENTS.md:33; agent-rules.md:6; testing-workflows.md:15 | PASS | Diff touches no `Cargo.toml` grammar pin, `grammars/`, or corpus fixture. `EXTRACTION_VERSION` is a new constant, not a pin change. |
| R16 | Skills mirror not edited — agent-rules.md:13 | PASS | `git diff --stat 9715440d..34f4ae71 -- .claude .agents scripts` is empty. |
| R17 | `--all-features` in cargo lanes — AGENTS.md:40; mise.toml | PASS | `mise.toml` unchanged; every cargo test/clippy/check lane already carries `--all-features` (mise.toml:27, 34-35, 40, 44, 48, 52, 75). |
| R18 | Task/plan/agent references out of production code — AGENTS.md:44 | PASS | `grep` for `TASK-`, `REQ-`, `INV-`, `AC-0`, `agent` across `+` lines hits only the task Markdown. |
| R19 | Task document structure — spec-driven-development.md:138-175 | FAIL (REPO-1) | `status: implemented` is not in the enumerated status set (`proposed`, `ready`, `in_progress`, `review`, `approved`, `superseded`). Frontmatter key shape (`maps`, `spec: …@10`, no `spec_revision`/`parent_task`/`remediates`) differs from the template at spec-driven-development.md:140-151 but is unchanged from base and therefore out of scope. |
| R20 | Evidence records exact commands, RED/GREEN, `git diff --check` — testing-workflows.md:17-20 | PASS (limited) | Task L68-78 records RED (compile failure of the extended journey), lanes with exit codes, and `git diff --check`. It does not record per-test `mise exec -- cargo test --locked … -- --exact` selectors for the named tests (testing-workflows.md:17-18). The task itself (L30, L40) names only the journey lane as the RED/GREEN command, and spec-driven-development.md:197-199 lets an implementation record the exact focused command once tests are named; the recorded lane runs those tests. Treated as a limit, not a finding; task-acceptance reviewers own evidence sufficiency. |
| R21 | fmt / clippy — mechanically enforced | PASS | `mise run fmt:check` → exit 0; `mise run lints` (clippy `-D warnings --all-features` + test-placement script) → exit 0. |

## 3. Material findings

### REPO-1 — task status value outside the defined lifecycle set

- Classification: documentation / lifecycle metadata, low severity.
- Root cause: the implementation evidence step wrote `status: implemented` into the task frontmatter instead of a value from the defined status enumeration.
- Defect class: invalid enumerated field value in a tracked lifecycle artifact.
- Location: `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md:4` (sole site; no sibling task in the diff).
- Violated rule: `architecture/references/languages/spec-driven-development.md:171-173` — "Task status moves through `proposed`, `ready`, `in_progress`, `review`, and `approved`; use `superseded` when …". `implemented` is not a permitted value. (`IMPLEMENTED` at implementation-execution.md:299 is the *report* status of the execution result, not the task's `status` field.)
- Evidence: base had `status: proposed` (`git show 9715440d:…TASK-001…md | sed -n 4p`); candidate line 4 is `status: implemented`.
- Consequence: any lifecycle tooling or reviewer keying on the enumerated statuses cannot classify the task; the packet misstates where TASK-001 sits (it is under independent review, i.e. `review`).
- Testable correction: set line 4 to `status: review` (the value for a candidate awaiting `cr-task-review`); `rg -n '^status:' changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` must return one of the six defined values.

No other violations introduced by the diff were found.

## 4. Verification limits

- Not run: `mise run gate` (excluded by the audit brief); owner test lanes and the journey lane were not re-run by this reviewer because standards compliance did not require them beyond `fmt:check` and `lints`; correctness of behavior is the task-acceptance reviewers' scope.
- Pre-existing, out of scope: `changes/active/repo-parser/spec.md:4` reads `status: draft` while the task cites "Revision 10 spec" as approved; the task frontmatter key shape predates the diff. Both are flagged for the orchestrator, not counted.
- `EXTRACTION_VERSION` currency relative to grammar pins was not independently derived; the constant is new, so no rule about pin changes is touched.
- `blob.changed.clone()` and `DeclarationFact::into_node` setting `end_column: 0` were checked: no production consumer of `end_column` exists in index/resolve/select (`rg end_column` hits only test fixtures), and the journey proves fresh-vs-reloaded rank/context equality for its fixture.

## 5. Overall

Overall: FAIL

One low-severity written-rule violation (REPO-1) introduced by the diff; all Rust and architecture rules pass, and both mechanical lanes exit 0.

```citations
[
  {"id": "REPO-1", "path": "changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md", "line": 4, "snippet": "status: implemented", "symbols": ["status", "implemented"]}
]
```
