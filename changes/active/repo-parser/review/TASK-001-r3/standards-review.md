# TASK-001 r3 — repository-standards review

Subject: base `9715440d` → candidate `1fbcc4a` (cumulative), diff `git diff 9715440d..1fbcc4a -- . ':!changes/active/repo-parser/review'`.
Reviewer: `repo-rev` (read-only). Prior review reports were not read.

## 1. Authority coverage

| Changed surface | Governing authorities |
|---|---|
| `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md` | `AGENTS.md` (lines 3, 16–22), `architecture/references/languages/spec-driven-development.md` (§Authority, §Specification contract lines 128–130, §Change artifact lifecycle lines 66–84) |
| `Cargo.lock`, `crates/bohmian-code-review/Cargo.toml` | `AGENTS.md:27,34`, `implementation-execution.md:99–112` (material class), task §Expected Write Set |
| `crates/bohmian-code-review-core/src/{facts.rs,lib.rs}` | `AGENTS.md:9,27,30,32`, `agent-rules.md:5,7,9,10,11`, `rust-core.md:3,7`, `design.md:20,34` |
| `crates/bohmian-code-review-extract/src/{extractor.rs,lib.rs}` | `AGENTS.md:10,32,33`, `agent-rules.md:6,9,11`, `rust-core.md:7`, `design.md:34` |
| `crates/bohmian-code-review-index/src/lib.rs` | `AGENTS.md:11,32`, `agent-rules.md:8,9,11`, `rust-core.md:5,7`, `symbolic-context.md:3` |
| `crates/bohmian-code-review-select/src/lib.rs` | `AGENTS.md:13,32`, `agent-rules.md:10,11`, `rust-core.md:3,7`, `symbolic-context.md:7`, `design.md:28–30,34` |
| `crates/bohmian-code-review/{src/lib.rs,tests/symbolic_context_journey.rs}` | `AGENTS.md:14,32,40`, `agent-rules.md:3,4,5,7,9,11`, `testing-workflows.md:5,9–15`, `design.md:20,34,38` |
| `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` | `spec-driven-development.md` §Task contract (140–175), §Test command precision (177–200), §TDD execution (202–223); `testing-workflows.md:17–20`; `implementation-execution.md` §Completion standard |

## 2. Rule compliance

