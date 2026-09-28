# TASK-001 round 4 — independent task review A

Subject: base `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab`, candidate `4d7115bbca74e7a4c27a16c4183f8eb5d754ae04`. Authority: approved `spec.md` revision 4, original TASK-001, and R1–R3 remediation tasks. I inspected the complete base-to-candidate diff and the surrounding provider, snapshot, extractor, and patch-parser contracts. HEAD matched the candidate during review.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001, REQ-007: both named repositories, credentialed read-only opt-in command outside gate | `main.rs:37-42,256-305`; `mise.toml:76-78` | Task graph and complete diff inspection | PASS |
| REQ-002: 50 recent qualifying merged PRs before the fixed cutoff, ordered by merge time then PR number; unusable identity or time fails | `sample.rs:133-209`; `main.rs:206-248` | Focused discovery tests passed; source trace of page settlement and `TooFew` path | PASS |
| REQ-002–003: one complete file listing, 2–15 eligible source files, modified indexed base file, another file with changed lines | `main.rs:102-117,165-196`; `sample.rs:241-279,314-379`; indexed snapshot records always have declarations (`snapshot.rs:178-224`) | Listing, screening, cap, and duplicate tests passed | PASS |
| REQ-002, INV-003: missing or malformed patch is an omission; rejected entry, incomplete listing or snapshot fails the run | `sample.rs:263-279,322-360`; `main.rs:126-156` | Omission and failure tests passed; complete-snapshot branch traced | PASS |
| INV-001, AC-005: existing fixed-commit snapshot and public path query, with no `PrContext`, second listing, replay, trial blobs, production API or gate change | `main.rs:102-196`; `sample.rs:314-379`; diff adds only private example, its manifest entry, live task, and change records | Complete diff and call-path inspection | PASS |
| INV-004–005: source-free output and in-memory read-only operation | `main.rs:120-157,270-295`; `sample.rs:296-312`; patch parser errors carry line numbers, not patch text | Source inspection | PASS |
| R1–R2 findings: 3,000-entry listing ceiling, rustdoc, duplicate paths, Gregorian instants, concrete IO owner | `sample.rs:211-279,382-425,625-714`; `main.rs:61-249` | Relevant focused tests passed; source inspection | PASS |
| R3 / FIND-TASK-001-6: reject merge time later than update time before filtering or settlement, retaining equal and later update times | `sample.rs:160-209,716-739`; `main.rs:219-239` | `discovery_rejects_merge_after_update` passed; check precedes branch/cutoff filter | PASS |

The exact focused command `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::` passed all eight tests. `git diff --check` passed. I did not rerun the full gate or use credentials for live sampling; those are not acceptance gates for this setup task, and the two-run comparison and checked-in reports belong to later tasks.

**Proposed findings:** none. The candidate closes prior FIND-TASK-001-1 through -6 without changing production behavior or adding speculative layers.

**Overall: PASS.**

```citations
[]
```
