# TASK-004 independent task review A

Subject: `a77c64d8194ff3b8e0bc9f51f68b2cc5ae0095ec..f5a6f4bbeae1877083f79f4198316205c75a63e8` under approved `SPEC-repo-parser@12`. I inspected the complete four-file diff, update and cold-build call paths, snapshot validation, provider blob verification, eligibility, persistence contract, and the named tests. The implementation-evidence section of the task was not used as proof.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-007 / S3: changed, deleted, renamed and ineligible paths yield fresh-tree facts while old object remains immutable | `update.rs:70-103`; `snapshot.rs:877-898` reuses only equal path/blob facts, then publishes a new object | `update.rs:225-311` checks old bytes, call counts, lookup absence and byte equality with fresh cold build | PASS |
| REQ-007 / INV-003: failed changed blob removes old facts from the attempt, reports coverage and does not move pointer | `snapshot.rs:905-934,966-984,643-659`; update never copies changed path facts | `update.rs:315-350` checks `FetchFailed`, old pointer/object/record and successful retry | PASS |
| S4 / S16: moving head and force-push are compared by tree, repeated calls converge, loser reports typed retry | `update.rs:73-77,101-103`; `snapshot.rs:728-735` uses pointer tree CAS | `update.rs:414-483` covers moving heads, CAS loser and reuse after retry | PASS |
| S8: version change builds under a new key without reusing old facts | `snapshot.rs:326-334`; `update.rs:72-76` reads current-version scope | `update.rs:489-528` tests prior extraction-version pointer isolation | PASS |
| S9: stale-pointer check is typed and does not build or wait for update | `update.rs:105-123` reads pointer and head only | `update.rs:414-447` checks stale result and zero tree calls | PASS |
| REQ-012 / REQ-013 / S17: dropped update leaves complete pointer, records objects and retry converges without transaction over IO | `update.rs:101-103` calls shared `snapshot.rs:682-736` upsert/create/CAS sequence | `update.rs:355-408` injects drops at publication awaits and checks pointer, records, objects and retry | PASS |
| REQ-013: changed blob and archive/fallback work remains bounded | `snapshot.rs:43-65,599-626`; `update.rs:93-100` selects individual blobs or bounded archive | `update.rs:533-558` checks threshold path; existing cold-build tests cover archive/fallback bounds | PASS |
| INV-002 / INV-004: complete coverage is required before publication; unchanged fact reuse preserves deterministic fresh parse | `snapshot.rs:643-680,877-898` | `update.rs:225-311,315-350` checks oracle object bytes, skips and pointer | PASS |
| Task non-goals: no queue, worker, detached task, cancel token, deadline, consumer code or new dependency | Complete diff touches only task evidence, outer crate `lib.rs`, `snapshot.rs`, `update.rs`; changed blobs are fetched sequentially | Diff and manifest inspection; `git diff --check` exited 0 | PASS |

## Proposed findings

None. The shared cold publication path owns the durable sequence, so the update adds no second pointer-publication rule. The added threshold is the approved 2,000-changed-blob ceiling. I found no material missing, incorrect, drifted, constraint-violating, or regressed behavior in this task's scope.

## Verification limit and result

The orchestrator ran `mise run gate` and reported exit 0 on this candidate; I did not run Cargo concurrently. The review of runtime assertions is based on source inspection plus that reported gate result. **PASS** for this task-review sample; the overall decision still requires the other independent reviews.

```citations
[]
```
