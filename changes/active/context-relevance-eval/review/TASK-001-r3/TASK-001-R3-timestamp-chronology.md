---
id: TASK-001-R3
kind: remediation
status: ready
spec: SPEC-context-relevance-eval
spec_revision: 4
requirements: [REQ-002, INV-003]
depends_on: [TASK-001, TASK-001-R1, TASK-001-R2]
parent_task: TASK-001
remediates: [FIND-TASK-001-6]
---

# Reject inconsistent PR timestamp pairs

Approved spec: `changes/active/context-relevance-eval/spec.md` revision 4. Original task: `changes/active/context-relevance-eval/tasks/TASK-001-pinned-sample.md`. Prior remediation tasks: `review/TASK-001-r1/TASK-001-R1-listing-completeness-and-rustdoc.md` and `review/TASK-001-r2/TASK-001-R2-input-integrity-and-workflow-owner.md`. Reviewed base: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab`; cumulative candidate: `c88bea58b62df3854b28a082fc7dd969ed8cfd6a`.

## Diagnosis and correction

**FIND-TASK-001-6 — valid instants form an impossible PR chronology.** `Discovery::offer` in `crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs:160-188` validates `updated_at` and `merged_at` separately but accepts a merged PR whose merge time is later than its update time. The defect class is an unchecked relationship between trusted provider fields. The module's settlement invariant at `sample.rs:3-5` requires `merged_at <= updated_at`. Sibling effects are `Discovery::settled` at `sample.rs:194-201`, which releases PRs against a page's oldest update time, and `Api::select` at `examples/context_relevance/main.rs:219-239`, which qualifies released PRs and may return as soon as 50 are collected. `Api::get` at `main.rs:80-94` supplies the decoded timestamps; the `listed` fixture at `sample.rs:383-395` supplies them in tests. No second producer or selection sink exists in this command.

With cutoff `2026-01-10T00:00:00Z`, a merged PR updated at `2026-01-05T00:00:00Z` and merged at `2026-01-06T00:00:00Z` passes both instant checks and enters `pending`; `settled(None)` releases it. Thus contradictory provider input can enter the sample instead of failing the run, contrary to INV-003, and the REQ-002 ordering proof lacks its required premise. The seven existing focused sampler tests pass because they check timestamp validity and ordering separately, not this relationship.

At the existing `Discovery::offer` input boundary, reject a merged PR when its merge time exceeds its update time through the existing `SampleError::UnusablePr` path. Apply the rule before a contradictory merged record can be filtered or queued. This keeps one provider listing and one discovery owner; no new timestamp parser, provider request, reconciliation layer, or public contract is needed. Equal timestamps and an update after merge remain valid. Preserve unmerged PR handling, cutoff and branch filtering, deterministic ordering, pagination, the R1 file ceiling, the R2 path and instant validation, and the concrete `Api` workflow owner.

## Acceptance and proof

1. A focused no-network test passes individually valid UTC instants with `merged_at > updated_at` to `Discovery::offer` and observes `SampleError::UnusablePr` before the PR reaches selection. It also covers equal timestamps and `merged_at < updated_at`, retaining the existing settlement order. This closes the `offer`, `settled`, and `Api::select` sibling path at the shared ingress.
2. Record the exact focused RED and GREEN commands and outcomes using `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features` with a selector that runs at least one test. Preserve the seven existing sampler tests, including the R1 cap and R2 duplicate-path and Gregorian-date cases.
3. Run `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Inspect the cumulative diff for unchanged production crates, public APIs, persisted formats, gate, `PrContext`, second listing, provider replay, and trial source blobs.

Write set: `crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs` and this task's implementation evidence. The correction belongs in the existing discovery owner; no production behavior or additional evaluation scope is requested. Route directly to `$cr-implement`, then review the full cumulative candidate against the original task.

## Implementation evidence

`Discovery::offer` in `sample.rs` now rejects a merged PR whose valid merge time is later than its valid update time through `SampleError::UnusablePr`, after both instants are validated and before branch/cutoff filtering or queuing. Equal times and update-after-merge remain accepted.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| 1. `merged_at > updated_at` fails with `UnusablePr` before selection; equal and earlier merge times settle in order | `Discovery::offer` in `sample.rs` | `sample::tests::discovery_rejects_merge_after_update` | PASS |
| 2. Focused RED/GREEN recorded; seven prior sampler tests preserved | — | RED/GREEN below; `sample::` 8 passed | PASS |
| 3. Pipeline, format, lint, diff checks; cumulative diff audit | — | Commands below | PASS |

Focused: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features <selector>`. RED (`discovery_rejects_merge_after_update`): 1 failed, 7 filtered out — `offer` accepted PR #14 updated `2026-01-05T00:00:00Z`, merged `2026-01-06T00:00:00Z`. GREEN (`sample::`): 8 passed, including `listing_fails_at_provider_limits` (R1 cap), `listing_rejects_repeated_paths`, and `discovery_rejects_impossible_timestamps` (R2). `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `git diff --check`, and `mise run gate` exited 0.

Non-goals held: R3 write set is `sample.rs` and this file. Cumulative diff from `e665bc1` touches only the `context_relevance` example, the pipeline `Cargo.toml` example entry, `mise.toml` live task, and change-packet docs; no production crate source, public API, persisted format, gate, `PrContext`, second listing, provider replay, or trial source blobs.
