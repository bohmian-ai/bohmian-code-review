# Domain review — source trust and bounded ingest

## Subject and boundary

Immutable base `60cc56e`, candidate `a10c874ff346d063714be75b46b6ec2c34b990b5`; approved `changes/active/repo-parser/spec.md` revision 11; original `TASK-003-cold-snapshot-and-scoped-context.md`; remediation `TASK-003-R1-publication-order-cancellation-coverage.md` and its prior `FIND-*` ledger. Reviewed the complete candidate `snapshot.rs`, `eligibility.rs`, and `testing.rs`, the base-to-candidate diff, and the `GitHubClient` archive/tree/blob and `Execution` boundaries they call. Governing rules: spec REQ-004–006, REQ-013–014, INV-002; AGENTS.md pipeline boundary and validation rules; architecture design and agent rules.

The candidate verifies archive bytes against the pinned tree blob ID before indexing, bounds compressed and expanded archive reads, accounts for rejected tree entries under `InvalidEntry`, and records a snapshot key before writing its object. The source-free snapshot and identity-scoped read path remain intact. The prior FIND-TASK-003-1 and FIND-TASK-003-3 corrections are supported by source and owner tests. The new checkpoint closes FIND-TASK-003-2 when cancellation precedes the next archive entry; it does not cover cancellation during an entry read.

## Finding

### DOM-TRUST-1 — cancellation is checked outside streamed archive reads

- **Classification:** `INCORRECT`; proposed `CLASS_INCOMPLETE` for prior `FIND-TASK-003-2`.
- **Defect class / root cause:** `Ingest::stream_entries` checks `Execution` only once per tar entry, before processing it. Its `entry.read_to_end` and the tar iterator's move to the next entry read from the archive through `BoundedReader` without checking the invocation's deadline or cancellation. The owner's checkpoint placement leaves a potentially long or blocked source read outside the cancellation contract; the shape is a reader that carries only a byte limit, not execution state.
- **Sibling locations:** `stream_entries` calls `tar.entries()` and advances the iterator before the loop-body check; `entry.read_to_end` can continue after a mid-entry cancellation. `consume_archive` wraps decompression with a byte-only reader. The fallback loop checks before each blob, so it does not share this gap.
- **Violated obligation:** spec REQ-005 says the cold call honors caller deadline and cancellation; REQ-013 says invocation duration is bounded and cancellation is reported honestly. R1's FIND-TASK-003-2 required archive-path cancellation, including the archive stream.
- **Reachable consequence:** With a slow archive response containing one eligible entry, cancellation after its header does not stop `read_to_end`; the call waits for that entry's remaining bytes or read timeout before reporting interruption. A large ignored entry can also be drained while advancing the tar iterator before the next check. The focused owner test cancels at the archive *request*, before the first entry, so it cannot exercise either path.
- **Testable correction:** Check `Execution` in the existing archive read path as bytes are consumed, carrying the interruption into `Ingest` so the current incomplete outcome and `NotFetched` accounting remain intact. Test cancellation or deadline after the first chunk of a single archive entry and assert prompt incomplete return, no storage object, and unchanged pointer. Keep blob-ID verification and both archive byte ceilings.

## Verification and limits

- `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib snapshot::tests::interruption_during_a_complete_archive_never_publishes -- --exact`: 1 passed. Its transport hook trips before entry processing, so it does not close DOM-TRUST-1.
- Source trace: `ColdBuild::run` → `Ingest::consume_archive` → `Ingest::stream_entries` → `entry.read_to_end`; `GitHubClient::archive` supplies a byte-bounded `Read` body. The checkpoint is at the start of each loop body, while the byte reads occur later and the iterator may read before the following loop body.
- No independent slow-stream reproduction was added, because this review cannot edit the immutable subject. Precise wall-time behavior of a consumer-provided `Read` depends on that implementation; the missing checks are visible in source.

**Overall: FAIL** — one material task-contract gap remains in the archive cancellation path.

```citations
[{"id":"DOM-TRUST-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":698,"snippet":"        for entry in tar.entries()? {","symbols":["stream_entries"]},{"id":"DOM-TRUST-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":699,"snippet":"            if let Err(interrupted) = execution.check() {","symbols":["stream_entries","check"]},{"id":"DOM-TRUST-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":726,"snippet":"            entry.read_to_end(&mut bytes)?;","symbols":["stream_entries","read_to_end"]},{"id":"DOM-TRUST-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":673,"snippet":"        let expanded = BoundedReader::new(","symbols":["consume_archive","BoundedReader"]},{"id":"DOM-TRUST-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":1758,"snippet":"            repo.transport.on_request(move |url| {","symbols":["interruption_during_a_complete_archive_never_publishes"]}]
```
