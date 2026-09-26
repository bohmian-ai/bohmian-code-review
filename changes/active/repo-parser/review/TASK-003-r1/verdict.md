# TASK-003 task review r1 — verdict

**Verdict: FIX_REQUIRED**

Remediation task: [TASK-003-R1-publication-order-cancellation-coverage.md](TASK-003-R1-publication-order-cancellation-coverage.md)

## Immutable subject

- Spec: `changes/active/repo-parser/spec.md` revision 11 (approved).
- Task: `changes/active/repo-parser/tasks/TASK-003-cold-snapshot-and-scoped-context.md`.
- Base `60cc56e4213aa90f2a13d3ac6b3d1de536ff81b2`, candidate `1144845a8deadda54ee546548f43f96b4d5a284f` (HEAD; unchanged through both waves).
- Scope: TASK-003 work began at `741c049`, before the base, so the subject is the complete candidate `crates/bohmian-code-review-pipeline/src/{snapshot,eligibility,testing}.rs` plus `git diff 60cc56e 1144845` (adds `lib.rs`, `persistence.rs`, `storage.rs`, task file). Other code passed TASK-002 (`review/TASK-002-r7`).

## Verification (orchestrator run, candidate HEAD)

| Lane | Result |
|---|---|
| `mise run test:pipeline` | exit 0, 27 passed |
| `mise run fmt:check` | exit 0 |
| `mise run lints` | exit 0 |
| `mise run test:symbolic-context:journey` | exit 0, 6 passed |

Green lanes do not cover the blocking defects below; each was reproduced by a scratch test (see `findings-validation.md`).

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| S1 complete cold snapshot: source-free, verified archive and fallback blobs, reload without source fetch | `ColdBuild::run`, `Ingest`, `read_snapshot`, `Snapshot::into_context` | `complete_cold_snapshot_is_verified_source_free_and_reloadable` | PASS |
| Immutable object, deterministic bytes per tree | `Coverage` holds only tree facts; per-attempt stats on `ColdBuildResult` | `same_tree_serializes_identically_across_ingest_paths` | PASS |
| Object-before-pointer publication, byte-compared adoption | `ColdBuild::publish` | `publication_is_crash_safe_and_retry_converges` | PASS |
| Interruption after object write is retry-safe and still collectable/removable (Scenario 3, REQ-006) | `publish` creates before `record_snapshot` | scratch repro: `collected=[] removed=[]`, object stored | **FAIL — FIND-TASK-003-1** |
| Caller-supplied deadline and cancellation honored (Owners, REQ-005, REQ-013) | `Execution::check` only in `fallback` | scratch repro: cancelled → `Published`; test at snapshot.rs:1362 encodes it | **FAIL — FIND-TASK-003-2** |
| S10–S11 incomplete attempts keep prior pointer with exact coverage and reason | `IncompleteReason`, `Coverage::is_complete` | `incomplete_attempts_report_coverage_and_keep_the_prior_pointer` | PASS |
| Exclusions reported by reason; coverage distinguishes excluded from missing (INV-002, stop condition) | `TreeListing::rejected` never read | scratch repro: backslash path absent from every count | **FAIL — FIND-TASK-003-3** |
| LFS pointers / verified oversize blobs as reasoned exclusions (S11) | `SkipReason` after verified bytes | `classification_reports_each_reason`, incomplete-attempt test | PASS |
| S14 / AC-005 / AC-009 identity isolation and fail-closed | exact-key `read_snapshot`, scoped `SnapshotStore`, `MemorySnapshotStore` | `identities_cannot_read_or_advance_each_others_snapshots` | PASS |
| Collection removes expired unreferenced, never pointed-to (sequential contract) | `collect_snapshots`, `SnapshotStore` cleanup methods | `collection_removes_only_expired_unreferenced_snapshots` | PASS |
| No library deadline maximum; no storage backend; no source/archive persistence; no new dependency | `ColdBuild::run` uses only `Execution`; manifests | lanes, diff | PASS |
| Repository rules: stateful workflows on concrete owners (AGENTS.md:27) | `collect_snapshots`, `read_snapshot` free fns | source | **FAIL — FIND-TASK-003-4** |
| Repository rules: bare imported types in signatures (AGENTS.md:31) | `stream_entries(&mut tar::Archive<R>)` | source | **FAIL — FIND-TASK-003-5** |

## Wave 1 results

