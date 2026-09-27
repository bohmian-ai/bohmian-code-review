# TASK-010 structured Ponytail validation

Subject: base `97fff9feaffa2ec46fef512f5f476133830f1ac3`; candidate `da81a598c10441b3b2e1231ce674d89003017f26`; approved `SPEC-repo-parser@14` (revision committed at `63d6bd0`); `TASK-010-compare-based-update.md`; maintainer decision `review/maintainer-decision-stateless-snapshot.md`.

## Claim ledger and decision

The citation-checked claim ledger is **empty**: task-review A, task-review B, repository standards, source trust, and lifecycle reported no findings. Independent inspection of the complete base-to-candidate diff, the approved task and spec, the maintainer decision, and the changed production paths found no additional material finding. **Validated blocking ledger: empty. Notes: none. Overall: PASS.** No remediation is warranted.

## Independent checks

- `GitHubClient::compare` validates the base and ahead head identities, maps added, removed, copied, changed and renamed paths, and detects the provider's 300-file ceiling (`github.rs:1153-1220`). `Update::run` takes that comparison from the pointer commit to the pinned head, routes non-ahead, ceiling, changed-blob-bound, and root `.gitattributes` cases to archive build, and otherwise lists only changed paths' parent directories at the head (`update.rs:81-145`, `update.rs:172-258`). This meets the named REQ-007 triggers without a base listing.
- `Ingest::carry` drops prior outcomes for every compared removed path; changed or added eligible entries remain pending for verified blob fetch. Unchanged indexed and skipped outcomes carry over (`snapshot.rs:911-1000`, `snapshot.rs:1120-1165`). Incomplete fetches cannot reach `ColdBuild::publish`, and publication retains record, immutable object, then pointer order (`snapshot.rs:744-850`). The existing update tests exercise a fresh-build byte oracle, failed-blob retry, dropped update, moving head, version change, and archive triggers (`update.rs:365-816`).
- The snapshot holds only per-path outcomes; exclusion counts are reported from the current build's observed entries (`snapshot.rs:253-313`, `snapshot.rs:878-953`). This follows the maintainer's stateless-snapshot decision and revision 14. The changed code adds no dependency or extra persistence layer; `Directories` is the smallest shared owner needed to cache and verify non-recursive ancestor listings for changed paths. No candidate-owned abstraction or test machinery has a supported deletion that preserves the required behavior.
- Fresh read-only verification: `mise run test:pipeline` exited 0; `mise run test:pipeline:journey` passed 2/2; `git diff --check 97fff9f da81a59` exited 0. The candidate commit was unchanged during this validation.

## Verification limits

Provider behavior was exercised through fake GitHub responses, not a live GHES or github.com update. I did not rerun `fmt:check`, `lints`, or the full gate in this independent pass; the orchestrator should record their available evidence. There were no surviving claims to reproduce or merge, and no prior `FIND-*` ledger for this first review.
