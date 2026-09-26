---
id: TASK-003-R3
kind: remediation
status: ready
spec: SPEC-repo-parser@11
remediates: TASK-003
review: changes/active/repo-parser/review/TASK-003-r3/verdict.md
findings: [FIND-TASK-003-2, FIND-TASK-003-7]
---

# TASK-003-R3 — Publication interruption and Git attribute resets

## Identities and scope

- Approved spec: `changes/active/repo-parser/spec.md` revision 11.
- Original task: `changes/active/repo-parser/tasks/TASK-003-cold-snapshot-and-scoped-context.md`.
- Prior remediation: `changes/active/repo-parser/review/TASK-003-r1/TASK-003-R1-publication-order-cancellation-coverage.md` and `changes/active/repo-parser/review/TASK-003-r2/TASK-003-R2-archive-read-cancellation-and-deadline-proof.md`; their verdicts are the prior `FIND-*` ledger.
- Reviewed base: `60cc56e4213aa90f2a13d3ac6b3d1de536ff81b2`; cumulative candidate: `d95346349988fed1d79dcb6e8b532b185e05b563`. TASK-003 began in `741c049` before the base, so review covered the complete candidate `snapshot.rs`, `eligibility.rs`, `testing.rs`, and the base-to-candidate diff.
- Validated diagnosis and citations: `changes/active/repo-parser/review/TASK-003-r3/findings-validation.md` and `citations.txt`.

## Issue diagnosis and required outcome

### FIND-TASK-003-2 — CLASS_INCOMPLETE: interruption can outlive ingest checks

**Root cause and defect class.** `ColdBuild::run` checks `Execution` at `snapshot.rs:525`, then finishes facts and serializes the snapshot before calling `publish` at `:552`. The candidate-owned `ColdBuild::publish` has no execution handle. It records at `:582`, creates/adopts at `:583`, reads the object back at `:587`, then advances the pointer at `:600-602` without observing cancellation or deadline expiry. The matching-pointer reuse branch at `:465-472` also returns `Reused` after a storage read without a post-read check. These are sibling completion paths of one missing execution-boundary defect. The R1 final check and R2 read-boundary wrapper cover ingest, but their shape permits interruption after the final check.

**Obligation, reproduction and consequence.** Spec REQ-005 and REQ-013 require caller-supplied deadline/cancellation and honest partial progress; TASK-003 Scenarios 2–3 require an interrupted attempt to leave the prior pointer intact. Public `ColdBuild::run` reaches `publish`. An injected `ObjectStorage::read` can trip the shared cancellation token before returning valid bytes; the current code then moves the pointer and reports `Published`. The same hook on the reuse read can report `Reused`. Existing archive tests cannot exercise storage IO, so their green result falls short. A deadline expiring during the same work has the same gap. An IO call already in progress need not be synchronously preempted.

**Decision-complete correction.** Reopen FIND-2 at the `ColdBuild` completion boundary: carry its existing `Execution` through publication, check after potentially blocking read-back and immediately before conditional pointer publication, and return the existing `Incomplete(Interrupted(..))` with computed coverage and ingest statistics when interruption is observed. Check after a matching-pointer object read before returning `Reused`. Reuse `Execution::check` and the existing interruption outcome; keep record-before-write and any already written object collectable. Preserve identity validation, byte-equal read-back, conditional pointer semantics and conflict handling. Cancellation racing inside the atomic store call follows that call's commit semantics; do not promise reversal of a committed pointer. No new trait, worker, timer, backend or deadline maximum is needed.

### FIND-TASK-003-7 — Git attribute reset admits excluded paths

**Root cause and defect class.** `parse_attribute` in `eligibility.rs:136-144` maps both `-linguist-*` and `!linguist-*` to `false`. `RootAttributes::parse/resolve` (`:108-132`) carries that false into `exclusion`, where `Some(false)` bypasses built-in vendor-segment and minified-output exclusions (`:279-283`). Git assigns `!attr` the *unspecified* state, while `-attr` is explicitly false. The internal representation collapses these states. Both `!linguist-vendored` and `!linguist-generated` are siblings of one root cause.

**Obligation, reproduction and consequence.** The spec's eligible-file contract and TASK-003 approach require root `.gitattributes` overrides to match Git semantics and preserve reasoned exclusions. In a temporary Git repository, `git check-attr linguist-vendored -- vendor/a.ts` with `vendor/a.ts !linguist-vendored` returns `unspecified`. The candidate resolves it to `Some(false)` and indexes `vendor/a.ts`; `!linguist-generated` similarly admits a `.min.js` path. Coverage can count unwanted files as eligible and publish their facts. Current attribute tests cover explicit false, not reset, so they do not prove the distinction.

**Decision-complete correction.** At the existing `RootAttributes` parser/resolver, retain separate states for explicit set, explicit false and reset-to-unspecified, with later matching rules taking precedence. Feed unspecified through the existing exclusion logic so built-in vendor/minified rules still apply. Keep `-attr` and `attr=false` as explicit false overrides. The representation is candidate-owned internal state; no new dependency or public API is needed.

## Constraints and preserved behavior

- Keep the r1 closures for recorded object tracking, rejected tree-entry coverage, concrete `Snapshots` IO ownership and imported bare signature types; keep the r2 archive-read cancellation and deterministic deadline proof.
- Keep verified Git blob and snapshot identity checks, source-free deterministic serialization, complete-only pointer movement, fallback accounting, byte/fallback bounds, tenant isolation and cleanup.
- Do not add scoped PR indexing, incremental update, backend, source persistence, consumer scheduler, library deadline maximum or new trait.
- Use repo-local Rust doc and inline owner-test rules. The correction belongs in `crates/bohmian-code-review-pipeline/src/snapshot.rs` and `eligibility.rs`; `testing.rs` may change only if a shared fake capability is necessary for the focused proof. Include any directly affected caller within these owners; do not narrow the write set below the root-cause correction.

## Acceptance and focused proof

| Finding / sibling | Required result | Focused proof |
|---|---|---|
| FIND-2: record/create/read-back before pointer CAS | Cancellation from an injected storage create or read-back yields `Incomplete(Interrupted(Cancelled))`, preserves the prior pointer and keeps any written object tracked for collection. | Controlled in-memory storage hook through public `ColdBuild::run`; inspect pointer, outcome and stored record. |
| FIND-2: deadline before CAS | An expired deadline at the final publication boundary yields `Interrupted(DeadlineExceeded)` and no pointer movement, without a sleep. | Focused owner proof with an already-expired execution at that boundary; no wall-clock polling. |
| FIND-2: matching-pointer reuse read | Cancellation during object read does not return `Reused`; it reports interruption without mutating the pointer. | Controlled read hook on the reuse path. |
| FIND-7: `!linguist-vendored` and `!linguist-generated` | Reset resolves to unspecified and built-in vendor/minified exclusions apply, even after a prior set rule. | Focused `eligibility` owner test with both reset cases and last-match order. |
| FIND-7: explicit false and ordinary set | `-attr`/`attr=false` still override built-in exclusions; set and unrelated attributes retain prior behavior. | Same focused owner test plus existing attribute tests. |

Run exact focused selectors for the new or modified tests using `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib <module::test> -- --exact`, confirming one selected test each. Record RED/GREEN and final evidence. Then run `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `git diff --check`, and inspect tracked/untracked changes. Route directly to `$cr-implement`; review the next cumulative candidate against original TASK-003 and both prior remediation tasks.
