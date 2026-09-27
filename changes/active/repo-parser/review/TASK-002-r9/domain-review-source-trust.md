# Domain review: source trust and provider budget

**Subject:** `ab2fa3a` → `e38c3ae165f8972037c6b823aa9c725782b4237c`; TASK-002 Revision 12 delta and TASK-002-R6 under approved spec revision 12. **Result: PASS.** No material findings.

## Boundary and coverage

| Obligation | Reviewed source and result |
|---|---|
| REQ-004 and AC-R12-1: verified GitHub/GHES source, no credential forwarding | `GitHubClient::request` and `build` authenticate API requests; `archive` checks the redirect against `GitHubHost` before one uncredentialed hop; `ReqwestTransport::new` disables automatic redirects. Repository, branch, commit, tree, blob, and PR identity checks still precede returned trusted data. PASS. |
| REQ-013 and AC-R12-1: bounded provider operations | `send_with_policy` retains the shared attempt and wait caps, typed 403/429/5xx/transport errors, and tokio waits. `read_bounded` rejects over-limit JSON/blob chunks before appending; `Archive::chunk` rejects an archive chunk that exceeds its cumulative compressed-byte limit before returning it. PASS. |
| FIND-TASK-002-13 and TASK-002-R6: bounded 403 classifier copy | `rate_limit_prefix` slices every chunk to remaining space before copying. Its buffer length and capacity are at most 4,096 bytes, and `names_rate_limit` inspects only that prefix. The focused test checks capacity plus markers on both sides of the boundary through `GitHubClient::repository`. No same-pattern sibling remains in `read_bounded` or `Archive::chunk`. PASS; prior finding closed. |
| AC-R12-4: caller-owned cancellation | The retry sleep remains inside `send_with_policy`; dropping its future stops later requests. `dropped_call_sends_no_further_requests` covers the paused-clock wait. PASS. |
| Requested TASK-003 limit | `snapshot.rs::consume_archive` compiles against the async `Archive`; its `ponytail:` note accurately discloses buffering of the compressed stream. `Archive::chunk` bounds accepted compressed bytes and `BoundedReader` bounds expansion. Cold-build semantics are outside this review. PASS within that limit. |

## Verification limits

The exact focused `github::tests::headerless_403_inspects_only_a_bounded_prefix` test passed (1 test). This review did not run a live GitHub/GHES contract test or a real `reqwest` stream; fake transport tests cannot establish a maximum upstream HTTP chunk allocation. The R6 obligation expressly concerns the classifier's own copy after the transport has materialized one chunk. The orchestrator owns the broader gates.

```citations
[]
```
