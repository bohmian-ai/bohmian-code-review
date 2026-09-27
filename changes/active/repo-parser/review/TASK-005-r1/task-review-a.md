# TASK-005 implementation review A

Subject: base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`, candidate `e68bcdb65b44095989b5f42dfda5c7d87295582b`. I reviewed the complete eight-file diff, approved revision 12, original TASK-005 (excluding its Implementation Evidence), repository design and test authorities, and the affected facade, stores and tests. This is a review of the immutable candidate.

Verification: `mise run test:pipeline` passed locally. The orchestrator independently reports passing `mise run gate`, `test:pipeline`, `test:symbolic-context:journey`, `fmt:check`, `lints`, and `git diff --check`. Passing existing tests do not exercise the three paths below.

## Acceptance matrix

| Requirement, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-009, S5-S7: private pinned head overlay and no stale facts | `pull_request.rs:151-220`, `snapshot.rs:916-951` | Concurrent PR and stale-base tests, pipeline lane | PASS |
| REQ-008, S10: bounded scoped fallback, honest coverage | `pull_request.rs:172-191,220-230,543-553` | Failed cold-build scoped test | PASS |
| REQ-010, S18: stable pagination; per-file head identity, patch and blob handling | `pull_request.rs:298-333,364-413` | Moving PR and bad-patch tests | PASS |
| REQ-010, S19, INV-006: bounded candidate selection, diff reserve, explicit exhaustion | `pull_request.rs:234-279,418-466` | Exhausted-budget test | PASS |
| REQ-011, REQ-013, REQ-015: immutable validated publication, retry convergence | `context.rs:292-320`, `persistence.rs:242-253` | Drop-after-upsert/create test; existing-object corruption path untested | FAIL (TASK-A-1) |
| REQ-014, INV-005, AC-005, S14-S15: identity-scoped read, expiry at 24 hours | `context.rs:335-375`; `ContextId` keyed by digest | Read/expiry/isolation tests; repeat publication path untested | FAIL (TASK-A-2) |
| REQ-012-015, AC-009: context cleanup deletes object then record, retries failures without defeating concurrent publication | `context.rs:388-411`, `persistence.rs:242-286` | Sequential cleanup/failure test; publication race untested | FAIL (TASK-A-3) |
| REQ-018: independently callable PR and read facades with narrow injected stores | `lib.rs:20-52`, `pull_request.rs:106-142`, `context.rs:269-282` | Pipeline lane | PASS |
| Non-goals: no model/comment publication, consumer integration, detached task, cancel token, deadline, or new dependency | Complete diff and manifests | Diff inspection | PASS |

## Proposed findings

### TASK-A-1 — INCORRECT: existing context object is accepted without validating its bytes

Defect class: missing integrity validation at an immutable-object adoption boundary. Owner shape: `Contexts::publish` treats `AlreadyExists` as proof that the existing object is the expected bytes; the trait gives only a key collision, not an integrity guarantee. Sibling: `Snapshots::publish` at `snapshot.rs:705-711` handles the same storage result correctly by reading and comparing; no other context publication site exists. Violates REQ-011's complete validated object, REQ-012/013 retry convergence, and task Scenario 3. Seed a different object at the same scoped digest key (or replace a prior object with corrupt bytes), call `publish`/`PrContext::run` for that context: record upsert succeeds, `create` returns `AlreadyExists`, facade returns success, but `Contexts::read` reports `Corrupt`. A retry repeats the false success. Correction: at the existing context publication owner, validate an existing object's bytes before returning a published record, as the snapshot owner does; a focused injected-storage test must show a collision with wrong bytes cannot return success and a matching preexisting object can be adopted.

### TASK-A-2 — INCORRECT: republishing the same content extends the old context ID's lifetime

Defect class: mutable expiry for a content-addressed immutable context ID. Owner shape: `ContextStore::upsert_context` explicitly replaces the record for an existing ID and `Contexts::publish` always sets `expires_at = now + 24h`; the digest ID is unchanged for identical content. Siblings: the one upsert call in `Contexts::publish` and the in-memory store's replacement implementation at `testing.rs:397-408`; reads trust the replaced expiry at `context.rs:352-354`. Violates REQ-014, INV-005 and TASK-005's 24-hour shared read. Publish at `t0`, then publish identical context at `t0 + 23h`; the original ID is readable at `t0 + 25h`, past its original `expires_at`. A retry after an interrupted call produces the same extension. Correction: make the first successful publication's lifetime stable for an existing ID while still allowing an absent-object record to converge; test repeated publish of identical bytes and a read at the first expiry. The record contract must resolve the existing-vs-new record atomically for concurrent publishers.

### TASK-A-3 — INCORRECT: expiry collection can delete a concurrently republished context

Defect class: collection selects by a stale expiry and deletes without an atomic claim or freshness condition. Owner shape: `ContextStore::expired_contexts` returns a snapshot of records, `upsert_context` may replace one, then `collect_expired` deletes its object and record unconditionally. Siblings: `context.rs:394-403` are the two destructive sinks; the store methods at `persistence.rs:242-286` have no coordination contract. By contrast, `SnapshotStore::unreferenced` claims records against upsert and pointer swaps. Violates REQ-012/013 retry-safe publication, REQ-014 lifetime, and task Scenario 3 cleanup. At `t0 + 24h`, pause collection after `expired_contexts`; republish the same content at `t1`, which returns success with a fresh expiry; resume collection. It deletes the object and record, so the newly returned ID is not readable. Correction: coordinate expiry selection with context upsert so a selected record cannot be refreshed and reported published while cleanup deletes it; exercise that interleaving with the injected store and verify publication either retries visibly or remains readable through its promised lifetime.

Overall: **FAIL**. The failures are source-traced and concern required publication, expiry, and cleanup contracts. They need focused concurrency and storage tests; the existing sequential tests and green lanes do not close them.

```citations
[{"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/context.rs","line":317,"snippet":"            Ok(()) | Err(StorageError::AlreadyExists { .. }) => Ok(record),","symbols":["publish","AlreadyExists"]},{"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/context.rs","line":313,"snippet":"            expires_at: now + CONTEXT_LIFETIME,","symbols":["publish","CONTEXT_LIFETIME"]},{"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/persistence.rs","line":243,"snippet":"    /// Insert `record` for `identity`, or replace the existing record with","symbols":["upsert_context"]},{"id":"TASK-A-3","path":"crates/bohmian-code-review-pipeline/src/context.rs","line":394,"snippet":"        for (identity, record) in self.store.expired_contexts(now, limit).await? {","symbols":["collect_expired","expired_contexts"]},{"id":"TASK-A-3","path":"crates/bohmian-code-review-pipeline/src/context.rs","line":398,"snippet":"                match self.storage.delete(&record.key).await {","symbols":["collect_expired","delete"]}]
```
