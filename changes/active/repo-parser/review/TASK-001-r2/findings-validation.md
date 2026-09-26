# TASK-001 review r2 — structured validation (ponytail-rev)

Subject: candidate `1108731565fc8801b47f286bd04351a3c467cf90` (cumulative: TASK-001 + TASK-001-R1) against base `9715440d`. Read-only; no tracked file modified.

## Merged claim table

| Merged ID | Wave 1 source IDs | Root cause | Sibling of FIND-TASK-001-1? | Result |
|---|---|---|---|---|
| FIND-TASK-001-2 | REPO-1 | Implementation Evidence records verification at lane granularity; eight named Rust tests carry no exact focused command | No. FIND-TASK-001-1 is an out-of-set enumerated value (`status:` frontmatter, spec-driven-development.md:171-173). REPO-1 is a missing required field (spec-driven-development.md:183-184, :196-200; testing-workflows.md:17-18). Different rule, different defect class, different line. Not reopened. | CONFIRMED, blocking (gate c) |

Only one claim survived the citation check; no merge needed.

## Per-claim validation

### REPO-1 -> FIND-TASK-001-2 — CONFIRMED

**Rule, verbatim.**

- `architecture/references/languages/spec-driven-development.md:183-184`: "Every named test in a task must also carry its exact focused command. Run the repository-pinned toolchain through `mise exec --`."
- `architecture/references/languages/spec-driven-development.md:196-200`: "Do not invent a selector. Confirm package, target, and test names from source and verify the exact command selects a test. A new task may specify only its owner lane until an implementation names the test; the implementer then records the exact focused command. A named test cannot be replaced by only a package-wide `mise run` lane."
- `architecture/references/languages/testing-workflows.md:17-18`: "When a task names a Rust test, record its exact package, target, and selector with `mise exec -- cargo test --locked`; prove that the selector runs a test."
- `architecture/references/languages/implementation-execution.md:241-242`: focused verification step 1 is "each named narrow test or reproduction through its exact `mise exec -- cargo test --locked -p <crate> --lib <test> -- --exact` command".

**Does the rule apply (exception check).** The only exception, spec-driven-development.md:197-198, covers a *new task* that names only its owner lane "until an implementation names the test". At base `9715440d` the task named no test (Verification and Evidence, task line 56, lists only `mise run` lanes) — the exception held. The candidate's Implementation Evidence (task lines 73-75) names eight tests. Once named, the same sentence obligates the implementer to record the exact focused command. The exception is exhausted; the rule applies.

**Diff hunk that introduced the offending text.** `/tmp/task001-r2-diff.patch:98-110` adds `## Implementation Evidence`, the four-row table naming the tests, and line 110: `+Commands run in this session, each exit 0: `mise run test:extract`, `mise run test:index`, ...` — lanes only.

**Reproduction.**

```
$ rg -n 'mise exec' changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md
(no output) rg exit=1
```

Task lines 73-75 name: `exported_facts_reload_without_source_and_reject_invalid_records`, `export_round_trips_through_load_and_rejects_invalid_records`, core `facts::tests` (module, two tests), `path_language_identifies_eligible_paths_before_download`, `path_language_reports_eligibility_without_source`, `multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only`, `multi_file_ranking_unions_names_and_excludes_every_changed_path`, `candidate_packing_counts_only_selected_candidates`. Line 78 records nine `mise run`/`git` commands and no `mise exec -- cargo test --locked ... -- --exact` selector.

**Precedent.** The rule has already been enforced as a blocking finding in this repository: `changes/active/review-helper-setup/review/TASK-002-20260925-01/TASK-002-R1-test-proof.md:18` (FIND-TASK-002-2, same rule, same gap), remediated by recording focused commands at `changes/active/review-helper-setup/tasks/TASK-002-index-resolver.md:120-123`. `changes/active/review-helper-setup/tasks/TASK-001-workspace-extractor.md:101` also records an exact `-- --exact` command. Precedent agrees with the written rule.

