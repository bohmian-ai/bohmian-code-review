# TASK-001 Wave 2 finding validation

Subject: base `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab`, candidate `c626450649edfdbea105011ab5293267df49ed74`. The complete diff adds the opt-in `context_relevance` example and tests, its Cargo target and mise lane, and task evidence. The approved authority is `changes/active/context-relevance-eval/spec.md` revision 4 and `tasks/TASK-001-pinned-sample.md`.

## Validated blocking ledger

### FIND-TASK-001-1 — Provider file cap can masquerade as a complete listing

- **Source ID/status:** `TASK-A-1` — CONFIRMED.
- **Gate/classification:** Task contract; INCORRECT. REQ-002 requires one *complete* PR file listing and failure on an incomplete listing; TASK-001 scenario 2 requires that failure before qualification.
- **Root cause/defect class:** `listing` decides completeness solely from the terminal page's `has_next` flag. GitHub's [List pull requests files documentation](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests-files) caps responses at 3,000 files. At that cap, absence of a next link does not prove that no more files belong to the PR. The candidate-owned `Listing::truncated` field permits a capped listing to be represented as complete.
- **Siblings:** None. `examples/context_relevance/main.rs:95-110` is the only producer of this listing and its completeness flag. `GitHubClient::pull_request_files` at `src/github.rs:1367-1405` supplies parsed entries and a Link-derived `has_next`, without a total count. `Screen::new` at `sample.rs:262-300` checks only `listing.truncated`; `qualify` at `main.rs:158-190` then selects or rejects the PR from that listing; `select` at `main.rs:199-247` can include it in the 50-PR sample.
- **Evidence/reproduction:** `main.rs:101-108` loops through 30 pages and sets `listing.truncated = read.has_next && page == MAX_FILE_PAGES`. The provider requests 100 files per page (`src/github.rs:38,1373-1375`). For 30 full pages with `has_next == false` on page 30, the expression is false despite reaching the 3,000-file ceiling. `Screen::new` then accepts the listing (`sample.rs:267-300`). This source-path reproduction requires no token or network; the provider cap is documented by GitHub.
- **Observable consequence:** A PR with more than 3,000 changed files can be screened from a partial file set and quietly counted as a qualifying PR, changing the sample and later trials. This violates the specified fail-on-incomplete behavior even if such PRs are uncommon.
- **Decision-complete correction:** At the `listing` owner, fail closed whenever the accumulated valid plus rejected entries reach the provider's 3,000-file cap, even if the final Link header lacks `next`. Keep the existing `has_next` check. Reuse `Listing::truncated` and `Screen::new`'s `IncompleteListing` error; do not add a second PR-context request, replay layer, or production-client change. An exactly 3,000-file response cannot establish completeness through this endpoint, so it must be treated as incomplete.
- **Focused closure proof:** A small no-network test of the listing completeness decision (or of a factored pure decision in this example) must show 30 full 100-entry pages with no final `next` produce `IncompleteListing`, while a shorter terminal page remains complete; retain the existing test for advertised truncation. Exercise valid plus rejected entries in the count so path rejection cannot hide the cap.

### FIND-TASK-001-2 — New test items omit required rustdoc

- **Source ID/status:** `STANDARDS-1` — CONFIRMED.
- **Gate/classification:** Repository standard; VIOLATION. `AGENTS.md:32` requires useful rustdoc for every new Rust item, including tests; `architecture/agent-rules.md:11` and `architecture/references/languages/rust-core.md:7` repeat the rule.
- **Root cause/defect class:** Two new Rust items were introduced without rustdoc. The owner is the inline test module in `examples/context_relevance/sample.rs`; its child fixture constant has the same violation.
- **Every sibling:** `sample.rs:323` (`mod tests {`) and `sample.rs:362` (`const PATCH: &str = "@@ -1 +1 @@\n-a\n+b";`). The test functions and helpers immediately around them carry rustdoc; these are the two missed items in the claim's class.
- **Evidence/reproduction:** `sample.rs:322-324` has `#[cfg(test)]` directly above `mod tests`, and `sample.rs:360-364` has no `///` above `PATCH`. Both lines are new in the base-to-candidate diff. The cited written rule applies to private items and tests, so the source inspection itself reproduces the violation.
- **Observable consequence:** The candidate fails the repository's documented Rust-item standard. Compiler, formatting, and Clippy success cannot establish this editorial rule.
- **Decision-complete correction:** Add short, useful `///` comments immediately above the test module and patch fixture explaining their role. Keep the inline tests and fixture; no test harness or code restructuring is needed.
- **Focused closure proof:** Inspect both item declarations and their preceding rustdoc in the final diff, then run `mise run fmt:check` and `mise run lints` for the touched Rust file.

The claims have different root causes and remain separate findings. Neither requires a spec revision or changes to production API or behavior.

## Citation rejects and notes

`TASK-B-1`, `SRC-1`, and `DOMAIN-SAMPLE-1` were rejected by the completed citation check and are dropped in full; they are not notes or blocking findings. No surviving claim was downgraded to a note. No prior `FIND-*` ledger applies to this first review round.

```citations
[{"id":"FIND-TASK-001-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":105,"snippet":"        listing.truncated = read.has_next && page == MAX_FILE_PAGES;","symbols":["listing"]},{"id":"FIND-TASK-001-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":323,"snippet":"mod tests {","symbols":["tests"]},{"id":"FIND-TASK-001-2","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":362,"snippet":"    const PATCH: &str = \"@@ -1 +1 @@\\n-a\\n+b\";","symbols":["PATCH"]}]
```
