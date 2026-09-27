---
id: TASK-007-R2
kind: remediation
status: ready
spec: SPEC-repo-parser@14
parent_task: TASK-007
remediates: [FIND-TASK-007-1, FIND-TASK-007-2]
depends_on: [TASK-007, TASK-007-R1]
---

# TASK-007-R2 — Stable capacity workload and executable provider contract

## Authority and subject

Approved [revision 14 specification](../../spec.md), [original TASK-007](../../tasks/TASK-007-integrated-pipeline-and-real-pr.md), [TASK-007-R1](../TASK-007-r1/TASK-007-R1-fixed-commit-and-provider-proof.md), and [stateless-snapshot decision](../maintainer-decision-stateless-snapshot.md). Reviewed base `2257992f36a2aa88f33f397af1e070d6ab35961b`; candidate `f820d662a841f7e13773223b2f93cf90e82b6274`. Implement within these approved contracts and route directly to `$cr-implement`.

## Diagnosis and correction outcomes

### FIND-TASK-007-1 — Capacity workload remains mutable

**Root cause and defect class.** R1 added `BCR_COMMIT` and `ColdBuild::run_at`, so the cold tree is fixed, but the benchmark entrypoint still accepts only a mutable PR number for measured PR work. `examples/live_pr.rs:117,121` calls `PrContext::run` for the first and every warm sample; `src/pull_request.rs:324-353` fetches the current PR and files each time. The report serializes only the first result's base/head (`live_pr.rs:143-145`) while aggregating later warm times and total API calls. A PR push between samples mixes workloads under one reported identity; a later push changes the input of the exact rerun command. This violates AC-007 and TASK-007's reproducible fixed-commit warm/call measurements. The fixed-commit cold test covers only the cold SHA, so it cannot close this path.

**Siblings.** The first and every warm call in `live_pr.rs`; `evidence/TASK-007/s20-wyrd-pr1.json`, `bench-vscode.json`, and `bench-rust.json`; all three capacity rows and their rerun commands in TASK-007. The negative PR #91 report is separate S20 evidence.

**Required outcome and recommendation.** Keep `PrContext::run`'s production fresh-PR behavior. Make the opt-in capacity measurement bind its first and warm samples to one declared or observed PR base/head workload, reject the run if any measured sample differs, and record that workload beside the fixed cold commit/tree. Reuse the identity already returned in `PrContextResult.context` and the existing verified provider path; do not add snapshot state or a second PR pipeline. Use a stable real PR fixture or equivalent reproducible input for refreshed size-class reports. If an old PR revision can no longer be fetched through the production facade, record that limit and use a stable fixture for the affected capacity row rather than presenting unlike inputs as one benchmark. Preserve ordinary S20's configurable current-PR path.

**Acceptance and focused proof.** A fake provider advances the PR between first and warm calls and the capacity command refuses to emit successful p50/p95/API-call evidence; a stable PR accepts all samples with the same recorded base/head. Refresh each affected Wyrd, VS Code and Rust capacity report and TASK-007 row with exact rerunnable cold commit/tree and PR identities, archive bytes, cold time, snapshot size, peak RSS, warm p50/p95, API calls and coverage. No benchmark is marked reproducible when its workload moved.

### FIND-TASK-007-2 — Provider cases do not decide command success

**Root cause and defect class.** R1 added a controlled real-provider report, but `examples/provider_contract.rs::run` only checks that some PR file lacks a patch before returning success (`:225-228`). It records recursive-tree `truncated` and `invalid_paths` (`:153-156`), archive gzip magic (`:162-173`), conditional status (`:199-214`), and rate headers (`Recording::get`, `:61-75`) without making failed values fail the command. A 200 conditional replay, invalid tree, non-gzip archive, or missing expected rate headers can therefore exit 0. The captured Wyrd report itself contains passing values; the command's result shape permits a false AC-006 pass on subsequent runs. Client-owned repository, commit, blob and HTTP failures already propagate and need no duplicate checks.

**Siblings.** Recursive tree usability, archive redirect/shape and gzip, conditional 304, and expected GitHub API rate headers in the one contract command; its report, opt-in lane and TASK-007 provider evidence row. Patch omission already fails when absent.

