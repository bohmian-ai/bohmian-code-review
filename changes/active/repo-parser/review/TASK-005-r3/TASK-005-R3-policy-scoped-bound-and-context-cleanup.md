---
id: TASK-005-R3
kind: remediation
status: proposed
spec: SPEC-repo-parser@12
parent_task: TASK-005
remediates: [FIND-TASK-005-4, FIND-TASK-005-6, FIND-TASK-005-7]
---

# Close policy, scoped-capacity, and context-cleanup gaps

Approved authority: `changes/active/repo-parser/spec.md` revision 12. Original task: `changes/active/repo-parser/tasks/TASK-005-pr-context-and-shared-read.md`. Prior remediation: `review/TASK-005-r1/TASK-005-R1-context-publication-and-coverage.md` and `review/TASK-005-r2/TASK-005-R2-rejected-entry-classification-and-changed-paths.md`. Immutable review subject: base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`, candidate `9ce990fe933ff69de464b3ffc0a4c073d1c4754b`. Validated evidence: `changes/active/repo-parser/review/TASK-005-r3/findings-validation.md`.

## FIND-TASK-005-4 — rejected root eligibility policy remains misclassified

**Diagnosis.** R2 made rejected entries benign when their canonical path is unsupported (`eligibility.rs:268–275,304–310`), but root `.gitattributes` is an input to the generated/vendored eligibility policy even though it is not a symbolic source. `GitHubClient::tree` can reject its canonical row for malformed SHA or mode (`github.rs:971–986,1381–1392`); `root_attributes` then sees only accepted entries and supplies defaults (`snapshot.rs:750–763`). A rule-sensitive supported path can be excluded or indexed under the wrong policy, while `Ingest::finish` counts the rejected policy row as `Unsupported`, not unknown (`snapshot.rs:1010–1027`). Cold build can publish a falsely complete snapshot and advance its pointer (`snapshot.rs:594–600,656–676`); update shares that path (`update.rs:88–102`); warm PR overlay may claim full coverage (`pull_request.rs:162–180,225–238`); scoped PR context can omit the root rejection from nearby diagnostics and apply the wrong policy (`pull_request.rs:182–190,552–573`). This reopens FIND-4 as `CLASS_INCOMPLETE`: the prior correction treated every unsupported path as independent of eligibility. It violates REQ-005/006/009, INV-002 and the R2 proven-ineligible exception. Existing R2 tests reject ordinary `README.md` and supported paths, so they do not cover the policy input.

**Outcome and correction.** At the shared root-policy/eligibility boundary, classify a rejected canonical root `.gitattributes` as unresolved eligibility policy, regardless of its source-language status. Carry that uncertainty into repository coverage and scoped diagnostics. Reuse the existing `RelativePath`, root-attribute classification, `InvalidEntry`/unindexed coverage and partial-result mechanisms. Continue using verified valid files for useful context. Preserve benign exclusion of an ordinary proven-ineligible rejected path; never fetch a rejected Git identity or silently trust default root policy as complete.

**Closure proof.** A fake-provider tree with a rejected invalid-SHA root `.gitattributes` and a supported path affected by a generated/vendored override keeps cold and update pointers unchanged, makes the warm overlay incomplete, and records policy uncertainty in scoped context while retaining valid nearby work. A valid root policy and a rejected ordinary unsupported path still allow their current complete outcomes. Cover all four sibling paths.

## FIND-TASK-005-6 — changed heads exceed the scoped index ceiling

**Diagnosis.** In a scoped PR run, `PrContext::run` passes every accepted PR file to `changed_head` before calculating `max_scoped_files.saturating_sub(ingest.indexed())` for fallback (`pull_request.rs:210–224`). `changed_head` ingests each pending, verified blob without scoped capacity admission (`pull_request.rs:375–415`). With two valid changed heads, no snapshot, and `max_scoped_files = 1`, both may be indexed before the fallback bound is applied. Indexed count and scoped indexing work exceed the configured ceiling; the current one-head scoped test cannot detect it. The same path applies to every accepted changed head. This violates REQ-008/013 and the public limit's stated meaning. The root cause is a capacity rule applied only to the later fallback path.

**Outcome and correction.** Enforce one scoped index-file ceiling across changed-head ingestion and fallback at the PR-to-ingest admission owner. Continue verifying, fetching and analyzing every valid changed head for its diff and ranking obligations under REQ-009/010; the scoped index cap must not discard changed-file analysis or be misreported as a cap on mandatory PR-head reads. Reuse `Ingest`'s indexed count and pending/`NotFetched` coverage to identify eligible paths not indexed. Keep repository-scope overlay behavior and changed-path secondary exclusion intact.

**Closure proof.** In a scoped fake-provider PR with two or more valid changed heads and one scoped index slot, all valid heads contribute analysis or explicit file-local failure, at most one path enters the scoped index, the remaining eligible paths are reported unindexed, and fallback does not take another index slot. Verify the one-head positive case and a warm overlay control. The proof must observe indexed count, omission/coverage, ranking inputs and provider calls so it distinguishes mandatory changed-head reads from extra scoped-index work.

## FIND-TASK-005-7 — expiry cleanup races an in-flight publication

**Diagnosis.** `Contexts::publish` upserts a record with `expires_at = now + 24h`, then awaits object creation (`context.rs:316–338`). `ContextStore::expired_contexts` selects solely by expiry (`persistence.rs:266–275`), and `Contexts::collect_expired` can delete the absent object and record (`context.rs:407–431`) while that create remains pending. If create completes afterward, `PrContext::run` can return an unreadable ID (`pull_request.rs:293–300`); source bytes can remain in an object with no record, so no later sweep can discover them. If bytes were written before the stalled future resumes, cleanup can retire the object and a resumed publication can still report success. The existing dropped-call and expiry tests do not interleave cleanup with the same pending publication. This violates REQ-012/013/014/015, INV-005 and TASK-005 Scenario 3. The candidate-owned context cleanup contract has no safe collection cutoff, unlike snapshot cleanup; the library intentionally has no invocation maximum.

**Outcome and correction.** Separate the fixed 24-hour **read** cutoff from safe physical collection. At the context cleanup/store boundary, let the caller supply a collection cutoff or grace longer than its publication timeout, following the approved snapshot `collect(older_than)` rule (`spec.md:63`) and caller-owned timeout rule (`spec.md:76`). A sweep must not retire records that an allowed publication may still write. Document the caller cutoff obligation in the public facade and injected store contract. Preserve record-before-object publication, object-before-record cleanup, missing-object success, identity scoping, lag reporting, idempotent retry and reads failing at or after `expires_at`. No library timeout, new backend, held transaction across storage IO, or change to the 24-hour read policy is authorized.

**Closure proof.** Pause object creation after record upsert, advance to the read expiry, sweep with a safe caller cutoff, and show the record remains while `read` rejects expiry. Resume creation and show no unrecorded object or false success. Drop another publication after upsert; once its caller timeout and collection grace have passed, show object-then-record cleanup removes its record, including the absent-object case. Recheck ordinary expiry, independent publications, failed deletion lag and retry.

## Scope and verification

The write set covers the existing outer-crate owners and nearest tests as needed: `crates/bohmian-code-review-pipeline/src/eligibility.rs`, `snapshot.rs`, `update.rs`, `pull_request.rs`, `context.rs`, `persistence.rs`, and `testing.rs`. Include any direct public facade tests affected by the context cleanup contract. Keep symbolic crates, storage backend implementations, grammar pins and source lifetime boundaries unchanged. Do not add models, comments, consumer scheduling, detached work, new dependencies or unrelated refactors.

Run exact focused selectors for the three gap scenarios and every sibling, then `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check`. Record RED/GREEN and final results. Route directly to `$cr-implement`; the next task review inspects the complete cumulative candidate.

## Implementation Evidence

FIND-4 follows `maintainer-decision-rejected-tree-entries.md`, not the correction above. Commits `04c1abb` (FIND-4, FIND-6) and `3959da4` (FIND-7).

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Unknown mode or malformed SHA fails the tree listing; nothing publishes, pointer unmoved | `github.rs` `GitHubClient::tree`, `validate_tree_entry` → `MalformedResponse` | `github::tests::verified_provider_identities_cross_and_mismatches_are_rejected`, `snapshot::tests::broken_tree_entry_fails_the_build`, `update::tests::broken_tree_entry_keeps_the_pointer`, `pull_request::tests::broken_head_tree_entry_fails_the_run` | PASS |
| Non-canonical tree path is an `Unsupported` exclusion; snapshot stays complete | `TreeListing::invalid_paths`, `Ingest::new`/`finish` | `snapshot::tests::non_canonical_tree_path_is_an_unsupported_exclusion`, `pull_request::tests::non_canonical_head_tree_path_keeps_full_coverage` | PASS |
| Tree `rejected` list, `ExclusionReason::InvalidEntry`, and every consumer deleted (cold, update, overlay, scoped `near`) | `github.rs`, `eligibility.rs`, `snapshot.rs`, `pull_request.rs` | `rg InvalidEntry\|listing.rejected` empty outside `changes/` | PASS |
| PR-file rejection (FIND-5) unchanged | `Pinned::rejected`, `rejected_changed` | `rejected_pr_file_is_not_full_coverage`, `rejected_changed_path_is_never_secondary` | PASS |
| FIND-6: one scoped index ceiling across changed heads and fallback; every valid head still analyzed | `PrContext::changed_head` `index_cap` | `pull_request::tests::changed_heads_share_the_scoped_index_ceiling` (RED without cap: indexed 2; GREEN: eligible 7, indexed 1, `src/pr1.ts` NotFetched, `not_analyzed` empty, 3 blob calls = root attributes + two heads); one-head case `scoped_bound_skips_excluded_neighbors`; warm controls unchanged | PASS |
| FIND-7: caller collection cutoff separate from 24h read expiry; in-flight publication never orphaned | `Contexts::collect_expired(older_than, ..)`, `ContextStore::expired_contexts` contract, `MemoryStorage::hold_creates` | `context::tests::cleanup_cutoff_spares_in_flight_publication` (record kept while read is `Expired`; resumed create leaves recorded object; dropped-after-upsert and held publications both cleaned after grace); existing ordinary expiry, independent publication, lag/retry tests | PASS |

Commands: `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate` — all exit 0 in this session. Non-goals held: symbolic crates, storage backends, grammar pins, dependencies, and the 24-hour read policy untouched.
