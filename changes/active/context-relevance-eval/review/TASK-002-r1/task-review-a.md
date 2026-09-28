# TASK-002 independent implementation review A

Subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `e3d194de32f456f5136a214aaf7d8087483e7013`; approved `spec.md` revision 4 and original `TASK-002-counterfactual-trials.md`. Reviewed the full five-file diff, evaluator flow, facade ranking and packing, patch parser, and extractor import facts. No other review was consulted.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004, AC-003: one trial per modified declared file; withhold one, exclude remaining paths, keep base candidate | `trial.rs:959-1044` | `withholding_keeps_base_file_rankable_and_a_is_production`; focused trial lane passed 3/3 | PASS |
| REQ-004, INV-002, AC-004: A uses public production rank and separate production pack | `trial.rs:823-859,974-1004` | Same focused test compares actual facade ranking | PASS |
| REQ-004, REQ-005: B directional lookup, full indexed base reference corpus, missing source failure | `trial.rs:225-277,710-815,1029-1044` | `variants_score_direction_rarity_and_imports` and missing-base assertion | PASS |
| REQ-005: C exact frequency ceiling, credits, deterministic ties | `trial.rs:296-312,730-813` | Variant fixture asserts a C score and ranks | PASS |
| REQ-005: D adds import scoping to C's fixed rarity score | `trial.rs:736-755,772-791` | Variant fixture checks D ranks but does not assert D credits | FAIL: TASK-A-2 |
| REQ-005: whole/snippet counts, common 100,000-token pack, distinct production observation, diff/prompt accounting | `trial.rs:24-34,874-1006`; `main.rs:378-397` | `packing_records_whole_spans_omitted_and_unranked` | PASS |
| REQ-005, INV-003: missing/malformed patch omissions; inconsistent analyzable head fails rather than silently shrinking a trial | `trial.rs:551-620` | Tests cover malformed patch, not inconsistent head | FAIL: TASK-A-1 |
| REQ-006, INV-004: source-free trials, all/non-test summaries, miss paths and PRs | `trial.rs:85-149`; `report.rs:15-113`; `main.rs:378-397` | Report test and source-text serialization assertion | PASS |
| INV-001, INV-005, AC-005: opt-in read-only evaluator, verified blobs, unchanged production crates/gate, no replay or `PrContext` | Full diff touches evaluator, task evidence, and mise lane description only; `main.rs:220-299` uses `GitHubClient::blob` | Diff inspection; focused trial lane | PASS |
| Task no-network test journey: excluded, unranked, whole, spans-only, omitted, malformed patch | `trial.rs:1110-1429` | Focused trial lane: 3 passed | PASS |

Focused check: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::` exited 0, 3 passed. Candidate task evidence records broader pipeline, journey, format, lint, diff, and gate checks; I did not rerun those broad lanes.

## Proposed findings

### TASK-A-1 — INCORRECT: analyzable head failures become silent omissions

- **Root cause / defect class:** `Change::new` folds failures of the public rank/extraction boundary into the same omission path reserved for missing or malformed patches. The owner's `omission: Option<String>` shape conflates allowed patch omissions with source/range inconsistencies that must stop a run.
- **Owner and siblings:** `trial.rs:584-590` is the sole analysis-failure sink. `Change::new` processes every listed source file in `evaluate` at `trial.rs:1032-1035`; the resulting `head: None` silently removes it from A at `trial.rs:828-838` and its names from B–D at `trial.rs:680-697`, while `trial.rs:990-997` reports an ordinary omission. The `main.rs:242-267` fetch path supplies hash-verified blobs but cannot establish that a changed-line patch fits the head blob.
- **Obligation:** Spec REQ-002 permits *missing or malformed per-file patches* as omissions and says provider inconsistency fails the run; REQ-004/INV-002 require the remaining changed files' names to be ranked. Task scenario 3 requests malformed-patch omission, not a blanket analysis-failure omission.
- **Observable consequence:** A valid patch whose added line is beyond its fetched head, or a non-UTF-8 head, produces `Ok(trials)` with that changed file excluded from every variant's query and candidate set, biasing ranks and sent outcomes without failing the run. This is reachable through the live `Api::trials -> evaluate -> Change::new` path.
- **Testable correction:** Keep patch parsing failures as recorded omissions; propagate rank/extraction failure as a run-level typed error. Focused proof supplies a valid hunk and an inconsistent head and expects `evaluate` to fail, while a malformed patch remains an omission.

### TASK-A-2 — INCORRECT: D changes C's ambiguity credit when a binding scopes lookup

- **Root cause / defect class:** `score` uses the number of retained modules after D import scoping as `declaring_file_count(name)`, so it changes the fixed C rarity formula as well as lookup. This is an incorrect denominator in the shared B–D scoring owner; no separate scoring site has this pattern.
- **Owner and siblings:** `trial.rs:747` chooses `1` whenever an explicit import resolves, then `trial.rs:754` uses that count in upstream credit. Every scoped upstream hit is affected. Downstream credit at `trial.rs:783-786` retains C's formula.
- **Obligation:** Spec's C formula is `1 / (df(name) × declaring_file_count(name))`; D is C with only the fixed explicit-import scoping rule. `df` is correctly computed from the complete base corpus at `trial.rs:732`; the declaring-file count must likewise describe that corpus, even when D retains one target.
- **Observable consequence:** If two base files declare `helper` and a changed file imports it from one, C credits either declaration `1/(df×2)`, while D credits the selected declaration `1/df`. D's score doubles and can change its rank relative to another candidate for a reason beyond import scoping. The current fixture includes ambiguous `helper` but asserts only D rank and scoped counts, not D score.
- **Testable correction:** Reuse the full declaration count already available in `declaring.len()` when computing the rarity credit; use the resolved module only to filter eligible paths. A focused ambiguous-import fixture should assert D's selected candidate keeps C's per-name credit and that the other declaration is excluded.

Overall result: **FAIL**. Both findings are bounded corrections within the evaluator; no approved spec change is needed.

```citations
[{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":584,"snippet":"        let analyzed = corpus","symbols":["Change::new"]},{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":590,"snippet":"            Err(error) => return Ok(change.omit(format!(\"analysis failed: {error}\"))),","symbols":["Change::new","Change::omit"]},{"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":747,"snippet":"            let count = if module.is_some() { 1 } else { declaring.len() };","symbols":["score"]},{"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":754,"snippet":"                    _ => 1.0 / (df * count) as f64,","symbols":["score"]}]
```