| Rule (file:line) | Result | Evidence |
|---|---|---|
| `AGENTS.md:32` rustdoc on every new/modified item incl. private and tests; `# Errors` on fallible | PASS | All new items documented: `facts.rs` (`DeclarationFact`, `into_node`, `validate`, `IndexedFileFacts`, `InvalidFact`, `mod tests`, `node()`, both tests), `extractor.rs:70` `path_language` + test, `extract/lib.rs:119` `EXTRACTION_VERSION`, `index/lib.rs:127,156` `export`/`load` + test, `select/lib.rs:141,209,397,419` `ChangedFile`, `query_names`, `rank_files`, `pack_candidates`, `pack_candidates_into` + two tests, facade `ChangedBlob`, two `Error` variants, `export`, `load`, `path_language:215`, `rank_files:269`, `prepare_changed:292`, `extract:309`, `build_candidate_context:330`, journey `REPORT` + three tests. Every `Result`-returning fn carries `# Errors`. `mise run lints` (clippy `-D warnings` with `missing_docs` lints as configured) exit 0. |
| `agent-rules.md:11` `# Panics` where relevant | PASS | No new `unwrap`/index/`expect` in production paths; `pack_candidates` documents the zero-`max_files` non-panic path. |
| `rust-core.md:7` no unchecked `unwrap` on input/parsing/IO; `thiserror` errors | PASS | `InvalidFact` and new `Error` variants use `thiserror`; `expect` occurs only in tests with named invariants. |
| `agent-rules.md:11` imports at module top, bare types in signatures | PASS | New `use` lines are at top of `facts.rs`, `index/lib.rs`, `facade/lib.rs:15`; signatures use bare `ChangedFile`, `IndexedFileFacts`, `Language`, `ExtractionResult`. Path-qualified *calls* (`bohmian_code_review_select::rank_files`) match the pre-existing facade style. |
| `AGENTS.md:29` / `agent-rules.md:10` concrete owners; free functions only stateless; no ZSTs/one-impl traits | PASS | `export`/`load` are inherent on `SymbolIndex`/`SymbolicContext`; `rank_files`, `pack_candidates`, `pack_candidates_into`, `build_candidate_context` are pure functions over borrowed inputs; no trait or ZST added. |
| `AGENTS.md:28` clone as ownership decision; borrow when possible | PASS | `ChangedFile<'a>` borrows; `rank_files` clones `blob.changed` because `ChangedLines::new` consumes a `Vec<LineRange>` (pre-existing API); `export` clones `path` into an owned persistable record. |
| `AGENTS.md:9–14,22` six crates IO-free; `design.md:20` dependency direction | PASS | No new crate dependency edges; `select` gains `ChangedFile`, facade imports `select::ChangedFile` and `extract::ExtractionResult` (both already downstream). No fs/network/`std::env` in any diff hunk. |
| `agent-rules.md:9` exported facts source-free and version-stamped; foreign version fails | PASS (public contract) | `facts.rs:107–111` carries `extraction_version`; `from_node` drops `docstring`/`signature`/etc.; facade `lib.rs:193` rejects mismatch before `index.load`. Journey asserts serialized JSON contains no source fragments. Note: `SymbolIndex::load` (index `lib.rs:156`) intentionally does not compare versions (documented at `lib.rs:149`); the version check lives on the facade, which is the public contract. Not a finding. |
| `agent-rules.md:8` malformed file must not corrupt other entries | PASS | `IndexedFileFacts::validate` runs before `upsert`; index test and journey assert `file_count`/lookup unchanged after rejection. |
| `agent-rules.md:7` / `rust-core.md:7` validate canonical paths at public ingress | PASS | `path_language`, `rank_files`, `load` route through `relative_path(...)`/`RelativePath`; `RelativePath` `Deserialize` (`relative_path.rs:83–96`) revalidates; journey asserts `../user.ts` fails to deserialize. |
| `symbolic-context.md:7` / `design.md:28` distinct-name scoring, changed file excluded, deterministic ordering | PASS | `rank_files` builds `excluded: BTreeSet<&RelativePath>` from all changed paths, `BTreeSet<String>` per candidate, sorts by score desc then path. |
| `agent-rules.md:5` tests inline; external only via public facade | PASS | New unit tests inline in owning modules; only external target is `crates/bohmian-code-review/tests/symbolic_context_journey.rs`. `scripts/check-test-placement.sh` (part of `lints`) exit 0. |
| `agent-rules.md:4` / `testing-workflows.md:5,9–10` output-changing task updates the real public journey | PASS | Three new journey tests drive `export`/`load`, `path_language`, `rank_files`, `build_candidate_context` through the facade only. |
| `agent-rules.md:6` grammar pins/fixtures intact | PASS | No `Cargo.toml` grammar pin, `grammars/`, or `fixtures/` change in the diff. |
| `AGENTS.md:34` no forbidden production dependency | PASS | Lockfile diff is one line (`serde_json` added to facade deps, already a workspace dep used by `core`); no new package entries. |
| `implementation-execution.md:105` adding a dependency is material | PASS (pre-authorized) | Task §Expected Write Set names "workspace manifest and lockfile only as needed"; the addition is a dev-dependency on an existing workspace crate and is recorded in evidence (task line 94). |
| `AGENTS.md:40` use `mise.toml` lanes; never weaken a gate | PASS | No `#[ignore]`, `#[allow]`, weakened assertion, or lane change in the diff. |
| `spec-driven-development.md:171` task status enumeration | PASS | Frontmatter `status: review` (task line 4) is a listed value. |
| `spec-driven-development.md:183` every named test carries an exact focused command | PASS | Nine named tests, nine `mise exec -- cargo test --locked -p … -- --exact` rows (task lines 84–92); each selector reproduced and selects exactly one test (§4). |
| `spec-driven-development.md:206–208` RED fails for the expected missing behavior, not compilation; `:218` record RED per scenario; `testing-workflows.md:19–20` record exact RED/GREEN | **FAIL** | REPO-1 |
| `spec-driven-development.md:128` only human approval moves draft → approved; `AGENTS.md:3` `changes/active/` holds approved specifications | **FAIL** | REPO-2 |
| `AGENTS.md:38–40` fmt/lints/skills-sync mechanical lanes | PASS | §4. |

## 3. Material findings

### REPO-1 — Evidence record substitutes one compile failure for per-scenario RED

