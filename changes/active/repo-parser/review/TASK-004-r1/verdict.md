# TASK-004 r1 verdict — PASS

## Immutable subject

Base `a77c64d8194ff3b8e0bc9f51f68b2cc5ae0095ec`; candidate `f5a6f4bbeae1877083f79f4198316205c75a63e8`. Approved authority: `SPEC-repo-parser@12`, original TASK-004, `AGENTS.md`, and applicable architecture references. The complete four-file base-to-candidate diff was reviewed; HEAD remained at the candidate.

## Acceptance matrix

| Obligation | Candidate evidence | Verification evidence | Result |
|---|---|---|---|
| S3 / REQ-007: changed, deleted, renamed and newly ineligible paths match a fresh parse; old snapshot stays immutable | `Update::run`, `Ingest::reuse`, shared publication in `snapshot.rs` | `change_delete_and_rename_match_a_fresh_parse` checks facts, serialized object equality, old bytes and provider calls | PASS |
| Failed changed blobs and honest coverage / INV-002–003 | Pending changed paths use verified fetch; `ColdBuild::complete` declines incomplete publication | `failed_changed_blob_keeps_the_pointer_until_a_retry` checks skip, pointer and retry | PASS |
| S4 / S16: moving heads, CAS loss and repeated convergence | Pinned tree result, pointer CAS, independently callable `Update::freshness` | `moving_head_is_reported_stale_and_updates_converge` checks `Published` for the pinned tree, typed `Stale` for the moved head, conflict and retry | PASS |
| S8: extraction-version change cannot reuse old facts | Versioned snapshot scope; absent compatible pointer takes cold path | `version_change_rebuilds_under_a_new_key` | PASS |
| S9: typed, nonblocking review-time stale-pointer check | `Update::freshness` reads pointer and head without building | Moving-head test checks typed state and zero tree calls | PASS |
| S17 / REQ-012–013: dropped update preserves a complete pointer, records objects and converges on retry | Shared record → object → CAS publication; no detached work or transaction spanning IO | `dropped_update_keeps_the_pointer_and_retry_converges` covers three await seams | PASS |
| Changed-blob and archive bounds / REQ-013 | `max_changed_blobs` switches to bounded cold archive path | `changes_past_the_ceiling_use_the_archive`; existing archive tests | PASS |
| Scope and non-goals | Diff adds only task evidence and outer pipeline code; no queue, worker, cancel token, deadline, consumer code or dependency | Diff inspection, `git diff --check`, full gate | PASS |

## Review results

Wave 1: task samples A and B **PASS**; repository standards **PASS**; source-trust domain **PASS**; durability domain **FAIL** with `DUR-1`. The final citation check exited 0 and accepted all seven `DUR-1` citations. An initial check rejected four qualified symbol names; the reviewer corrected only citation metadata and the check was rerun. Wave 2 reproduced the race and **REJECTED** `DUR-1`: `Published` describes the pinned tree, while `Update::freshness` reports the later head as typed `Stale`; the task leaves invocation timing to the caller. The validated blocking ledger is empty, with no notes. No prior TASK-004 finding exists to close.

## Verification and limits

I ran `mise run gate` and `git diff --check <base> <candidate>`: both exited 0. The focused update suite passed six tests; Wave 2 independently ran the exact moving-head test, which passed. The in-memory adapters exercise publication and interruption contracts; a consumer's durable adapter, scheduler, and live provider are outside this task review. A branch can move after any observation, so the caller uses `freshness` or invokes update again for a later observation.

**Verdict: PASS.** No remediation task is required.
