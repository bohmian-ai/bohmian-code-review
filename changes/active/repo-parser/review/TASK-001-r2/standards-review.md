# TASK-001 r2 — repository-standards review (`repo-rev`)

Subject: base `9715440d` → candidate `1108731565` (cumulative, includes TASK-001-R1). Scope: repository-rule compliance of the complete diff only. Read-only; no tracked file modified.

## 1. Authority coverage

| Changed surface | Governing authority (file:lines) |
|---|---|
| `crates/bohmian-code-review-core/src/facts.rs` (new), `core/src/lib.rs` | AGENTS.md:13,27-32; agent-rules.md:7,9,10; design.md:13,20,34; rust-core.md:3-5; symbolic-context.md:3 |
| `crates/bohmian-code-review-extract/src/extractor.rs`, `extract/src/lib.rs` (`EXTRACTION_VERSION`, `path_language`) | AGENTS.md:14,27-33; agent-rules.md:5,9,10; design.md:14,34; rust-core.md:3-5 |
| `crates/bohmian-code-review-index/src/lib.rs` (`export`/`load`) | AGENTS.md:15,27-32; agent-rules.md:7,8,9,10; design.md:15,26,34; rust-core.md:3-5; symbolic-context.md:3 |
| `crates/bohmian-code-review-select/src/lib.rs` (`ChangedFile`, `rank_files`, `pack_candidates`) | AGENTS.md:17,27-32; agent-rules.md:6,9,10; design.md:17,20,28-30,34; symbolic-context.md:5-7; rust-core.md:3-5 |
| `crates/bohmian-code-review/src/lib.rs` (facade) | AGENTS.md:18,21,27-32; agent-rules.md:2,9,10; design.md:18,20,34; rust-core.md:3-5 |
| `crates/bohmian-code-review/Cargo.toml`, `Cargo.lock` (`serde_json` dev-dep) | AGENTS.md:27,34; implementation-execution.md:104-106,265-276; task "Expected Write Set" (TASK-001:56-58) |
| `crates/bohmian-code-review/tests/symbolic_context_journey.rs` | AGENTS.md:38-40; agent-rules.md:4,5,12; design.md:38; testing-workflows.md:3-15; mise.toml `test:symbolic-context:journey`, `lints` (`check-test-placement.sh`) |
| `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md` edits | spec-driven-development.md:81-84 (authority must reflect lasting contracts); TASK-001:16 (ordered update of the three authorities); spec.md:19,85 |
| `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` (status + Implementation Evidence) | spec-driven-development.md:138-175,177-200,202-223; testing-workflows.md:17-20; implementation-execution.md:114-137,265-292 |
| `changes/active/repo-parser/review/TASK-001-r1/TASK-001-R1-task-status.md` (outside the immutable diff; reviewed on request) | spec-driven-development.md:61-64,138-175,239-246; `.agents/skills/cr-task-review/SKILL.md:310-320` (placement, required sections) |

Authorities loaded: AGENTS.md, architecture/agent-rules.md, architecture/bohmian-code-review-design.md, architecture/references/README.md, languages/spec-driven-development.md, languages/implementation-execution.md, languages/rust-core.md, languages/testing-workflows.md, domain/symbolic-context.md, mise.toml. No `.codegraph/` index; `rg`/`git diff` used.

## 2. Rule-by-rule result

