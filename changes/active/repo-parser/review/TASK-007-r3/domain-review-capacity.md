# Capacity and PR consistency domain review

**Subject:** `2257992f36a2aa88f33f397af1e070d6ab35961b..b721917420113ab2e3530966e9e456eecc89dcff` (cumulative, immutable). **Result: PASS.** No material findings.

## Reviewed boundary and authority

Reviewed the cold snapshot selector, PR pagination and pinning, first and warm measurements, and the three checked-in capacity reports. Traced `examples/live_pr.rs` through `ColdBuild::run_at`/`build`, `PrContext::run_at`/`list`/`publish`, and the existing `PrContext::run` path. Applicable authority: approved `spec.md` revision 14 (REQ-006, REQ-009, REQ-010, REQ-013, AC-007), original TASK-007, TASK-007-R1/R2, the stateless-snapshot decision, `AGENTS.md`, and `architecture/bohmian-code-review-design.md`.

## Result and evidence

- The fixed cold commit is verified by the provider and enters the existing cold build; normal `run` still pins the current branch. The focused fixed-commit test in `snapshot.rs` covers a branch advance.
- `live_pr.rs` reads one PR, then uses that `PullRequest` for the first and all three warm samples. `PrContext::run_at` lists files and re-reads the PR before publishing; a changed base/head returns `PullRequestMoved` instead of a successful sample. Normal `PrContext::run` retains its bounded fresh-PR retry path through the same `list` owner. The exact `samples_at_one_read_pr_refuse_a_moved_head` focused test passed: 1 selected, 0 failed.
- All three reports parse, contain cold commit/tree and PR base/head, three warm samples, five PR reads (one initial plus one per sample), full coverage and equal read-back. Their eligible counts reconcile with indexed plus recorded NoFacts skips: Wyrd 1,681 = 1,461 + 220; VS Code 14,691 = 12,762 + 1,929; Rust 39,496 = 36,780 + 2,716. TASK-007 records exact rerun commands, archive bytes, cold/warm times, snapshot bytes, peak RSS, API calls and named macOS/Apple silicon environment.
- The per-path snapshot payload remains source-free and without aggregate counts. `run_at` reuses the existing build path; R2 did not add snapshot state or a second PR pipeline.

## Verification limits

The live GitHub runs, archive byte counts and peak RSS measurements were not repeated in this review; the checked-in reports and task record are their evidence. The focused fake-provider test proves detected movement between samples, not every possible provider race after the second read. The production pinning contract also ends its stability check at that read under REQ-010.

```citations
[]
```
