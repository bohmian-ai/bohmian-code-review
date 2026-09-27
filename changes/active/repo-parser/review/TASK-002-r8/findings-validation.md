# TASK-002 r8 Wave 2 findings validation

Subject: `ab2fa3a..3979b427ed52e298f5fa88ef0c00bb324f0acc6b`, limited to the Revision 12 delta of `TASK-002`. The rest passed r7. `snapshot.rs` cold-build behavior belongs to TASK-003.

## Claim decision

| Wave 1 sources | Decision | Blocking gate | Reason |
|---|---|---|---|
| `TASK-B-1`, `S-1` | **CONFIRMED**, `FIND-TASK-002-13` | Task contract: AC-R12-1 requires bounded bodies (`tasks/TASK-002-github-source-and-stage-contracts.md:88`); REQ-013 requires bounded memory (`spec.md:76`). | `names_rate_limit` appends a whole transport chunk, then truncates it to 4,096 bytes. One chunk may exceed 4,096 bytes. |

## Deduplicated blocking ledger

### FIND-TASK-002-13 — CONFIRMED

- **Source IDs:** `TASK-B-1`, `S-1`. One root cause, one finding. This is new to the async body port; it does not reopen the earlier rate-limit classification finding `FIND-TASK-002-9`.
- **Classification / defect class:** INCORRECT; the rate-limit inspection buffer is capped after copying rather than before it.
- **Owner and siblings:** `crates/bohmian-code-review-pipeline/src/github.rs:815-823`, `names_rate_limit`; no sibling with the same copy-before-check pattern. `read_bounded` at `github.rs:1357-1373` checks remaining capacity before appending; `Archive::chunk` at `github.rs:594-605` checks before returning a chunk. Both consume the same `HttpBody::chunk` seam, but neither copies an over-limit chunk into its bounded accumulation buffer.
- **Obligation:** AC-R12-1 at `tasks/TASK-002-github-source-and-stage-contracts.md:88` preserves bounded bodies in the `reqwest` port. REQ-013 at `spec.md:76` requires bounded memory. The function's own rustdoc at `github.rs:813-814` claims no more than 4,096 bytes are read.
- **Reproduction:** `GitHubClient::repository` → `get_json` → `request` → `send_with_policy` (`github.rs:1288-1294`) reaches `names_rate_limit` for a headerless 403. `HttpBody::chunk` returns an owned `Vec<u8>`; the production implementation returns one `reqwest::Response::chunk` as a `Vec` (`github.rs:75-87`), and the scripted `FixedBody` returns the entire body in one chunk (`testing.rs:19-36`). For a single chunk of 4,097 bytes, `body.extend_from_slice(&chunk)` at `github.rs:819` grows the buffer to 4,097 before `truncate(4096)` at line 823. Larger chunks make this excess copy proportionally larger. The existing 403 test at `github.rs:1920-1925` uses a short body and cannot expose the issue. This is a source-path reproduction; no Cargo job was started because the orchestrator owns verification runs.
- **Observable consequence:** A provider-controlled 403 body can make rate-limit classification allocate and copy more than its stated 4,096-byte inspection bound before yielding `Forbidden` or `RateLimited`. That violates the preserved provider bound even though classification observes only the truncated prefix.
- **Smallest safe correction:** In `names_rate_limit`, append only the part of each received chunk that fits the remaining prefix budget, and make the rustdoc distinguish inspected bytes from an already-materialized transport chunk. Keep 403/429 classification, retry timing, and transport ownership as they are. The candidate-owned `HttpBody` still materializes one chunk upstream; this finding requires a bounded inspection buffer, not a new transport framework.
- **Focused closure proof:** Add or extend the existing fake-provider rate-limit test with a headerless 403 whose single `FixedBody` chunk exceeds 4,096 bytes. Exercise the same `GitHubClient::repository` path and verify classification uses only the first 4,096 bytes, including a `rate limit` marker wholly beyond the prefix. A direct bounded-buffer assertion or focused helper test is needed to prove the copy bound itself, since output classification alone cannot observe a temporary over-allocation. Run the focused provider test and `test:pipeline`, `fmt:check`, and `lints` lanes.

## Notes and limits

- No separate note survived validation. Earlier `FIND-TASK-002-1` through `-12` remain closed in r7; no claim in this round reopens them.
- Source inspection proves the over-limit copy. It does not measure allocator peak or establish a maximum size for one `reqwest` chunk; that upstream allocation is outside this specific copied-prefix finding.
- `snapshot.rs` cold-build semantics were excluded as instructed; its compile, bound, and ponytail-buffering checks remain with the orchestrator's verification.

**Wave 2 result: FIX_REQUIRED** for `FIND-TASK-002-13`.