| Rule | Citation | Evidence | Result |
|---|---|---|---|
| Formatting | AGENTS.md:40; mise `fmt:check` | `mise run fmt:check` → exit 0 (observed) | PASS |
| Clippy `-D warnings` + inline owner-test placement | AGENTS.md:40; agent-rules.md:4; mise `lints` | `mise run lints` → exit 0, `check-test-placement.sh` passed (observed) | PASS |
| No unchecked `unwrap()`; `expect()` only for named invariants | AGENTS.md:30; rust-core.md:5 | `git diff … -- crates \| grep -c 'unwrap()'` = 0; every added `expect(` is inside `#[cfg(test)]`/integration tests with a named reason | PASS |
| `thiserror` library errors | AGENTS.md:30 | `InvalidFact` (`facts.rs:129-146`) and new `Error` variants (`facade lib.rs:80-97`) derive `thiserror::Error` | PASS |
| Imports at module top; bare imported types in signatures | AGENTS.md:31; agent-rules.md:9 | `facts.rs:10-14`, `index lib.rs:9-12`, `facade lib.rs:12-16`; signatures use `IndexedFileFacts`, `ChangedFile`, `ExtractionResult` bare | PASS |
| Rustdoc on every new/materially modified item incl. private items and tests; `# Errors` on fallible fns | AGENTS.md:32; agent-rules.md:9; rust-core.md:5 | All new pub/private items and each new `#[test]` carry doc comments; `# Errors` present on `DeclarationFact::validate`, `IndexedFileFacts::validate`, `SymbolIndex::load`, `SymbolicContext::load`/`path_language`/`rank_files`/`prepare_changed`/`extract`, `pack_candidates`, `pack_candidates_into`, `build_candidate_context`. `# Panics`: no new panicking production path | PASS |
| Ownership: references/slices where ownership unnecessary; `.clone()` deliberate | AGENTS.md:29 | `ChangedFile<'a>` is all borrows; `export` clones `path` into an owned export record; `rank_files` clones `blob.changed` because `ChangedLines::new` takes `Vec<LineRange>` by value; `load` clones `path` once for the error context | PASS |
| Concrete owners / inherent methods; free fns only for stateless transforms; no one-impl traits | AGENTS.md:27; agent-rules.md:8; rust-core.md:3 | `export`/`load`/`path_language`/`rank_files` are inherent methods on `SymbolIndex`, `Extractor`, `SymbolicContext`; `rank_files`, `pack_candidates`, `build_candidate_context` are deterministic free fns; no new trait | PASS |
| Core stays IO-free, no parser dependency | design.md:13,20; rust-core.md:3 | `facts.rs` depends only on `crate::{language,node,relative_path}`, serde, thiserror | PASS |
| Index retains no source text; exported facts source-free and version-stamped; loading other version fails; malformed record leaves other files intact | agent-rules.md:6,7; design.md:34; symbolic-context.md:3 | `DeclarationFact` has no docstring/signature/return-type fields (`facts.rs:17-35`); `IndexedFileFacts.extraction_version` (`facts.rs:109`); `SymbolicContext::load` compares to `EXTRACTION_VERSION` before touching the index (`facade lib.rs:193-205`); `SymbolIndex::load` validates before `upsert` (`index lib.rs:157-166`); journey asserts no `console.log`/`(id: string)` in JSON and `file_count()==4` after rejects | PASS |
| Replacement exposes new complete facts or preserves old on failure | agent-rules.md:6 | `load` → `validate()?` then `upsert`; index test asserts postings unchanged after `InvalidSpan` | PASS |
| Facade composes, does not duplicate stage logic | AGENTS.md:18; design.md:20 | `SymbolicContext::rank` and `rank_files` both route to `select::rank`/`rank_files`; `build_candidate_context` is a one-call delegate to `select::pack_candidates`; `select::rank` delegates to `rank_files`, `pack` and `pack_candidates` share `pack_candidates_into` | PASS |
| Distinct-name scoring, every changed path excluded, deterministic order | design.md:28,34; symbolic-context.md:7 | `rank_files`: `BTreeSet<&RelativePath>` exclusion, `BTreeSet<String>` per-file names, sort by score desc then path (`select lib.rs:207-243`) | PASS |
| No silent truncation, omissions explicit, typed errors for missing blob | design.md:30; symbolic-context.md:7 | `pack_candidates_into` reuses existing omission/`MissingBlob` paths; tests assert `OmissionReason::TokenLimit`/`FileLimit` and `SelectError::MissingBlob` | PASS |
| Grammar pins and fixtures intact | AGENTS.md:33; agent-rules.md:4 | Diff touches no `grammars/`, fixture, or pin; `tree-sitter = "=0.25.10"` unchanged and matches the `EXTRACTION_VERSION` string | PASS |
| Dependency rules: no prohibited production deps; dependency change inside approved write set | AGENTS.md:34; implementation-execution.md:104-106; TASK-001:56-58 | Only change is `serde_json.workspace = true` under `[dev-dependencies]` of the facade; `serde_json = "1"` was already a workspace dependency used by core/extract; `Cargo.lock` gains one edge, no new package; task write set explicitly allows "workspace manifest and lockfile only as needed" | PASS |
| External test earns its place by crossing crates via the public facade; fast lane credential/network-free | agent-rules.md:3; testing-workflows.md:3,15 | New journey tests call only `bohmian_code_review::*` on in-memory strings | PASS |
| Task changing reviewer output extends the real public journey, incl. negative flows | agent-rules.md:2; testing-workflows.md:9-11 | Three journey tests added with version-mismatch, malformed-fact, invalid-path, missing-blob, deletion-only, and starved-budget negatives | PASS |
| No hidden failures (`#[allow]`, `#[ignore]`, weakened assertions, changed goldens) | agent-rules.md:10; AGENTS.md:40 | `git diff` shows no `#[allow`/`#[ignore]`; no assertion removed; no golden touched | PASS |
| Whitespace hygiene | implementation-execution.md:247 | `git diff --check 9715440d..1108731565` → exit 0 | PASS |
| Authority docs updated before/with the lasting contract change and consistent with spec | spec-driven-development.md:81-84; TASK-001:16 | AGENTS.md:21-23, agent-rules.md:1,7, design.md:5,22,32-34 describe the six-crate/outer-crate split, source-free versioned facts, path-only eligibility, multi-file ranking; wording matches spec.md:19,85 | PASS |
| Task frontmatter status in enumerated set | spec-driven-development.md:171-173 | `TASK-001…md:4` = `status: review` (fixed by R1); `rg -n '^status: implemented' changes/` → no match | PASS |
| Task evidence records RED, GREEN, final verification | spec-driven-development.md:218-219; testing-workflows.md:19-20 | TASK-001:68 records RED (compile failure on missing API), GREEN, lanes, `git diff --check`, `gate` | PASS |
| Every named test in a task carries its exact focused command; a named test cannot be replaced by only a package-wide `mise run` lane | spec-driven-development.md:183-184,196-200; testing-workflows.md:17-18 | TASK-001:73-76 name eight tests; TASK-001:78 records only `mise run` lanes; no `mise exec -- cargo test --locked … -- --exact` selector appears anywhere in the task | **FAIL → REPO-1** |
| Remediation task: parent task, validated finding IDs, diagnosis, outcome, decision-complete recommendation, acceptance, proof, evidence | spec-driven-development.md:164,239-246; SKILL.md:310-320 | `TASK-001-R1-task-status.md:1-8` (`kind: remediation`, `remediates`, `findings`), sections present, evidence table with observed outputs; status `review` is in-set | PASS |

