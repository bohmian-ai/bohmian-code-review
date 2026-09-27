# TASK-005 implementation review B

Subject: base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`, candidate `e68bcdb65b44095989b5f42dfda5c7d87295582b`; approved `changes/active/repo-parser/spec.md` revision 12 and original `tasks/TASK-005-pr-context-and-shared-read.md`. I inspected the cumulative diff and owning source without relying on the task's Implementation Evidence.

## Acceptance matrix

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| S5–S6; REQ-009/INV-004: private pinned PR overlay, from-scratch equivalence | `pull_request.rs:149-236`, `snapshot.rs:905-926` | `concurrent_prs_see_only_their_own_heads`; pipeline lane passes | PASS |
| S7: stale default snapshot reconciles head drift and reports incomplete overlay | `pull_request.rs:162-230`; `Ingest::reuse/fallback` | `stale_base_overlays_drift_or_reports_incomplete` | PASS |
| S10; REQ-008: failed cold build indexes eligible nearby files within scoped file bound | `pull_request.rs:179-190,543-551` | Existing scoped test has no ineligible files before an eligible candidate | FAIL |
| S18; REQ-010: stable pagination, file-local patch/changed-blob failure, changed files excluded from candidates | `pull_request.rs:298-405,234-236`; `github.rs:1118-1158` | moving and perpetually moving PR tests | PASS |
| S19; REQ-010/INV-006: budget accounts for diff and prompt; exhausted allowance visible; estimate is qualified | `pull_request.rs:238-279`; `context.rs:160-175` | `exhausted_budget_packs_nothing_and_says_so` | PASS |
| REQ-009/011: rank with public symbolic facade, fetch selected head blobs, store selected spans and separate diff | `pull_request.rs:235-285,415-459,471-516` | concurrent PR oracle and public symbolic journey | PASS |
| S14–S15; REQ-014/015/INV-005/007/AC-005/009: scoped exact-key read, 24-hour expiry, source-free metadata, separate context store | `context.rs:335-375`; `persistence.rs:107-125,242-297` | context read/isolation/expiry tests | PASS |
| REQ-011/012/013: return an ID only for a complete validated context object; a retry converges | `context.rs:292-319` | dropped-publication test covers missing object, not preexisting corrupt object | FAIL |
| REQ-012–015: expiry cleanup deletes object then record, reports lag, and preserves a retry-safe concurrent publication | `context.rs:388-413`; `persistence.rs:242-286` | serial cleanup test does not cover cleanup/publication overlap | FAIL |
| REQ-016/018: independently callable read facade with injected object storage and context persistence | `lib.rs` exports; `Contexts::new/read` | context owner tests; pipeline lane passes | PASS |
| Non-goals: no model, comments, clone, checkout, persisted PR snapshot, consumer, new dependency | complete diff limited to outer crate and task evidence | `git diff --name-only` and manifests | PASS |

Verification run: `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, and `git diff --check` all exited 0. These existing checks do not exercise the three failing cases below. I did not modify reviewed source.

## Findings

### TASK-B-1 — INCORRECT: ineligible tree entries consume the scoped eligible-file limit

**Root cause and defect class:** `near` truncates raw same-directory tree entries before `Ingest::new` classifies eligible paths. The owner shape is a raw-entry selection function with no eligibility predicate; the same issue applies to unsupported extensions, generated/vendor exclusions, symlinks already filtered by mode, and oversize files. Relevant owner and caller: `pull_request.rs:543-551,179-190`.

**Violated obligation:** REQ-008 and Scenario 1's S10 require a bounded scoped index of *eligible* files near changed paths. **Reachable consequence:** with `max_scoped_files = 2`, a changed `src/handler.ts`, `src/a.md`, and eligible related `src/z.ts` in listing order, `near` chooses the changed file and `.md`, then classification drops `.md`; `src/z.ts` is omitted despite an available eligible slot, so the scoped result misses useful context. This is a real cold-build fallback path and the omission is not reported as a skipped eligible file. **Correction:** apply the existing eligibility classification before consuming the scoped file limit, retaining changed paths first and the same bound; add one focused scoped test with an ineligible entry ahead of a matching eligible candidate.

