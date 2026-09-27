# Domain review: budget safety and coverage honesty

Subject: base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`, candidate `e68bcdb65b44095989b5f42dfda5c7d87295582b`.

## Boundary and coverage

Reviewed the complete task and approved revision 12 spec; the PR facade's pinning, scoped fallback, changed-head processing, ranking, candidate packing, and budget output; the shared context types; existing ingest classification and fallback; symbolic candidate packing; and the focused PR tests. Applicable obligations: REQ-008–010, REQ-013, INV-002, INV-006, and `architecture/agent-rules.md` input and omission rules. `mise run test:pipeline` passed. The zero-allowance path returns an empty context with `budget.exhausted`; positive allowances use the symbolic packer, which reports file and token omissions. Candidate fetch failures become `not_analyzed`, and full coverage requires repository scope plus complete snapshot, overlay, and PR listing.

## Proposed finding BUDGET-1 — FAIL

- Classification: INCORRECT. Defect class: scoped file ceiling is applied before eligibility classification. Root cause is `near`: it filters only Git modes, puts changed paths first, then takes `max`. `Ingest::new` classifies only that already truncated list. The owning shape is a selection helper whose input lacks the root attributes and path eligibility decision needed to spend the ceiling on eligible files.
- Violated obligation: REQ-008 requires the bounded scoped index to cover eligible files near changed paths and report scoped counts and omissions. This also weakens the task's S10 useful-scoped-context outcome.
- Reachable path: with no compatible snapshot, a changed `src/handler.ts`, 299 unsupported `src/*.md` entries before `src/user.ts` in the tree listing, and the default `max_scoped_files = 300`, `near` returns the changed file and 299 unsupported files. `classify` excludes those markdown entries after the ceiling has been spent. `src/user.ts` is never indexed, even if it declares the only matching symbol. The result reports `Scope::Scoped` and one eligible/indexed file, with no skip or omission for `src/user.ts`. A snapshot cannot repair this because this is precisely the no-snapshot path.
- Sibling sweep: the only scoped selection call is `PrContext::run` into `near`; repository-scope `Ingest::new` receives the whole tree, and `Ingest::fallback` applies its fetch bound after classification. Generated, vendored, oversize, and unsupported blobs all consume scoped slots by the same mechanism.
- Testable correction: apply the scoped file ceiling to entries after the existing eligibility rules, preserving changed-path priority and the request bound; ensure eligible near files are considered until the ceiling, and report when eligible near files are left out. A focused fake-provider test should place enough ineligible neighbors before a matching eligible candidate to fill the old raw-entry ceiling and show that the candidate is indexed and ranked.

## Verification limits and result

The existing S10 fixture has only eligible nearby source files, so it does not exercise the finding. This is a source-traced reachable path, not a new executable reproduction. I found no separate material budget defect. Overall **FAIL** on BUDGET-1.

```citations
[{"id":"BUDGET-1","path":"crates/bohmian-code-review-pipeline/src/pull_request.rs","line":547,"snippet":"        .filter(|entry| matches!(entry.mode, TreeEntryMode::File | TreeEntryMode::Executable))","symbols":["near"]},{"id":"BUDGET-1","path":"crates/bohmian-code-review-pipeline/src/pull_request.rs","line":551,"snippet":"    picked.into_iter().take(max).cloned().collect()","symbols":["near"]},{"id":"BUDGET-1","path":"crates/bohmian-code-review-pipeline/src/pull_request.rs","line":188,"snippet":"                    Ingest::new(&near, 0, &attributes, limits.max_file_bytes),","symbols":["PrContext::run","Ingest::new"]},{"id":"BUDGET-1","path":"crates/bohmian-code-review-pipeline/src/eligibility.rs","line":252,"snippet":"        match exclusion(entry, attributes, context, max_file_bytes) {","symbols":["classify"]}]
```