## 3. Findings

### REPO-1 — Task evidence names tests without their exact focused commands

- **Classification:** documentation-contract violation (task evidence), non-code.
- **Root cause:** the Implementation Evidence section added to `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` records verification at lane granularity only. The reference lets a *proposed* task name only its owner lane, but once the implementer names a test the same reference requires the exact `mise exec -- cargo test --locked -p <crate> --<target> <selector> -- --exact` command to be recorded and proven to select a test.
- **Defect class:** missing required field in a spec-driven document (named test without focused command).
- **Locations (one finding):** `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md:73` (names `exported_facts_reload_without_source_and_reject_invalid_records`, `export_round_trips_through_load_and_rejects_invalid_records`, `facts::tests`), `:74` (`path_language_identifies_eligible_paths_before_download`, `path_language_reports_eligibility_without_source`), `:75` (`multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only`, `multi_file_ranking_unions_names_and_excludes_every_changed_path`, `candidate_packing_counts_only_selected_candidates`), `:78` (lane-only command list). Sibling sweep: the R1 remediation task names no Rust test, so it is not affected.
- **Violated rule:** `architecture/references/languages/spec-driven-development.md:183-184` ("Every named test in a task must also carry its exact focused command"), `:196-200` ("the implementer then records the exact focused command. A named test cannot be replaced by only a package-wide `mise run` lane"); `architecture/references/languages/testing-workflows.md:17-18`.
- **Evidence:** `rg -n 'mise exec' changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` → no match; line 78 lists only `mise run …` lanes.
- **Observable consequence:** a reviewer cannot reproduce the per-criterion proof from the document; the selectors had to be derived from source. This is a document defect only — I derived and ran the focused commands and all pass (see §4), so no behavioral gap exists.
- **Testable correction:** append to the task's evidence the exact commands below (each verified to select exactly the named test, all exit 0), or one per named test:
  - `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey exported_facts_reload_without_source_and_reject_invalid_records -- --exact`
  - `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey path_language_identifies_eligible_paths_before_download -- --exact`
  - `mise exec -- cargo test --locked -p bohmian-code-review --test symbolic_context_journey multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only -- --exact`
  - `mise exec -- cargo test --locked -p bohmian-code-review-index --lib tests::export_round_trips_through_load_and_rejects_invalid_records -- --exact`
  - `mise exec -- cargo test --locked -p bohmian-code-review-extract --lib extractor::tests::path_language_reports_eligibility_without_source -- --exact`
  - `mise exec -- cargo test --locked -p bohmian-code-review-select --lib tests::multi_file_ranking_unions_names_and_excludes_every_changed_path -- --exact`
  - `mise exec -- cargo test --locked -p bohmian-code-review-select --lib tests::candidate_packing_counts_only_selected_candidates -- --exact`
  - `mise exec -- cargo test --locked -p bohmian-code-review-core --lib facts::tests::projection_is_source_free_and_rebuilds_a_node -- --exact` and `… facts::tests::validation_rejects_empty_identity_and_bad_spans -- --exact`
  Acceptance: `rg -c 'mise exec -- cargo test --locked' changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` ≥ 8 and each command exits 0 with `1 passed`.

