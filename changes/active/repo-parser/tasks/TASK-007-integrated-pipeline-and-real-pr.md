---
id: TASK-007
kind: implementation
status: proposed
spec: SPEC-repo-parser@14
depends_on: [TASK-001, TASK-002, TASK-003, TASK-004, TASK-005, TASK-008, TASK-009, TASK-010]
maps: [REQ-004, REQ-008, REQ-009, REQ-010, REQ-011, REQ-012, REQ-013, REQ-017, REQ-018, INV-001, INV-002, INV-003, INV-004, INV-005, INV-006, INV-007, AC-002, AC-003, AC-004, AC-006, AC-007, AC-008, AC-010, AC-011]
---

## Outcome and Value

The complete pipeline is proven from this repository through separate cold-build, update, PR-context and read facades, including a configurable run against an existing PR in a real large GitHub repository.

## Owners, Scope, Consumers, and Prohibited Changes

All code, test commands and evidence for this task live in this repository. Normal CI uses fake GitHub and injected test dependencies. The real-provider contract and PR runs are explicitly invoked, credentialed, read-only and excluded from `mise run gate`. No consumer service, schema, route, job or worker is a prerequisite. Final task and change reviews happen after task completion under the repository lifecycle.

## Approach

1. Call each public facade separately, then compose a cold PR, update, warm PR and read-back with the same injected capabilities.
2. Complete S1–S19 and from-scratch/property oracles, including negative coverage, rate, identity and crash cases.
3. Add a dedicated configurable S20 command, defaulting to the existing `bohmian-ai/wyrd` PR #91, that calls this library directly with real GitHub and injected in-memory object storage, snapshot store and context store.
4. Run the controlled provider contract check and fixed-commit 1k/10k/50k benchmarks; record results and any needed remediation.
5. Re-run the capacity evidence (AC-007) after TASK-008, TASK-009 and TASK-010 pass. Evidence recorded before them is superseded.

## Ordered Implementation Scenarios

### Scenario 1 — Composed public library journey

**Behavior.** Separate calls build a complete snapshot, advance it for a new default-branch tree, build pinned PR context and read identical selected spans by ID. Cold and warm calls use different source budgets, and incomplete input never claims full coverage. A cold build dropped by the caller's `tokio::time::timeout` leaves no pointer. The next PR-context call then uses scoped fallback without consuming the review's own timeout (AC-002). A retried cold build converges and publishes.

**RED.** Add a public outer-crate async integration journey over fake GitHub and the injected test capabilities. It fails before composition because there is no valid pointer, context ID, or read-back equivalence.

**GREEN.** Close only cross-stage seams and missing behavior found by the journey while retaining the stage owners from earlier tasks.

**REFACTOR.** Keep one identity and coverage result through the facades. Do not add an all-in-one production method, a detached task, or a library deadline.

### Scenario 2 — Real existing PR

**Behavior.** With configured GitHub credentials, an on-demand command reads an existing PR in the real `bohmian-ai/wyrd` repository, invokes the library facades directly, analyzes a supported changed file, publishes nonempty selected context and reads it back by ID. Missing credentials or an inaccessible PR fail visibly; GitHub is never mutated. The command bounds each facade call with its own `tokio::time::timeout`.

**RED.** Add the explicitly invoked S20 integration test; it initially cannot produce a verified context ID and read-back through real GitHub.

**GREEN.** Use the same production facades with injected in-memory object storage, snapshot store and context store, recording PR URL/base/head, coverage, omissions, spans, object/metadata identity, calls, elapsed time and memory.

**REFACTOR.** Keep the command a thin configurable entrypoint to existing facades, not a second implementation of the pipeline.

## Proof Strategy

The controlled provider contract and benchmarks measure existing behavior, so no manufactured RED applies. They remain explicitly invoked evidence runs outside normal CI. Failing results return to the owning task or a reviewed remediation task.

## Acceptance Criteria

S1–S19, the complete-coverage from-scratch oracle, independent facade calls and composed journey pass in this repository. S20 passes at least once against an existing real PR with nonempty selected spans and read-back. The provider contract and reproducible benchmark reports are recorded. The capacity report is re-run after TASK-008, TASK-009 and TASK-010 and shows complete published snapshots with measured warm reuse, or it names the eligible files still blocking completeness and why. No consuming repository change or hosted shadow run is required. Final reviews remain separate lifecycle gates.

## Expected Write Set and Consumer Closure

Outer-crate public integration tests, fake-provider/injected-adapter support, scoped `mise` lanes, on-demand real-PR command and recorded evidence. No files outside this repository are required.

## Verification and Evidence

Run `mise run gate`, the new focused pipeline journey, and the explicitly invoked provider contract, S20 and benchmark commands; record exact commands and reports. S20 must not be added to normal CI.

