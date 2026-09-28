---
id: TASK-002
kind: implementation
status: review
spec: SPEC-context-relevance-eval
spec_revision: 4
requirements: [REQ-004, REQ-005, REQ-006, INV-001, INV-002, INV-003, INV-004, INV-005, AC-003, AC-004, AC-005]
depends_on: [TASK-001]
parent_task:
remediates: []
---

# Leave-one-out trials and measurements

## Outcome and Value

Every withholdable file yields comparable A–D ranks and sent outcomes under one experimental packing policy, plus a separate faithful production packing observation for A. One source-free JSON report per repository contains per-trial evidence and the required aggregates.

## Owners, Scope, Consumers, and Prohibited Changes

The opt-in pipeline evaluation surface consumes TASK-001's PR metadata, one file listing per PR, and complete base snapshot. Every `FileOutcome::Indexed` file in that snapshot is in the candidate universe for A–D. Fetch its base blob through the existing hash-verifying GitHub client when B–D need its references; a missing blob fails the run. Use the public symbolic facade's extraction output, including its import facts. Production extraction, indexing, ranking, packing, snapshot, and PR-context code remain unchanged. Experimental reverse reference lookup, rarity weighting, import scoping, packing, and reporting live only in the evaluator. Keep fetched source transient and report fields source-free. Do not run `PrContext` or add a provider replay layer.

## Approach

1. Load A's declaration index from TASK-001's complete base snapshot; obtain hash-verified base blobs for every indexed candidate to build B–D's reference facts, and head blobs for remaining changed files.
2. Form one counterfactual changed set per withholdable file and reuse the same candidate universe for all variants; preserve deterministic ties and explicit ambiguity.
3. Measure whole-file and matched-span costs with the approved token estimate and common 100,000-token candidate allowance; separately record A's unmodified production packing outcome.
4. Emit trial records, omissions, and aggregate summaries with deterministic ordering and no source text.

## Ordered Implementation Scenarios

### Scenario 1 — Withholding and faithful A

**Behavior.** A trial removes exactly the withheld path from changed inputs, keeps its base declaration rankable, excludes all remaining changed paths, and gives the same A ranking as the public facade over the complete base snapshot (REQ-004, INV-002, AC-003–AC-004).

**RED.** Add a focused no-network trial test against the real symbolic facade; run its exact command and observe a missing or incorrect counterfactual result.

**GREEN.** Produce the trial from TASK-001 inputs with production ranking; rerun the focused test.

**REFACTOR.** Share prepared inputs among variants without copying production ranking logic.

### Scenario 2 — Directional, rarity, and import variants

**Behavior.** B applies the approved upstream and downstream mapping over references from every indexed base candidate without double-counting a name; C applies the document-frequency ceiling, weights, ambiguity split, and stable ties; D scopes only from public extracted import facts when a unique base module is identified, and otherwise falls back to C. Removed or renamed declarations contribute base-side names. A missing indexed base blob fails the run instead of shrinking B–D's corpus (REQ-004–REQ-005, AC-003).

**RED.** Add a focused no-network test covering both languages, a reference in a base candidate away from the changed files, an explicit and unresolved import, and a missing base blob; run its exact command and observe the missing variant results or corpus failure.

**GREEN.** Add the smallest evaluation-only lookup and scoring needed; rerun both scenario tests.

**REFACTOR.** Consolidate only repeated evaluation transformations while preserving directional reasons and deterministic ranks.

### Scenario 3 — Packing, omissions, and report

**Behavior.** Every variant records whole-file and snippet counts, unranked/whole/spans-only/omitted sent outcomes under the common 100,000-token allowance, while A also records production packing under that allowance. Remaining diff patches and the 4,096-token prompt reserve are counted separately; the withheld patch is excluded. The source-free report includes every requested aggregate and miss (REQ-005–REQ-006, INV-002–INV-004, AC-003).

**RED.** Add a focused no-network test with a whole file that fits, a file that fits only as spans, an unranked file, a budget omission, and a malformed patch; run its exact command and observe the missing measurements or report fields.

**GREEN.** Implement the common experimental packing and report aggregation, then rerun all scenario tests.

**REFACTOR.** Keep the production packer call distinct from experimental packing and remove duplicated report calculations.

## Acceptance Criteria

- A ranking matches production on identical counterfactual inputs; its production packing observation is distinct from the common experimental sent result.
- B–D implement the approved lookup, scoring, fallback, and deterministic ordering using reference facts from the full indexed base candidate set. Missing required base source fails the run.
- Every trial and aggregate required by REQ-005–REQ-006 is present; no source text is serialized.
- The no-network trial proves withheld, excluded, unranked, whole, spans-only, omitted, and malformed-patch outcomes.

## Size Expectation

Use one evaluator over the supplied PR listing and verified blobs. Keep tests centered on the required leave-one-out journey and scoring rules; do not build a second GitHub ingestion or PR-context pipeline to test the evaluator. If that becomes necessary, stop and revisit the task boundary.

