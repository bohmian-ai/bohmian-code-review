---
id: TASK-002
kind: implementation
status: proposed
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
