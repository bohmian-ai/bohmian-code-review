# Co-change ranking experiment

Completed 2026-09-29. Approved specification `SPEC-cochange-eval` revision 2. Reviewed target `c3145c64a26ca98b699105668d54f8bdc861f9ec`. Delivery reference: not supplied.

This read-only experiment tested whether earlier merged PRs' co-changed files improve related-file ranking over C, the existing directional, rarity-weighted control. It compared C with co-change alone (E) and reciprocal-rank fusion (F) on 50 fresh PRs each from pydantic-ai, tokio, and zod. Each PR contributes up to five deterministic leave-one-out trials; the outcome is hit@10, averaged per PR. It did not measure packing or model exposure.

History has ten non-mass-edit PRs before the earliest sampled base commit and extends with eligible merges before each trial's base commit. PRs changing more than 30 files are excluded. Eligibility uses merge and base-commit timestamps, not ancestry. The opt-in credentialed command obtains source and provider data read-only, keeps source transiently in memory, and emits source-free JSON only after all repositories succeed. Production ranking, packing, public APIs, persisted formats, and the gate remain unchanged.

The fixed decision rule requires F−C to improve mean per-PR hit@10 by at least five points in every repository with each 95% bootstrap lower bound above zero. The three checked-in reports meet that rule: pydantic-ai +14.1 points (interval +4.5 to +24.2), tokio +17.5 (+9.5 to +26.7), and zod +32.3 (+22.8 to +42.0). This is an experimental ranking result, not a production adoption. The design fixes depth 10, a 30-file mass-edit threshold, fusion constant 60, 10,000 bootstrap resamples, and seed `0x2026_0929_c0c4_a17e`; no tuning was done on this sample.

The approved revision 2 replaced revision 1 with the C/E/F paired design, new non-overlapping samples, a third repository, time-based history eligibility, and a ranking-only decision rule. There were no later approved deviations.

| Closure | Evidence |
|---|---|
| Retrieval, history, bounded requests, and no leakage (REQ-001/002, INV-003/005) | [history owner](../../../crates/bohmian-code-review-pipeline/examples/context_relevance/history.rs), [command owner](../../../crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs), fake-provider tests |
| Paired C/E/F ranking, seeded inference, and decision (AC-003) | [trial owner](../../../crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs), [report owner](../../../crates/bohmian-code-review-pipeline/examples/context_relevance/report.rs), no-network example tests |
| Complete, reproducible reports (REQ-003, AC-001/002) | [Python](cochange-eval-reports/pydantic-ai.json), [Rust](cochange-eval-reports/tokio.json), [TypeScript](cochange-eval-reports/zod.json); two byte-identical live runs recorded in the reviewed task evidence |
| Read-only opt-in and unchanged production (REQ-004, INV-001/004, AC-004) | [mise command](../../../mise.toml), full diff inspection, passing `mise run gate`, [final review](../../reviews/cochange-eval/change-review.md) |

Current authority: [repository design](../../../architecture/bohmian-code-review-design.md), [agent rules](../../../architecture/agent-rules.md), and [testing workflow](../../../architecture/references/languages/testing-workflows.md).
