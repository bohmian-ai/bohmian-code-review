# Capacity, budgets, and PR pinning domain review

**Subject:** `2257992f36a2aa88f33f397af1e070d6ab35961b..7f7b75d5d8645f5dd2530779fb6e146dc122efe1` (cumulative). **Result: PASS.** No material findings.

## Boundary and authority

Reviewed the fixed-commit cold path, shared snapshot publication and pointer behavior, PR base/head pinning and pagination, first and warm sample calls, budget allocation, and the Wyrd, VS Code, and Rust capacity reports. Authority: approved `spec.md` revision 14 (REQ-005–006, REQ-009–010, REQ-013, AC-007, INV-003/006), original TASK-007, R1–R3 remediation, the stateless-snapshot decision, `AGENTS.md`, and `architecture/bohmian-code-review-design.md`. Inspected the complete base-to-candidate code diff and affected source, tests, `mise.toml`, and evidence; the large JSON reports were checked through selected fields and counts.

## Result and evidence

- `ColdBuild::run_at` verifies the requested commit through the existing GitHub client and enters the same tree listing, archive ingest, completeness check, and three-step pointer publication as ordinary `run`. The pointer advances only after a complete snapshot. The head-advance test passed: the fixed commit is reused while ordinary `run` follows the new branch head.
- `live_pr.rs` reads one PR, then uses `PrContext::run_at` for its first and every warm sample. The shared `list` method checks base/head after listing pages, and `run_at` refuses a changed pair before publication; ordinary `run` retains its bounded retry. The moved-head test passed: two stable samples publish, then a moved head returns `PullRequestMoved` without a new context.
- The measured 1k/10k/50k rows record fixed cold commit/tree and a single merged PR base/head per run. The reports contain three warm samples each, five PR reads each, full coverage, equal read-back, and eligible = indexed + `NoFacts` skips: Wyrd 1,681 = 1,461 + 220; VS Code 14,691 = 12,762 + 1,929; Rust 39,496 = 36,780 + 2,716. TASK-007 gives rerun commands, archive bytes, cold and warm times, peak RSS, snapshot bytes, and call counts in a named macOS/Apple silicon environment.
- The PR path retains the configured 64,000-token allocation and 4,096-token reserve; a zero remainder yields `exhausted: true`. The candidate adds no aggregate snapshot state, new resource ceiling, or independent PR implementation.

## Verification limits

Focused local checks passed: `cargo test --locked -p bohmian-code-review-pipeline --lib fixed_commit_build_ignores_head_advance_and_run_follows_it --all-features` and the same command selecting `samples_at_one_read_pr_refuse_a_moved_head` (one passed each). Live GitHub calls, archive sizes, and peak RSS were inspected as checked-in evidence, not rerun. The second PR read is the specified consistency boundary; movement after it is outside the pinning guarantee. No blocking gap arose from those limits.

```citations
[]
```
