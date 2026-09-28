# Sampling domain review

Subject: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab` → `a1342a5a3c154b7678cf59764dab1ceb0d70cc76`. Boundary: PR discovery order, one paginated file listing, source-file qualification, and base-snapshot completeness in the opt-in evaluator.

## Authority and source coverage

| Authority | Reviewed source and result |
|---|---|
| Approved spec revision 4: REQ-002–003, INV-001, INV-003, AC-005 | `sample.rs` discovery, listing, screening, tests; `main.rs` listing, snapshot, qualification, selection. One logical file listing and qualifying filters are present. Timestamp validation fails the unusable-input rule below. |
| Original TASK-001 and R1 remediation | The 3,000-entry ceiling, accepted plus rejected counting, advertised next-page failure, and shorter terminal page are covered by `Listing::push`, `Screen::new`, and `listing_fails_at_provider_limits`. R1's two missing rustdoc items are documented. |
| `AGENTS.md`, `architecture/agent-rules.md`, design and reference router | No production crate, public API, persisted format, or gate behavior changed in this domain. The sampler uses the public path-language query and fixed-commit `ColdBuild::run_at`; `ColdBuildOutcome::Incomplete` fails. |

The GitHub client's `pull_request_files` supplies accepted and rejected entries and `has_next`; the public `parse_hunks` supplies head-side changed ranges. I traced their consumers through `Listing::push`, `Screen::new`, `qualify`, and `select`. The focused no-network sampler command passed: 5 tests, 0 failures. Credentialed live sampling and two-run reproducibility remain outside TASK-001 and were not run.

## Proposed finding

**SAMPLING-1 — INCORRECT: timestamps are shape-checked, not date-checked.** Root cause and defect class: `is_timestamp` accepts any ASCII digits in date/time fields without validating their ranges or calendar day. Its three sibling uses are `Discovery::new` for the user cutoff and `Discovery::offer` for listed update and merge times. The raw `String` timestamp shape permits the defect. The approved spec's REQ-002 calls for a fixed cutoff date and requires an unusable merge timestamp to fail the run; TASK-001 scenario 2 also requires this failure. A cutoff such as `2026-13-01T00:00:00Z` passes `Discovery::new`, and a listed merge time such as `2026-02-31T00:00:00Z` passes `offer`; either can be ordered and sampled as if it were a real instant. The correction belongs in the shared timestamp validator: reject impossible calendar dates and clock fields for all three uses, preserving the lexically sortable UTC representation. Add a focused no-network assertion for invalid cutoff, update, and merge values, including a calendar-invalid day. This is the only candidate timestamp validator; the call-site sweep found no sibling implementation.

Overall: **FAIL** (SAMPLING-1). The R1 pagination and rustdoc findings appear closed.

```citations
 [{"id":"SAMPLING-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":209,"snippet":"    text.len() == SHAPE.len()","symbols":["is_timestamp","Discovery","offer"]}]
```
