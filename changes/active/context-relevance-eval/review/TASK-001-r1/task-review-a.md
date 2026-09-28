# Task review A — TASK-001 pinned sample

Subject: base `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab`, candidate `c626450649edfdbea105011ab5293267df49ed74`. Reviewed the complete diff and the approved revision 4 spec. The focused no-network test was rerun: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::` (4 passed).

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001: two named repositories via GitHub API | `main.rs` repository list, `Api::get`, `GitHubClient` | Source inspection; live run belongs to TASK-003 | PASS |
| REQ-002: 50 most recently merged default-branch PRs before cutoff, deterministic tie order | `Discovery::offer` and `settled`, `select` | `discovery_orders_and_settles_merged_prs` | PASS |
| REQ-002: one complete file listing per PR; reject incomplete listing | `listing` follows pages; `Screen::new` rejects `truncated` | `screen_fails_on_rejected_or_incomplete_listing`; see TASK-A-1 | FAIL |
| REQ-002: 2–15 eligible source files; another file has changed lines; patch omission stays local | `Screen::new`, `plausible`, `qualifies` | `screen_qualifies_and_records_omissions` | PASS |
| REQ-003: modified file with base declarations is withholdable | `Screen::qualifies`, snapshot indexed paths | `screen_qualifies_and_records_omissions` | PASS |
| REQ-007: credentialed opt-in read-only lane outside gate | `main.rs` environment and client; `mise.toml` live task | Source and task graph inspection | PASS |
| INV-001 and AC-005: production APIs and behavior unchanged; no `PrContext`, second listing, or replay | Diff confined to example, manifest, task, and mise lane | Complete diff inspection | PASS |
| INV-003: fixed cutoff and source inputs support reruns | Cutoff validation, ordered discovery, pinned base and head commits | Ordering test; actual rerun comparison belongs to TASK-003 | PASS |
| INV-004 and INV-005: source-free output, in-memory and read-only | Output JSON carries paths, commits, and omission reasons; `MemoryStorage` and GET calls | Source inspection | PASS |
| Task scope: no trial blobs or new public contract | Only sampler/example added | Complete diff inspection | PASS |

## Finding

**TASK-A-1 — INCORRECT. Defect class: provider ceiling is not treated as incomplete when the terminal page lacks `next`.** Root cause: `listing` marks a PR incomplete only if page 30 advertises another page. GitHub's PR-files endpoint returns at most 3,000 files, so a PR with more files can return 30 full pages and no next link. The method then hands a partial listing to `Screen::new` as complete. This violates REQ-002, TASK-001 Scenario 2, and its acceptance criterion requiring an incomplete listing to fail the run. Observable consequence: a qualifying subset of a larger PR could be sampled, or a truncated PR could be silently rejected as nonqualifying, instead of failing the run. The owner is `listing`; the only downstream consumer is `Screen::new`, which trusts its `truncated` flag. No sibling pagination loop exists in the candidate. Correction: conservatively mark a listing incomplete when the page limit is reached with a full final page, including when `has_next` is absent. Add one focused no-network check of the terminal-page boundary, preserving one logical listing and avoiding a second provider workflow. GitHub endpoint limit: [List pull request files](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests-files).

Overall: **FAIL**.

```citations
[{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":105,"snippet":"        listing.truncated = read.has_next && page == MAX_FILE_PAGES;","symbols":["listing","MAX_FILE_PAGES"]}]
```
