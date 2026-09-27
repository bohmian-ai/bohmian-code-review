# TASK-007 findings validation

Subject: `2257992f36a2aa88f33f397af1e070d6ab35961b..6e9ab0f764f9d7961924dac7fb1e5c04be02a38c`; `SPEC-repo-parser@14`; original `TASK-007`; maintainer decision `review/maintainer-decision-stateless-snapshot.md`.

Recommendation: **FIX_REQUIRED**. Two distinct task-contract gaps remain. The refreshed diff changes evidence reports and the task record, not production code. I parsed the complete changed JSON reports and compared their top-level and nested fields against the base versions; the removed `bench-wyrd.json` is superseded by `s20-wyrd-pr1.json`.

## Blocking ledger

### FIND-TASK-007-1 — Capacity runs are bound to a moving branch

- **Sources/status:** `TASK-B-1` **CONFIRMED** and `STD-1` **CONFIRMED**, merged by root cause.
- **Gate/classification:** Task contract (AC-007, TASK-007 approach step 4 and acceptance criteria); repository standard ([design](../../../../../architecture/bohmian-code-review-design.md), line 38). `MISSING` reproducible fixed-commit benchmark evidence.
- **Root cause/defect class:** The capacity entrypoint has no fixed-commit input. `examples/live_pr.rs:98` calls `ColdBuild::run`; `snapshot.rs:621-625` obtains `head` through `pin`; `snapshot.rs:633-638` resolves the repository's current default branch. The SHA printed in each report is an observed output, not a selectable benchmark input. This is an evidence-entrypoint shape defect; it need not change the production default-branch contract.
- **Siblings:** `s20-wyrd-pr1.json`, `bench-vscode.json`, `bench-rust.json`; the capacity table and reproducibility qualification at `tasks/TASK-007-integrated-pipeline-and-real-pr.md:95-102`. The removed `bench-wyrd.json` is not a further live sibling.
- **Reproduction:** Static public path: `mise run live:pr` (`mise.toml:65-69`) → `examples/live_pr.rs:98` → `ColdBuild::run` → `ColdBuild::pin` → `GitHubClient::repository` and `branch_head`. `BCR_REPO`, `BCR_PR`, `BCR_REPO_ID`, and timeout/warm-run settings provide no commit selection. The task record expressly says a row is reproducible only while its SHA is still the default head. Once it advances, rerunning the recorded command measures another tree. The JSON reports contain the observed `cold_build.commit` but no fixed-commit invocation or input. `Update::run` (`update.rs:86`) also pins the default branch; `PrContext::run` (`pull_request.rs:155`) pins the PR, which is adjacent review behavior and does not repair cold benchmark input.
- **Consequence:** The recorded 1k/10k/50k cold timings, memory, snapshot sizes, and warm figures cannot be independently reproduced at their named commits after branch movement. AC-007 and the written benchmark rule remain unproved despite the reported full snapshots.
- **Minimum safe correction:** Keep normal `ColdBuild::run`, update, and PR behavior unchanged. Give the explicitly invoked benchmark path a way to build the named immutable commit through the existing verified cold-ingest owner and record the exact input command, SHA/tree, environment, and outputs for each size. Do not substitute an archive-only measurement that bypasses the public cold path, and do not use a moving default-branch selector. The benchmark-only input can be candidate-owned; no new persistence state or changes to stateless snapshots are needed.
- **Closure proof:** Rerun or provide reproducible fixed-SHA evidence for all three repositories with cold, warm, archive size, snapshot size, peak RSS and API calls. A focused fake-provider check should advance the default branch between runs and prove the requested benchmark SHA stays fixed while the normal default-branch call follows the new head.

### FIND-TASK-007-2 — Controlled real-provider contract run is absent

- **Sources/status:** `TASK-A-2` **CONFIRMED**.
- **Gate/classification:** Task contract (REQ-017 and AC-006 in `spec.md:80,120`; TASK-007 approach step 4 and acceptance criteria). `MISSING` required evidence, with the conditional behavior unexercised.
- **Root cause/defect class:** Public-repository S20 and capacity observations plus an informal `gh api -i` header capture were used as the separate controlled real-provider contract. The only opt-in executable in `crates/bohmian-code-review-pipeline/examples/` is `live_pr.rs`; its `run` body (`:65-173`) builds a snapshot, PR context and read-back for an existing PR, and accepts no controlled fixture or conditional request configuration. The four JSON reports all have the same S20 report shape, with no contract-case results.
- **Siblings:** The contract evidence row at `tasks/TASK-007-integrated-pipeline-and-real-pr.md:86`; all four live reports under `evidence/TASK-007/`; the `live:pr` lane at `mise.toml:65-69`. The fake `github.rs` rate/GHES tests cover the local half of AC-006 but are not a controlled real-provider run.
- **Reproduction:** `rg --files` over `evidence/TASK-007`, pipeline examples and integration tests yields four S20/capacity JSON reports, `live_pr.rs`, and `pipeline_journey.rs`, with no controlled contract command or captured case report. `live_pr.rs:86-98` constructs a normal GitHub client and calls `ColdBuild::run`; `:107-118` calls PR context. `github.rs:1471-1525` handles rate responses, but these live reports contain operation counts, not observed response statuses/headers or conditional outcomes. The task record itself says no live 403/429 was provoked and the client sends no conditional requests. Thus its asserted PASS does not demonstrate AC-006's controlled real-provider conditional/rate behavior.
- **Consequence:** Required pre-rollout provider compatibility evidence is missing. A passing S20 run demonstrates an existing PR journey, not the controlled contract's conditional and rate cases.
- **Minimum safe correction:** Run and capture the explicitly requested credentialed, opt-in controlled real-GitHub contract against a repository the caller controls, exercising and recording archive redirect/shape, recursive tree, blob, PR patch omission and conditional/rate behavior. Reuse the production GitHub transport and its existing local fake rate/GHES contract tests where live quota exhaustion would be unsafe. If the intended acceptance is only header observation and local simulations because the production client has no conditional requests, that relaxes the spec-named AC-006 obligation and requires an approved spec revision rather than an unqualified PASS. Keep S20, capacity evidence, and normal CI separate.
- **Closure proof:** A named, rerunnable opt-in command and captured case results identify the controlled repository, immutable refs, requests/statuses and observed outcomes; the relevant local fake checks remain green. The evidence must explicitly account for conditional behavior and rate limits rather than treating their absence from S20 as a pass.

## Claim disposition and limits

| Source claim | Result | Reason |
|---|---|---|
| `TASK-B-1` | CONFIRMED; `FIND-TASK-007-1` | Reproduced through the benchmark entrypoint and `ColdBuild::pin`. |
| `STD-1` | CONFIRMED; `FIND-TASK-007-1` | Same root cause, plus the written reproducibility rule. |
| `TASK-A-2` | CONFIRMED; `FIND-TASK-007-2` | Required controlled run and captured cases are absent. |

No notes. `TASK-A-1` was rejected by citation verification before this wave and was not revalidated here. I used source-path reproduction and report-structure comparison; I did not rerun credentialed GitHub calls or the broad gate. This review does not call for changing snapshot content, source-free persistence, PR admission, or update semantics.
