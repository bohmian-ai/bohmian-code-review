---
id: TASK-001-R1
kind: remediation
status: proposed
spec: SPEC-context-relevance-eval
spec_revision: 4
requirements: [REQ-002, AC-005]
depends_on: [TASK-001]
parent_task: TASK-001
remediates: [FIND-TASK-001-1, FIND-TASK-001-2]
---

# Close TASK-001 listing-completeness and rustdoc findings

Approved spec: `changes/active/context-relevance-eval/spec.md` revision 4. Original task: `changes/active/context-relevance-eval/tasks/TASK-001-pinned-sample.md`. Reviewed base: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab`; cumulative candidate: `c626450649edfdbea105011ab5293267df49ed74`. Implement against the approved behavior, then review the complete base-to-new-candidate range.

## Diagnosis and correction

**FIND-TASK-001-1 — incomplete PR listing can appear complete.** The root-cause owner is `listing` in `crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs:95-110`. Its terminal decision at line 105 is `listing.truncated = read.has_next && page == MAX_FILE_PAGES;`. The GitHub PR-files API [returns at most 3,000 files](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests-files), while this command requests 100 per page for up to 30 pages. A full final page without `next` therefore cannot prove the PR's listing ended. The current flag can be false; `Screen::new` at `sample.rs:262-300` trusts that flag, and `qualify` and `select` can accept a partial PR into the 50-PR sample. This violates REQ-002 and TASK-001's explicit run failure for an incomplete listing. The existing test covers `truncated: true`, but never this provider-ceiling input. No sibling pagination loop exists in the candidate.

**Required outcome:** Treat a listing that reaches the provider's 3,000-file ceiling as incomplete, whether or not the final response advertises another page. Count both accepted and rejected entries toward the ceiling. Reuse the existing `Listing::truncated` to `Screen::new` to `SampleError::IncompleteListing` path at the sampler's `listing` owner. Keep the existing next-page failure and one logical PR file listing. An exactly 3,000-file response also fails closed because this endpoint cannot establish that it is complete. This local decision closes the provider-ceiling class without a second listing, `PrContext`, replay, or production-client change.

**FIND-TASK-001-2 — two new test items lack required rustdoc.** The new inline `mod tests` at `crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs:323` and its `PATCH` fixture at line 362 have no rustdoc. They are the two sibling locations for this rule violation. `AGENTS.md:32` and `architecture/agent-rules.md:11` require useful rustdoc on every new Rust item, including private tests. Format and lint passes cannot prove this editorial requirement. Add concise item rustdoc stating the test module's qualification contract and the fixture's one-line head-side change. Preserve the tests and fixture behavior.

## Constraints and acceptance

Write set: the opt-in example's `main.rs` and `sample.rs`, plus this task's implementation evidence if recorded. Preserve sample ordering and size rules, patch omission classification, source-free output, read-only in-memory behavior, and existing provider/storage APIs. Do not fetch trial source blobs, add a provider abstraction, change persisted formats, alter the gate, or widen this correction to production ranking or PR-context behavior.

1. A focused no-network check demonstrates that a 30-page, 3,000-entry listing with no final `next` yields `IncompleteListing`; accepted and rejected entries both contribute to the ceiling. A shorter terminal page remains complete, and an advertised next page still yields `IncompleteListing`.
2. Final source inspection shows useful rustdoc immediately preceding both `sample.rs` items at the two sibling locations.
3. Run the exact focused sampler test command, `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Record the focused RED failure, GREEN result, and final commands. The credentialed live sample and reproducibility comparison remain TASK-003 obligations.

Route this remediation directly to `$cr-implement`.
