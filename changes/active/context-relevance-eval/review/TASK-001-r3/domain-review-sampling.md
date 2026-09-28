# TASK-001 round 3: sampling domain review

Subject: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab` → `c88bea58b62df3854b28a082fc7dd969ed8cfd6a`.

## Boundary and coverage

I reviewed the complete cumulative example diff against approved spec revision 4 (REQ-002–003, INV-001, INV-003, AC-005), original TASK-001, both remediation tasks, `AGENTS.md`, `architecture/agent-rules.md`, and the pipeline ownership and source-free snapshot rules in `architecture/bohmian-code-review-design.md`. I traced `Api::select` through direct GitHub discovery, `Discovery::offer`/`settled`, `Api::listing`, `Listing::push`, `Screen::new`/`qualifies`, and `Api::base_snapshot`. I also inspected the provider's `pull_request_files` parser and `IndexedFile`'s nonempty-declarations invariant.

The discovery holds merged PRs until the page's oldest update time makes their merge-time order final, then sorts by merge time and PR number descending. Qualification reads one logical paginated file listing, screens Python/Rust paths and patches, and checks modified paths against declaration-bearing indexed base files. A missing or malformed patch contributes no changed lines and is recorded as an omission. A rejected or incomplete listing and an incomplete base snapshot stop sampling. `TooFew` prevents a successful sample shorter than 50. I found no material sampling or determinism defect in this path.

## Prior finding closure

- **FIND-TASK-001-1:** Closed. `Listing::push` counts accepted and rejected entries toward the 3,000-file ceiling, and `Screen::new` rejects truncation. The cap, advertised-next-page, and shorter-terminal-page cases are covered by `listing_fails_at_provider_limits`.
- **FIND-TASK-001-2:** Closed. The test module and patch fixture have rustdoc.
- **FIND-TASK-001-3:** Closed. `Listing::push` rejects repeated canonical paths across or within pages before `Screen::new` can count files. `listing_rejects_repeated_paths` covers differing status and patch plus distinct-path qualification.
- **FIND-TASK-001-4:** Closed. The shared `is_timestamp` validates Gregorian month/day and 24-hour clock fields for cutoff, update, and merge values. `discovery_rejects_impossible_timestamps` covers invalid values at all three ingress points, leap days, and tie ordering.
- **FIND-TASK-001-5:** Closed. `listing`, `base_snapshot`, `qualify`, and `select` are inherent methods of the concrete `Api` workflow owner; `run` and `main` remain entry points.

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::` passed all seven tests. `git diff --check` passed for the immutable range. This no-network review does not establish the two credentialed live samples or a same-cutoff rerun; the approved task assigns those to TASK-003. Pagination stability during concurrent GitHub updates is likewise assessed by that rerun rather than an in-run reconciliation.

**Proposed findings:** None. **Overall: PASS.**

```citations
[]
```