## Material Stop Conditions

Stop if the four public facades cannot compose under the approved injected contracts, if real GitHub invalidates a pinned-source assumption, or if large-repository bounds require a material change to coverage or bounds.

## Authority Links

[Revision 14 spec](../spec.md), [pipeline plan](../symbolic-context-service-plan.md), [library design](../../../../architecture/bohmian-code-review-design.md).

## Implementation Evidence

Recorded 2026-09-27 against revision 14 at `2257992`, refreshed for TASK-007-R1 with fixed-commit runs and for TASK-007-R2 with one PR base/head per run, after TASK-008, TASK-009 and TASK-010 passed review. It supersedes the revision-13 evidence, including its material finding: revision 14 resolved that finding, and every large repository now publishes a complete snapshot.

Environment: macOS 15 (Darwin 24.6.0), Apple silicon, release build, residential network. Live runs used a read-only `gh` user token (`GITHUB_TOKEN=$(gh auth token)`) and only read from GitHub. Peak RSS comes from `/usr/bin/time -l`. Reports in [evidence/TASK-007](../evidence/TASK-007/) carry paths, ranges and identities, never source text.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Independent facade calls | `crates/bohmian-code-review-pipeline/tests/pipeline_journey.rs` `each_facade_is_independently_callable` | `mise run test:pipeline:journey` (2 passed) | PASS |
| Composed journey: a dropped cold build falls back to scoped context without spending review allowance (AC-002), and a retry converges; the compare-based update fetches only the changed blob; the warm PR fetches no archive, matches from-scratch ranking and has full coverage; read-back by ID makes no provider calls | same file, `cold_update_warm_review_and_read_compose` | `mise run test:pipeline:journey` | PASS |
| S1–S19, from-scratch/property oracles | Owner tests from TASK-002–005 and 008–010 (each reviewed PASS) | `mise run gate` | PASS |
| S20 fails visibly without credentials or access | `examples/live_pr.rs` | `env -u GITHUB_TOKEN …/live_pr` → `GITHUB_TOKEN is required`, exit 1. `BCR_PR=999999` → `not found: /repos/bohmian-ai/wyrd/pulls/999999`, exit 1. `BCR_REPO_ID=1` → `identity mismatch`, exit 1 | PASS |
| S20 cannot pass with empty context | same | Default PR #91: diff ≈422k est. tokens > 64k allocation → `exhausted: true`, `spans: []`, `selected context is empty`, exit 1 ([report](../evidence/TASK-007/s20-wyrd-pr91.json)) | PASS |
| S20 recorded passing run: nonempty spans, read-back by ID | same | `GITHUB_TOKEN=$(gh auth token) BCR_REPO_ID=1383739221 BCR_PR=1 BCR_COMMIT=2855eac00a15581cede6e29ec5a0dde0b8bc4d48 BCR_WARM_RUNS=3 mise run live:pr` → repository scope, `full_coverage: true`, 50 spans, `read_back_equal: true`, exit 0, 102 s wall, 181 MiB peak RSS ([report](../evidence/TASK-007/s20-wyrd-pr1.json)) | PASS |
| Provider contract (AC-006) | `examples/provider_contract.rs`, lane `live:provider-contract` (production `GitHubClient` over `ReqwestTransport`, recording status and non-secret headers) | `GITHUB_TOKEN=$(gh auth token) BCR_REPO_ID=1383739221 BCR_COMMIT=2855eac00a15581cede6e29ec5a0dde0b8bc4d48 mise run live:provider-contract`, exit 0, against the controlled `bohmian-ai/wyrd` at commit `2855eac…`/tree `e52bce5…` and PR #91 ([report](../evidence/TASK-007/provider-contract-wyrd.json)): repository 200; commit 200 resolves the tree; recursive tree 200, 3,210 entries, not truncated; blob 200, SHA-verified; archive 302 to `https://codeload.github.com` then 200, 8,486,574 gzip bytes with gzip magic; PR files 3 pages of 200, 38 of 277 files without a patch. Every `api.github.com` response carries `x-ratelimit-limit/remaining/reset/used/resource`. Provider observation only, outside the client: replaying the repository `ETag` in `If-None-Match` returns 304. The command exits nonzero unless every named case is present and passes: `failures` requires an untruncated nonempty tree without invalid paths, a 302 archive redirect to a 200 gzip body, a PR file without a patch, a 304 observation and the five rate headers on every API response; `provider_contract::tests::every_named_case_decides_the_verdict` passes the captured report and fails each broken or missing case in turn. A fresh R2 run exited 0 with the same case outcomes, so the captured report stands. 403/429 retry and GHES header variants are proven by the fake-transport tests in `github.rs` (`mise run test:pipeline`), not live; no quota was exhausted | PASS (live 403/429 not provoked) |
| Capacity (AC-007), re-run for TASK-007-R2 | `BCR_COMMIT`, `BCR_WARM_RUNS=3`; `PrContext::run_at` | Table below. All three repositories publish a complete snapshot, and warm PR context reuses it with no archive fetch. Every row measures one merged PR's base/head; `pull_request::tests::samples_at_one_read_pr_refuse_a_moved_head` shows a stable PR accepts every sample at one pair and an advanced head is refused with nothing published | PASS |

