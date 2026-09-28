# Domain review: PR sample discovery and qualification

Subject: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab` → `c626450649edfdbea105011ab5293267df49ed74`. Reviewed the complete diff, approved `spec.md` revision 4, `TASK-001-pinned-sample.md`, `AGENTS.md`, `architecture/agent-rules.md`, the design and spec-driven development reference, `sample.rs`, `main.rs`, and the provider, snapshot, and patch-parser consumers they call.

## Boundary and evidence

`select` reads closed PR pages in descending update order, `Discovery` settles merge order with PR-number ties, `qualify` reads each PR's paginated file listing once, `Screen` filters public language eligibility and classifies missing or malformed patches, and `ColdBuild::run_at` supplies the base snapshot whose coverage is checked before qualification. The source-file count, modified-file declaration condition, and other-file changed-line condition match REQ-002–REQ-003. The provider file-listing page limit advertises incompleteness through `has_next`; the command fails when page 30 still advertises another page. The focused command `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::` passed 4 tests.

## Proposed finding

### DOMAIN-SAMPLE-1 — Invalid calendar timestamps pass discovery

- **Classification / defect class:** INCORRECT; timestamps are checked for character shape but never checked as real UTC dates and times. The owner is the shared `is_timestamp` validator in `sample.rs`; its boolean shape permits the defect.
- **Siblings:** `Discovery::new` accepts a shape-valid impossible cutoff; `Discovery::offer` accepts shape-valid impossible `updated_at` and `merged_at`. These all use the same validator. No other timestamp producer is introduced by the candidate.
- **Violated obligation:** REQ-002 requires a fixed cutoff date and ordered merge timestamps; it says an unusable merge timestamp fails the run. The task repeats that failure boundary. `AGENTS.md` requires validation at trust boundaries.
- **Evidence / reproduction:** `is_timestamp("2026-99-99T99:99:99Z")` evaluates true because every `d` position is only tested with `is_ascii_digit`. `Discovery::new` therefore accepts that cutoff, and `Discovery::offer` accepts the same value as a listed update or merge time. The latter enters `pending` if its branch and cutoff comparison permit it; `settled` then orders it lexicographically as though it were a real merge date. The existing unusable-input test covers only a shape-invalid value (`2026-01-09`).
- **Observable consequence:** an impossible user cutoff or provider merge time can produce a successful but invalid sample instead of failing the run.
- **Testable correction:** validate a real UTC calendar timestamp at `is_timestamp`, preserving the canonical string format used for ordering. Prove impossible month, day, hour, minute, and second values fail for cutoff and listed PR input while valid timestamps still settle in the required order.

## Verification limits

Live GitHub completeness and rerun equivalence belong to TASK-003 and were not available in this review. Offset pagination can skip an entry if PR update times move while pages are read; the approved spec explicitly uses listed metadata as run input and disallows a second PR-context or reconciliation workflow, so this is recorded as a limit rather than a separate proposed finding. No production code or provider fixtures were changed.

Overall: **FAIL** (DOMAIN-SAMPLE-1).

```citations
[{"id":"DOMAIN-SAMPLE-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":201,"snippet":"fn is_timestamp(text: &str) -> bool {","symbols":["is_timestamp","Discovery::new","Discovery::offer"]},{"id":"DOMAIN-SAMPLE-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":204,"snippet":"        && text.bytes().zip(SHAPE).all(|(byte, &shape)| match shape {","symbols":["is_timestamp"]}]
```
