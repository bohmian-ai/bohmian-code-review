# Capacity, concurrency, and budget domain review

**Subject:** `2257992f36a2aa88f33f397af1e070d6ab35961b..317b2da9a8cd45755418dca05111c96beb19eaa4` (complete cumulative candidate). **Result: PASS.** No material findings.

## Boundary and coverage

Reviewed fixed-commit cold build, complete snapshot and pointer publication, PR base/head pinning, warm sampling, cancellation and resource budgets, context publication and read-back, and the three capacity reports. Authorities were approved `SPEC-repo-parser@14` (REQ-005–006, REQ-009–010, REQ-012–014, AC-002, AC-004, AC-007), original TASK-007, R1–R4 remediation and prior findings, `review/maintainer-decision-stateless-snapshot.md`, `AGENTS.md`, and `architecture/bohmian-code-review-design.md`. Source coverage included the cumulative diff, surrounding `snapshot.rs`, `pull_request.rs`, `live_pr.rs`, the public journey, `mise.toml`, and the Wyrd, VS Code, and Rust evidence. R4 changes only the durable provider test fixture and task evidence; it does not alter this boundary.

## Result

- `ColdBuild::run_at` verifies the requested commit, then uses the same listing, bounded archive/fallback ingest, completeness gate, and record/object/pointer publication as `run`. The pointer is not advanced for an incomplete ingest. The snapshot payload remains per-path outcomes and contains no aggregate ingest counts or source text.
- The live command reads one PR and calls `PrContext::run_at` for the first and every warm sample. The shared listing method rereads the PR after pagination and refuses a different base/head before publishing; ordinary `run` retains bounded retry for its own newly pinned PR. A moved PR therefore cannot make one capacity report mix revisions.
- The fixed-commit and moved-PR focused tests each selected one test and passed locally. The three checked-in reports record exact cold commit/tree and PR base/head, three warm samples, published full coverage, and equal read-back. Eligible files reconcile with indexed plus `NoFacts`: Wyrd 1,681 = 1,461 + 220; VS Code 14,691 = 12,762 + 1,929; Rust 39,496 = 36,780 + 2,716. The task records archive bytes, cold and warm timings, snapshot bytes, peak RSS, call counts, and the macOS/Apple silicon release environment.
- Budget exhaustion remains explicit, with empty selected context failing the live command. Caller timeouts drop facade futures; no detached work or new library deadline appears in the candidate. The existing source and tests cover bounded archive, fallback, PR-file pagination, selected context, and incomplete coverage paths.

## Verification limits

The exact focused commands were `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib fixed_commit_build_ignores_head_advance_and_run_follows_it --all-features` and the same command selecting `samples_at_one_read_pr_refuse_a_moved_head`; each passed one test. Credentialed GitHub requests, wall time, archive bytes, and peak RSS were assessed from the checked-in reports and task record, not rerun. The PR's second read is the specified consistency boundary; movement after it is not detected. No blocking gap arose from these limits.

```citations
[]
```
