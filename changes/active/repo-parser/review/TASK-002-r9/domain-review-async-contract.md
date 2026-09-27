# TASK-002 r9: async contract domain review

**Subject:** `ab2fa3a..e38c3ae165f8972037c6b823aa9c725782b4237c`; approved repo-parser spec revision 12; TASK-002 Revision 12 delta and TASK-002-R6. **Overall: PASS.**

## Boundary, authority, and coverage

| Boundary | Authority | Candidate evidence | Result |
|---|---|---|---|
| Async injectable stores and `Send` facade futures | TASK-002 AC-R12-2; spec REQ-015–REQ-016 | `storage.rs:213-243` and `persistence.rs:134-285` declare `Send` futures; `testing.rs:127-315` implements the traits with `async fn`; `snapshot.rs:233,404` uses generic stores and `snapshot.rs:1755-1767` asserts facade futures are `Send`. | PASS |
| Caller-owned cancellation and no detached work | TASK-002 AC-R12-3–5; spec revision 12, REQ-013 | `execution.rs` is deleted and `lib.rs` drops its exports. `github.rs:1283-1325` awaits transport and Tokio retry sleep in the calling future. Search of the pipeline crate and root manifest found no `Execution`, `CancelToken`, `Interrupted`, `tokio::spawn`, or library deadline parameter. The focused paused-clock drop test passed. | PASS |
| Bounded provider rate-limit classification | TASK-002-R6; AC-R12-1; spec REQ-013 | `github.rs:815-838` slices each chunk to remaining prefix capacity before copying. The focused oversized-chunk test passed and checks length, capacity, and markers on either side of byte 4,096. | PASS |
| Snapshot compile port and bounded temporary buffering | TASK-002 consumer closure; user-scoped TASK-003 exception | `snapshot.rs:440-590` awaits provider and store calls and records before object creation and pointer publication. `snapshot.rs:653-672` plainly marks the compressed archive buffer; `Archive::chunk` in `github.rs:594-606` rejects bytes beyond `max_archive_bytes`, and `BoundedReader` limits decompression. The crate compiles in both focused test runs. | PASS within stated scope |

## Verification and limits

- Independently ran the exact `dropped_call_sends_no_further_requests` and `headerless_403_inspects_only_a_bounded_prefix` focused tests; each selected one test and passed. `git diff --check ab2fa3a e38c3ae165f8972037c6b823aa9c725782b4237c` passed.
- `snapshot.rs` performs synchronous tar expansion after buffering and cannot observe a drop until that synchronous work returns. TASK-003 owns those internals; this review judges their compilation, resource bounds, and honest buffering note only.
- These fake-transport tests establish cancellation during retry sleep and the classifier's own copy bound. They do not measure a real network cancellation or the transport's unavoidable single-chunk allocation.

## Proposed findings

None.

```citations
[]
```