### TASK-B-2 — INCORRECT: publication accepts an existing context object without validating it

**Root cause and defect class:** `Contexts::publish` treats any `AlreadyExists` at the digest-derived key as successful without comparing object bytes or digest. The public storage trait explicitly permits an existing key irrespective of content; the snapshot publisher already validates this case at `snapshot.rs:703-711`. The owner is `context.rs:315-318`; the same pattern has no other production sibling in the context publication path.

**Violated obligation:** REQ-011 requires a complete validated object before handing off an ID; REQ-012 and TASK-005 require a retry to converge to a readable context. **Reachable consequence:** an interrupted or corrupted existing object at the key causes `create` to return `AlreadyExists`; `run` returns `Ok(PrContextResult)` and an ID, while `Contexts::read` later returns `Corrupt` or `NotFound`. **Correction:** on `AlreadyExists`, read and verify the object against the serialized bytes or digest before returning success, as the existing snapshot publication does; add a focused test with a mismatched object at the exact context key.

### TASK-B-3 — INCORRECT: context cleanup can delete a concurrently republished record or orphan its object

**Root cause and defect class:** `expired_contexts` only lists rows; `upsert_context` may replace one while `collect_expired` is deleting the old object and then calls unconditional `delete_context` by ID. Unlike the snapshot store's claim protocol (`persistence.rs:134-140`), the context-store contract has no atomic claim or conditional delete for this race. Sibling sinks are the object delete and record delete in `context.rs:398-403`, plus record upsert and object create in `context.rs:315-317`.

**Violated obligation:** REQ-012/013 require retry-safe publication and every written object recorded; REQ-014/015 require cleanup without making an unexpired context unreadable. **Reachable consequence:** after a previously expired identical context is selected for cleanup, a retry upserts the same content-addressed ID with a fresh expiry. Cleanup can then delete that new row, either after publication succeeds (returned ID becomes unreadable), or before its object create (an object is written with no record). The in-memory store's `expired_contexts` and `delete_context` show this interleaving directly at `testing.rs:439-474`. **Correction:** make expiry collection claim or conditional deletion atomic against a concurrent upsert for the same identity and ID, and ensure the publication side handles a claimed record by retrying; prove an interleaving with publication held between upsert and object create, as the snapshot race test does.

## Result

**FAIL.** The scoped fallback and shared handoff/cleanup contracts have reachable gaps. The ordinary journey, isolation, and serial drop/expiry checks pass.

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/pull_request.rs","line":551,"snippet":"    picked.into_iter().take(max).cloned().collect()","symbols":["near","Ingest"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/src/pull_request.rs","line":188,"snippet":"                    Ingest::new(&near, 0, &attributes, limits.max_file_bytes),","symbols":["run","Ingest"]},{"id":"TASK-B-2","path":"crates/bohmian-code-review-pipeline/src/context.rs","line":317,"snippet":"            Ok(()) | Err(StorageError::AlreadyExists { .. }) => Ok(record),","symbols":["publish","create"]},{"id":"TASK-B-3","path":"crates/bohmian-code-review-pipeline/src/context.rs","line":394,"snippet":"        for (identity, record) in self.store.expired_contexts(now, limit).await? {","symbols":["collect_expired","expired_contexts"]},{"id":"TASK-B-3","path":"crates/bohmian-code-review-pipeline/src/context.rs","line":401,"snippet":"                        .delete_context(&identity, &record.id)","symbols":["collect_expired","delete_context"]},{"id":"TASK-B-3","path":"crates/bohmian-code-review-pipeline/src/context.rs","line":315,"snippet":"        self.store.upsert_context(identity, &record).await?;","symbols":["publish","upsert_context"]}]
```
