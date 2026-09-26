# TASK-003 r2 verdict — FIX_REQUIRED

## Immutable subject

- Base: `60cc56e`; candidate: `a10c874ff346d063714be75b46b6ec2c34b990b5` (HEAD stayed at this commit throughout review). TASK-003 began in `741c049`, before the base, so the subject includes the **complete candidate contents** of `crates/bohmian-code-review-pipeline/src/{snapshot,eligibility,testing}.rs` and `git diff 60cc56e a10c874ff346d063714be75b46b6ec2c34b990b5`.
- Authority: approved [spec revision 11](../../spec.md), [original TASK-003](../../tasks/TASK-003-cold-snapshot-and-scoped-context.md), [TASK-003-R1](../TASK-003-r1/TASK-003-R1-publication-order-cancellation-coverage.md), and the [r1 FIND ledger](../TASK-003-r1/verdict.md).
- `.codegraph/` is absent. The reviewed source was not edited.

## Acceptance matrix

| Obligation | Candidate evidence | Verification evidence | Result |
|---|---|---|---|
| S1 / REQ-005: pin tree, classify eligible paths, verify archive and fallback blobs, export source-free facts | `ColdBuild::run`, `Ingest`, `Snapshot::into_context` (`snapshot.rs:153-222,443-557,625-837`); `eligibility::classify` | Pipeline owner tests and `mise run test:pipeline` | PASS |
| REQ-006 / INV-003: only a validated complete object advances the pointer; retry and read-back preserve exact identity | `ColdBuild::publish`, `Snapshots::read` (`snapshot.rs:252-276,569-608`) | Publication, retry, deterministic snapshot, and identity owner tests | PASS |
| R1 FIND-1: every object written is tracked before write and collectable after interruption | `record_snapshot` precedes `storage.create` (`snapshot.rs:581-587`); `Snapshots::collect` | `every_interrupted_publication_leaves_a_tracked_key` | PASS |
| R1 FIND-2 / REQ-005 / REQ-013: cancellation and deadline honored through streamed archive consumption | Checks at each tar entry, fallback, and before publication (`snapshot.rs:518-533,696-726,740-775`); archive byte reads have no check | Existing test cancels before the response; independent source-path reproduction in `findings-validation.md` | **FAIL**: FIND-TASK-003-2 |
| R1 FIND-3 / INV-002: invalid tree entries counted separately from eligible paths | `InvalidEntry` and `listing.rejected.len()` (`eligibility.rs:46-47`, `snapshot.rs:503-508,807-823`) | `rejected_tree_entries_are_counted_as_invalid_exclusions` | PASS |
| S11 / INV-002: incomplete/truncated attempts report coverage and preserve prior pointer | `Coverage`, `IncompleteReason`, `Ingest::finish`, `ColdBuild::run` | Incomplete-attempt and interruption owner tests | PASS subject to archive cancellation gap |
| S14 / AC-009 / INV-007: identity-scoped snapshot read, cleanup, and namespace removal through injected stores | `Snapshots::read/collect`, `SnapshotStore`, `MemorySnapshotStore` | Identity-isolation, cleanup, and interrupted-publication owner tests | PASS |
| R1 FIND-4/5: concrete owner for snapshot IO; bare imported types in changed signatures | `Snapshots` and `stream_entries<R: Read>` | Source/diff inspection and lints | PASS |
| Test discipline: no sleep in place of deterministic synchronization | Deadline branch of `interruption_during_a_complete_archive_never_publishes` (`snapshot.rs:1791-1799`) | Written rule `architecture/references/languages/implementation-execution.md:224-229`; focused test takes about 1.01 s | **FAIL**: FIND-TASK-003-6 |
| Non-goals: no checkout, storage backend, persisted source/archive, consumer scheduling, or TASK-004/005 work | Pipeline manifest, snapshot fields, source/diff inspection | Diff and full gate | PASS |

## Independent review and citation check

| Wave 1 role | Report | Result |
|---|---|---|
| task-rev A | `task-review-a.md` | PASS, no claims |
| task-rev B | `task-review-b.md` | PASS, no claims |
| repo-rev | `standards-review.md` | FAIL, STD-R2-1 |
| domain-rev source trust | `domain-review-source-trust.md` | FAIL, DOM-TRUST-1 |
| domain-rev publication | `domain-review-publication.md` | PASS, no claims |

`mise run review:verify-citations changes/active/repo-parser/review/TASK-003-r2 a10c874ff346d063714be75b46b6ec2c34b990b5` exited 0: six `OK`, zero `REJECT`. Wave 2 independently validated both claims in `findings-validation.md`; its claim input is `claim-ledger.md`.

## Validated blocking ledger

| ID | Source | Status | Gate | Root cause and consequence |
|---|---|---|---|---|
| **FIND-TASK-003-2** | DOM-TRUST-1 | CLASS_INCOMPLETE | REQ-005 / REQ-013 task contract | The entry-level check leaves archive byte reads and tar iterator advancement outside cancellation checks. A cancelled build can keep consuming source until the read or entry ends. |
| **FIND-TASK-003-6** | STD-R2-1 | CONFIRMED | Written repository standard | The new deadline test sleeps and polls real time to force expiry, violating the deterministic-test rule. |

The root causes, sibling sites, source trace, correction boundary, and focused closure proof are in `findings-validation.md`. There are no rejected claims or new notes. Prior r1 FIND-TASK-003-{1,3,4,5} are closed; FIND-TASK-003-2 is reopened because its byte-read class remains. The prior nonblocking collection/adoption race and file-size knob remain notes outside this task's correction.

## Verification and limits

`mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check 60cc56e a10c874ff346d063714be75b46b6ec2c34b990b5` exited 0 in this review. The focused snapshot suite passed 10 tests; the specific cancellation/deadline test passed in 1.01 s. These checks do not exercise cancellation after an archive body chunk. Wave 2 reproduced that gap by tracing the public facade through the `Read` chain; no slow-stream scratch test was added to the immutable candidate. An in-progress blocking transport read can remain blocked until its configured timeout; the required correction checks between reads and prevents further ingest after cancellation.

**Verdict: FIX_REQUIRED.** Remediation: [TASK-003-R2](TASK-003-R2-archive-read-cancellation-and-deadline-proof.md).