| Reviewer | Report | Result | Findings |
|---|---|---|---|
| task-rev A | `task-review-a.md` | FAIL | TASK-A-1, TASK-A-2 |
| task-rev B | `task-review-b.md` | FAIL | TASK-B-1, TASK-B-2 |
| repo-rev | `standards-review.md` | FAIL | REPO-1, REPO-2, REPO-3 |
| domain-rev source trust & bounded ingest | `domain-review-source-trust.md` | FAIL | DOM-TRUST-1, DOM-TRUST-2 |
| domain-rev publication, isolation & cleanup | `domain-review-publication.md` | FAIL | DOM-PUB-1, DOM-PUB-2 |

## Citation check

`citations.txt`: every citation `OK`, exit 0. The first run rejected entries in all five reports solely on the symbol-name rule (qualified names such as `ColdBuild::publish` are not whole words in the source); path and snippet checks had passed. Reviewers corrected only their `symbols` arrays to bare identifiers and the rerun passed. No finding was dropped.

## Validated blocking ledger

Full diagnosis, reproduction and correction: `findings-validation.md`. Claim ledger passed to Wave 2: `claim-ledger.md`.

| ID | Sources | Status | Gate | Root cause |
|---|---|---|---|---|
| FIND-TASK-003-1 | TASK-A-1, DOM-PUB-2 | CONFIRMED | task contract | `ColdBuild::publish` writes the object before `record_snapshot`; an interrupted publish leaves an untracked object |
| FIND-TASK-003-2 | TASK-B-2, DOM-TRUST-2 | CONFIRMED | task contract | archive ingest (`consume_archive`/`stream_entries`) and pre-publish have no `execution.check()` |
| FIND-TASK-003-3 | DOM-TRUST-1 | CONFIRMED | task contract | `ColdBuild::run` ignores `TreeListing::rejected`; rejected paths vanish from `Coverage` |
| FIND-TASK-003-4 | REPO-1 | CONFIRMED | repository standard | `collect_snapshots` and `read_snapshot` are IO workflows as public free functions |
| FIND-TASK-003-5 | REPO-2 | CONFIRMED | repository standard | `stream_entries` signature names path-qualified `tar::Archive<R>` |

## Notes

- TASK-B-1 / DOM-PUB-1: the collection/adopt race is real, but closing it needs a lease/claim semantic that REQ-012 assigns to consumers; the `ponytail:` rustdoc at snapshot.rs:268 should state the caller obligation (no concurrent collection and builds for one identity) rather than promise self-healing.
- TASK-A-2: `ColdBuildLimits::max_file_bytes` is a candidate-owned knob outside the snapshot key; the spec fixes eligibility at "initially at most 1 MiB". Deleting it in favour of `MAX_ELIGIBLE_FILE_BYTES` is recommended; not blocking because the default is correct and a mismatch fails visibly (`Corrupt`).

## Rejected

- TASK-B-1, DOM-PUB-1 as blocking: out of approved scope (REQ-012 "Job queues, scheduling and cross-installation claims are consumer responsibilities"; REQ-006 "callers choose when to invoke it"). Retained as a note.
- REPO-3: `ponytail:` is an established repository marker for simplification ceilings (base: `eligibility.rs:151`, `select/src/diff.rs:388`, `select/src/lib.rs:477`), not a plan/task/agent reference under AGENTS.md:44.

## Implementer-flagged decisions

1. `SnapshotStore::{record_snapshot, unreferenced_snapshots, delete_snapshot}` — valid (REQ-015 "expired-snapshot cleanup", REQ-006). Ordering defect is FIND-TASK-003-1.
2. LFS pointers and verified oversize blobs as non-blocking reasoned exclusions — valid (spec "Eligible file" definition; plan S11 "reasoned exclusion, no false complete pointer"). Task Scenario 2 wording is looser; the spec governs.
3. Reuse/cleanup race left with `ponytail:` — valid under REQ-012 scoping; wording note above.

## Verification limits

- Blocking findings were reproduced by `ponytail-rev` in a throwaway worktree (removed); the scratch tests are not in the tree. The remediation must add equivalent owner tests.
- The downstream effect of a dangling pointer on TASK-004/005 facades is unverified; those facades do not exist yet.
- `ponytail-rev` could not write its own report (harness write block); the orchestrator saved its hand-back verbatim as `findings-validation.md`.

## Prior-finding closure

None; this is the first TASK-003 review.