**Gate.** (c) — the diff violates a rule written in `architecture/` cited by file and line. Gates (a), (b), (d) do not apply: no production path is affected; no task acceptance criterion is violated (all nine focused runs pass, see below); no source-trust/security/durability impact.

**Ladder.**
1. Delete? The Implementation Evidence section is required completion evidence (spec-driven-development.md:163); the named tests are the proof of REQ-001..003. Cannot delete.
2. Existing repo behavior? `mise run gate` does not validate task evidence; `mise.toml:52` `test:symbolic-context:journey` runs the whole target, which is exactly the package-wide lane the rule says cannot substitute. No.
3-5. stdlib/native/dependency: not applicable to a document.
6-7. Minimum correction: append the nine focused commands with observed `1 passed` results to the task's Implementation Evidence. **No code change required**; docs-only.

**Focused closure proof (derived from source, run sequentially via `mise exec --`, all exit 0).** Package names from `crates/*/Cargo.toml:2`; module paths from `crates/bohmian-code-review-extract/src/lib.rs:59` (`mod extractor;`), `extractor.rs:298` (`mod tests`), `bohmian-code-review-index/src/lib.rs:205`, `bohmian-code-review-select/src/lib.rs:600`, `bohmian-code-review-core/src/facts.rs:154`; test names from `symbolic_context_journey.rs:389,488,516`, `index/src/lib.rs:409`, `extract/src/extractor.rs:312`, `select/src/lib.rs:769,962`, `core/src/facts.rs:187,200`.

| Command (`mise exec -- ` prefix) | Observed |
|---|---|
| `cargo test --locked -p bohmian-code-review --test symbolic_context_journey exported_facts_reload_without_source_and_reject_invalid_records -- --exact` | `1 passed; 0 failed; 5 filtered out` |
| `cargo test --locked -p bohmian-code-review --test symbolic_context_journey path_language_identifies_eligible_paths_before_download -- --exact` | `1 passed; 0 failed; 5 filtered out` |
| `cargo test --locked -p bohmian-code-review --test symbolic_context_journey multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only -- --exact` | `1 passed; 0 failed; 5 filtered out` |
| `cargo test --locked -p bohmian-code-review-index --lib tests::export_round_trips_through_load_and_rejects_invalid_records -- --exact` | `1 passed; 0 failed; 3 filtered out` |
| `cargo test --locked -p bohmian-code-review-extract --lib extractor::tests::path_language_reports_eligibility_without_source -- --exact` | `1 passed; 0 failed; 139 filtered out` |
| `cargo test --locked -p bohmian-code-review-select --lib tests::multi_file_ranking_unions_names_and_excludes_every_changed_path -- --exact` | `1 passed; 0 failed; 9 filtered out` |
| `cargo test --locked -p bohmian-code-review-select --lib tests::candidate_packing_counts_only_selected_candidates -- --exact` | `1 passed; 0 failed; 9 filtered out` |
| `cargo test --locked -p bohmian-code-review-core --lib facts::tests::projection_is_source_free_and_rebuilds_a_node -- --exact` | `1 passed; 0 failed; 26 filtered out` |
| `cargo test --locked -p bohmian-code-review-core --lib facts::tests::validation_rejects_empty_identity_and_bad_spans -- --exact` | `1 passed; 0 failed; 26 filtered out` |

Every selector selects exactly one test; the code is correct. The defect is confined to the evidence record.

## Retained blocking ledger

### FIND-TASK-001-2