- **Violated rule.** `architecture/references/languages/spec-driven-development.md:206–208`: "**RED** — add or select one test, run its exact focused command, and confirm it fails for the expected missing behavior rather than setup, compilation, or an unrelated defect." and `:218`: "Record the expected RED failure, GREEN result, and final verification." `architecture/references/languages/testing-workflows.md:19–20`: "Record exact RED/GREEN results, broader lane results, `git diff --check`, and tracked/untracked diff inspection."
- **Root cause.** The Implementation Evidence section was written as an aggregate closure, not as the per-scenario execution record the reference requires.
- **Defect class.** Verification-evidence record does not conform to the RED/GREEN recording contract.
- **Locations.** `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md:68` (single RED: "the extended journey failed to compile (missing …)"); `:80–92` (nine focused rows, every one labelled "current GREEN closure" only — no RED result for any owner test: `export_round_trips_…`, `path_language_reports_…`, `multi_file_ranking_…`, `candidate_packing_…`, the two `facts::tests`).
- **Evidence.** Line 68 names a compilation failure as the only RED. Lines 80–92 carry no RED column. No "tracked/untracked diff inspection" result is recorded (line 94 records non-goal exclusions and `Cargo.lock` scope, and line 78 records `git diff --check`, but not an untracked-file inspection).
- **Consequence.** The record cannot show that any of the nine named tests discriminates its behavior: a test that only failed to compile proves the symbols were absent, not that its assertions would reject a wrong implementation (e.g., a `load` that skipped the version check, or a `rank_files` that failed to exclude the second changed path). The reviewer must re-derive that from source rather than from evidence.
- **Testable correction.** Amend the evidence table so each named test row records its RED observation (an assertion failure under the exact focused command, or an explicit "verification-only" rationale per `spec-driven-development.md:219–223` where a runtime RED was impossible for a not-yet-existing public symbol) alongside its GREEN, and add the tracked/untracked inspection result (`git status --porcelain` output or equivalent). Verify by reading the table: every row has RED, GREEN, and the command.

### REPO-2 — Authority documents assert an approval the spec record contradicts

- **Violated rule.** `architecture/references/languages/spec-driven-development.md:128–129`: "Use `draft`, `approved`, and `superseded` status. Only explicit human approval moves a draft to approved." `AGENTS.md:3`: "`changes/active/` holds approved change specifications and task evidence."
- **Root cause.** The diff inserted the phrase "approved" for `changes/active/repo-parser/spec.md` into two durable authority documents while the spec's own status field, the record the reference designates, still reads `status: draft` (`changes/active/repo-parser/spec.md:4`, and its prose at line 9: "revision 10 … awaits approval"). The authority text and the packet record now disagree on the spec's lifecycle state.
- **Defect class.** Durable authority document records an unrecorded lifecycle transition.
- **Locations.** `AGENTS.md:23` ("The approved outer pipeline library crate (`changes/active/repo-parser/spec.md`) …"); `architecture/bohmian-code-review-design.md:5` ("… an outer pipeline library crate, approved in `changes/active/repo-parser/spec.md`, …").
- **Evidence.** `spec.md:4` `status: draft`; `spec.md:9` "Revision 9 was approved; revision 10 … awaits approval." Both authority sentences were added by commit `a59e51d` in this candidate. Secondary: both sentences link to a path under `changes/active/`, which `spec-driven-development.md:66–68` says completion removes; after `$cr-complete` the authority documents will cite a deleted file.
- **Consequence.** A reader of `AGENTS.md` or the design authority is told the outer crate boundary is approved; the repository record says it is a draft awaiting approval. Whichever is true, one authority is wrong, and the spec-driven ordering (`spec-driven-development.md:10–18`) has the authority documents above the spec, so the wrong one wins.
- **Testable correction.** One of: (a) the human records approval by setting `spec.md:4` to `status: approved` (and updating line 9), after which the two sentences are consistent; or (b) reword `AGENTS.md:23` and `design.md:5` to describe the boundary without asserting approval and without linking the transient packet path (e.g., "the outer pipeline library crate specified in the `repo-parser` change"). Verify: `grep -n "approved" AGENTS.md architecture/bohmian-code-review-design.md` and `sed -n 4p changes/active/repo-parser/spec.md` agree. Option (a) is a human decision, not an implementer's.

## 4. Verification commands and results

All run sequentially from the repository root at `1fbcc4a`.

