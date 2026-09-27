# TASK-003 r5 — independent Ponytail validation

Subject: base `c74871cbc78140820c9b0a5fda60e0952c4265e4`, candidate `3ada5c59931f236716ed027d773a2746ccf3c3af`, approved `SPEC-repo-parser` revision 12, original `TASK-003`, and `TASK-003-R4`. `.codegraph/` is absent. The citation-checked Wave 1 claim ledger is empty: the verifier exited 0 with no rejected citations. I used the approved authorities, complete cumulative diff, candidate source and r4 `FIND-*` ledger; I did not use Wave 1 report prose or results.

## Validated ledger

**No retained blocking findings and no notes.** The public cold-build path pins a commit and tree, classifies eligible files, streams and verifies archive entries, falls back to bounded verified blobs, and publishes only after complete coverage (`snapshot.rs:463-575`). The source-free snapshot is reloaded with key, digest, identity, version, tree and completeness checks (`snapshot.rs:172-223,240-271`). Publication writes record, object and pointer in that order (`snapshot.rs:592-633`). Collection claims stale unreferenced records before object-then-record deletion (`snapshot.rs:274-309`; `persistence.rs:134-231`). The cumulative diff adds no consumer backend, detached task, persistent source, or scoped PR indexing. The push tar parser is justified by the async archive stream: replacing it with the already installed synchronous `tar::Archive` would need a blocking bridge or whole-archive buffer, contrary to this task's bounds and drop semantics.

## Prior finding closure

| Prior finding | Source and independent closure check | Status |
|---|---|
| `FIND-TASK-003-8` | `Ingest::decompress` drains verified entries after every gzip write (`snapshot.rs:728-759`), limiting the queue to one decompressor output batch rather than one provider chunk. `one_chunk_archive_keeps_verified_bytes_bounded` indexes 128 wanted files and asserts a bounded peak (`snapshot.rs:1554-1597`). | CLOSED |
| `FIND-TASK-003-9` | `SnapshotStore::unreferenced` atomically claims selection against record upserts and pointer swaps; claimed records reject `upsert_record` (`persistence.rs:134-209`). `Snapshots::collect` deletes object before record (`snapshot.rs:274-309`); the in-memory store implements the claim (`testing.rs:273-333`). `collection_claim_and_publication_never_race` covers collector-first and builder-first orderings (`snapshot.rs:1752-1839`). I ran its exact focused command: one test passed. The caller-supplied cutoff must remain older than the build timeout, as required by REQ-006. | CLOSED |
| `FIND-TASK-003-10` | Fallback treats provider `ResponseTooLarge` as benign oversize only when its transfer limit is at least the eligible file bound (`snapshot.rs:764-794`); otherwise it records an incomplete fetch. `blob_ceiling_below_file_bound_is_not_oversize` covers both relationships (`snapshot.rs:1479-1552`). | CLOSED |
| `FIND-TASK-003-11` | Compressed and expanded archive-bound exhaustion reaches `Incomplete(ArchiveTooLarge)` after exact fallback accounting and before publication (`snapshot.rs:525-558,698-725`). `incomplete_attempts_report_coverage_and_keep_the_prior_pointer` covers both bounds, complete fallback and no fallback (`snapshot.rs:1258-1442`). | CLOSED |

Earlier r1–r3 findings concern the superseded synchronous design. Their cancellation/deadline obligations do not carry into approved revision 12; this review used the current full range and r4 ledger.

## Verification limits and decision

The interleaving proof uses the injected in-memory store; the specification does not require a concrete SQL or object-store adapter in this task. I independently ran only the focused collection race test; the orchestrator owns aggregate lane evidence and the final acceptance matrix. No scratch source or test was changed. **Decision: validated empty ledger; recommend `PASS` if the orchestrator confirms all required reports and verification gates.**
