# Citation-checked Wave 1 claims

Subject: `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41` → `e3d194de32f456f5136a214aaf7d8087483e7013`. Authority: `changes/active/context-relevance-eval/spec.md` revision 4, `tasks/TASK-002-counterfactual-trials.md`, `AGENTS.md`, `architecture/`.

Citation check rejected `TASK-A-1` because its second citation names absent symbol `Change::omit`. Drop the entire finding. All citations below passed.

| Source IDs / reviewer | One-sentence claim | Root cause and defect class | Cited candidate source | Siblings |
|---|---|---|---|---|
| `TASK-A-2` / task A; `TASK-B-1` / task B; `DOMAIN-BUDGET-1` / budget domain | D gives a scoped upstream hit more credit than C's formula allows. | `score` changes the global declaration-count divisor to one when a module is resolved; incorrect scoring denominator. | `trial.rs:747` `let count = if module.is_some() { 1 } else { declaring.len() };`; `trial.rs:749` `if excluded.contains(path) || module.is_some_and(|module| module != path) {`; `trial.rs:754` `_ => 1.0 / (df * count) as f64,` | Every D scoped upstream name in `score`; C and downstream D are separate paths. |
| `DOMAIN-SOURCE-1` / source domain | Same-line Python imports can be associated with the wrong module, making D scope a reference incorrectly. | `bindings` pairs import reference to the first containing-line import node instead of the actual binding; incorrect association of extractor facts. | `trial.rs:389` `.find(|node| {`; `trial.rs:391` `&& node.start_line <= reference.line`; `trial.rs:267` `let bindings = bindings(&paths, &fact.path, &nodes, &references);`; `trial.rs:612` `change.bindings = bindings(`; `trial.rs:776` `if !sources.contains(module) {` | Base candidate binding in `Corpus::new` and remaining changed head binding in `Change::new`; downstream and upstream D filters. |

All paths in the cited-source column are under `crates/bohmian-code-review-pipeline/examples/context_relevance/`. Wave 2 should reproduce or reject independently from source and applicable authority, not rely on Wave 1 prose.