## Expected Write Set and Consumer Closure

Likely: the TASK-001 evaluation surface and its no-network tests. TASK-003 consumes the report format and live invocation. No symbolic or production pipeline implementation file should need a behavior change.

## Verification and Evidence

For each new test, record the actual focused `mise exec -- cargo test --locked` command and RED/GREEN result. Run `mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `git diff --check` sequentially. Inspect report serialization for source text and inspect the diff for production behavior, PR-context calls, replay code, or gate changes.

## Material Stop Conditions

Stop if the approved C/D semantics cannot be expressed from existing public extraction output, if the complete base reference corpus cannot be obtained through the outer pipeline's verified source boundary, if baseline fidelity requires changing production behavior, or if the report cannot remain source-free.

## Authority Links

[Approved spec](../spec.md); [contributor rules](../../../../AGENTS.md); [design](../../../../architecture/bohmian-code-review-design.md); [symbolic context](../../../../architecture/references/domain/symbolic-context.md).

## Implementation Evidence

Focused command: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::` (and `report::`). RED: 65 compile errors, `evaluate`, `rank_a`, `excluded`, `estimate_tokens` not found. GREEN: 3 passed (`withholding_keeps_base_file_rankable_and_a_is_production`, `variants_score_direction_rarity_and_imports`, `packing_records_whole_spans_omitted_and_unranked`); `report::tests::summaries_and_misses_follow_outcomes` passed. Two hand-computed expectations were wrong in the first draft (trial order is listing order; B ranks `pkg/models.py` fourth behind `loose` score 2 and the `far`/`legacy` path ties) and were corrected from the scoring rule, not from output.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| A ranking matches production on identical counterfactual inputs; production packing distinct from common experimental sent result | `trial.rs` `rank_a` (facade `rank_files` over the snapshot-loaded index, remaining changed paths excluded), `trial` (`build_candidate_context` with production `max_candidate_files` and 100,000 tokens → `production_selected`), `outcome` (common policy) | `withholding_keeps_base_file_rankable_and_a_is_production` asserts `rank_a` equals a fresh facade `rank_files`; `packing_…` asserts `production_selected` separately from `sent` | PASS |
| B–D implement approved lookup, scoring, fallback, deterministic ordering over full indexed base references; missing base source fails | `Corpus::new` extracts every indexed base blob; `score` with `Rule::{Directional,Rare,Scoped}`; `bindings`/`python_module`/`rust_module` from public extraction import facts; `removed_lines` for base-side names; `TrialError::MissingSource` | `variants_score_direction_rarity_and_imports`: Python and Rust, far-away base reference, explicit, relative (unresolved), and aliased-module imports, removed `gone` declaration, exact B/C/D ranks, C score 1/6, scoped/unscoped 3/2, missing `tokio/src/alt.rs` → `MissingSource` | PASS |
| Every REQ-005–REQ-006 trial field and aggregate present; no source text serialized | `Trial`/`Outcome` (rank or `"unranked"`, score, names, direction, whole/snippet tokens, sent, diff tokens, omissions, production), `report.rs` (hit rate at 1/5/10/50, unranked, whole, spans-only, hit/miss score distributions, all and non-test, misses), `main.rs` report header (cutoff, command, estimate, allowance, prompt reserve, df ceiling, rarity rule) | `packing_…` serializes trials and asserts no fixture source text; `summaries_and_misses_follow_outcomes` | PASS |
| No-network trial proves withheld, excluded, unranked, whole, spans-only, omitted, malformed-patch outcomes | `evaluate` | `withholding_…` (withheld/excluded, added file with missing patch recorded), `packing_…` (whole, spans-only, omitted, unranked for all variants, malformed patch excluded and recorded) | PASS |

Verification, sequential, exit 0 each: `mise run test:pipeline` (77 + 2 + 12 + 1 passed), `mise run test:pipeline:journey` (2 passed), `mise run test:symbolic-context:journey` (6 passed), `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate`.

Boundaries: diff touches only `examples/context_relevance/{main,trial,report}.rs` and the `live:context-relevance` description in `mise.toml`; no production crate, `PrContext` call, replay layer, or gate change. Report serialization holds paths, commits, names, ranks, scores, and counts only.

Material limits: "hit" in score distributions and misses means sent (whole or spans) under the common policy. D scoping counts one lookup per name and referencing file. Python modules resolve by unique path suffix; relative Python imports and Rust paths outside `crate`/`self`/`super` stay unscoped; a Rust `use … as alias` or Python `import … as` binds the alias, which never declares, so it falls back to C. A remaining changed file whose base is not indexed contributes no base-side names. Every distinct blob of a repository run stays cached in memory to bound provider requests; TASK-003 will show whether live request volume fits the token's rate limit.
