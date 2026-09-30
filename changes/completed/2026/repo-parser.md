# Repo parser and symbolic review context

- Change ID: `SPEC-repo-parser` (approved revision 14)
- Completed: 2026-09-27
- Reviewed target: `3e3e90daf1d0bcf7f20efbc91d375edf77702d7e` (base `9715440d6969dd317a62bd7c2a4ff30082f6082e`)
- Delivery reference: not supplied

## Intent and shipped behavior

Review agents can obtain bounded, relevant cross-file context for a GitHub or GHES pull request without a repository checkout. The public symbolic facade exports and reloads source-free facts, reports extraction compatibility and path eligibility, ranks candidates across changed files, and packs selected candidate spans. The independent async pipeline facades build and update verified default-branch snapshots, overlay a PR head privately, publish selected context, and read it by context ID. They report scoped or incomplete coverage explicitly. The consumer supplies identity, credentials, storage, persistence, cancellation, and any review or comment workflow.

## Lasting contracts and decisions

The six symbolic crates remain IO-free; only the outer pipeline crate accesses Git providers or injected storage. Snapshot objects are immutable, versioned, tenant-scoped, and source-free. A pointer names only a validated complete snapshot. Failed blob replacements remove stale facts, while ambiguous name matches remain ambiguous. PR overlays are private; candidate ordering and budget outcomes are deterministic. Only selected context objects persist source text, with a store-clock 24-hour read cutoff and identity-checked lookup. Object storage, snapshot persistence, and context persistence are separate narrow traits. Calls own their concurrent work, so dropping a call leaves valid durable state and retryable publication.

Revision 14 is the approved authority. Earlier approved revisions settled the standalone injected-library boundary, independent facades, caller-owned cancellation, and store-owned time. Revision 14 retained declarations recovered inside syntax-error regions, required complete tree traversal by splitting truncated listings, switched updates to compare-based changes, and removed aggregate state from snapshots. The injected-infrastructure revision withdrew TASK-006. No material deviation remains in the final PASS review.

## Acceptance closure

The final review passed REQ-001–REQ-018, INV-001–INV-007, and AC-001–AC-011. The [symbolic public journey](../../../crates/bohmian-code-review/tests/symbolic_context_journey.rs) covers fact round trips, two-pass ranking, and packing (AC-001). The [pipeline public journey](../../../crates/bohmian-code-review-pipeline/tests/pipeline_journey.rs) and owner tests cover cold and warm flows, updates, overlays, bounded failures, isolation, expiry, injected stores, and independent facade calls (AC-002–AC-005, AC-009–AC-010). The [provider contract runner](../../../crates/bohmian-code-review-pipeline/examples/provider_contract.rs) and [live PR runner](../../../crates/bohmian-code-review-pipeline/examples/live_pr.rs) produced the checked-in controlled-provider, fixed-commit capacity, and passing Wyrd PR #1 reports (AC-006–AC-007, AC-011); PR #91 visibly exhausted its budget. Final task verdicts and the integrated review closed AC-008. `mise run gate` passed at the reviewed target.

Current authority: [contributor rules](../../../AGENTS.md), [library design](../../../architecture/bohmian-code-review-design.md), [symbolic context contract](../../../architecture/references/domain/symbolic-context.md), and [spec-driven development](../../../architecture/references/languages/spec-driven-development.md). Code owners: [symbolic facade](../../../crates/bohmian-code-review/src/lib.rs) and [pipeline facade](../../../crates/bohmian-code-review-pipeline/src/lib.rs).