## 4. Observed verification (this review, read-only)

- `mise run fmt:check` → exit 0.
- `mise run lints` (clippy `-D warnings --all-features --all-targets` + `check-test-placement.sh`) → exit 0.
- `git diff --check 9715440d..1108731565` → exit 0.
- Focused commands listed under REPO-1: 9 tests, each `ok`, `0 failed` (journey 3/3, index 1/1, extract 1/1, select 2/2, core facts 2/2).

## 5. Observations (not findings)

- `changes/active/repo-parser/spec.md:4` is `status: draft` while TASK-001 cites `SPEC-repo-parser@10`; spec-driven-development.md:134 derives tasks only from an approved revision. Pre-existing at base (`9715440` "adding tasks"), not introduced by this diff; excluded per the finding contract.
- The remediation task lives in `review/TASK-001-r1/` per `.agents/skills/cr-task-review/SKILL.md:310-311`, while spec-driven-development.md:43-51 sketches `tasks/TASK-001-R1-<name>.md`. The skill is the operative user-instructed workflow and the file is outside the immutable diff; recorded only so the packet convention is visible.
- Task frontmatter uses `spec: SPEC-repo-parser@10` / `maps:` rather than the reference's illustrative `spec:`/`spec_revision:`/`requirements:` keys; this shape predates the diff for every task in the packet and carries the same information.

## Verification limits

Did not rerun `mise run gate` or the full `test:*` lanes (implementer reports exit 0; focused selectors and `lints`/`fmt:check` reproduced here). No Ponytail audit performed (out of scope).

```citations
[{"id": "REPO-1", "path": "changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md", "line": 73, "snippet": "| REQ-001 source-free facts export/load, invalid facts fail without corrupting others |", "symbols": ["exported_facts_reload_without_source_and_reject_invalid_records", "export_round_trips_through_load_and_rejects_invalid_records"]},
 {"id": "REPO-1", "path": "changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md", "line": 74, "snippet": "| REQ-002 extraction version invalidates old facts; path-only eligibility |", "symbols": ["path_language_identifies_eligible_paths_before_download", "path_language_reports_eligibility_without_source"]},
 {"id": "REPO-1", "path": "changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md", "line": 75, "snippet": "| REQ-003 multi-file ranking, all changed paths excluded, distinct-name scoring, deterministic ties, candidate-only packing with omissions |", "symbols": ["multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only", "multi_file_ranking_unions_names_and_excludes_every_changed_path", "candidate_packing_counts_only_selected_candidates"]},
 {"id": "REPO-1", "path": "changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md", "line": 78, "snippet": "Commands run in this session, each exit 0: `mise run test:extract`, `mise run test:index`, `mise run test:select`, `mise run test:resolve`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`.", "symbols": []}]
```

Overall: FAIL
