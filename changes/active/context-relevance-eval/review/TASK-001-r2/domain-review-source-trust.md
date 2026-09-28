# Source trust and GitHub IO domain review

**Subject:** `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab..a1342a5a3c154b7678cf59764dab1ceb0d70cc76`
**Result:** FAIL

## Boundary and coverage

Reviewed the cumulative example and task diff against approved spec revision 4, original TASK-001, R1 remediation, `AGENTS.md`, `architecture/agent-rules.md`, and the applicable design rules. Traced direct GitHub discovery through `Discovery`, the one paged `GitHubClient::pull_request_files` listing through `Listing` and `Screen`, and the fixed-commit `ColdBuild::run_at` through complete coverage and snapshot read-back. The existing provider validates individual file paths, statuses, and SHAs. Indexed snapshot files carry at least one declaration. The candidate uses in-memory stores, performs read-only GitHub requests, keeps source-free output, and does not call `PrContext` or fetch trial blobs. R1's 3,000-entry ceiling and final-page-next checks close its stated listing limit case.

## SRC-R2-1 — duplicate PR-file paths are counted as distinct files

- **Classification / gate:** INCORRECT; REQ-002 and the task's run-failure rule for inconsistent provider input.
- **Root cause / defect class:** `Listing::push` appends valid entries without checking whether a canonical path was already seen. `Screen::new` keeps each entry, and `Screen::qualifies` distinguishes files by vector index rather than path. The missing uniqueness check is at the listing owner; the screening and qualification sites are consequences, not separate defects. No other PR-file listing fold exists in this candidate.
- **Obligation:** The sample needs 2–15 changed Python/Rust **files**, a withholdable file and **another** file with changed lines. Spec REQ-002 says a rejected file entry or incomplete listing fails the run, and INV-003 says provider inconsistency fails the run.
- **Reachable path:** Two pages can each contain the same individually valid `src/a.py` entry after provider pagination shifts. `GitHubClient::pull_request_files` validates each entry but does not reject repetition across calls. `Listing::push` appends both; `Screen::new` records both; with both `Modified`, valid patches, and a base declaration for `src/a.py`, `qualifies` returns true because the indices differ. The single physical source file has no other changed file. The same defect admits a duplicate within one page and can distort the 15-file limit.
- **Consequence:** An inconsistent listing can silently qualify a PR and alter the 50-PR sample instead of failing the run.
- **Testable correction:** At the listing fold, reject repeated canonical paths across the whole logical PR listing as a run-level error. A focused no-network check should feed the same path twice, including across page boundaries, and show that no qualifying sample results; distinct paths retain current behavior.

## SRC-R2-2 — shape-only timestamps are treated as usable instants

- **Classification / gate:** INCORRECT; REQ-002's unusable merge-time failure boundary.
- **Root cause / defect class:** `is_timestamp` checks only byte positions, so calendar-invalid dates and clock-invalid times pass. `Discovery::offer` and `Discovery::new` then use that predicate as validation. Sibling sites are cutoff, update time, and merge time; all use the same owner.
- **Obligation:** The task explicitly requires an unusable merge timestamp to fail the run, while REQ-002 orders by actual merge time.
- **Reachable path:** A merged PR with `merged_at = "2026-99-99T99:99:99Z"`, matching `updated_at`, valid commits and branch, and a later cutoff passes `is_timestamp`, enters `pending`, and can qualify. The existing test rejects malformed shape but does not cover impossible date or time values. This claim appeared in R1 but its citations were rejected for a nonexistent qualified symbol; the citations below use actual candidate symbols.
- **Consequence:** Unusable provider metadata can enter the sample and be ordered as if it represented an instant.
- **Testable correction:** Validate calendar date and clock time for all three timestamp inputs before comparison. A focused check should reject impossible merge, update, and cutoff instants while preserving order for valid instants.

## Verification limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::` passed 5 tests. Findings are reproduced by tracing reachable source paths; the immutable candidate was not edited. No credentialed live run was performed. A second-run sample comparison belongs to TASK-003. Provider pagination can shift during a live listing; the duplicate-path claim concerns the candidate's behavior if that happens, not a claim that GitHub currently returned such data.

```citations
[{"id":"SRC-R2-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":235,"snippet":"        self.files.extend(page.files);","symbols":["push","files"]},{"id":"SRC-R2-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":319,"snippet":"            screen.changed_lines.push(changed);\n            screen.source.push(file);","symbols":["new","source"]},{"id":"SRC-R2-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":334,"snippet":"        (MIN_SOURCE_FILES..=MAX_SOURCE_FILES).contains(&self.source.len())","symbols":["qualifies","MIN_SOURCE_FILES"]},{"id":"SRC-R2-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":338,"snippet":"                    && (0..self.source.len())\n                        .any(|other| other != withheld && self.changed_lines[other])","symbols":["qualifies","changed_lines"]},{"id":"SRC-R2-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":207,"snippet":"fn is_timestamp(text: &str) -> bool {","symbols":["is_timestamp"]},{"id":"SRC-R2-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":209,"snippet":"    text.len() == SHAPE.len()","symbols":["is_timestamp"]},{"id":"SRC-R2-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":175,"snippet":"        if !is_timestamp(&merged_at) {","symbols":["offer","is_timestamp"]},{"id":"SRC-R2-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":169,"snippet":"        if !is_timestamp(&listed.updated_at) {","symbols":["offer","is_timestamp"]},{"id":"SRC-R2-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":141,"snippet":"        if !is_timestamp(cutoff) {","symbols":["new","is_timestamp"]}]
```
