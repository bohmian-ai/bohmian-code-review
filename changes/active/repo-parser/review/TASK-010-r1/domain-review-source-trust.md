# TASK-010 source trust and snapshot provenance review

Subject: base `97fff9f`, candidate `da81a59`; approved `SPEC-repo-parser@14` at `63d6bd0`, `TASK-010`, and `review/maintainer-decision-stateless-snapshot.md`.

## Boundary and authority

Reviewed the provider compare response, changed-path validation, head tree mode lookup, verified blob ingest, archive rebuild triggers, and source-free snapshot carry/publication. The governing contracts are `spec.md` REQ-005–007, REQ-012–014, INV-003, AC-003; the maintainer's stateless-snapshot decision; `AGENTS.md` product boundary and Rust rules; `architecture/bohmian-code-review-design.md` pipeline and source-lifetime boundaries; and `architecture/agent-rules.md` input validation and source-retention rules.

## Source coverage

- `github.rs`: `Comparison`, `GitHubClient::compare`, `GitHubClient::directory`, `tree_wire`, `validate_tree_entry`, and `GitHubClient::blob`. Compare validates its base and, when ahead, its head; the 300-file ceiling marks the result truncated. Changed paths become canonical `RelativePath`s before update uses them. Directory listings verify tree IDs and supply actual modes and blob IDs. Blob responses are byte bounded and checked against their Git blob SHA.
- `update.rs`: `Update::run` through `Directories::listing` and `entries`. A non-ahead comparison, root `.gitattributes` change, compare ceiling, or changed-blob ceiling routes to cold archive build. The incremental path lists only parent directories at the pinned head; missing changed entries fail, and truncated listings produce an incomplete result. Removed paths cannot retain prior facts.
- `snapshot.rs`: `Ingest::new`, `carry`, `fallback`, `finish`, `ColdBuild::complete`, and snapshot decoding. The persisted payload contains only eligible per-path outcomes; exclusion totals stay invocation-local. Carry loads prior facts only outside the removed set, and a failed load queues a verified refetch. Publication requires complete coverage.
- `eligibility.rs`: `classify` excludes symlinks, submodules, unsupported or generated paths, and oversize entries using the head listing's mode and size.

The main source-trust paths were traced from the public `Update::run` facade through the provider and ingest owners. This is the complete sensitive-domain slice of the base-to-candidate diff; adjacent storage and concurrency review belongs to the separately assigned reviewers.

## Verification and limits

Ran `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib update::tests -- --nocapture`: 8 passed, 0 failed. The tests cover changed, deleted, renamed, symlinked, and excluded paths against a cold-build oracle; verified changed-blob failure; archive rebuild triggers; cancellation retry; and stale head detection. I also inspected the provider's compare identity and status tests and the candidate's fake-provider routing. All tests are fake-provider tests; this review made no live GitHub/GHES requests or hosted transport measurements. The domain result rests on source tracing and these local tests.

## Findings and result

No material source-trust or parser-provenance finding. The reviewed boundary satisfies its mapped obligations: **PASS**.

```citations
[]
```
