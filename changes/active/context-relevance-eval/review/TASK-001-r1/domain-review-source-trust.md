# Source trust and sample completeness review

**Subject:** `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab..c626450649edfdbea105011ab5293267df49ed74`
**Result:** FAIL

## Boundary and coverage

Reviewed the complete candidate diff, approved revision 4 of `changes/active/context-relevance-eval/spec.md`, `TASK-001-pinned-sample.md`, `AGENTS.md`, `architecture/agent-rules.md`, and the relevant design and spec-driven references. Traced `select` through direct GitHub PR discovery, `Discovery::offer` and `settled`, the existing `GitHubClient::pull_request_files` validation and page signal, `Screen::new` and `qualifies`, and `ColdBuild::run_at` through coverage, publication, and snapshot read-back. The new command does not call `PrContext`, fetch a second PR file listing, or retain trial source blobs. The provider validates file paths, statuses, and SHAs before `Screen` sees them. A truncated file listing and incomplete cold build stop the run.

## Finding SRC-1 — merge timestamp validation accepts impossible dates

- **Classification:** INCORRECT; task-contract gate.
- **Root cause / defect class:** `is_timestamp` checks the 20-byte shape and digit positions, but does not validate a date and time. `Discovery::offer` then treats that shape check as proof that `merged_at` is usable for cutoff comparison and sample order. The same shape-only check also admits invalid `updated_at` and cutoff values; these are sibling sites of the timestamp-value defect.
- **Obligation:** Spec REQ-002 and task lines 19–21 require an unusable merge time to fail the run. Spec INV-003 requires deterministic sample selection from usable provider input.
- **Reachable reproduction:** A listed merged PR with `merged_at = "2026-99-99T99:99:99Z"`, valid SHA strings, matching base branch, and `updated_at = "2026-99-99T99:99:99Z"` passes `is_timestamp` and enters `pending`. With cutoff `"2027-01-01T00:00:00Z"`, it can be settled and qualified instead of failing. `select` passes every listed PR to `offer`, so this is the actual discovery input path. The focused test covers malformed *shape*, but no impossible date or time.
- **Observable consequence:** An unusable PR record can enter the 50-PR sample, violating the specified run-failure boundary and potentially changing selection order.
- **Testable correction:** Validate the actual calendar date and clock time for cutoff, update, and merge timestamps before ordering or admitting a PR. Keep the one-listing discovery path. A focused test should show an impossible merged timestamp fails and a valid timestamp retains current ordering.

## Verification limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::` passed all four existing sampler tests. This is a source-trace reproduction; I did not modify the immutable candidate or require credentials for a live run. Provider page movement across concurrent GitHub updates remains the task's documented live-run limit, with reproducibility checked by a second run in the later task.

```citations
[{"id":"SRC-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":169,"snippet":"        if !is_timestamp(&merged_at) {","symbols":["Discovery::offer","is_timestamp"]},{"id":"SRC-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":201,"snippet":"fn is_timestamp(text: &str) -> bool {","symbols":["is_timestamp"]},{"id":"SRC-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":203,"snippet":"    text.len() == SHAPE.len()","symbols":["is_timestamp"]},{"id":"SRC-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":163,"snippet":"        if !is_timestamp(&listed.updated_at) {","symbols":["Discovery::offer","is_timestamp"]},{"id":"SRC-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":135,"snippet":"        if !is_timestamp(cutoff) {","symbols":["Discovery::new","is_timestamp"]}]
```
