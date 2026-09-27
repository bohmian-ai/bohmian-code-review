---
id: TASK-005-R1
kind: remediation
status: proposed
spec: SPEC-repo-parser@12
parent_task: TASK-005
remediates: [FIND-TASK-005-1, FIND-TASK-005-2, FIND-TASK-005-3, FIND-TASK-005-4]
---

# Restore validated context publication and honest PR coverage

Approved authority: `changes/active/repo-parser/spec.md` revision 12. Original task: `changes/active/repo-parser/tasks/TASK-005-pr-context-and-shared-read.md`. Review subject: base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`, candidate `e68bcdb65b44095989b5f42dfda5c7d87295582b`. Implement the four validated corrections below against the cumulative candidate. See `findings-validation.md` for independent reproduction and the Wave 1 reports for acceptance coverage.

## Diagnosis and required outcomes

### FIND-TASK-005-1 — Validate an adopted context object

`Contexts::publish` treats `ObjectStorage::create` returning `AlreadyExists` as proof of matching content (`context.rs:315–319`). The trait promises only that the key is occupied (`storage.rs:216–228`). A corrupt or different object already at the digest-derived key lets the public `PrContext::run` return a context ID that `Contexts::read` immediately rejects as `Corrupt` (`context.rs:355–365`). The existing dropped-call proof covers absent objects, not wrong existing bytes. This violates the complete validated object and readable retry obligations in REQ-011–013 and TASK-005 Scenario 3. The sibling effects are the publication success result and downstream read failure; there is one publication owner.

At the `Contexts::publish` owner, verify exact stored bytes before accepting `AlreadyExists`, reusing the existing `Snapshots::publish` read-and-compare behavior (`snapshot.rs:703–711`). A mismatch or failed read must be an explicit error; equal bytes remain an idempotent success. The correction needs no storage trait or backend expansion. Prove both mismatched and matching preexisting objects with an injected store, including the returned ID's read-back.

### FIND-TASK-005-2 — Give each publication its own lifetime and cleanup target

The context ID and key are solely the serialized-content digest (`context.rs:298–306`), while each call replaces the record with a fresh `expires_at` (`context.rs:313–315`, `persistence.rs:243–253`). Publishing identical bytes later extends the first returned ID's 24-hour read window. The same alias lets `collect_expired` act on a copied expired row (`context.rs:394–403`) after an upsert refreshes that ID: it can delete a fresh object's bytes and record or leave an object without its record. `MemoryContextStore` replaces, lists, and deletes by the shared `(identity,id)` (`testing.rs:403–468`). Sequential expiry and cleanup tests do not cover the alias. This violates REQ-012–015, INV-005, and TASK-005 Scenario 3. Siblings are the old ID's read authorization, new publication success, object deletion, and record deletion.

At the candidate-owned context identity/record boundary, give logically new publications distinct IDs and object keys while keeping a separate content digest for integrity. A digest plus publication time is a sufficient identity shape; retrying one durable step with the same identity must remain idempotent. A later dropped-call retry may produce a new readable ID, as the approved spec requires convergence to a valid result rather than the same ID. Keep record-before-object publication, object-before-record cleanup, exact tenant-scoped key checks, and the 24-hour cutoff of each returned ID. Do not replace this with independent guards at read and delete sinks. Prove two identical-content publications at different paused-clock times have distinct IDs; the first expires at its original cutoff while the second remains readable. Pause collection after it lists the first, publish the second, then resume: only the first may be removed. Include a dropped object-create retry and assert no unrecorded object or readable half-context. This is a correction to candidate-owned ID/record semantics, not a change to a spec-named public interface.

### FIND-TASK-005-3 — Spend scoped capacity on eligible files

In the no-snapshot/scoped path, `near` takes `max_scoped_files` from raw nearby file/executable entries (`pull_request.rs:543–551`) before `Ingest::new` applies shared eligibility (`pull_request.rs:180–190`, `eligibility.rs:244–297`). An unsupported, generated, vendored, or oversized neighbor consumes a slot and can hide an eligible same-directory symbol source beyond the raw cap. The scoped result then omits that source from ranking without an eligible-file omission. The current S10 fixture has only eligible neighbors. This violates REQ-008 and the task's useful scoped fallback. The sibling exclusion classes share this one selection boundary.

Reuse the existing eligibility classification when selecting nearby entries, before spending the eligible-file ceiling. Preserve changed-path priority, bounded blob requests, and explicit scoped coverage/exclusion reporting; do not create a second eligibility rule. Prove a low-limit fake-provider scoped PR with each exclusion class ahead of a matching eligible neighbor selects that neighbor while respecting the eligible bound and reporting any remaining eligible omission.

### FIND-TASK-005-4 — Mark rejected provider entries as incomplete coverage

`GitHubClient::pull_request_files` can reject invalid file records (`github.rs:1141–1157`). `PrContext::run` records them as `NotAnalyzed` (`pull_request.rs:199–208`), yet `ContextCoverage::is_full` checks only the pagination ceiling and other coverage flags (`context.rs:57–65`, `pull_request.rs:219–232`). Invalid SHA, status, path, or previous path can therefore coexist with a full-coverage result. Rejected head-tree entries are counted only as `InvalidEntry` exclusions (`snapshot.rs:994–1010`), and `Coverage::is_complete` ignores those exclusions (`snapshot.rs:130–134`). A missing candidate under either path can falsely look like repository-wide absence. Existing moving-PR and malformed-patch tests do not exercise rejected provider records. This violates REQ-009–010 and INV-002. The rejected PR-file reasons and rejected tree entries are sibling sources of unknown coverage.

Carry rejection completeness from both provider lists into the existing coverage axes. A rejected PR entry must make PR-file coverage partial; a rejected tree entry must make snapshot/overlay coverage partial unless validated facts prove it ineligible. Retain exact rejection diagnostics, excluded counts, valid-file processing, and base/head pinning. Do not treat an ordinary validated unsupported path as a coverage failure. Prove separate fake-provider invalid PR-file and invalid tree-entry cases each return `!is_full()` with their reason, while valid entries still yield context and a validated unsupported path remains a benign exclusion.

## Write set and constraints

Own the correction across `crates/bohmian-code-review-pipeline/src/context.rs`, `pull_request.rs`, `persistence.rs`, `testing.rs`, `snapshot.rs`, `github.rs`, `eligibility.rs`, and their nearest inline tests as needed by the root-cause changes. Keep the six symbolic crates unchanged. Preserve source-free persistence metadata, tenant/installation/repository isolation, exact-key reads, digest validation, bounded provider and source work, diff separation, file-local omissions, and the existing public symbolic journey. No model call, comment publication, consumer adapter, detached task, deadline, new dependency, or unrelated refactor belongs here.

## Acceptance and verification

Each finding closes only when the focused proof above exercises every named sibling sink and its positive counterpart. Run the outer crate's scoped lane, the public symbolic journey, `fmt:check`, `lints`, and `gate` using `mise.toml` tasks; record exact commands, results, and `git diff --check`. Preserve the existing tests and report any unmet prerequisite or remaining limit. Route this task directly to `$cr-implement`; review the cumulative implementation against TASK-005 and revision 12 afterward.

## Implementation evidence

Commits `b82fb19` (FIND-1, FIND-2) and the following FIND-3/FIND-4 commit, on top of `0f8e31c`. Symbolic crates unchanged; no dependency, trait, or backend change.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| FIND-TASK-005-1: adopted object validated | `Contexts::publish` reads and compares bytes on `AlreadyExists` (reuses `Snapshots::publish` shape); mismatch → `Corrupt` | `context::tests::existing_object_is_adopted_only_when_identical` (mismatch fails; equal bytes adopt and read back) | PASS |
| FIND-TASK-005-2: per-publication id, lifetime, cleanup target | id = `<digest>-<publication nanos>`; record keeps `digest` for integrity; record-before-object and object-before-record order unchanged | `context::tests::identical_publications_expire_and_clean_up_independently` (distinct ids/keys, first expires at original cutoff, sweep held after listing the first removes only the first); `pull_request::tests::dropped_publication_leaves_no_readable_half_context` (dropped upsert/create retry: every object recorded, half context not found, retry id reads back) | PASS |
| FIND-TASK-005-3: scoped bound spent on eligible files | `near` no longer caps; `Ingest::new` classifies every nearby entry; fallback bound = `max_scoped_files - indexed`, remainder reported `NotFetched` | `pull_request::tests::scoped_bound_skips_excluded_neighbors` (unsupported, generated, vendored, minified, oversize ahead of `src/user.ts`; user selected, `src/zz.ts` NotFetched, eligible 3 / indexed 2) | PASS |
| FIND-TASK-005-4: rejected provider entries lower coverage | `ContextCoverage::pr_files_rejected` checked by `is_full`; `Coverage::is_complete`/`unindexed` count `InvalidEntry` | `pull_request::tests::rejected_pr_file_is_not_full_coverage`, `rejected_head_tree_entry_is_not_full_coverage`; `snapshot::tests::rejected_tree_entries_are_counted_as_invalid_exclusions` (cold build now `Incomplete { count: 2 }`, unpublished); benign unsupported `README.md` still full in `concurrent_prs_*` (`is_full` assert) | PASS |

Commands, all exit 0 in this session: `mise run test:pipeline` (61 passed), `mise run test:symbolic-context:journey` (6 passed), `mise run fmt:check`, `mise run lints`, `mise run gate`, `git diff --check 0f8e31c HEAD`.

Non-goals held: no model call, comment publication, consumer adapter, detached task, deadline, new dependency, or storage/persistence trait change. Behavior change to note for review: a default-branch tree with any rejected listing entry now yields an incomplete (unpublished) snapshot, so such repositories take the scoped path; `ContextCoverage` gains `pr_files_rejected` (format version unchanged, unreleased).
