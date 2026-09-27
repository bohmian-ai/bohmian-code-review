# TASK-002 r9 verdict

## Immutable subject and scope

- Repository: `bohmian-code-review`; base `ab2fa3a8615431f7c32c719a0f321f2f252e68fe`; candidate `e38c3ae165f8972037c6b823aa9c725782b4237c`. HEAD matched the candidate before and after the review; only this new review directory was untracked.
- Authority: approved `changes/active/repo-parser/spec.md` revision 12; `tasks/TASK-002-github-source-and-stage-contracts.md`, **Revision 12 delta**; and `review/TASK-002-r8/TASK-002-R6-bounded-rate-limit-prefix.md`. The prior non-delta TASK-002 obligations passed in `review/TASK-002-r7`.
- Reviewed the cumulative `ab2fa3a..e38c3ae` diff for this scope, including the async port and R6. `snapshot.rs` cold-build internals are TASK-003 work; this review checks only compilation, bounds, and the honesty of its `ponytail:` buffering note.

## Verdict: PASS

Wave 2 validated an empty blocking ledger. **FIND-TASK-002-13 is CLOSED**: `rate_limit_prefix` copies no more than 4,096 bytes into its own buffer before classification, including when one transport chunk is larger. No remediation task is needed.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| AC-R12-1: async `reqwest` client, preserved trust, identity, bounds, and shared retry policy; no `ureq` | `github.rs` uses `ReqwestTransport`, disabled redirects, checked archive hop, bounded body consumers, verified identities, and one `send_with_policy`; manifest/lock use `reqwest` and tokio | Pipeline tests and full gate passed; dependency tree has `reqwest 0.13.5` and no `ureq` | PASS |
| AC-R12-2: async `ObjectStorage`, `SnapshotStore`, `ContextStore` with `Send` futures, generic consumers and no `async_trait` | `storage.rs` and `persistence.rs` return `impl Future + Send`; `testing.rs` implements with `async fn`; `snapshot.rs` accepts generic stores | Full gate includes `cold_build_future_is_send`; forbidden-symbol sweep empty | PASS |
| AC-R12-3: remove library cancellation/deadline types and cases | `execution.rs` deleted; exports and provider parameters removed | Search found no `Execution`, `CancelToken`, or `Interrupted` in the pipeline crate | PASS |
| AC-R12-4: dropping a retry wait sends no later request | `send_with_policy` awaits tokio sleep in the caller's future | Exact `dropped_call_sends_no_further_requests` test: 1 passed | PASS |
| AC-R12-5: no detached tasks | Provider and storage work stays in the caller's future | No `tokio::spawn` in the pipeline crate; gate passed | PASS |
| R6 / FIND-TASK-002-13: bounded headerless-403 prefix and unchanged classification | `rate_limit_prefix` slices each chunk to remaining space before copying; `names_rate_limit` inspects that prefix | Exact `headerless_403_inspects_only_a_bounded_prefix` test: 1 passed; checks length, capacity, marker boundary and request count. Existing rate-policy tests passed in gate | PASS |
| Scoped TASK-003 seam: compile, remain bounded, disclose buffering | `snapshot.rs:653-675` marks the compressed buffer; `Archive::chunk` enforces `max_archive_bytes`; `BoundedReader` bounds expansion | Full gate compiled and tested the pipeline; source review confirmed the stated bounds | PASS within scope |
| Repository rules and non-goals | No SDK, backend, source checkout, symbolic stage duplication, extra trait method, or speculative framework in the scoped diff | Standards review PASS; `fmt:check`, `lints`, `git diff --check`, full gate exit 0 | PASS |

## Review evidence

| Reviewer | Report | Result | Proposed findings |
|---|---|---|---|
| Task implementation A | `task-review-a.md` | PASS | none |
| Task implementation B | `task-review-b.md` | PASS | none |
| Repository standards | `standards-review.md` | PASS | none |
| Source trust and provider budget | `domain-review-source-trust.md` | PASS | none |
| Async contracts and cancellation | `domain-review-async-contract.md` | PASS | none |
| Ponytail validation | `findings-validation.md` | PASS | validated empty ledger |

`mise run review:verify-citations changes/active/repo-parser/review/TASK-002-r9 e38c3ae165f8972037c6b823aa9c725782b4237c` exited 0. Every Wave 1 citations block was empty; no citation was rejected. `claim-ledger.md` gave Wave 2 an empty claim union. No reviewer exceeded the 20-minute limit, and no sub-reviewer evidence is missing.

## Prior findings, notes, and limits

- `FIND-TASK-002-13` is closed across the shared provider policy; `read_bounded` and `Archive::chunk` already check bounds before copying. FIND-TASK-002-1 through -12 remain closed by r7; no scoped regression reopened them.
- No retained notes or rejected findings. The transport materializes one response chunk before the 4,096-byte inspection copy, as R6 expressly allows. The `ponytail:` note accurately describes compressed archive buffering bounded by `max_archive_bytes`.
- No live GitHub/GHES run or allocator peak measurement was made. Scripted provider tests prove the R6 boundary, and the full gate passed; TASK-003 cold-build semantics were outside this review.

## Orchestrator verification

- `mise run gate`: exit 0, including pipeline tests, symbolic journey, formatting, lints, review-script check, skill sync and workspace checks.
- Exact prefix and dropped-call focused tests: each selected 1 test and passed.
- `git diff --check ab2fa3a e38c3ae165f8972037c6b823aa9c725782b4237c`: exit 0. Forbidden-symbol search for `ureq`, removed cancellation types, `tokio::spawn`, and `async_trait` returned no matches in the pipeline crate and manifests.