| Command | Result |
|---|---|
| `mise run fmt:check` | exit 0 |
| `mise run lints` (clippy `--workspace --all-targets --all-features -D warnings` + `scripts/check-test-placement.sh`) | exit 0 |
| `mise run check:skills-sync` | exit 0 |
| `git diff --check` | exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey exported_facts_reload_without_source_and_reject_invalid_records -- --exact` | 1 passed, 5 filtered, exit 0 |
| `… --test symbolic_context_journey path_language_identifies_eligible_paths_before_download -- --exact` | 1 passed, exit 0 |
| `… --test symbolic_context_journey multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-index --lib tests::export_round_trips_through_load_and_rejects_invalid_records -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-extract --lib extractor::tests::path_language_reports_eligibility_without_source -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-select --lib tests::multi_file_ranking_unions_names_and_excludes_every_changed_path -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-select --lib tests::candidate_packing_counts_only_selected_candidates -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-core --lib facts::tests::projection_is_source_free_and_rebuilds_a_node -- --exact` | 1 passed, exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-core --lib facts::tests::validation_rejects_empty_identity_and_bad_spans -- --exact` | 1 passed, exit 0 |

## 5. Verification limits

- `mise run gate` and the full `test:*` lanes were not re-run under the time cap; the nine focused selectors, `fmt:check`, `lints`, and `check:skills-sync` were reproduced.
- Pre-existing, untouched conditions were noted but not counted: the task frontmatter uses `spec: SPEC-repo-parser@10` / `maps:` rather than the `spec_revision:` / `requirements:` / `parent_task:` / `remediates:` keys at `spec-driven-development.md:140–152`; `spec-driven-development.md:134` derives tasks only from an approved revision while `spec.md:4` is `draft` (the spec file is outside the diff; its interaction with the diff is REPO-2).
- `SymbolIndex::load` ignoring `extraction_version` was judged consistent with the design (the facade owns the version check; `index` cannot depend on `extract`) and is recorded as a note, not a finding.
- No Ponytail audit, acceptance review, or optional-improvement review was performed.

Overall: FAIL

