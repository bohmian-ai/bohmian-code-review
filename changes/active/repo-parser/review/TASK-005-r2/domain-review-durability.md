# Domain review: context durability and coverage

Subject: base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`, candidate `31e71a2e8867215e0c3f8958bdcee5f94da17ad4`. Boundary: `PrContext::run` through `Contexts::publish`, `read`, and `collect_expired`, injected `ContextStore`/`ObjectStorage`, and shared tree coverage through cold publication and PR overlay.

## Authority and source coverage

| Authority | Source traced | Result |
|---|---|---|
| Spec rev. 12 REQ-005/006/008/009, INV-002; TASK-005; TASK-005-R1 FIND-4 | `GitHubClient::tree` and `validate_tree_entry`; `Ingest::new`/`finish`; `Coverage::unindexed`; `ColdBuild::start`/`complete`; `PrContext::run` | One proposed finding below. Unknown rejected entries correctly prevent a full snapshot and make PR coverage partial. |
| Spec rev. 12 REQ-011–016, INV-005; TASK-005-R1 FIND-1/2 | `Contexts::publish`/`read`/`collect_expired`; `ContextRecord`, `ContextStore`; `MemoryContextStore`; `ObjectKey`; owner tests | Pass for the inspected lifecycle: record precedes object, `AlreadyExists` checks bytes, ID includes publication time, reads check exact identity/key/digest/expiry, cleanup deletes object before record. |
| `AGENTS.md` and `architecture/agent-rules.md` source and persistence boundaries | Outer pipeline modules above, public symbolic facade calls | No additional material violation found in this boundary. |

## Proposed finding

**DOMAIN-DUR-1 — INCORRECT; rejection count loses proven ineligibility.** `ColdBuild::start` passes every rejected listing entry as an undifferentiated count to `Ingest::new` (`snapshot.rs:593–598`); `Ingest::finish` marks all of them `InvalidEntry`, and `Coverage::unindexed` counts each as missing. The same `Ingest::new` count path is used for a PR head overlay (`pull_request.rs:170–175`). Yet `validate_tree_entry` can reject an entry solely for invalid SHA after its canonical path and mode have validated (`github.rs:1381–1392`). For a `README.md` blob with valid path and mode but invalid SHA, the existing path-language check proves it unsupported (`eligibility.rs:277–280`), so it cannot hide an eligible symbolic candidate. Nevertheless `ColdBuild::complete` returns `Incomplete` before publication (`snapshot.rs:654–675`), and every retry of that unchanged tree repeats the result; `PrContext::run` falls to scoped coverage without a snapshot (`pull_request.rs:164–184`). This is the specific overreach: unknown rejected entries must stay incomplete under REQ-006/INV-002, while entries proven ineligible by validated facts are allowed by TASK-005-R1 FIND-4 and REQ-005's eligibility rule. The defect class is **rejection metadata reduced to a count before eligibility classification**; the owner's shape permits no distinction and also drops each rejected tree path/reason from final coverage diagnostics. Sibling sinks are cold snapshot publication and PR overlay completeness.

**Testable correction:** At the shared tree-ingest boundary, retain enough validated rejection facts to distinguish entries whose ineligibility is certain from those that may be eligible. Keep unknown entries incomplete; classify a valid, path-only unsupported rejected entry as benign exclusion. Preserve the rejection reason in the relevant coverage report. Prove an invalid-SHA `README.md` alongside a valid source file still publishes a complete snapshot and yields full PR overlay coverage, while an invalid-SHA `src/file.ts` keeps both incomplete. No provider or storage trait expansion is needed.

## Verification limits

This is a source-traced review of the immutable cumulative candidate. Existing `rejected_tree_entries_are_counted_as_invalid_exclusions` and `rejected_head_tree_entry_is_not_full_coverage` exercise potentially eligible invalid entries. There is no focused case for a rejected but provably unsupported entry, and I did not modify source or tests. I did not independently rerun the broad gates; their implementation records report green results.

Overall: **FAIL** (one material proposed finding; final disposition belongs to the independent validation wave).

```citations
[{"id":"DOMAIN-DUR-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":593,"snippet":"        Ok(Ok(Ingest::new(","symbols":["start","Ingest::new"]},{"id":"DOMAIN-DUR-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":1015,"snippet":"        if self.rejected > 0 {","symbols":["finish","unindexed"]},{"id":"DOMAIN-DUR-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1382,"snippet":"    let path =","symbols":["validate_tree_entry"]},{"id":"DOMAIN-DUR-1","path":"crates/bohmian-code-review-pipeline/src/eligibility.rs","line":278,"snippet":"    if context.path_language(path).ok().flatten().is_none() {","symbols":["exclusion"]},{"id":"DOMAIN-DUR-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":654,"snippet":"        if !coverage.is_complete() {","symbols":["complete"]},{"id":"DOMAIN-DUR-1","path":"crates/bohmian-code-review-pipeline/src/pull_request.rs","line":170,"snippet":"            Some(_) => Ingest::new(","symbols":["PrContext::run","Ingest::new"]}]
```
