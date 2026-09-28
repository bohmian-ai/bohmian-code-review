# Source trust, extraction provenance, and import scoping review

Subject: `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41` → `e3d194de32f456f5136a214aaf7d8087483e7013`; approved `changes/active/context-relevance-eval/spec.md` revision 4; `TASK-002-counterfactual-trials.md`.

## Boundary and coverage

Reviewed the complete candidate diff, especially `examples/context_relevance/main.rs` and `trial.rs`; followed base snapshot publication/read-back and `Snapshot::into_context`, `GitHubClient::blob`, `SymbolicContext::upsert`/`export`, and the Python/Rust public extractor's import facts. Applicable obligations: REQ-004–005, INV-001–005, AC-003–005 and the task's Scenario 2. No CodeGraph index exists.

The base source path is sound in this scope: `Api::trials` selects every `FileOutcome::Indexed`, fetches each snapshot blob by SHA through `GitHubClient::blob` (which verifies the Git blob hash), and passes every indexed path to `Corpus::new`. Missing source returns `TrialError::MissingSource`; extraction failure returns `TrialError::Symbolic`. `Snapshot::into_context` loads the same indexed facts with extraction-version validation. Head blobs are also fetched by SHA from the PR listing. Neither the reviewed code nor the traced facade persists source or changes production extraction/ranking.

## Proposed finding

### DOMAIN-SOURCE-1 — `INCORRECT`: Python import references are paired with modules by line, not by import statement

**Root cause and defect class.** `bindings` finds the first import node whose line range contains an `imports` reference and whose name differs from the reference name. The public Python extractor emits one import node and one reference per statement/binding. Two statements on one physical line therefore share the same `reference.line`, but the evaluator has discarded the originating node relationship. The candidate-owned `Bindings` map permits an apparently resolved module even when the reference belongs to a different import statement.

**Reachable case.** In a base candidate, `from pkg.a import first; from pkg.b import helper` on one line, followed by `helper()`, yields an import node for `pkg.a` and one for `pkg.b`. The `helper` import reference is paired with the first node (`pkg.a`) by the `.find` predicate. If both indexed modules declare `helper`, D scopes the candidate's downstream `helper` reference to `pkg.a`; the changed file `pkg.b` then fails `sources.contains(module)` and the candidate is wrongly dropped. A–C retain it. The same mistake applies to remaining changed heads on D's upstream path. It also hides ambiguity for two same-line imports of one name from different modules. The root-cause sibling sites are the two calls to `bindings` in `Corpus::new` (base candidate) and `Change::new` (head changed file), with scoping applied in both halves of `score`.

**Violated obligation.** Spec revision 4 D rule: scope only when an explicit import binding and repository-relative module path identify exactly one indexed base module; otherwise retain C's unscoped lookup. Task Scenario 2 requires that fallback. This is a deterministic wrong rank/score for valid Python input.

**Correction and closure proof.** Preserve the actual binding-to-module relationship from public extraction facts when it is unambiguous; where those facts cannot distinguish same-line statements, leave the name unscoped. Exercise both an upstream and a downstream same-line import case and show D either chooses the correct module or equals C for ambiguity, while the existing separate-line explicit-import behavior remains.

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::tests::variants_score_direction_rarity_and_imports -- --exact` passed (1 test). It covers separate-line explicit Python and Rust imports and an unresolved relative Python import, not same-line Python statements. The proposed finding is traced through source and public extractor behavior; no new executable reproduction was added to the immutable candidate. No live GitHub run was performed here. Other review domains own ranking, budget, reporting, and sample qualification.

**Overall result: FAIL** for this domain, subject to independent citation and finding validation.

```citations
[{"id":"DOMAIN-SOURCE-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":389,"snippet":"                .find(|node| {","symbols":["bindings"]},{"id":"DOMAIN-SOURCE-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":391,"snippet":"                        && node.start_line <= reference.line","symbols":["bindings"]},{"id":"DOMAIN-SOURCE-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":267,"snippet":"            let bindings = bindings(&paths, &fact.path, &nodes, &references);","symbols":["Corpus::new","bindings"]},{"id":"DOMAIN-SOURCE-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":612,"snippet":"        change.bindings = bindings(","symbols":["Change::new","bindings"]},{"id":"DOMAIN-SOURCE-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":776,"snippet":"                        if !sources.contains(module) {","symbols":["score"]}]
```