```citations
[
  {"id":"REPO-1","path":"changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md","line":68,"snippet":"Commits `a59e51d..HEAD` on `review-helper-setup`. RED: the extended journey failed to compile (missing `export`, `load`, `path_language`, `rank_files`, `build_candidate_context`, `EXTRACTION_VERSION`, `IndexedFileFacts`, `Error::InvalidFacts`, `Error::ExtractionVersionMismatch`). GREEN: every lane below exited 0.","symbols":[]},
  {"id":"REPO-1","path":"changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md","line":80,"snippet":"Focused commands (current GREEN closure, TASK-001-R2; each run sequentially in this session):","symbols":[]},
  {"id":"REPO-1","path":"architecture/references/languages/spec-driven-development.md","line":206,"snippet":"1. **RED** — add or select one test, run its exact focused command, and confirm","symbols":[]},
  {"id":"REPO-1","path":"architecture/references/languages/spec-driven-development.md","line":207,"snippet":"   it fails for the expected missing behavior rather than setup, compilation,","symbols":[]},
  {"id":"REPO-1","path":"architecture/references/languages/spec-driven-development.md","line":218,"snippet":"Record the expected RED failure, GREEN result, and final verification. A","symbols":[]},
  {"id":"REPO-1","path":"architecture/references/languages/testing-workflows.md","line":19,"snippet":"Run Cargo-backed checks sequentially. Record exact RED/GREEN results, broader","symbols":[]},
  {"id":"REPO-1","path":"architecture/references/languages/testing-workflows.md","line":20,"snippet":"lane results, `git diff --check`, and tracked/untracked diff inspection.","symbols":[]},
  {"id":"REPO-2","path":"AGENTS.md","line":23,"snippet":"The approved outer pipeline library crate (`changes/active/repo-parser/spec.md`) is the only crate permitted Git provider and storage IO. It owns GitHub/GHES source access, Git identity and blob verification, repository eligibility, bounded ingest, snapshot build/update, PR overlays, context publication and read-back, and the narrow storage/persistence traits a consumer implements. It depends on the public `bohmian-code-review` facade only and must not duplicate extraction, indexing, resolution, scoring, or packing. A consuming service owns credentials, webhooks, scheduling, review agents, models, comment publication, and cross-repository search. Never report a partial caller-supplied index as complete repository coverage.","symbols":[]},
  {"id":"REPO-2","path":"architecture/bohmian-code-review-design.md","line":5,"snippet":"This Rust workspace builds symbolic context for code review from **caller-supplied, in-memory source**. Six symbolic crates form an IO-free library; an outer pipeline library crate, approved in `changes/active/repo-parser/spec.md`, gets Git tree diffs and blobs through provider APIs, verifies repository/commit/blob identity, and passes bounded source into the symbolic facade. Neither clones or holds a working copy. Runtime source bytes are transient: the index retains extracted facts, exported facts are source-free, and the returned context owns only selected text.","symbols":[]},
  {"id":"REPO-2","path":"changes/active/repo-parser/spec.md","line":4,"snippet":"status: draft","symbols":[]},
  {"id":"REPO-2","path":"architecture/references/languages/spec-driven-development.md","line":128,"snippet":"Use `draft`, `approved`, and `superseded` status. Only explicit human approval","symbols":[]},
  {"id":"REPO-2","path":"AGENTS.md","line":3,"snippet":"This repository owns reusable Rust helpers for in-memory symbolic code-review context. `architecture/bohmian-code-review-design.md` is the design authority. `changes/active/` holds approved change specifications and task evidence. Wyrd and codegraph-rs are reference repositories, not runtime dependencies or authorities for this repository.","symbols":[]},
  {"id":"PASS-rustdoc","path":"AGENTS.md","line":32,"snippet":"- Every new or materially modified Rust item, including private items and tests, requires useful rustdoc describing intent, workflow, and invariants. Fallible functions need `# Errors`; document remaining panics and cancellation/partial progress where applicable.","symbols":[]},
  {"id":"PASS-version","path":"crates/bohmian-code-review-extract/src/lib.rs","line":119,"snippet":"pub const EXTRACTION_VERSION: &str = \"codegraph-kernel-572d22bf.tree-sitter-0.25.10.facts-1\";","symbols":["EXTRACTION_VERSION"]},
  {"id":"PASS-version","path":"crates/bohmian-code-review/src/lib.rs","line":193,"snippet":"        if facts.extraction_version != EXTRACTION_VERSION {","symbols":["SymbolicContext::load","EXTRACTION_VERSION"]},
  {"id":"PASS-version","path":"crates/bohmian-code-review-index/src/lib.rs","line":156,"snippet":"    pub fn load(&mut self, facts: IndexedFileFacts) -> Result<(), InvalidFact> {","symbols":["SymbolIndex::load"]},
  {"id":"PASS-facts","path":"crates/bohmian-code-review-core/src/facts.rs","line":107,"snippet":"pub struct IndexedFileFacts {","symbols":["IndexedFileFacts"]},
  {"id":"PASS-facts","path":"crates/bohmian-code-review-core/src/facts.rs","line":111,"snippet":"    pub extraction_version: String,","symbols":["IndexedFileFacts"]},
  {"id":"PASS-select","path":"crates/bohmian-code-review-select/src/lib.rs","line":209,"snippet":"pub fn rank_files(index: &SymbolIndex, changed: &[ChangedFile<'_>]) -> Vec<RankedFile> {","symbols":["rank_files"]},
  {"id":"PASS-facade","path":"crates/bohmian-code-review/src/lib.rs","line":215,"snippet":"    pub fn path_language(&self, path: &str) -> Result<Option<Language>, Error> {","symbols":["SymbolicContext::path_language"]},
  {"id":"PASS-deps","path":"crates/bohmian-code-review/Cargo.toml","line":20,"snippet":"serde_json.workspace = true","symbols":[]},
  {"id":"PASS-deps","path":"architecture/references/languages/implementation-execution.md","line":105,"snippet":"- adding a dependency or Cargo feature;","symbols":[]},
  {"id":"PASS-commands","path":"architecture/references/languages/spec-driven-development.md","line":183,"snippet":"Every named test in a task must also carry its exact focused command. Run the","symbols":[]},
  {"id":"PASS-placement","path":"architecture/agent-rules.md","line":5,"snippet":"- Tests are inline with an owning Rust module by default. An external test earns its place by crossing crate boundaries through the public facade. Keep the fast lane credential-free and network-free.","symbols":[]},
  {"id":"PASS-sourcefree","path":"architecture/agent-rules.md","line":9,"snippet":"- Do not retain source blobs in the index, fetch source in a symbolic crate, or copy the donor's PostgreSQL, SDK, or `SourceTree` orchestration. Exported facts carry no source fragments and are stamped with the extraction version; loading facts from another version fails. Only the outer pipeline crate fetches source, and it never persists it outside the selected review-context object.","symbols":[]}
]
```
