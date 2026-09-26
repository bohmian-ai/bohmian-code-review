# TASK-002 task review r6: verdict

## Immutable subject

- **Repository:** `bohmian-code-review`, branch `review-helper-setup`.
- **Base:** `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`.
- **Candidate:** `7983907a6aa0b01a942fb964362da73bc9e7dba6`. It was read from a detached worktree at `/tmp/cr-r6/wt`, and the worktree's `git rev-parse HEAD` was `7983907` before Wave 1 and again after Wave 2.
- **Approved spec:** `changes/active/repo-parser/spec.md`, revision 11 (approved in `d2c8f9c`).
- **Original task:** `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`, including its Revision 11 delta (the `Persistence` split into `SnapshotStore` and `ContextStore`).
- **Remediation assessed:** `../TASK-002-r5/TASK-002-R4-key-layout-rate-limits-and-host-shape.md` (FIND-TASK-002-1, -8, -9, -10, -11).
- **Prior verdicts:**

  | Round | Verdict |
  |---|---|
  | r1 | `FIX_REQUIRED` |
  | r2 | `FIX_REQUIRED` |
  | r3 | `FIX_REQUIRED` |
  | r4 | `PASS` (withdrawn by r5) |
  | r5 | `FIX_REQUIRED` |

- **Diff reviewed:** the complete cumulative diff `1842eaf..7983907` over `crates/`, `Cargo.toml`, `Cargo.lock` and `mise.toml`. The code commits since r5 are `b83d286` (the Revision 11 store split) and `54232b2` (R4).
- **Out of scope:**
  - `741c049`, the TASK-003 work in progress: `snapshot.rs`, `eligibility.rs` and the TASK-003 parts of `testing.rs`. Only the edits needed to compile against the new traits and keep the lanes green were reviewed.
  - `b83d286`'s changes to those files: the trait rename, rustfmt, clippy fixes, the trailing-`**` glob match fix in `eligibility.rs`, and the tar traversal fixture in `snapshot.rs`. These are lane-greening edits. TASK-003's review owns their behavior.
  - The docs-only commits `169bfaf`, `d2c8f9c` and `1f6d538`, and the review directories.

## Verdict

**FIX_REQUIRED.** Wave 2 reproduced one new blocking finding:

- **FIND-TASK-002-12 (CONFIRMED):** a tenant id of `.` or `..` is accepted, so both constructors build, and reload accepts, an object key that traverses out of `tenants/`.

The Revision 11 store split and all five R4 findings are closed.

Remediation: `TASK-002-R5-tenant-dot-segments.md`.

## Wave 1 results

| Reviewer | Report | Result | Proposed findings |
|---|---|---|---|
| task-rev A | `task-review-a.md` | PASS | none |
| task-rev B | `task-review-b.md` | PASS | none |
| repo-rev | `standards-review.md` | FAIL | REPO-1 |
| domain-rev source trust | `domain-review-source-trust.md` | PASS | none |
| domain-rev provider bounds | `domain-review-provider-bounds.md` | PASS | none |
| domain-rev durable seams (store split, `ObjectKey` reload) | `domain-review-durable-seams.md` | FAIL | DOM-DS-1, DOM-DS-2 |

- **Timing:** Wave 1 ran 16:57–17:00, and no reviewer came near the 20-minute cap. Wave 2 (`ponytail-rev`) ran 17:01–17:03.
- **Write denial:** the harness refused `ponytail-rev`'s write to the review directory. The orchestrator saved the ledger it returned verbatim as `findings-validation.md`, and re-read `identity.rs:18-24`, `:70-78` and `:141-151` at the candidate to confirm the owner.

## Citation check

`mise run review:verify-citations changes/active/repo-parser/review/TASK-002-r6 7983907` exited 0 (`citations.txt`). All 7 entries were OK, so none were rejected and no finding was dropped. `claim-ledger.md` carries the three claims into Wave 2.

## Acceptance matrix

