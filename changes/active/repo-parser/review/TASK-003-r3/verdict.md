# TASK-003 r3 verdict — FIX_REQUIRED

## Immutable subject

- Base `60cc56e4213aa90f2a13d3ac6b3d1de536ff81b2`; candidate `d95346349988fed1d79dcb6e8b532b185e05b563`. HEAD remained at the candidate throughout review.
- Approved [spec revision 11](../../spec.md), [original TASK-003](../../tasks/TASK-003-cold-snapshot-and-scoped-context.md), [R1 remediation](../TASK-003-r1/TASK-003-R1-publication-order-cancellation-coverage.md), [R2 remediation](../TASK-003-r2/TASK-003-R2-archive-read-cancellation-and-deadline-proof.md), and the r1/r2 `FIND-*` ledgers.
- TASK-003 began in `741c049`, before the base. The subject includes the complete candidate `crates/bohmian-code-review-pipeline/src/{snapshot,eligibility,testing}.rs` and `git diff 60cc56e d95346349988fed1d79dcb6e8b532b185e05b563`. `.codegraph/` is absent.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| S1 / REQ-005: pinned tree, eligibility, verified archive/fallback, source-free reload | `snapshot.rs` `ColdBuild::run`, `Ingest`, `Snapshot::into_context`; `eligibility.rs` | Complete cold-snapshot and deterministic-serialization owner tests; pipeline lane | PASS except eligibility reset below |
| S11 / INV-002: explicit incomplete coverage and complete-only publication | `Coverage`, `IncompleteReason`, `Ingest::finish`; rejected-entry count | Incomplete-attempt and rejected-tree owner tests | PASS except interruption after ingest below |
| REQ-005 / REQ-013 and R1/R2 FIND-2: cancellation and deadline through archive reads, fallback, publication and reuse | `Interruptible` guards compressed/expanded reads; final `run` check at `snapshot.rs:525`; publication at `:581-605` and reuse at `:465-472` have no later check | Streaming cancellation and expired-deadline tests pass; Wave 2 traced the public storage-read path | **FAIL — FIND-TASK-003-2** |
| TASK-003 eligibility: root `.gitattributes` overrides and reasoned vendor/generated exclusions | `eligibility.rs:108-144,279-283` | Wave 2 local `git check-attr` reproduction; current owner tests do not cover `!attr` reset | **FAIL — FIND-TASK-003-7** |
| REQ-006 / INV-003 and R1 FIND-1: validated object, record-before-write, retry, cleanup | `ColdBuild::publish`, `Snapshots::collect`, `SnapshotStore` | Interrupted-publication, crash/retry and collection owner tests | PASS subject to interruption boundary above |
| S14 / AC-009 / INV-007: tenant and installation isolation, separate injected snapshot store | `Snapshots::read/collect`, `MemorySnapshotStore` | Identity-isolation owner test | PASS |
| R1 FIND-3/4/5: rejected entries counted; concrete IO owner; bare types in signatures | `InvalidEntry`, `Snapshots`, `stream_entries<R: Read>` | Source/diff inspection and standards review | PASS |
| R2 FIND-6: deterministic deadline proof | `expired_deadline_stops_archive_ingest`; no sleep in corrected snapshot test | Exact focused selector and source inspection | PASS |
| Non-goals: no backend, checkout, source/archive persistence, consumer scheduling, TASK-004/005 work or library deadline maximum | Candidate source, manifests and diff | Source/diff inspection; full gate | PASS |

## Independent review and citation check

| Wave 1 role | Report | Result |
|---|---|---|
| task-rev A | `task-review-a.md` | PASS; no claims |
| task-rev B | `task-review-b.md` | FAIL; `TASK-B-1` |
| repo-rev | `standards-review.md` | PASS; no claims |
| domain-rev publication | `domain-review-publication.md` | PASS; no claims |
| domain-rev source trust | `domain-review-source-trust.md` | FAIL; `DOM-TRUST-R3-1`, `DOM-TRUST-R3-2` |

The reviewer citation metadata was corrected to literal candidate symbols, then `mise run review:verify-citations changes/active/repo-parser/review/TASK-003-r3 d95346349988fed1d79dcb6e8b532b185e05b563` exited 0: 13 `OK`, zero `REJECT`. The checked union is in `claim-ledger.md`. Fresh Wave 2 independently merged and validated it in `findings-validation.md`; no claim was rejected or reduced to a new note.

## Validated blocking ledger and prior closure

| ID | Source claims | Status / gate | Root cause and consequence |
|---|---|---|---|
| **FIND-TASK-003-2** | `TASK-B-1`, `DOM-TRUST-R3-1` | `CLASS_INCOMPLETE`; REQ-005/REQ-013 task contract | Execution is checked during ingest but not after publication IO or reuse read. Cancellation can advance the pointer or return `Reused`. R2 closed archive byte reads but left the completion boundary open. |
| **FIND-TASK-003-7** | `DOM-TRUST-R3-2` | `CONFIRMED`; REQ-005 eligibility contract | `!linguist-*` reset is parsed as explicit false, so built-in vendored/minified exclusions are bypassed and unwanted files are indexed. |

Wave 2 records every sibling, source trace, reproduction, correction boundary and focused closure proof in `findings-validation.md`. R1 FIND-1, -3, -4 and -5 remain closed. R2 FIND-6 is closed. FIND-2 remains open under its original ID. The previously documented concurrent collection/adoption race remains a nonblocking consumer-scheduling note; no new note was added.

## Verification and limits

`mise run test:pipeline`, `mise run test:symbolic-context:journey`, `git diff --check 60cc56e d95346349988fed1d79dcb6e8b532b185e05b563`, and the second `mise run gate` exited 0 in this review. The first gate run failed once in unchanged `execution.rs:105`: an `Instant::now()` deadline returned `Ok(0ns)` rather than expired. Its exact focused rerun and the full second gate passed; this timing-sensitive observation is not evidence of a candidate change. Wave 1 ran focused owner and cancellation tests. Wave 2 used a traced public storage path and local `git check-attr`; it did not mutate the candidate or add a scratch test. An already-blocked transport read still waits for its configured timeout.

**Verdict: FIX_REQUIRED.** Remediation: [TASK-003-R3](TASK-003-R3-publication-interruption-and-attribute-resets.md).
