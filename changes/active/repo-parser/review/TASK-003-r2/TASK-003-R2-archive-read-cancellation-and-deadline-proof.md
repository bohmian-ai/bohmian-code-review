---
id: TASK-003-R2
kind: remediation
status: ready
spec: SPEC-repo-parser@11
remediates: TASK-003
review: changes/active/repo-parser/review/TASK-003-r2/verdict.md
findings: [FIND-TASK-003-2, FIND-TASK-003-6]
---

# TASK-003-R2 — Archive read cancellation and deterministic deadline proof

## Identities and scope

- Approved spec: `changes/active/repo-parser/spec.md` revision 11.
- Original task: `changes/active/repo-parser/tasks/TASK-003-cold-snapshot-and-scoped-context.md`.
- Prior remediation: `changes/active/repo-parser/review/TASK-003-r1/TASK-003-R1-publication-order-cancellation-coverage.md` and its `FIND-*` ledger.
- Reviewed base: `60cc56e`; cumulative candidate: `a10c874ff346d063714be75b46b6ec2c34b990b5`. TASK-003 began in `741c049`; review includes the complete candidate `snapshot.rs`, `eligibility.rs`, and `testing.rs` and the base-to-candidate diff.
- Validated diagnosis: `changes/active/repo-parser/review/TASK-003-r2/findings-validation.md`.

## Issue diagnosis

### FIND-TASK-003-2 — CLASS_INCOMPLETE: archive reads outlive cancellation

**Root cause and defect class.** R1 put `Execution::check()` at the start of each tar entry in `Ingest::stream_entries` (`snapshot.rs:696-703`), but the archive reader itself carries only compressed and expanded byte limits. `entry.read_to_end(&mut bytes)` (`snapshot.rs:726`) can read a whole eligible entry without another check, and advancing `tar.entries()` (`snapshot.rs:698`) can drain a large unselected entry before the next loop-body check. `consume_archive` builds the byte-only read chain at `snapshot.rs:667-680`. These are siblings of one missing-check-at-read-boundary defect. The fallback loop already checks execution and does not need a second fix.

**Obligation and consequence.** Spec REQ-005 requires the cold-build call to honor the caller's cancellation and deadline; REQ-013 bounds invocation work and requires honest cancellation. TASK-003 owns caller-supplied cancellation and archive ingest. On a real streaming body (`GitHubClient::archive`, `github.rs:990-1023`), cancellation after the entry check can leave the call consuming or decompressing more source until the entry or read completes. The archive byte ceilings prevent unlimited total bytes but do not make cancellation observable during byte consumption. The prior test cancels in the transport request hook before an archive response is read (`snapshot.rs:1756-1762,1782-1789`), so its green result does not close this path. Wave 2 independently traced the public `ColdBuild::run` → `consume_archive` → `stream_entries` → `Read` chain; no scratch test was added to the immutable candidate.

**Correction outcome and boundary.** Reopen the earlier FIND-2 design at `Ingest`'s shared archive read boundary. Reuse `Execution::check` while source bytes are consumed, so cancellation stops subsequent reads and carries `Interrupted` into the existing `fallback` and incomplete-outcome path. Preserve the compressed and expanded byte ceilings, tar safety, Git blob verification, `NotFetched` accounting, and final pre-publication check. This is one owner correction for both eligible-entry reads and iterator advancement. A transport read already in progress may still wait for its configured timeout; do not promise synchronous preemption of an underlying blocking `Read`.

### FIND-TASK-003-6 — newly added deadline proof sleeps

**Root cause and defect class.** The deadline half of `interruption_during_a_complete_archive_never_publishes` sets a deadline one second ahead, then its archive request hook polls `Instant::now()` and sleeps 5 ms until expiry (`snapshot.rs:1791-1799`). This violates `architecture/references/languages/implementation-execution.md:224-229`, which prohibits adding sleeps instead of deterministic synchronization. The focused test passes in about 1.01 s, but that success depends on real elapsed time. No other added sleep was found in the candidate's changed tests.

**Correction outcome and boundary.** Remove the wait loop and prove deadline expiry through an already-expired `Execution` at the ingest owner, where the archive path can be exercised without waiting. Retain the public cold-build cancellation case and its prior-pointer/no-object assertions. If an early public-facade expiry assertion remains, label it as early rejection; it does not prove expiration during an archive read. No fake-clock framework, timer abstraction, dependency, or weakened deadline assertion is needed.

## Constraints and preserved behavior

- Keep all r1 FIND-1, FIND-3, FIND-4, and FIND-5 closures: record-before-write, rejected-entry coverage, `Snapshots` IO owner, and bare imported signature types.
- Keep exact identity/blob checks, complete-only pointer publication, source-free deterministic snapshots, LFS/oversize classifications, all byte and fallback ceilings, prior-pointer isolation, and the existing interruption outcome types.
- Preserve the caller's independent deadline and cancellation; add no library-wide maximum, new storage/provider trait, backend, worker, source persistence, or TASK-004/005 behavior.
- Follow `AGENTS.md` rustdoc and test-placement rules. Correct the proof without deleting required coverage or replacing it with a trivial assertion.

## Acceptance criteria and proof

| Finding and sibling | Acceptance criterion | Focused proof |
|---|---|---|
| FIND-2: `entry.read_to_end` | A cancellation after the first chunk of an eligible archive entry prevents any later body chunk from being read; the call returns `Incomplete(Interrupted(Cancelled))`, marks pending eligible paths `NotFetched`, writes no object, and leaves the prior pointer unchanged. | A controlled streaming fake body through the cold-build facade. |
| FIND-2: tar iterator advancement across an unselected entry | A cancellation before a large unselected entry is drained stops archive consumption at the shared read boundary with the same incomplete/no-publication behavior. | A separate controlled archive stream with an unselected entry, or one focused test covering both reader sites. |
| FIND-2: deadline read boundary | The archive ingest owner recognizes an already-expired deadline during consumption without waiting for wall time; it returns the deadline interruption and does not publish. | Focused owner test with `Execution::new(Instant::now(), ...)` or equivalent controlled expiry. |
| FIND-2: sibling behavior | Compressed and expanded bounds, blob verification, existing fallback reasons, and the final pre-publication check still pass. | Existing snapshot owner tests plus the new streaming proof. |
| FIND-6: deadline test at `snapshot.rs:1791-1799` | No sleep or real-time polling remains in the corrected deadline proof; cancellation and deadline variants still have meaningful assertions. | Focused exact test command and source inspection. |

Run the exact focused selectors for the added/modified tests using `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib <test-name> -- --exact`, recording that each selects and passes one test. Then run `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `git diff --check`, and inspect tracked/untracked changes. The implementation must record RED/GREEN and final verification per the task workflow.

Expected write set: `crates/bohmian-code-review-pipeline/src/snapshot.rs` and, only if the controlled streaming body needs a shared fake capability, `crates/bohmian-code-review-pipeline/src/testing.rs`. Do not edit the reviewed r2 verdict. Route this task directly to `$cr-implement`.