This matrix consolidates the two task-review samples, the domain reviewers and Wave 2.

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Rev 11 / REQ-015 / AC-009: `Persistence` replaced by `SnapshotStore` and `ContextStore` | `persistence.rs`: `SnapshotStore` has `snapshot_pointer`, `publish_snapshot_pointer` and `remove_repository`. `ContextStore` has `publish_context`, `context`, `expired_contexts`, `delete_context` and `remove_repository`. The two traits share only `PersistenceError` | Checked by both task-rev samples and the durable-seams reviewer. `rg 'trait Persistence'` finds nothing | PASS |
| Rev 11: each store's removal covers only its own records; no cross-store coupling; no pool, SDK or consumer type | trait docs; conditional pointer publication (compare-and-set on the tree) is unchanged | durable-seams reviewer | PASS |
| Rev 11 consumer closure: no type implements both traits | `testing.rs` has only `MemorySnapshotStore`; `snapshot.rs` holds `&dyn SnapshotStore` | 23 tests pass | PASS |
| AC-R4-1 / FIND-1 / INV-007: reload accepts only the constructor layouts | `storage.rs` `parse`/`canonical` | `hostile_keys` tests and `long_constructor_segments_round_trip` pass | PASS for layout |
| INV-007: no constructed or reloaded key traverses its scope | `TenantId::new` (`identity.rs:73-77`) accepts `.` and `..` | Wave 2 probe: `tenants/../installations/1/...` is constructed and reloads `Ok` | **FAIL** (FIND-12) |
| AC-R4-2: `validate_key_segment` and `KeySegment` deleted | `InvalidIdentity::ObjectKey` | `rg` | PASS |
| AC-R4-3 / FIND-8: imported bare types in signatures | `Deserializer` imported; `JsonError` alias | `rg`; repo-rev's crate-wide sweep | PASS. REPO-1 was ruled a NOTE: a field is not a signature |
| AC-R4-4 / FIND-9 / REQ-013: a secondary limit is retried with an honest wait; a permission 403 stays terminal | `names_rate_limit`, `SECONDARY_RATE_LIMIT_WAIT`, the 429 path | `rate_limits_retry_within_bounds_or_report_typed_outcomes`; a provider-bounds probe covering the permission, OAuth-restriction, secondary-limit body, 429 and cancel-in-sleep cases | PASS |
| AC-R4-5 / FIND-10 / REQ-014: no URL or token in error text | `UntrustedRedirect` is a unit variant | archive test; source-trust sweep of every Display and Debug, and of ureq errors | PASS |
| AC-R4-6 / FIND-11 / REQ-004 / AC-006: GHES `codeload.<host>` and `<host>` trusted, nothing else | `GitHubHost { api_base, archive_hosts }` with private fields; exact case-insensitive authority match | host tests; source-trust lookalike probes (userinfo, suffix, trailing dot, port, backslash, `#@`, `?@`, relative) | PASS |
| AC-R4-7: seal scaffolding deleted | enum, `#[non_exhaustive]`, the `ApiBase` newtype and the `compile_fail` doctests are gone | sweep. The literal regex still matches `InvalidIdentity::ApiBase`, which R4 step 5 keeps on purpose | PASS |
| AC-R4-8: write set | `54232b2` touches `github.rs`, `identity.rs`, `persistence.rs` and `storage.rs`, plus one `snapshot.rs` call site | `git show --stat` | PASS |
| S1 / INV-003: identities and blob bytes verified | SHA, id, name and PR checks; git SHA-1 over blob bytes | source-trust reviewer | PASS |
| REQ-013 / INV-006: cancellation, deadline, bounded reads, no partial result reported as complete | `Execution`, `read_bounded`, truncation and `has_next` pass-through | provider-bounds reviewer | PASS |
| INV-001 / REQ-016: no clone, no symbolic duplication, no server, no source retention, dependency limits | facade-only dependency; manifest | repo-rev | PASS |
| Repository rules: rustdoc, `# Errors`, `thiserror`, no unchecked `unwrap`, tests inline, no `#[allow]`/`#[ignore]` | whole in-scope diff | repo-rev; `lints` | PASS |
| Verification lanes | none | orchestrator at the candidate: `fmt:check` 0; `lints` 0; `test:pipeline` 23 lib tests pass, 0 doctests (the `compile_fail` doctests were deleted under FIND-11, and `--doc` is still in the lane); `cargo doc` has no pipeline warnings | PASS (no lane exercises FIND-12) |

