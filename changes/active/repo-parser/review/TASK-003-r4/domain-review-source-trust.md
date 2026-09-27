# TASK-003 r4 — source trust and archive provenance

## Boundary, authority, and source coverage

Reviewed immutable `c74871cbc78140820c9b0a5fda60e0952c4265e4` → `0a1dea22b206e6174d4421d5ba7f7590ded93673` against approved spec revision 12, TASK-003, `AGENTS.md`, `architecture/agent-rules.md`, and `architecture/bohmian-code-review-design.md`. `.codegraph/` is absent. I inspected the full changed `archive.rs`, the `ColdBuild::run` → `Ingest::consume_archive` → `Ingest::fallback` path, GitHub archive/blob transport and Git SHA verification, eligibility and path validation, the pipeline manifest and lockfile, owner tests, and prior r1–r3 source-trust findings only where their defect class still applies.

Archive paths are reduced to a validated `RelativePath` and matched against the pinned tree's wanted path/blob map. Regular-file bytes enter `verified` only after `git_blob_sha` matches the listed blob identity; mismatches and omissions remain pending for the bounded `GitHubClient::blob` fallback, which checks the same hash. Symlink entries are skipped. The compressed-body and expanded-byte limits are enforced, and `GzDecoder::try_finish` checks the gzip trailer. Root `.gitattributes` is fetched as a verified blob. The revised three-state `!attr` handling closes the old r3 eligibility class. Revision 12 removed library cancellation tokens and deadlines, so the old r3 publication cancellation finding is superseded rather than reopened.

## Proposed finding

### DOM-TRUST-R4-1 — verified files accumulate for an entire compressed chunk

- **Classification:** INCORRECT. **Defect class:** the push tar reader queues completed file payloads without a per-entry drain; `Ingest::consume_archive` drains only after `GzDecoder::write_all` has consumed the entire provider chunk. The owner is the archive-to-ingest handoff, not the blob fetcher. No public API shape causes the defect.
- **Root cause and siblings:** `TarStream::finish_entry` pushes each matching file and its bytes into `verified` (`archive.rs:215–219`). `GzDecoder::write_all(&chunk)` can complete arbitrarily many entries before `ingest_verified` runs (`snapshot.rs:682–692`). The final `try_finish` drain is a sibling handoff with the same delayed release, although no new compressed input is supplied there. Each file is individually limited, but the sum of queued files is bounded only by the 4 GiB expanded-archive ceiling, not by one file's ceiling.
- **Violated obligation:** spec revision 12 REQ-005 requires streaming without retaining the full source set; REQ-013 and REQ-014 require bounded process memory during ingest. TASK-003 outcome and approach require a streamed archive. The module's own one-wanted-file memory invariant (`archive.rs:3–9`) is false at this handoff.
- **Evidence and reachable consequence:** A valid archive containing many eligible, individually small files whose bytes hash to their pinned tree entries can be strongly compressed into one provider chunk. `Archive::chunk` returns that chunk within its compressed limit. The decoder then delivers each file to `TarStream`, which retains every completed payload in `verified` until `write_all` returns. A 4 GiB expanded archive of 1 MiB files can therefore leave gigabytes of source live in `verified` before any indexing or release, risking process exhaustion during a cold build. Hash verification does not limit the aggregate retained bytes.
- **Testable correction:** Release and index verified entries as they complete, or put an explicit small aggregate queue bound on the archive-to-ingest handoff while preserving verification before indexing and the existing archive/fallback coverage semantics. A focused check should feed one compressed body chunk containing many valid eligible files and assert the peak retained verified payload stays independent of the total eligible source size.

## Existing `tar` dependency assessment

`tar` is already pinned in the workspace and was a production dependency in the base, so adding another archive parser dependency is unwarranted. Its `Archive<R: Read>` API is pull-driven, while the approved source body is async and push-driven; directly using it would require an adapter or blocking bridge with cancellation and ownership semantics. The hand-written reader is therefore a plausible local adaptation, not a standalone DRIFT finding. Its reduced format surface still needs to meet the streaming-memory contract above. Header parsing failures fall back to verified blobs and do not themselves establish a source-trust breach.

## Verification limits and result

This finding is a traced public cold-build path, not a measured peak-memory test. I did not alter the candidate or run a scratch benchmark. The existing archive tests check chunk-boundary correctness, file size, and expansion limits; they do not check the aggregate `verified` queue within one compressed chunk. **Overall: FAIL** on the bounded-streaming obligation; no separate parser-provenance or blob-identity finding.

```citations
[{"id":"DOM-TRUST-R4-1","path":"crates/bohmian-code-review-pipeline/src/archive.rs","line":218,"snippet":"                    self.verified.push((path, blob, bytes));","symbols":["finish_entry"]},{"id":"DOM-TRUST-R4-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":683,"snippet":"                Ok(Some(chunk)) => decoder.write_all(&chunk),","symbols":["consume_archive"]},{"id":"DOM-TRUST-R4-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":687,"snippet":"            self.ingest_verified(&mut decoder.get_mut().verified);","symbols":["consume_archive","ingest_verified"]}]
```
