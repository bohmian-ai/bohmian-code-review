# TASK-002 Wave 2 findings validation

Subject: `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41` → `e3d194de32f456f5136a214aaf7d8087483e7013`; approved `changes/active/context-relevance-eval/spec.md` revision 4. The complete changed surface is the opt-in `context_relevance` example, its task evidence, and one `mise.toml` description. No prior `FIND-*` IDs exist for this task.

## Blocking findings

### FIND-TASK-002-1 — CONFIRMED

- **Wave 1 IDs:** `TASK-A-2`, `TASK-B-1`, `DOMAIN-BUDGET-1`.
- **Gate/classification:** Task contract; INCORRECT (`REQ-004`, variant C/D ranking definition at `spec.md:23-34`, and TASK-002 Scenario 2).
- **Root cause and defect class:** `score` replaces the global `declaring_file_count(name)` with one for an imported upstream reference (`trial.rs:747`). This is an incorrect scoring denominator. The owner's shared `score` function is the right correction boundary; no public surface redesign is needed.
- **Siblings:** Every D scoped upstream name uses this branch. C and unscoped D retain the prescribed denominator; downstream D uses its separate `1 / df` rule.
- **Evidence/reproduction:** The public `evaluate` path (`trial.rs:1022-1044`) calls `trial` (`959-1007`), which calls `score(..., Rule::Scoped)` (`984`). A unique imported module is found at `741-745`; the chosen file alone passes the filter at `748-751`, then its credit becomes `1 / (df × 1)` at `754`. The approved formula at `spec.md:30` still divides upstream credit by the number of *all* declaring base files; `spec.md:32` only scopes which file receives it. With two declaring files and `df = 6`, C credits each `1/12`, while D credits the selected module `1/6` although the contract permits only `1/12`. Other names can therefore reverse D's rank and sent outcome. The current Python/Rust fixture already has two `helper` or `read_all` declarers (`trial.rs:1226-1231`, `1281-1300`), but its D assertions do not check that exact credit.
- **Smallest safe correction:** Keep the existing eligible-module filter and use the complete `declaring.len()` for the C/D upstream weight. Do not alter B's one-point rule, df ceiling, unique-module qualification, or downstream credits. No new abstraction or dependency is needed.
- **Focused closure proof:** A no-network trial with two base declarers and one uniquely imported module asserts D retains only that module but gives it exactly `1 / (df × 2)`; add a competing candidate credit that makes an incorrect `1 / df` change the ordering. Check C, D, and sent results through `evaluate`.

### FIND-TASK-002-2 — CONFIRMED

- **Wave 1 ID:** `DOMAIN-SOURCE-1`.
- **Gate/classification:** Task contract; INCORRECT (`spec.md:32` D's explicit import binding and unresolved/ambiguous fallback, TASK-002 Scenario 2).
- **Root cause and defect class:** Python `bindings` pairs an import reference with the first import node whose *line* contains it (`trial.rs:386-400`), rather than the import statement containing its column. This is incorrect association of public extraction facts. `Node` carries start/end columns and `RawReference` carries a column (`crates/bohmian-code-review-core/src/node.rs:242-248`, `reference.rs:26-28`), so the existing facts can establish containment without parsing source again.
- **Siblings:** `Corpus::new` uses `bindings` for every indexed base candidate (`trial.rs:267`), then D downstream filtering reads it (`772-782`). `Change::new` uses it for every remaining changed head (`612-617`), then D upstream filtering reads it (`740-751`). Both are produced by the same `bindings` owner.
- **Evidence/reproduction:** The public `evaluate` call builds both producers (`trial.rs:1030-1035`) and calls D scoring (`984`). The Python extractor emits a separate import node and import references for each `from` statement (`crates/bohmian-code-review-extract/src/python.rs:624-675`) and records coordinates. Legal Python `from pkg.a import x; from pkg.b import y` puts both statements on one line. For the `y` reference, `bindings` finds the first node (`pkg.a`) because it compares only line numbers and `node.name != "y"`; `python_module` then resolves `pkg.a` as `y`'s module. D can suppress the correct `pkg.b` upstream declaration or wrongly accept a downstream reference. With repeated binding names on one line, it can also miss the required ambiguous fallback. This violates the explicit-binding condition even if B/C remain correct.
- **Smallest safe correction:** Associate each Python import reference only with its actual import node using the existing line and column spans, then preserve `None` for ambiguous, wildcard, relative, or unresolved bindings. Keep the same `bindings` owner for both base and head producers; do not move or duplicate extraction, source ownership, or scoring.
- **Focused closure proof:** A no-network `evaluate` fixture with two same-line `from` imports and distinct indexed modules checks that D's upstream and downstream results follow each actual binding. Include same-line repeated binding names and assert the ambiguous case falls back to C. Keep existing separate-line and Rust assertions green.

## Rejected claims and notes

- `TASK-A-1` — REJECTED by the mandatory citation check: its cited `Change::omit` symbol did not validate against the candidate. Omitted from the blocking ledger.
- No nonblocking notes. The two retained findings are directly required by the approved scoring and scoping contract; neither is a speculative hardening request.

## Validation limits

Reproduction is by the complete source path from public `evaluate` and the extractor to `score`, with a hand-computed scoring example and legal same-line Python syntax. No reviewed source was edited and no new executable test was added during this review. The closure proofs above are the smallest tests the remediation should add.
