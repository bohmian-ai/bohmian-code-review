# Wave 1 task review A

Subject: `322a2b9e0984062625d54274798a9c32cb01142d..100249c3031d645ce8db9148b4b6fa43e71481d1`. Authority: `changes/reviews/ranker-pipeline/intent.md` decisions 1–8, `architecture/references/domain/pipeline-consumer-ergonomics.md`, `AGENTS.md`, and `architecture/agent-rules.md`. No task file exists by maintainer direction. Reviewed the cumulative diff and its public consumers. The orchestrator reports `mise run gate` passed at candidate HEAD and `git diff --check` passed; those checks do not settle contract gaps below.

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| 1. Public IO-free `Ranker` and S/C/E/F semantics | `select/src/rank.rs` defines all four through the index and resolver; facade re-exports them | Ranker owner tests and public symbolic journey in passing gate | PASS |
| 2. F default, C order with empty history | `DEFAULT_RANKER` and `Fused::rank`; `rank_files` uses default | Empty-history fusion test in passing gate | PASS |
| 3. Feature-gated reusable eval taking a sample and named rankers, with existing report and thin live binary | Eval feature and moved modules exist; `trial::evaluate` hard-wires global `ARMS` | Eval lane in passing gate exercises only fixed S/C/E/F | FAIL |
| 4. Source-free distinct reference and candidate names persisted; snapshot v4 | `IndexedFileFacts.references`, index export/load, snapshot version 4 | Index, snapshot, journey lanes in passing gate | PASS |
| 5. Fractional selected scores; context v2 | `SelectedSpan.score: f64`, context version 2 | Context and pipeline lanes in passing gate | PASS |
| 6. Co-change-only candidates pack declarations by location within budgets | `pack_candidates_into` and `candidate_spans` use indexed declarations when reasons are empty | Select and public journey lanes in passing gate | PASS |
| 7. Bounded complete-or-fail history in both passes, persisted source-free; F always ranks with time-filtered history | `CoChangeHistory::read`, snapshot/update, `for_review`, `pull_request::rank` | History tests and pipeline journey in passing gate; bound test demonstrates success with missing history | FAIL |
| 8. One consumer `ReviewPipeline` with one storage/store, shared effective file ceiling, per-call client, cold build only without pointer, refresh/read, public stages | `review.rs`, public re-exports and README; stage facades remain | Public pipeline journey in passing gate | PASS |
| Repository constraints: no source persistence, store clock, crate ownership, Rust docs/errors, public journeys | Source-free facts and history; pipeline alone owns provider/storage; contexts retain selected text | Passing gate includes lint, formatting and journeys | PASS |

## Proposed findings

### TASK-A-1 — MISSING: eval arm list is fixed inside the reusable harness

**Root cause and defect class:** `trial::evaluate` has no named-ranker parameter and `trial` reads the global `ARMS` constant. The public function shape permits only the four built-in implementations. **Siblings:** `live::record` also iterates `ARMS` for reported effects, and `live::run` has no ranker parameter. These are the same fixed-arm boundary, not separate findings.

**Obligation:** Intent decision 3 requires a reusable hit@10 harness that *takes* a repository sample and named `&dyn Ranker`s. **Evidence:** `trial.rs:297–303` accepts only PR/index/listing/head/history inputs; `trial.rs:273–276` evaluates `ARMS`; `live.rs:530–535` reports effects only for `ARMS`. **Consequence:** A caller cannot evaluate a new `Ranker` or even a different named set without editing and rebuilding this crate; the live command's fixed arms have become the harness contract. **Testable correction:** Take a named `&dyn Ranker` slice at the reusable trial and report boundary, pass the fixed S/C/E/F set from the thin live command, and test a caller-supplied ranker ID through hit@10 and the report without changing the built-ins.

### TASK-A-2 — INCORRECT: discovery cap silently converts incomplete history into success

**Root cause and defect class:** `CoChangeHistory::read` treats exhaustion of its discovery page ceiling as a successful shorter history, even when it cannot prove the previous snapshot's span has been reached. The decision is produced at the cap branch, before snapshot persistence. **Siblings:** Both cold build and update call this same reader through `ColdBuild::history`; no other history discovery owner exists. The existing `empty_and_page_bounded_history_is_valid` test expressly accepts losing the prior history.

**Obligation:** Intent decision 7 requires bounded, complete-or-fail co-change history and update extension with PRs merged since the previous snapshot. **Evidence:** `history.rs:385–386` ends discovery at ten pages; `history.rs:413–417` discards `prior` when `joined` is false and returns `Ok(history)`; the test at `history.rs:704–707` expects an empty successful result despite a nonempty prior. **Consequence:** In a busy repository whose ten most recently updated closed-PR pages do not establish the merge-time cutoff, refresh can publish a snapshot with missing history and F silently falls back toward C or misorders candidates. **Testable correction:** When the cap is reached before proving the cold depth or the prior cutoff, return a typed history error and publish no replacement snapshot; test both cold and update paths with a discovery sequence that exceeds the cap.

Overall: **FAIL**. These findings are limited to named acceptance obligations; no optional refactor is proposed.

```citations
[{"id":"TASK-A-1","path":"crates/bohmian-code-review-eval/src/trial.rs","line":273,"snippet":"        arms: ARMS","symbols":["trial","ARMS","evaluate"]},{"id":"TASK-A-1","path":"crates/bohmian-code-review-eval/src/trial.rs","line":297,"snippet":"pub fn evaluate(","symbols":["evaluate"]},{"id":"TASK-A-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":530,"snippet":"    let effects: BTreeMap<String, Value> = ARMS","symbols":["record","ARMS"]},{"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":385,"snippet":"            if pages == MAX_DISCOVERY_PAGES {","symbols":["CoChangeHistory::read","MAX_DISCOVERY_PAGES"]},{"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":413,"snippet":"        if let Some(prior) = prior.filter(|_| joined) {","symbols":["CoChangeHistory::read"]},{"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":417,"snippet":"        Ok(history)","symbols":["CoChangeHistory::read"]},{"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/history.rs","line":704,"snippet":"        let bounded = read(&client, \"2026-01-31T00:00:00Z\", Some(&prior))","symbols":["empty_and_page_bounded_history_is_valid"]}]
```
