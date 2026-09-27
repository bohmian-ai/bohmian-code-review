---
id: TASK-002-R6
kind: remediation
status: proposed
spec: SPEC-repo-parser@12
parent_task: TASK-002
remediates: [FIND-TASK-002-13]
---

# Restore the bounded rate-limit prefix

## Subject and authority

- Approved spec: `changes/active/repo-parser/spec.md`, revision 12 (REQ-013).
- Original task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`, Revision 12 delta (AC-R12-1).
- Reviewed base: `ab2fa3a8615431f7c32c719a0f321f2f252e68fe`.
- Reviewed candidate: `3979b427ed52e298f5fa88ef0c00bb324f0acc6b`.
- Validated finding: `FIND-TASK-002-13` in `findings-validation.md`.

## Diagnosis

The async port changed `github.rs`'s headerless-403 classifier from a `Read::take(4096)` prefix read to `HttpBody::chunk()` followed by `body.extend_from_slice(&chunk)` at line 819 and `body.truncate(4096)` at line 823. The bound now runs after the copy. One provider-controlled chunk larger than 4096 bytes makes the classifier allocate and copy the whole chunk even though it inspects only the prefix. This violates AC-R12-1's preserved bounded-body behavior and REQ-013's memory bound. The existing short-body 403 tests and passing lanes do not exercise that case.

**Defect class:** the inspection limit is enforced after accumulation. The owner is `names_rate_limit` in `crates/bohmian-code-review-pipeline/src/github.rs`; its body-consumption shape permits the excess copy. The `GitHubClient::repository` → JSON request → shared `send_with_policy` path reaches it on a 403 without rate headers, so the class affects every provider operation using that policy. The sibling consumers of `HttpBody::chunk` already check their limits before copying: `read_bounded` and `Archive::chunk`. There is no same-pattern sibling to change. The transport necessarily materializes one chunk before handing it to the classifier; that upstream allocation is outside this finding.

## Correction outcome

Restore a hard 4096-byte bound on the classifier's own inspection buffer and copy work while retaining the current headerless 403 classification, retry timing, typed errors, and caller-owned cancellation. Correct its rustdoc so it describes the inspection bound accurately. Use the existing classifier and `RATE_LIMIT_BODY_BYTES`; no new transport abstraction, dependency, provider policy, or snapshot change is needed.

## Acceptance and proof

1. For a single headerless 403 chunk larger than 4096 bytes, the classifier's accumulated prefix never exceeds 4096 bytes. A `rate limit` marker wholly beyond that prefix does not affect classification; a marker within it still does.
2. The shared provider path yields the existing `Forbidden` or `RateLimited` outcome as appropriate, without extra retry requests or a changed wait policy.
3. A focused test through the existing fake transport directly proves the prefix copy bound as well as the classification boundary. An output-only assertion is insufficient because truncation hides a temporary excess allocation.
4. Run the exact focused test, `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. The original Revision 12 drop test remains green; run `mise run gate` because the task requires it.

Write set: `crates/bohmian-code-review-pipeline/src/github.rs` and its inline owner tests. Preserve all previously verified source-trust, identity, archive, storage-trait, and async behavior. This task does not implement TASK-003 cold-build internals or a new provider transport.

Route to `$cr-implement`; the next task review reassesses the cumulative candidate against TASK-002.

## Implementation evidence

Candidate: commit `02cfefa` on `review-helper-setup`. `names_rate_limit` now reads through `rate_limit_prefix`, which allocates `RATE_LIMIT_BODY_BYTES` once and slices each chunk to the remaining room before copying.

| Acceptance criterion | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| 1. Prefix never exceeds 4096 bytes; marker past it ignored, inside it honoured | `github::rate_limit_prefix`, `github::names_rate_limit` | `github::tests::headerless_403_inspects_only_a_bounded_prefix`: a 16 KiB single chunk yields `len == 4096` and `capacity <= 4096`; marker at byte 4096 → `Forbidden`, marker ending at byte 4096 in a 12 KiB chunk → `RateLimited` | PASS |
| 2. Shared provider path keeps `Forbidden`/`RateLimited`, no extra retries or wait change | `send_with_policy` unchanged | same test drives `GitHubClient::repository` and asserts 2 requests for 2 calls, `retry_after == SECONDARY_RATE_LIMIT_WAIT`; `rate_limits_retry_within_bounds_or_report_typed_outcomes` still passes | PASS |
| 3. Focused test proves the copy bound directly | capacity assertion on the returned buffer; the old `extend`-then-`truncate` shape keeps a ≥16 KiB capacity and fails it | focused test | PASS |
| 4. Verification | — | focused test, `dropped_call_sends_no_further_requests`, `mise run test:pipeline` (30 passed), `mise run fmt:check`, `mise run lints`, `git diff --check`, `mise run gate` — all exit 0 | PASS |

Only `github.rs` changed (plus this record). Transport, retry policy, errors, and async behavior are untouched; the transport's own one-chunk materialisation stays out of scope.
