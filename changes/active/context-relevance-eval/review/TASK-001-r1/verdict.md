# TASK-001 review verdict — round 1

**Verdict: FIX_REQUIRED.** Immutable base `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab`; candidate `c626450649edfdbea105011ab5293267df49ed74`. Authority: approved revision 4 of `changes/active/context-relevance-eval/spec.md` and `tasks/TASK-001-pinned-sample.md`. The complete five-file base-to-candidate diff was reviewed; reviewed source stayed unchanged.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001, REQ-007: named repositories, credentialed read-only opt-in command | `main.rs` repository list, GitHub client and environment; `mise.toml` live lane outside `gate` | Source and task-graph inspection | PASS |
| REQ-002: 50 PRs before cutoff in merge-time and PR-number order | `Discovery::offer`, `settled`, and `select` | Focused sampler test: 4 passed | PASS |
| REQ-002: one complete file listing; incomplete listings fail | `main.rs:95-110`; `Screen::new` rejects `truncated` | Source-path reproduction in `FIND-TASK-001-1`; existing tests cover only advertised truncation | FAIL |
| REQ-002–003: 2–15 eligible paths, modified base declaration, another changed-line file, patch omissions | `Screen::new`, `plausible`, `qualifies`; indexed snapshot facts | Focused sampler test: 4 passed | PASS |
| INV-001, AC-005: existing production contracts unchanged; no `PrContext`, second listing, or replay | Diff limited to example, example registration, opt-in lane, and task evidence | Complete diff inspection | PASS |
| INV-003: deterministic supplied-input selection | Cutoff and ordered discovery in `sample.rs` | Focused ordering test; live rerun belongs to TASK-003 | PASS within TASK-001 scope |
| INV-004–005: source-free report, in-memory source, read-only access | `main.rs` output and memory stores | Source inspection | PASS |
| Repository Rust-item documentation rule | `sample.rs` inline tests and fixture | `FIND-TASK-001-2`; written rule at `AGENTS.md:32` | FAIL |
| Task non-goals: no trial blobs, new production API, persisted-format or gate change | Complete diff | Complete diff inspection | PASS |

## Review results and findings

Wave 1: task review A **FAIL** (`TASK-A-1`); task review B **FAIL** (`TASK-B-1`); standards review **FAIL** (`STANDARDS-1`); source-trust domain review **FAIL** (`SRC-1`); sampling domain review **FAIL** (`DOMAIN-SAMPLE-1`). The citation check exited 0. It accepted `TASK-A-1` and `STANDARDS-1`, but printed `REJECT` for `TASK-B-1`, `SRC-1`, and `DOMAIN-SAMPLE-1` because their cited symbol `Discovery::offer` is absent verbatim. Those three findings were dropped in full. See `citations.txt`.

Independent Wave 2 validation confirmed two distinct blocking root causes in `findings-validation.md`:

- **FIND-TASK-001-1** (task contract): the listing's completeness flag can be false after a full 3,000-file response with no next link. GitHub's [PR-files endpoint](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests-files) caps responses at 3,000 files. `Screen::new` can then treat an incomplete listing as complete. Correct the completeness decision at `listing`; prove both the capped and shorter terminal-page cases.
- **FIND-TASK-001-2** (repository standard): the new `sample.rs` test module and `PATCH` fixture omit rustdoc required by `AGENTS.md:32`. Document both items and verify the final diff.

No validated notes or prior `FIND-*` IDs apply. The self-contained correction is `TASK-001-R1-listing-completeness-and-rustdoc.md` in this directory.

## Verification limits

The focused no-network sampler command passed all four tests, and `mise run test:pipeline` passed locally. `git diff --check` passed. The candidate's task evidence records passing format, lint, and gate commands; this review did not repeat those broader lanes. A credentialed GitHub run and second-run comparison belong to TASK-003. Concurrent updates during GitHub offset pagination remain an input-stability limit under the approved one-listing, no-reconciliation design.
