# Wave 2 findings validation

Subject: `597f5180808b78e1b104c10c1a375d5604b89a2e..ef48e308838fd081561754059e78a3814f03cf8a` (`HEAD`), `SPEC-repo-parser@14`, `TASK-009`.

## Validated ledger

**No retained blocking findings or notes.** The citation-checked claim ledger is empty. `TREE-1` was rejected by the citation check because three cited symbols did not match the candidate; it is omitted rather than recast as advice. Independent source inspection found no disagreement requiring its restoration.

| Obligation | Independent validation | Result |
|---|---|---|
| REQ-005; truncated root and nested trees produce the complete listing | `GitHubClient::tree` delegates to `list_into`, which replaces each truncated recursive result with its non-recursive entries, prefixes subtree paths, and recursively lists each subtree (`github.rs:995-1079`). The cold-build test compares published coverage and snapshot bytes with an untruncated oracle (`snapshot.rs:2573-2642`). | PASS |
| INV-002, AC-004; unsplittable listing never publishes as complete | A truncated non-recursive listing sets `TreeListing::truncated` (`github.rs:1031-1034`); `ColdBuild::start` returns an incomplete result before ingest or publication (`snapshot.rs:675-704`). The test checks the incomplete reason, no object, and unchanged prior pointer (`snapshot.rs:2644-2682`). | PASS |
| REQ-008; truncation does not itself select scoped PR fallback | `PrContext::run` loads the snapshot after listing and selects scoped ingest only when no snapshot is available (`pull_request.rs:163-205`). The resulting overlay carries `listing.truncated` into coverage (`pull_request.rs:240-250`). | PASS |
| Constraint; Git tree listings only | `tree_wire` uses the existing `Operation::Tree` endpoint with or without `?recursive=1` (`github.rs:1062-1079`); the diff adds no provider endpoint or clone path. | PASS |

Focused reproduction: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib truncated` passed (2 tests, 0 failed). `git diff --check` passed. The complete base-to-candidate diff was inspected. No candidate source was changed.

Verification limit: this validation did not run a live large-repository GitHub request. The `rust-lang/rust` capacity rerun remains assigned to TASK-007; the in-memory provider tests establish the TASK-009 behavior but do not measure live call count or provider latency.

Recommendation to orchestrator: **PASS**, subject to its complete Wave 1 and repository verification accounting. No remediation task is warranted from the validated ledger.
