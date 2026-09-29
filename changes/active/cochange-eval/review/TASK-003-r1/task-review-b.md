# TASK-003 Wave 1 task review B

Subject: base `61d6df683b0cbe7aee34b819012e9c10c9f99802`, candidate `b75079fa2c32710924a230c224eda8b55aeb456e`; approved `changes/active/cochange-eval/spec.md` revision 2; original `tasks/TASK-003-reports.md`. Reviewed the complete six-file diff, the three JSON artifacts, example owners and tests, repository rules, and recorded verification. This review is independent of implementation and other reviewers.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-002: production cold, incremental, and zero review provider costs; evaluation timestamps separate | `main.rs:581-594`; all three reports' `requests` | Fake-provider `run_reads_each_input_once_and_reports_costs`; JSON inspection | PASS |
| REQ-003: cutoff, cold point, seed, sample commits, trial paths/ranks/hits, PR history/means, effects/intervals, decision, threats | `main.rs:537-610`; all three reports | `run_report_is_complete_and_source_free`; independent JSON check found 50 PRs per report and recomputed all PR means and paired effects | PASS |
| REQ-003: exact rerun command | `main.rs:574`; each report's `command` | Reproduced literal command syntax failure with `zsh -fc 'GITHUB_TOKEN=<read-only token> true'` (exit 1, `no such file or directory: read-only`) | FAIL |
| REQ-004: credentialed, opt-in, read-only command outside gate | `mise.toml` unchanged; `main.rs:489-500,618-633` | Task records successful credentialed live runs; gate definition excludes live command | PASS |
| PR-weighted paired effects, 10,000 seeded resamples, 95% intervals, F adoption rule | `report.rs:79-170`; `main.rs:567-609` | Owner tests and independent report arithmetic; task records `mise run test:pipeline` pass | PASS |
| AC-001: three checked-in source-free reports | `reports/{pydantic-ai,tokio,zod}.json` | JSON parsed; 50 PRs and 157/143/165 trials; no omitted patch text | PASS, subject to rerun-command failure above |
| AC-002 / INV-002: two same-cutoff runs reproduce reports | Fixed `REPOSITORIES` and seed; committed task evidence | Task records equal 267,627-byte outputs and matching SHA-256 on two successful runs; raw second-run file is not retained | PASS, with verification limit |
| AC-003: fake provider proves selection, history, E/F scoring, ties, unranked, inference | Earlier candidate owners `sample.rs`, `history.rs`, `trial.rs`; added `report.rs` tests | Task records `mise run test:pipeline` pass; inspected existing owner tests and new inference tests | PASS |
| INV-001 / AC-004: production crates, APIs, persisted formats, gate unchanged | Diff limited to example, task, and three reports | `git diff --name-status` and unchanged `mise.toml` | PASS |
| INV-003/004/005: no future history, source retention, writes, partial rate-limit report | Existing `History::for_trial`, `run` completes before `print`; report uses paths and counts | Existing fake provider negative tests; reports inspected | PASS |
| Non-goals: no A/B/D, packing outcome, sensitivity study, tuning, or production adoption | Complete diff and report schema | Diff and artifact inspection | PASS |

## Finding TASK-B-1 — INCORRECT

- **Defect class/root cause:** The report owner serializes a shell placeholder containing spaces and redirection metacharacters as the *exact rerun command*. The candidate-owned command string's shape permits a non-executable value; the previous executable `mise run live:context-relevance` was replaced.
- **Violated obligation:** Approved spec REQ-003 and TASK-003 scenario 2 require an exact rerun command in every report.
- **Owner and siblings:** `crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs:574` produces it; the assertion at `main.rs:918` enshrines it; `changes/active/cochange-eval/reports/{pydantic-ai,tokio,zod}.json:8` contain it.
- **Evidence and consequence:** A shell interprets `<read-only` as input redirection and errors before running `mise`. `zsh -fc 'GITHUB_TOKEN=<read-only token> true'` exits 1 with `no such file or directory: read-only`. A reader cannot rerun the recorded command literally, even with a token already in the environment.
- **Testable correction:** Record the executable opt-in command `mise run live:context-relevance` (with `GITHUB_TOKEN` supplied in the environment), correct the matching assertion and all three artifacts, and execute the recorded command under the required environment to establish it parses and starts the example.

Overall: **FAIL**.

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":574,"snippet":"        \"command\": \"GITHUB_TOKEN=<read-only token> mise run live:context-relevance\",","symbols":["record"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":918,"snippet":"            \"GITHUB_TOKEN=<read-only token> mise run live:context-relevance\"","symbols":["run_report_is_complete_and_source_free"]},{"id":"TASK-B-1","path":"changes/active/cochange-eval/reports/pydantic-ai.json","line":8,"snippet":"  \"command\": \"GITHUB_TOKEN=<read-only token> mise run live:context-relevance\",","symbols":[]},{"id":"TASK-B-1","path":"changes/active/cochange-eval/reports/tokio.json","line":8,"snippet":"  \"command\": \"GITHUB_TOKEN=<read-only token> mise run live:context-relevance\",","symbols":[]},{"id":"TASK-B-1","path":"changes/active/cochange-eval/reports/zod.json","line":8,"snippet":"  \"command\": \"GITHUB_TOKEN=<read-only token> mise run live:context-relevance\",","symbols":[]}]
```
