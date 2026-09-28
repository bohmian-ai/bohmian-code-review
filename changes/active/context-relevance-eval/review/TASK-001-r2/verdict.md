# TASK-001 task review — round 2

**Verdict: FIX_REQUIRED.** Immutable base `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab`; cumulative candidate `a1342a5a3c154b7678cf59764dab1ceb0d70cc76`. Approved authority: `changes/active/context-relevance-eval/spec.md` revision 4, original `tasks/TASK-001-pinned-sample.md`, and prior remediation `review/TASK-001-r1/TASK-001-R1-listing-completeness-and-rustdoc.md`. The complete base-to-candidate diff was reviewed; the candidate stayed at the pinned commit. Review artifacts are the only working-tree additions.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001, REQ-007: two named repositories and credentialed, read-only, opt-in command | `main.rs:37-42,251-289`; `mise.toml` live lane outside `gate` | Source and task graph inspection | PASS |
| REQ-002: discover 50 PRs before cutoff in merge-time and PR-number order, or fail clearly | `sample.rs:126-214`; `main.rs:197-244` | Focused sampler test: five passed; impossible timestamp path in FIND-TASK-001-4 | **FAIL** |
| REQ-002, INV-003: one complete, consistent PR file listing; invalid entry/incomplete listing fails | `main.rs:96-108`; `sample.rs:218-324` | Provider-ceiling test passes; duplicate-path source reproduction in FIND-TASK-001-3 | **FAIL** |
| REQ-002–003: 2–15 distinct eligible files, one modified file with base declaration, another with changed lines; patch omissions remain file-local | `sample.rs:276-339`; `main.rs:156-188` | Focused sampler tests; duplicate entries can falsely satisfy distinct-file condition | **FAIL** |
| INV-001, AC-005: no production, persisted-format, gate, second listing, `PrContext`, or replay change | Diff adds only example, example registration, opt-in lane, and task/review evidence | Complete diff and source inspection | PASS |
| INV-004–005: source-free output, in-memory source, read-only provider use | `main.rs:117-147,251-289` | Source inspection | PASS |
| Repository rule: stateful workflows have cohesive concrete owners | `main.rs:61-88,96-244` | Written rule and source inspection in FIND-TASK-001-5 | **FAIL** |
| Prior R1: cap at 3,000 accepted/rejected entries and rustdoc on test module/fixture | `sample.rs:234-240,289-294,344-347,386-387` | `listing_fails_at_provider_limits`, source inspection, five focused tests pass | PASS; FIND-TASK-001-1/-2 closed |

## Independent review and validated findings

Wave 1: task reviews A and B **PASS** with no findings; standards review **FAIL** (`STD-R2-1`); source-trust domain review **FAIL** (`SRC-R2-1`, `SRC-R2-2`); sampling domain review **FAIL** (`SAMPLING-1`). The final `mise run review:verify-citations` exited 0 and accepted every cited location. There are no rejected citations. The two accepted timestamp claims share one root cause and were merged by Wave 2.

Wave 2 independently reproduced and retained three distinct blocking root causes in `findings-validation.md`:

- **FIND-TASK-001-3** (`SRC-R2-1`, CONFIRMED, task contract): `Listing::push` permits one canonical path twice, so a one-file PR can count as two files and satisfy the other-file condition. Correct uniqueness at the listing fold and fail the run for duplicates across or within pages.
- **FIND-TASK-001-4** (`SRC-R2-2`, `SAMPLING-1`, CONFIRMED, task contract): the shared timestamp validator checks shape, so impossible cutoff, update, and merge instants can enter discovery. Validate actual Gregorian date and clock values once at that owner.
- **FIND-TASK-001-5** (`STD-R2-1`, REVISED, repository standard): the four stateful IO workflow functions in `main.rs` are free functions, contrary to `AGENTS.md:27` and `architecture/agent-rules.md:10`. Keep `run` and `main` as entry points; place the four operations on the existing concrete command owner.

There are no validated notes or unresolved citation claims. The single self-contained remediation task is `TASK-001-R2-input-integrity-and-workflow-owner.md` in this directory. No spec revision is needed: all corrections stay in the candidate-owned private example.

## Verification limits

Locally observed exit 0: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::` (five tests), `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `git diff --check e665bc17ce91e855e4a744f3231b58c1cd9cd0ab a1342a5a3c154b7678cf59764dab1ceb0d70cc76`. The candidate evidence records `mise run gate` passing; this review did not rerun it. Credentialed live sampling and same-cutoff rerun comparison belong to TASK-003. The duplicate-path and timestamp defects were independently reproduced by traced reachable source paths; the candidate was not edited to add failing tests during review.
