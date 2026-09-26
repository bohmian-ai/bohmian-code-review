# TASK-003 r1 — Domain review: source trust and bounded ingest

Candidate `1144845a8deadda54ee546548f43f96b4d5a284f`, base `60cc56e`. Spec `SPEC-repo-parser@11`.

## Boundary

Eligibility (path, mode, size, root `.gitattributes`, LFS detection, glob matching); archive streaming and per-entry blob verification against the pinned tree; bounded verified blob fallback; byte, file, deadline and cancellation bounds; whether coverage tells excluded paths apart from eligible files that were never indexed. Publication ordering, identity isolation and collection belong to other reviewers. I only looked at them where they gate unverified bytes.

## Authority and source coverage

- Spec rev 11: glossary (Eligible file, Complete snapshot), REQ-005, REQ-006, INV-002, INV-003, operational ceilings. Plan S10/S11 rows (`symbolic-context-service-plan.md:89-90`).
- Task TASK-003 up to `## Implementation Evidence`: Scenario 2 and the Material Stop Conditions.
- Read in full: `eligibility.rs`, `snapshot.rs` (production and test fixture). Read the relevant parts of `github.rs`: `blob`, `archive`, `BoundedReader`, `tree`, `validate_tree_entry`, `RequestLimits`. Also read `core/relative_path.rs` (`RelativePath::new`). Read the `testing.rs` and `lib.rs` diffs.
- Traced this path through the public facade: `ColdBuild::run` → `GitHubClient::tree` → `classify` → `Ingest::consume_archive`/`stream_entries` → `Ingest::fallback` → `GitHubClient::blob` → `Ingest::ingest` → `Coverage::is_complete` → `publish`.

## Verification

- `cargo test -p bohmian-code-review-pipeline --all-features snapshot` ran 7 tests, all passed. The eligibility owner tests were read, not re-run.
- Both findings come from reading the source. I wrote no scratch reproduction because this review is read-only on source. The existing test at `snapshot.rs:1362` directly shows DOM-TRUST-2.

## Confirmed sound (no finding)

- **Injection, replacement and hiding by the archive.** `stream_entries` only uses tar entries whose path is still in `pending`, which comes from the tree listing. It hashes the full bytes with `git_blob_sha` and compares them to the listed blob id before calling `ingest`. Non-regular entries, oversize headers, mismatched hashes and unknown paths are all left in `pending`. The archive cannot add a path, replace content, or turn an eligible file into a benign skip. LFS and oversize are decided only on verified bytes.
- **Unverified bytes.** `GitHubClient::blob` checks size against `max_blob_bytes` and verifies the Git id before returning. The root `.gitattributes` is fetched through the same verified call. No path from the archive or from fallback reaches `SymbolicContext::upsert` without verification.
- **Incomplete outcomes.** A truncated tree returns `Incomplete(TruncatedTree)` before any ingest. An archive error or mismatch leaves entries in `pending`, and fallback is capped at `max_fallback_blobs` with the remainder marked `NotFetched`. A parse error becomes `ParseFailed`. An expansion over `max_expanded_archive_bytes` is an IO error that leaves the rest pending. Every one of these makes `is_complete()` false, and in each case nothing is written and every path is listed in `skipped`.
- **LFS and oversize as benign skips.** This matches the spec. The glossary says "LFS pointers are skipped after content inspection" and defines an eligible file as "at most 1 MiB". S11 asks for a "reasoned exclusion, no false complete pointer". Both skips are reported per path with a reason and blob id, so a complete pointer with them is not a false claim. The Scenario 2 wording ("LFS pointer ... never advances a complete pointer") is looser than the spec, and the spec governs.
- **Resource bounds.** `glob_matches` is O(pattern × path) and `segment_matches` is iterative with only the last-star retained. The adversarial test finishes immediately. Each tar entry is read into memory only after its header size is checked against `max_file_bytes`, and the bytes are dropped after parsing. The compressed stream is bounded by `max_archive_bytes` and the expanded stream by `max_expanded_archive_bytes`. The deadline is enforced on the archive body through the transport's global timeout, which is capped at the time remaining.

## Findings

### DOM-TRUST-1 — Tree entries rejected during validation are dropped from coverage, so a snapshot can be published as complete while eligible-looking files are missing