**Required outcome and recommendation.** Make `provider_contract.rs` reject every failed named compatibility case before success, using values already available to that command. Keep the production `GitHubClient` and `ReqwestTransport`, the raw conditional probe labeled as a provider observation, and the separate local fake proof for 403/429 and GHES. Do not add conditional requests to the production client or deliberately exhaust a live quota. Preserve the captured case format and replace its passing report only if a fresh controlled run changes the evidence.

**Acceptance and focused proof.** A local scripted transport or equivalent no-network check supplies each failed sibling outcome in turn and observes a nonzero command result; valid outcomes pass. The named controlled real-GitHub invocation records passing request statuses, archive redirect/shape, tree/blob outcomes, patch omission, conditional observation and rate headers. A missing required case cannot be reported as PASS.

## Constraints, write set and verification

Preserve the independent production facades, authenticated source and blob verification, complete source-free per-path snapshots, stateless updates, private PR overlays, honest coverage, context read-back, and normal PR pinning. No consumer service, clone, server, worker, persistence change, durable source archive, production conditional-request support or credentialed normal CI lane is in scope.

The correction write set includes `crates/bohmian-code-review-pipeline/examples/live_pr.rs`, `examples/provider_contract.rs`, their owning focused test surface, affected `changes/active/repo-parser/evidence/TASK-007/` reports, and `changes/active/repo-parser/tasks/TASK-007-integrated-pipeline-and-real-pr.md`; update `mise.toml` only if the opt-in invocation must change. Record exact focused test selectors after naming the checks. Run `mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check`. Keep live provider and capacity runs explicitly invoked outside the gate; report unavailable live conditions as verification limits.

## Implementation Evidence

Implemented from candidate `79d73f3`. Live runs used `GITHUB_TOKEN=$(gh auth token)`, read-only, on 2026-09-27.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| FIND-TASK-007-1: every measured sample binds to one PR base/head; a moved PR yields no successful evidence; `PrContext::run` unchanged | `PrContext::run_at` lists files under a caller-read `PullRequest` and publishes only when a re-read names the same pair, else `PullRequestMoved { attempts: 1 }`; `run` still pins with retries through the shared `list`/`publish`. `examples/live_pr.rs` reads the PR once and sends the first and every warm sample through `run_at` | `pull_request::tests::samples_at_one_read_pr_refuse_a_moved_head`: two stable samples share base/head, an advanced head is refused and publishes nothing; existing `moving_pr_retries_and_bad_patches_stay_file_local` and `perpetually_moving_pr_publishes_nothing` still pass | PASS |
| Refreshed Wyrd, VS Code and Rust capacity reports with rerunnable cold commit/tree and PR identities | `evidence/TASK-007/s20-wyrd-pr1.json`, `bench-vscode.json`, `bench-rust.json`; TASK-007 capacity table and commands | `mise run live:pr` with `BCR_COMMIT` and `BCR_WARM_RUNS=3` for wyrd #1, vscode #338185, rust #163379 (all merged, so stable), each exit 0, `read_back_equal: true`, `full_coverage: true`, 5 `PullRequest` calls (one read plus one re-read per sample) | PASS |
| FIND-TASK-007-2: each named provider case decides exit status; a missing case cannot pass | `examples/provider_contract.rs` `failures` over `REQUIRED` cases and `RATE_HEADERS`; `run` returns every failure | `provider_contract::tests::every_named_case_decides_the_verdict` (no network): captured report passes; truncated or invalid tree, non-gzip archive, non-redirect archive, no patch omission, 200 conditional replay, missing rate headers, and a missing case each fail | PASS |
| Controlled real-GitHub invocation passes | same | `BCR_REPO_ID=1383739221 BCR_COMMIT=2855eac00a15581cede6e29ec5a0dde0b8bc4d48 mise run live:provider-contract` exit 0 with the same case outcomes as the captured report, which therefore stands | PASS |
| Verification | — | `mise run test:pipeline`, `mise run test:pipeline:journey`, `mise run fmt:check`, `mise run lints`, `mise run gate`, `git diff --check 79d73f3 HEAD`: all exit 0 | PASS |

Non-goals stayed excluded: no production conditional requests, snapshot state, persistence change, consumer service, second PR pipeline, or credentialed gate lane; `mise.toml` is unchanged. Limits: live 403/429 and GHES remain proven only by fake-transport tests; peak RSS is macOS `/usr/bin/time -l`.
