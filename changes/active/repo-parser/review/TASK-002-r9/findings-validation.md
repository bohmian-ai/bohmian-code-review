# TASK-002 r9 Wave 2 findings validation

Subject: `ab2fa3a8615431f7c32c719a0f321f2f252e68fe..e38c3ae165f8972037c6b823aa9c725782b4237c`. Scope: TASK-002 Revision 12 delta and TASK-002-R6. Earlier TASK-002 behavior passed r7; TASK-003 owns `snapshot.rs` cold-build semantics.

## Validated ledger

**Empty.** All five Wave 1 reports supplied empty citation arrays, and `mise run review:verify-citations changes/active/repo-parser/review/TASK-002-r9 e38c3ae165f8972037c6b823aa9c725782b4237c` exited 0. Independent inspection of the cumulative diff and the Revision 12 contract found no blocking claim to add. There are no rejected citations or retained notes.

## Prior finding closure

**FIND-TASK-002-13 — CLOSED.** The shared `GitHubClient::send_with_policy` path calls `names_rate_limit` for a headerless 403 (`github.rs:1283-1310`). `names_rate_limit` now obtains its bytes from `rate_limit_prefix` (`github.rs:813-839`), which allocates 4,096 bytes and slices every chunk to the remaining room *before* copying. The preceding copy-then-truncate defect is gone. The focused `github::tests::headerless_403_inspects_only_a_bounded_prefix` test passed (1 test, 29 filtered): it directly checks prefix length and capacity after a 16 KiB chunk, then verifies `Forbidden` for a marker beyond the prefix, `RateLimited` for a marker inside it, and two requests for two calls (`github.rs:1873-1915`). The new helper is load-bearing because the test must inspect the actual buffer to distinguish a bounded copy from truncation after an oversized copy. No prior remediation check became redundant.

Sibling sweep: `read_bounded` checks the remaining limit before copying (`github.rs:1369-1386`); `Archive::chunk` checks the byte count before returning a chunk (`github.rs:594-608`). Neither has the copy-before-limit defect. Every provider operation using the shared policy receives the corrected 403 classification. This closes the same root cause rather than reopening another `FIND-*` ID.

## Scope and limits

- The transport still materializes one `reqwest` body chunk before prefix inspection (`github.rs:75-87`), as R6 explicitly permits. The corrected rustdoc states a bound on the **copied inspection buffer**; it does not claim bounded upstream chunk allocation.
- `snapshot.rs:653-675` honestly marks compressed archive buffering with `ponytail:` and copies only chunks returned under `Archive::chunk`'s `max_archive_bytes` check; expansion uses `BoundedReader`. This review makes no TASK-003 cold-build behavior judgment.
- No live GitHub/GHES call or allocator peak measurement was made. The focused fake transport checks the exact R6 copy boundary and classifications; broader build and gate evidence belongs in the orchestrator's verdict.

**Wave 2 result: PASS.** The citation-checked claim union and independently validated blocking ledger are empty; FIND-TASK-002-13 is closed.