- **Defect class:** a provider entry that failed validation is silently missing from coverage accounting.
- **Violated obligation:** the spec glossary says "Eligibility and exclusions are reported by reason." INV-002 says "Missing facts, skipped files ... remain distinguishable". REQ-006 says "Only complete attempts publish a reusable snapshot". The task stop condition says "Stop if ... coverage cannot distinguish excluded from missing eligible files."
- **Root cause, first owner:** `ColdBuild::run` (`snapshot.rs:484`) passes only `listing.entries` to `Ingest::new`. `GitHubClient::tree` puts every entry that fails `validate_tree_entry` into `TreeListing::rejected` (`github.rs:456`, `github.rs:938`). Those failures are a `RelativePath::new` failure (a backslash, a path over the byte limit, a drive-like prefix such as `c:x.ts`, a noncanonical segment), an unknown mode (for example legacy `100664`), or a bad sha. Nothing in `snapshot.rs` or `eligibility.rs` reads `rejected`.
- **Does the owner's shape permit it?** Yes. `Coverage.excluded` is keyed by `ExclusionReason`, which has no variant for an invalid entry, and `Coverage` has no field for rejected entries. The information exists on `TreeListing` and is thrown away at the facade.
- **Observable consequence:** take a default branch containing `src\util.ts` (a legal Git path) or a `100664` `.ts` blob. It builds and publishes `ColdBuildOutcome::Published`. `Coverage` shows `eligible`, `indexed` and `excluded` with no trace of that file. Later PR context then treats the snapshot as full coverage, and the file's declarations cannot be found. This is the "false full coverage" INV-002 forbids.
- **Sibling locations:** `TreeListing::rejected` has only the one consumer in this candidate. `PullRequestFilesPage::rejected` (`github.rs:547`, `github.rs:1107`) is the same pattern for PR files and belongs to TASK-004/005. It is noted so the fix follows one rule, but it is not a finding against this task.
- **Testable correction:** carry `listing.rejected` into coverage. Either add an `ExclusionReason::InvalidEntry` counted in `excluded`, or, if a rejected entry could be eligible, add a coverage field that makes the attempt incomplete. Pick one rule and document it. Add a `snapshot.rs` test with a tree that includes a `100664` or backslash-path `.ts` entry, and assert that the entry appears in `coverage` with a reason. If the chosen rule is "incomplete", also assert the outcome is not `Published`.

### DOM-TRUST-2 — Cancellation is not observed while the archive is streamed and parsed

- **Defect class:** a long-running loop never polls the caller's cancel token.
- **Violated obligation:** REQ-005 says "The call honors a caller-supplied deadline and cancellation".
- **Root cause, first owner:** `Ingest::stream_entries` (`snapshot.rs:646-647`) loops over every tar entry, reading, hashing and parsing, and never calls `execution.check()`. `consume_archive` does not receive the `Execution` at all. The transport's global timeout enforces the deadline on the body, but a `CancelToken` does not reach the body reader. Cancellation is first seen in `Ingest::fallback`, after the whole archive has been consumed.
- **Does the owner's shape permit it?** Yes. `Execution` is available in `run` but is not passed into archive ingest.
- **Observable consequence:** the candidate's own test `interruption_is_explicit_and_deadlines_are_caller_owned` cancels when the tarball is requested, yet asserts `archive_entries == 4` (`snapshot.rs:1362`). Every entry is still parsed after the cancel. For a multi-GiB archive with a long caller deadline and `request_timeout` raised to fit it, a cancelled build keeps downloading and parsing until the archive ends or times out. The outcome is still honestly `Incomplete(Interrupted)`, so this is a boundedness defect, not an integrity one.
- **Sibling locations:** none in this candidate. The fallback loop already checks cancellation per blob.
- **Testable correction:** pass `&Execution` into `consume_archive`/`stream_entries` and check it once per tar entry. On interruption, stop reading and leave the rest pending, so it becomes `NotFetched` and `Incomplete(Interrupted)`. Change the existing test to assert `archive_entries == 0`, or a small bounded count, after a cancel at the tarball request.

## Overall

**FAIL.** DOM-TRUST-1 hits the task's stated stop condition: coverage cannot account for eligible-looking entries that fail tree-entry validation, and the snapshot is still published as complete. DOM-TRUST-2 is a narrower REQ-005 gap. Archive verification, fallback verification, the incomplete-outcome paths, the LFS and oversize handling, and the glob and memory bounds are sound.

```citations
[
  {"id":"DOM-TRUST-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":484,"snippet":"        let mut ingest = Ingest::new(&listing.entries, &attributes, self.limits.max_file_bytes);","symbols":["ColdBuild","Ingest","listing"]},
  {"id":"DOM-TRUST-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":456,"snippet":"    pub rejected: Vec<RejectedPath>,","symbols":["TreeListing","rejected","RejectedPath"]},
  {"id":"DOM-TRUST-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":938,"snippet":"                Err(reason) => rejected.push(RejectedPath {","symbols":["RejectedPath","validate_tree_entry"]},
  {"id":"DOM-TRUST-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":120,"snippet":"    pub excluded: BTreeMap<ExclusionReason, usize>,","symbols":["Coverage","ExclusionReason"]},
  {"id":"DOM-TRUST-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":646,"snippet":"    fn stream_entries<R: Read>(&mut self, tar: &mut tar::Archive<R>) -> io::Result<()> {","symbols":["stream_entries","consume_archive"]},
  {"id":"DOM-TRUST-2","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":1362,"snippet":"        assert_eq!(result.ingest.archive_entries, 4);","symbols":["interruption_is_explicit_and_deadlines_are_caller_owned"]}
]
```
