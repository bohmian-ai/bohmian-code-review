# Domain review: context durability and concurrency

Subject: `fa4fa2101950626a9fe60b0fdff0439b04c16f01..62346cb030354596cdf3dd7596de63dbd8d99fcb`. Reviewed boundary: `PrContext::run` through context publication, identity-scoped read, expiry, and physical cleanup. This covers the complete cumulative TASK-005 candidate, approved spec revision 12, prior R1–R3 findings, and the maintainer decision that replaces R3 FIND-4. The decision concerns tree-entry validation; it does not change the context durability contract.

| Authority | Source and verification coverage | Result |
|---|---|---|
| REQ-009, INV-004: PR-private overlay | `pull_request.rs` creates a fresh `Ingest`, loads snapshot facts into that instance, builds the context, and publishes only its selected spans. Concurrent-head journey evidence exists in `pull_request::tests::concurrent_prs_see_only_their_own_heads`. | PASS |
| REQ-011–012, TASK-005 Scenario 3: record-before-object publication, exact-key read, retry after interruption | `Contexts::publish` upserts the record before create and checks bytes on `AlreadyExists`; `Contexts::read` requires exact identity and ID, verifies key, digest and context identity, and treats an absent object as `NotFound`. `PrContext::run` returns the record only after publication completes. Existing dropped-publication, adoption, corruption and public-facade read-back tests cover these paths. | PASS |
| REQ-014–015, INV-005, R3 FIND-7: 24-hour read cutoff and safe physical collection | `Contexts::read` rejects at `expires_at`. `collect_expired(older_than, limit)` lists records by a caller cutoff, deletes object before record, and retains failed deletes as lag. `ContextStore::expired_contexts` documents the grace longer than publication timeout; the in-memory store filters by that cutoff. `context::tests::cleanup_cutoff_spares_in_flight_publication` pauses create after upsert, sweeps during read expiry, resumes publication, and later collects both completed and dropped publications. The exact focused test passed (1 test, 66 filtered). | PASS |

## Findings and limits

No material finding. R3 FIND-TASK-005-7 is closed at the context cleanup boundary: with the documented caller cutoff, an in-flight publication is not selected for deletion, while reads stop at the fixed expiry. Independent publications retain separate IDs and keys; existing tests cover failed deletion lag and retry. The cleanup cutoff is a caller obligation, as specified; this review did not exercise a production storage adapter or a real caller timeout. I did not rerun the full gate or external GitHub tests for this domain review.

Overall: **PASS** for the reviewed durability and concurrency boundary.

```citations
[]
```