## Validated blocking ledger

The full diagnosis, reproduction and correction are in `findings-validation.md` and the remediation task.

| ID | Wave 1 sources | Status | Gate | Class | Root-cause owner |
|---|---|---|---|---|---|
| FIND-TASK-002-12 | DOM-DS-1 (C1) | CONFIRMED | 2 (INV-007, `spec.md:91`) and 4 (durability and trust boundary) | INCORRECT | `TenantId::new` (`identity.rs:73-77`) does not refuse `.` or `..`, although `validate_name` does |

Siblings: `identity.rs:21-24`, `:70-72` and `:92-96`; `storage.rs:44-46`, `:53-60`, `:122` and `:237`.

## Notes (non-blocking)

- **C2 (DOM-DS-2):** the version segment accepts hex that is not valid UTF-8, such as `ff`, and an empty extraction version would not round-trip. Neither blocks:
  - The key stays scoped.
  - Every producer passes the nonempty `EXTRACTION_VERSION`.
  - The check is exactly what R4 prescribed.
  - It belongs to FIND-1's class, but FIND-1 is not reopened.
- **C3 (REPO-1):** `github.rs:107` `agent: ureq::Agent,` is a private field, not a signature. `AGENTS.md:31` governs signatures, and the repo has precedent for qualified field types (`extract/src/lua.rs:94`). FIND-8 stays closed.
- **For the TASK-003 review:**
  - `snapshot.rs:215` puts `bohmian_code_review::Error` in a function signature.
  - `github.rs:1055` copies the provider's PR `state` string into `MalformedResponse.detail`, and the field doc says "never body text". That value carries no credential or source.
- **AC-R4-7 wording:** the literal regex conflicts with R4's own requirement to keep `InvalidIdentity::ApiBase`. The intent is met.
- **Carried from r4 and r5:** notes N-5 to N-8, and the deferred binding of a reloaded key to the caller's identity (TASK-003..005), remain non-blocking.

## Rejected

No finding was rejected. No citation was rejected.

## Prior-finding closure

| ID | Status | Remaining siblings |
|---|---|---|
| FIND-TASK-002-1 | CLOSED. Layout parsing replaced the per-segment rule. C2 is recorded as a note | none |
| FIND-TASK-002-2 to FIND-TASK-002-7 | CLOSED | none |
| FIND-TASK-002-8 | CLOSED. The six signature sites are fixed. REPO-1 was ruled a field, not a signature | none |
| FIND-TASK-002-9 | CLOSED | none |
| FIND-TASK-002-10 | CLOSED | none |
| FIND-TASK-002-11 | CLOSED. `GitHubHost` is a private-field struct, and the R2 and R3 seal guards are deleted | none |
| FIND-TASK-002-12 | **OPEN (new)** | see the ledger |

## Verification limits

- **No live GitHub or GHES run.** Rate-limit and redirect behavior was checked against documentation and the scripted `FakeTransport`.
- **ureq error text** was audited by reading its source only.
- **Scratch reproductions** ran in `/tmp/cr-r6/scratch-*` with plain `cargo --all-features`, because mise refuses untrusted configs in the copies. They are not checked-in tests.
- **`gate` was not run.** Only the pipeline crate changed since r5. task-rev B ran every workspace test suite green with `--all-features` on an export of the candidate.
- **Wave 2 write denial:** the ledger was saved by the orchestrator, verbatim.
- **No sub-reviewer gap:** every reviewer finished well inside the 20-minute cap. Review wall clock: 16:54–17:06.

## Orchestrator-observed commands (candidate `7983907`, worktree `/tmp/cr-r6/wt`)

| Command | Observed |
|---|---|
| `mise run fmt:check` | exit 0 |
| `mise run lints` | exit 0. Only the vendored COBOL grammar C emits warnings |
| `mise run test:pipeline` | 23 lib tests passed; doc-tests: 0 run, 0 failed |
| `mise exec -- cargo doc --no-deps -p bohmian-code-review-pipeline --all-features` | generated; no pipeline warnings |
| `mise run review:verify-citations …/TASK-002-r6 7983907` | exit 0; 7 entries OK, 0 REJECT |