- **Wave 1 source IDs:** REPO-1
- **Status:** CONFIRMED
- **Gate:** (c) written rule in `architecture/` violated by the diff
- **Classification:** VIOLATION (documentation contract)
- **Root cause:** the implementer recorded TASK-001 verification at `mise run` lane granularity while naming eight specific Rust tests as the acceptance evidence.
- **Defect class:** missing required field in a spec-driven document (named test without its exact focused command and observed selection).
- **Sibling locations:** `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md:73`, `:74`, `:75` (the named tests) and `:78` (the lane-only command line). No other site: `TASK-001-R1-task-status.md` names no Rust test (its named proofs are `sed`/`rg` commands, recorded exactly at lines 62-63); `rg -n 'mise exec' changes/active/repo-parser/tasks/` returns nothing.
- **Violated obligation:** `architecture/references/languages/spec-driven-development.md:183-184`, `:196-200`; `architecture/references/languages/testing-workflows.md:17-18`; `architecture/references/languages/implementation-execution.md:241-242`.
- **Evidence:** diff hunk `/tmp/task001-r2-diff.patch:98-110`; task file lines 73-78 at candidate.
- **Reproduction:** `rg -n 'mise exec' changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` -> no output, exit 1.
- **Observable consequence:** the task record cannot demonstrate that any named test was individually selected and passed; a reader or tooling following the reference has no replayable per-test command, and a later regression in one named test could hide behind a green package-wide lane. Precedent FIND-TASK-002-2 treated the identical gap as blocking.
- **Decision-complete correction (owner: TASK-001 implementer; file: `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md`, Implementation Evidence section only):** after line 78, add a "Focused commands" table containing the nine `mise exec -- cargo test --locked ... -- --exact` rows above with their observed `1 passed` results, labelled as current GREEN closure (do not retroactively claim they were used for RED; the recorded RED was a compile failure of the journey target, which is accurate). No change to frontmatter, code, tests, manifests, lockfile, pins, or fixtures.
- **Focused closure proof covering every sibling:** `rg -c 'mise exec -- cargo test --locked' changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` >= 9; each of the nine commands re-run exits 0 with `1 passed`; `git diff --stat <candidate>..HEAD` lists only the task file; `mise run gate` and `mise run check:skills-sync` exit 0.

## Notes (non-blocking)

- REPO-1 (aside): task line 73 cites core `facts::tests` as a module, not a specifically named test. Recording both of its tests (above) is the conservative closure; the module name alone would arguably not trigger the rule.

## Rejected claims

None. The three additional REPO-1 citations at lines 73-75 were dropped by the citation checker for snippet form, not substance; they are retained here as sibling locations of FIND-TASK-001-2, verified directly.

## Prior-finding closure

**FIND-TASK-001-1 — CLOSED by the cumulative candidate.** Evidence: `sed -n 4p changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` -> `status: review`, in the enumerated set at spec-driven-development.md:171-172; commit `932f6a5` changes only that line; `git diff --check 9715440d..1108731565 -- . ':!changes/active/repo-parser/review'` exits 0. FIND-TASK-001-2 is not a sibling (different rule and defect class), so FIND-TASK-001-1 is not reopened.

## Verification limits

- Broad lanes (`test:extract`, `test:index`, `test:select`, `test:symbolic-context:journey`, `fmt:check`, `lints`, `gate`) were not re-run in this scope; implementer reports exit 0 and the nine focused runs above exercised every named test.
- An `rg` precedent sweep over `changes/` incidentally surfaced grep-line matches from Wave 1 `standards-review.md`; those lines were not used — all commands, module paths, and test names above were derived from source and executed independently.
- Working tree carried only the untracked `review/TASK-001-r2/` directory; no tracked file was modified.

```citations
[{"id": "FIND-TASK-001-2", "path": "changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md", "line": 78, "snippet": "Commands run in this session, each exit 0: `mise run test:extract`", "symbols": []},
 {"id": "FIND-TASK-001-2", "path": "changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md", "line": 73, "snippet": "| REQ-001 source-free facts export/load,", "symbols": ["exported_facts_reload_without_source_and_reject_invalid_records", "export_round_trips_through_load_and_rejects_invalid_records"]},
 {"id": "FIND-TASK-001-2", "path": "architecture/references/languages/spec-driven-development.md", "line": 183, "snippet": "Every named test in a task must also carry its exact focused command. Run the", "symbols": []},
 {"id": "FIND-TASK-001-2", "path": "architecture/references/languages/spec-driven-development.md", "line": 200, "snippet": "package-wide `mise run` lane.", "symbols": []},
 {"id": "FIND-TASK-001-2", "path": "architecture/references/languages/testing-workflows.md", "line": 17, "snippet": "When a task names a Rust test, record its exact package, target, and selector", "symbols": []}]
```

Overall: FAIL
