# Domain review: scoring, packing, and report integrity

Subject: base `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41`, candidate `e3d194de32f456f5136a214aaf7d8087483e7013`; approved `changes/active/context-relevance-eval/spec.md` revision 4 and `tasks/TASK-002-counterfactual-trials.md`.

## Coverage

Reviewed the complete base-to-candidate diff for `trial.rs`, `report.rs`, and the report assembly in `main.rs`. Traced the full indexed corpus through `Corpus::new`, the A facade path, B–D query/scoring, common whole/span packing, and per-trial and aggregate serialization. Authorities: spec ranking-variant rules and REQ-004–REQ-006, INV-002–INV-004, TASK-002 scenarios 1–3, `AGENTS.md`, and `architecture/agent-rules.md`. The declaration index exports every indexed file, including files with no declarations, so the corpus's use of `index.export()` does not itself narrow the candidate universe.

The B credit map deduplicates by `(file, name)` and retains both directions. C computes document frequency from the union of declaring and referencing files, applies the ceiling, and sorts by descending score then path. Whole and span packing share a 100,000-token allowance across ranked candidates. Report summaries cover all and non-test trials, with rank hit rates, sent shares, score arrays, and PR/path misses. The report assembly serializes paths, names, scores, and counts; no source blob field enters `Trial` or the JSON record.

## Finding DOMAIN-BUDGET-1 — D changes C's ambiguity weight

**Classification:** INCORRECT. **Defect class:** an import scope changes the scoring denominator as well as filtering the lookup. **Owner shape:** `score` mixes candidate filtering and the fixed C weight calculation in its upstream loop, permitting a scope decision to alter both.

The spec makes D equal C plus explicit-import scoping. C's upstream credit is `1 / (df(name) × declaring_file_count(name))`. At `trial.rs:747`, `score` substitutes `1` for the full declaring-file count whenever D resolves an import. The next lines filter to the imported module *and* award `1 / df`, rather than `1 / (df × declaring_file_count)`. This occurs for every scoped upstream lookup with two or more base declarers; the Rust `read_all` fixture has precisely that shape (`io.rs` and `alt.rs`), while its assertions check D's rank but not its score. The same loop is the sole sibling site for all upstream D matches; C and downstream D retain the declared formula.

**Consequence:** D's score and score distribution are inflated for scoped names, and adding an import can alter ranks through an unapproved scoring change. This compromises the neighbour comparison that is intended to isolate import scoping. **Testable correction:** retain the full base declaring-file count when assigning upstream credit, while continuing to restrict D's candidate paths to the uniquely imported module; add a focused score assertion for an imported name declared by two base files and verify the resulting D order and report scores.

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --features testing --example context_relevance variants_score_direction_rarity_and_imports` passed (one test); that test does not assert the scoped score. The initial command with `--exact` selected zero tests and is not evidence. The finding is reproduced by the cited source path and the approved formula; no reviewed source was changed. Live GitHub runs and checked-in reports belong to TASK-003 and were not available for this boundary review.

Overall result: **FAIL**.

```citations
[{"id":"DOMAIN-BUDGET-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":747,"snippet":"            let count = if module.is_some() { 1 } else { declaring.len() };","symbols":["score","Rule::Scoped"]},{"id":"DOMAIN-BUDGET-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":749,"snippet":"                if excluded.contains(path) || module.is_some_and(|module| module != path) {","symbols":["score","Rule::Scoped"]},{"id":"DOMAIN-BUDGET-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":754,"snippet":"                    _ => 1.0 / (df * count) as f64,","symbols":["score","Rule::Scoped"]}]
```
