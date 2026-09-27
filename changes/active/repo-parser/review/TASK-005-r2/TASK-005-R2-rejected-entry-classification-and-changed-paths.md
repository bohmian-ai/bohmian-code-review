---
id: TASK-005-R2
kind: remediation
status: proposed
spec: SPEC-repo-parser@12
parent_task: TASK-005
remediates: [FIND-TASK-005-4, FIND-TASK-005-5]
---

# Classify rejected entries and retain known changed paths

Approved authority: `changes/active/repo-parser/spec.md` revision 12. Original task: `changes/active/repo-parser/tasks/TASK-005-pr-context-and-shared-read.md`. Prior remediation: `changes/active/repo-parser/review/TASK-005-r1/TASK-005-R1-context-publication-and-coverage.md`. Immutable review subject: base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`, candidate `31e71a2e8867215e0c3f8958bdcee5f94da17ad4`. Validated root-cause evidence: `changes/active/repo-parser/review/TASK-005-r2/findings-validation.md`.

## FIND-TASK-005-4 — prior correction incomplete

**Diagnosis.** The R1 fix made `Coverage::unindexed` count every `InvalidEntry` (`snapshot.rs:130–150`). `GitHubClient::tree` rejects a malformed row before eligibility classification (`github.rs:967–985,1382–1404`), and `Ingest::new` receives only its rejection count (`snapshot.rs:778–813`). A canonical unsupported `README.md` with invalid SHA is therefore counted as a possibly eligible missing source even though the existing path-language query proves it ineligible (`eligibility.rs:264–280`). `ColdBuild::complete` returns `Incomplete(UnindexedFiles)` and never publishes a snapshot for that unchanged tree (`snapshot.rs:637–681`); `Update::run` shares the same path and cannot advance its pointer. A warm PR overlay is marked incomplete for the same proven exclusion. The scoped PR branch passes zero rejections (`pull_request.rs:175–190`), losing nearby rejected-entry diagnostics, although its scope remains partial. Current rejected-tree tests cover possibly eligible paths and therefore cannot establish the required distinction. This violates REQ-005/006/009, INV-002, and R1 FIND-4's explicit proven-ineligible exception.

**Required outcome and correction boundary.** At the shared provider-to-eligibility boundary, retain validated rejection facts sufficient to distinguish proven ineligibility from unknown or possible eligibility. Reuse the existing `RelativePath` and eligibility rules; do not add a second policy. A rejected entry may be a benign exclusion only when trusted facts prove it ineligible. Unknown or possibly eligible rejected entries must still make cold builds, updates and overlays incomplete. Carry the same classification and reason into coverage, including nearby rejected entries in scoped PR context. This replaces the blanket `InvalidEntry` completeness rule at its owner; preserve strict validation of Git identities and bytes, prior pointer on incomplete attempts, valid-file processing, bounded scoped work, and explicit diagnostics. The correction must not claim repository coverage from a scoped result.

**Acceptance/proof.** With a valid eligible source and invalid-SHA canonical unsupported path, a fake-provider cold build publishes a complete snapshot; a same-class incremental update can advance the pointer; a PR overlay reports the proven exclusion without false incompleteness. With an invalid supported path or unknown eligibility, the cold/update pointer stays unchanged, the warm overlay remains incomplete, and scoped context retains the rejection reason/count while still returning valid nearby context. Check the existing unknown-mode rejection case. The proof must cover cold, update, warm overlay and scoped diagnostic siblings.

## FIND-TASK-005-5 — rejected PR file selected as secondary

**Diagnosis.** `validate_pull_request_file` validates a canonical filename before status/SHA (`github.rs:1404`), but rejected PR records leave `PrContext::run`'s `changed` set because it is built only from accepted `pinned.files` (`pull_request.rs:234–236`). A rejected `src/user.ts` entry with valid filename and invalid SHA/status can remain a verified source in the pinned head tree. If another changed file references it, ranking and packing can publish `src/user.ts` as secondary context. Rejected entries already become `NotAnalyzed` and lower PR-file coverage, but neither prevents this REQ-009 violation. The accepted-only changed set also seeds scoped nearby selection (`pull_request.rs:175–190`). The existing rejection test uses an unrelated path and misses the failure.

**Required outcome and correction boundary.** Preserve the canonical changed filename independently when the rest of a PR-file record is rejected. Exclude the union of accepted and canonically identifiable rejected changed paths from secondary candidates, and use that identity in scoped nearby selection where applicable. Reuse `RelativePath` validation. Never treat invalid filename, status, SHA, previous path or patch as trusted content; retain rejection diagnostics and partial coverage. No provider or storage trait expansion is required.

**Acceptance/proof.** A fake-provider PR with a rejected canonical rankable changed path and another valid changed file must still produce valid context, list the rejection as not analyzed, and never select the rejected path as secondary. Accepted changed paths remain excluded. An invalid filename remains only a diagnostic and cannot seed a trusted path. Exercise both snapshot-backed and scoped selection where the changed-path set matters.

## Scope and verification

The write set includes the existing outer-crate owners of tree/PR validation, shared eligibility and coverage, cold/update publication, PR overlay/ranking, and their nearest tests: `github.rs`, `eligibility.rs`, `snapshot.rs`, `update.rs`, and `pull_request.rs` as needed. Keep the six symbolic crates, storage and persistence traits, grammar pins, and source-lifetime boundaries unchanged. Do not add model calls, comments, consumer backends, new dependencies, detached work or unrelated refactors.

Run exact focused test selectors for new scenarios, then `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check`. Record Red/Green and final results. Route directly to `$cr-implement`; the next review must inspect the cumulative candidate.

## Implementation Evidence

Candidate: `2a1d8d5`. One owner classifies rejected tree entries: `eligibility::classify` now takes the listing's `RejectedPath`s and applies the same path rules (`path_exclusion`, extracted from `exclusion`) to canonical paths. A proven-ineligible entry is excluded under its path reason; any other rejected entry is counted in `EligibleTree::unknown` and reported as `InvalidEntry`, which alone keeps `Coverage::is_complete` false. `Ingest::new` passes the rejected entries through, so cold build, update, warm overlay and scoped context share the rule; `near` also selects nearby rejected entries for scoped diagnostics. `PrContext::run` keeps canonical filenames of rejected PR-file records in the changed set (secondary-candidate exclusion) and in scoped seeds. Superseded: the `usize` rejection count and the scoped `0` argument.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Rejected entry excluded only when its canonical path proves ineligibility | `eligibility::classify`, `path_exclusion` | `eligibility::tests::rejected_entries_are_excluded_only_when_their_path_proves_it` (unsupported, generated, vendored-segment → excluded; supported and non-canonical → unknown 2) | PASS |
| Cold: invalid-SHA `README.md` publishes; invalid-SHA `src/bad.ts` does not | `Ingest::new` | `snapshot::tests::rejected_entry_blocks_publication_only_when_it_may_be_eligible` | PASS |
| Update: proven exclusion advances pointer; supported rejection keeps it | shared `ColdBuild::start` | `update::tests::rejected_entry_keeps_the_pointer_only_when_it_may_be_eligible` | PASS |
| Warm overlay: proven exclusion keeps full coverage; unknown/unknown-mode stays incomplete | shared `Ingest` | `pull_request::tests::proven_ineligible_head_tree_entry_keeps_full_coverage`; existing `rejected_head_tree_entry_is_not_full_coverage` and `snapshot::tests::rejected_tree_entries_are_counted_as_invalid_exclusions` (unknown mode, invalid path) | PASS |
| Scoped: nearby rejection counted, far one not, valid context returned | `near` | `failed_cold_build_falls_back_to_scoped_context` (`InvalidEntry` = 1 for `src/bad.ts`, not `lib/bad.ts`) | PASS |
| Rejected canonical changed path never secondary; seeds scoped selection; invalid filename diagnostic only | `PrContext::run` `rejected_changed` | `pull_request::tests::rejected_changed_path_is_never_secondary` (warm and scoped; `src/user.ts` not selected, `lib/` seeded only via rejected `lib/gone.ts`, `src/../x.ts` only in `not_analyzed`) | PASS |

RED: with both fixes neutralized, the five new tests failed (61 passed, 5 failed). Commands (this session, all exit 0): `cargo test --all-features -p bohmian-code-review-pipeline`, `mise run test:pipeline` (66), `mise run test:symbolic-context:journey` (6), `mise run fmt:check`, `mise run lints`, `mise run gate`, `git diff --check`.

Limits: a proven-ineligible rejected entry is counted under its path reason; its provider rejection text is not retained in snapshot coverage. Oversize cannot be proven for a rejected entry (size is not carried), so it stays unknown. Non-goals held: symbolic crates, storage/persistence traits, grammar pins, dependencies untouched. Changed files: `eligibility.rs`, `snapshot.rs`, `update.rs`, `pull_request.rs`, this task.
