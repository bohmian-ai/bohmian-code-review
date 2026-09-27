# Resource budget domain review

**Subject:** `c74871cbc78140820c9b0a5fda60e0952c4265e4` → `3ada5c59931f236716ed027d773a2746ccf3c3af`
**Result:** PASS

## Boundary and authority coverage

Reviewed the complete cumulative diff, approved spec REQ-005/REQ-013 and INV-002/INV-003, original TASK-003 Scenarios 1–2, and TASK-003-R4 FIND-8/10/11. Traced `GitHubClient::archive`/`Archive::chunk` and `GitHubClient::blob` through `Ingest::consume_archive`, `decompress`, `ingest_verified`, `fallback`, `ColdBuild::run`, and `TarStream`'s tar and expansion bounds. Read the relevant default limits and tests. Also checked the collection and publication calls for a budget-related sibling effect.

| Boundary | Source evidence | Result |
|---|---|---|
| Compressed archive and expansion | `github.rs:577-605`; `snapshot.rs:698-725`; `archive.rs:227-244` stop at the respective configured bounds and propagate bound exhaustion to `Incomplete(ArchiveTooLarge)` at `snapshot.rs:544-549`. | PASS |
| Retained verified source | `snapshot.rs:737-761` drains the verified batch after each decoder write, including an error return; `archive.rs:107-132,209-223` buffers only wanted bounded files before verification. | PASS |
| Fallback count and size | `snapshot.rs:769-795` caps blob requests and treats a provider transfer ceiling below the file ceiling as `FetchFailed`, which keeps coverage incomplete; `github.rs:997-1020` verifies blobs before returning bytes. | PASS |
| Incomplete coverage | `snapshot.rs:543-558` chooses incomplete before snapshot serialization or publication when archive bounds or unindexed files remain. | PASS |

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib snapshot::tests -- --nocapture` passed: 15 tests, including the one-chunk retained-byte test, provider blob ceiling, both archive bounds, and incomplete coverage. This is a focused local check, not a peak resident-memory measurement or a real GitHub archive run; those capacity and provider-contract checks belong to later integrated criteria. No material budget finding survived source inspection.

```citations
[]
```
