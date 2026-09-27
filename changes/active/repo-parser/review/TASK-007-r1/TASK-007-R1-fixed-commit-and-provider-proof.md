---
id: TASK-007-R1
kind: remediation
status: ready
spec: SPEC-repo-parser@14
parent_task: TASK-007
remediates: [FIND-TASK-007-1, FIND-TASK-007-2]
depends_on: [TASK-007]
---

# TASK-007-R1 — Fixed-commit capacity and controlled-provider proof

## Authority and subject

Approved specification: `changes/active/repo-parser/spec.md` revision 14. Original task: `changes/active/repo-parser/tasks/TASK-007-integrated-pipeline-and-real-pr.md`. Maintainer decision: `changes/active/repo-parser/review/maintainer-decision-stateless-snapshot.md`. Reviewed base: `2257992f36a2aa88f33f397af1e070d6ab35961b`; candidate: `6e9ab0f764f9d7961924dac7fb1e5c04be02a38c`. This task closes the validated findings in `findings-validation.md` without changing the approved snapshot, source-trust, or production review behavior.

## Diagnosis

### FIND-TASK-007-1 — Capacity benchmark input moves

AC-007, TASK-007 approach step 4, and `architecture/bohmian-code-review-design.md:38` require reproducible fixed-commit measurements. The sole live capacity route in `crates/bohmian-code-review-pipeline/examples/live_pr.rs:98` calls `ColdBuild::run`; `crates/bohmian-code-review-pipeline/src/snapshot.rs:621-638` obtains the repository's *current* default-branch head. The three siblings are `evidence/TASK-007/s20-wyrd-pr1.json`, `bench-vscode.json`, and `bench-rust.json`, plus their capacity rows and reproducibility note in TASK-007 lines 89–95. Each report records a SHA only after the run. TASK-007:95 explicitly says the row can be repeated only while that SHA remains default head. Once the branch moves, the documented command samples a different tree, so its cold time, warm p50/p95, peak memory, snapshot size and API calls are not reproducible at the named commit. The existing reports prove complete snapshots and warm reuse at the time observed, but do not close the fixed-commit obligation.

**Outcome and recommendation:** Make the opt-in capacity path accept an immutable commit chosen before measurement and pass it through the existing verified cold-ingest owner. Preserve the ordinary default-branch `ColdBuild::run` and PR-context behavior. Reuse the existing GitHub identity, commit/tree and blob checks; a separate archive-only timing path would measure the wrong workflow. Record exact rerunnable commands and commit/tree identities with all required metrics for the Wyrd, VS Code and Rust size classes. The candidate owns the benchmark entrypoint shape; no new persisted snapshot fields or aggregate counts are needed. A focused fake-provider proof must advance default head between repeated capacity calls and show the requested SHA remains fixed while the normal cold facade follows the new head.

### FIND-TASK-007-2 — Controlled-provider contract evidence is missing

REQ-017, AC-006 and TASK-007 approach step 4 require a separate credentialed, opt-in contract run against a controlled real GitHub repository. TASK-007:86 presents public S20/capacity observations and an informal `gh api -i` header description as a PASS. Its sibling evidence is all four S20-shaped JSON reports, `crates/bohmian-code-review-pipeline/examples/live_pr.rs:86-118`, and the `live:pr` lane in `mise.toml:65-69`. None records controlled contract cases, exact request/response statuses or captured conditional/rate outcomes. The task record explicitly says no live 403/429 occurred and no conditional request is sent. Local fake tests cover rate and GHES paths but cannot themselves prove the real-provider contract. Thus S20 success does not establish the separate AC-006 gate.

**Outcome and recommendation:** Add or use one explicitly invoked read-only controlled-provider contract command and preserve a compact case report with exact invocation, controlled repository and immutable refs, observed response statuses/headers, and outcomes for archive redirect/shape, recursive tree, blob and PR patch omission. Exercise production GitHub transport for those live cases. Capture a real conditional response against the controlled repository separately, since this client sends no conditional request; capture real rate-limit headers on normal responses, and use the existing fake tests to prove the client's 403/429 and GHES branches. State these separate proof boundaries in the report. Do not deliberately exhaust a production rate quota. Keep the command outside the credential-free gate and separate from S20.

## Preserved behavior and non-goals

Keep the four independent production facades, verified Git identity and blob inputs, complete source-free per-path snapshot outcomes, stateless updates, private PR overlays, honest coverage, bounded packing, and context read-back. Do not add a consumer service, server, worker, clone, credentialed normal-CI lane, durable source archive, or new benchmark persistence state. Do not rewrite a golden, weaken a gate, or relabel an incomplete run as complete.

## Acceptance and proof

| Finding and siblings | Closure evidence |
|---|---|
| `FIND-TASK-007-1`: Wyrd, VS Code, Rust reports and TASK-007 capacity rows | Exact commands select immutable commits; reports verify those commits and trees and include archive bytes, cold time, memory peak, snapshot size, warm p50/p95 and API calls. A fake-provider head-advance check proves fixed-SHA selection and ordinary default-head behavior. |
| `FIND-TASK-007-2`: provider-contract row, four live reports, `live_pr.rs`, `mise.toml` | Controlled real-GitHub case report identifies the repository, refs, invocation, requests, statuses/headers and observed results for each named AC-006 case. It captures a real conditional response and rate headers, identifies those as provider-contract observations, and records passing local fake client tests for 403/429 and GHES. |

Run the exact new focused checks, `mise run test:pipeline:journey`, the narrowest pipeline owner lane, `mise run fmt:check`, `mise run lints`, and `mise run gate`. Keep credentialed runs explicitly invoked and report unavailable live conditions as limits, never as passes. The remediation write set includes the benchmark/contract entrypoint and its owning source or tests where the verified input route requires them, the scoped `mise.toml` lane if needed, `changes/active/repo-parser/evidence/TASK-007/`, and the TASK-007 evidence record. Implement once at each root-cause owner, then review the original task against the full cumulative candidate.