| Repository @ commit (PR) | Eligible / indexed / NoFacts | Archive bytes gz / expanded | Snapshot bytes | Cold ms | First PR ms | Warm p50 / p95 ms | Peak RSS | Calls (Blob / Tree / Archive) | PR coverage |
|---|---|---|---|---|---|---|---|---|---|
| bohmian-ai/wyrd @2855eac (#1 f7c6133…9e8d2a1) | 1,681 / 1,461 / 220 | 8.5 MB / 44.2 MB | 7.7 MB | 3,824 | 24,161 | 24,332 / 25,752 | 181 MiB | 545 / 5 / 2 | Full |
| microsoft/vscode @4336606 (#338185 43dd907…c384177) | 14,691 / 12,762 / 1,929 | 56.8 MB / 296.5 MB | 60.4 MB | 19,150 | 8,864 | 8,953 / 9,190 | 1,093 MiB | 117 / 5 / 2 | Full |
| rust-lang/rust @b20a7eb (#163379 a22b02e…b73bcdb) | 39,496 / 36,780 / 2,716 | 40.3 MB / 278.4 MB | 66.1 MB | 20,388 | 12,746 | 12,754 / 12,850 | 1,010 MiB | 153 / 40 / 2 | Full |

Archive bytes come from `gh api repos/<repo>/tarball/<commit> | wc -c` (and `| gzip -dc | wc -c`) at the pinned commit. Every `NoFacts` skip on wyrd is a file with no extractable declarations: an empty file, a doc-only or `mod`-only Rust module, a vitest body, or YAML. Calls include the cold build, the first PR and three warm runs. Warm time is dominated by overlay and candidate blob fetches, since each run fetches its PR's changed and selected blobs again. Each row is pinned before measurement with `BCR_COMMIT`, which routes the cold build through `ColdBuild::run_at` (repository identity, then the commit's verified tree, then the same verified archive ingest as `ColdBuild::run`), so it reruns at the same commit and tree however the default branch moves. The PR is read once and the first and every warm sample go through `PrContext::run_at` at that base/head, which lists the files, reads the PR again and publishes only when it still names that pair; a PR that moves fails the run instead of mixing workloads. Each row uses a merged PR, whose base and head no longer move, and its report records them beside the cold commit and tree. Commands, each `GITHUB_TOKEN=$(gh auth token) BCR_WARM_RUNS=3 … mise run live:pr`: wyrd `BCR_REPO_ID=1383739221 BCR_PR=1 BCR_COMMIT=2855eac00a15581cede6e29ec5a0dde0b8bc4d48` (tree `e52bce5c1a1c5188ed698d541f8128f90474f2bc`); vscode `BCR_REPO=microsoft/vscode BCR_REPO_ID=41881900 BCR_PR=338185 BCR_COMMIT=4336606aa949efdcddb89b3db3e216562730b2bf` (tree `991515e042ae24e34f21e176a186b2d7ec17968b`); rust `BCR_REPO=rust-lang/rust BCR_REPO_ID=724712 BCR_PR=163379 BCR_COMMIT=b20a7ebab6f2556bd5461931d146b99f18dba693` (tree `9e8a9f39e23c7ed653b219bb864aa68743f04700`). All exit 0 with read-back equal. `snapshot::tests::fixed_commit_build_ignores_head_advance_and_run_follows_it` advances the fake default head between calls and shows `run_at` stays at the requested commit while `run` follows the new head. The open PRs vscode #338165 and rust #163394 used before could still move, so R2 replaced them with merged PRs #338185 and #163379; vscode's former overlay-incomplete row came from #338165's stale base, not the size class.

Non-goals stayed excluded. There is no consumer code and no server or worker, and S20, the provider contract and the live runs stay outside `gate`. TASK-007-R1 added only `ColdBuild::run_at`, its test, the `BCR_COMMIT` switch, the contract example and its lane; TASK-007-R2 added only `PrContext::run_at` with its test, the one-read sampling in `live_pr.rs`, and the contract verdict with its test; the superseded `bench-wyrd.json` was removed because `s20-wyrd-pr1.json` is the 1k-class wyrd measurement.
