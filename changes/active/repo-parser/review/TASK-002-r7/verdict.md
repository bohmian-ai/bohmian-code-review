# TASK-002 task review r7: verdict

## Immutable subject

- **Repository:** `bohmian-code-review`, branch `review-helper-setup`.
- **Base:** `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`.
- **Candidate:** `9fab199dfb282d07186e9f66ec3b3686328421e7`. All reviewers read it from a detached worktree at `/tmp/cr-r7/wt`. Every reviewer and the orchestrator confirmed `git rev-parse HEAD` was `9fab199` at the start and at the end.
- **Approved spec:** `changes/active/repo-parser/spec.md`, revision 11.
- **Original task:** `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`, including its Revision 11 delta (`Persistence` split into `SnapshotStore` and `ContextStore`).
- **Remediation assessed:** `../TASK-002-r6/TASK-002-R5-tenant-dot-segments.md` (FIND-TASK-002-12), implemented in `6c0e386`.
- **Prior verdicts:**

  | Round | Verdict |
  |---|---|
  | r1 | `FIX_REQUIRED` |
  | r2 | `FIX_REQUIRED` |
  | r3 | `FIX_REQUIRED` |
  | r4 | `PASS` (withdrawn by r5) |
  | r5 | `FIX_REQUIRED` |
  | r6 | `FIX_REQUIRED` |

- **Diff reviewed:** the complete cumulative diff `1842eaf..9fab199` over `crates/`, `Cargo.toml`, `Cargo.lock` and `mise.toml`. The only code commit since r6 is `6c0e386`, which touches `identity.rs` and `storage.rs`.
- **Out of scope:**
  - `741c049` (TASK-003 work in progress): `snapshot.rs`, `eligibility.rs` and the TASK-003 parts of `testing.rs`. Only the edits needed to compile against the new traits were reviewed.
  - The docs-only commits `169bfaf`, `d2c8f9c` and `1f6d538`, the review directories, and the skill files.

## Verdict

**PASS.** Wave 2 validated an empty blocking ledger:

- FIND-TASK-002-12 is closed across its whole class.
- FIND-TASK-002-1 to FIND-TASK-002-11 remain closed.
- No code added by an earlier remediation round has been made redundant.

## Wave 1 results

| Reviewer | Report | Result | Proposed findings |
|---|---|---|---|
| task-rev A | `task-review-a.md` | PASS | none |
| task-rev B | `task-review-b.md` | PASS | none |
| repo-rev | `standards-review.md` | PASS | none |
| domain-rev durable seams (identity → `ObjectKey` construction and reload, store traits) | `domain-review-durable-seams.md` | PASS | none |

- **Domain review scope:** only one domain review was spawned, because R5 changed only the storage-key trust boundary. r6 covered source trust and provider bounds at `7983907`, and `6c0e386` does not touch them.
- **Timing:** Wave 1 ran 17:08–17:11. No reviewer came near the 20-minute cap. Wave 2 (`ponytail-rev`) ran 17:12–17:14.

## Citation check

`mise run review:verify-citations changes/active/repo-parser/review/TASK-002-r7 9fab199` exited 0 (`citations.txt`). Every report ended with an empty `citations` array, so there were no entries to reject. `claim-ledger.md` carried an empty ledger into Wave 2.

## Acceptance matrix

This matrix consolidates the task-rev samples, repo-rev, the domain reviewer and Wave 2.

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| AC-R5-1 / FIND-12: `TenantId::new` and its `Deserialize` refuse `.` and `..` | `is_key_segment` (`identity.rs:57`); `TenantId::new` (`identity.rs:82-83`); serde calls `new` | `identities_validate_key_safe_spellings`. Removing the dot checks in a scratch copy failed 3–4 tests (task-rev A and B) | PASS |
| AC-R5-2: reload refuses dot-only tenant keys | `ObjectKey::parse` (`storage.rs:97-100`) runs each part's validator and requires a canonical re-format | `hostile_keys` has the two new rows; Wave 2 probe test (tenant, context and `01` installation) | PASS |
| AC-R5-3: docs and error text state the rule; INV-007 "never traverses its scope" holds | `InvalidIdentity::Tenant` doc and message; `# Errors` | task-rev A and B, repo-rev | PASS |
| AC-R5-4: write set | `6c0e386` touches `identity.rs` and `storage.rs` | `git show --stat` | PASS |
| INV-007 class sweep: every verbatim key segment goes through one rule | Tenant directly. Owner, repository name and `ContextId` through `validate_name` → `is_key_segment`. Every other segment is numeric, SHA-validated or hex-encoded | domain-rev probes; Wave 2 sibling table | PASS |
| Rev 11 / REQ-015 / AC-009: `SnapshotStore` and `ContextStore` replace `Persistence`, and no type implements both | `persistence.rs` | `rg 'trait Persistence'` is empty; task-rev A and B, domain-rev | PASS |
| Original TASK-002: verified identities and blobs, GitHub and GHES hosts, bounded provider policy, typed rate-limit/retry/deadline outcomes, no secrets in errors | unchanged since r6 PASS rows | 23 pipeline tests; r6 domain reviews | PASS |
| Ponytail / DRIFT: no code made redundant by later rounds | R5 deleted `is_key_safe` and the duplicate dot checks in `validate_name`. The canonical re-format and the version hex check remain load-bearing | Wave 2 DRIFT audit | PASS |
| Repository rules: rustdoc, `# Errors`, `thiserror`, no unchecked `unwrap`, bare imported types, dependency limits, no `#[allow]`/`#[ignore]` | whole in-scope diff | repo-rev | PASS |
| Verification lanes | none | orchestrator commands below | PASS |

**R5 deviation, accepted.** R5 recommended keeping `is_key_safe`. The implementation replaced it with the shared `is_key_segment`. The character rule is the same, and `validate_name`'s behavior is unchanged. task-rev A and B judged this a simplification that removes duplicated checks, not drift.

## Validated blocking ledger

None. See `findings-validation.md`.

## Notes (non-blocking)

- **Wave 2:** the `ContextId::new` rustdoc (`persistence.rs:74-75`) restates the segment rule in prose. This is documentation only.
- **domain-rev:** `ObjectKey::snapshot` with an empty extraction version would not reload. The only producer passes the nonempty `EXTRACTION_VERSION`. R5 lists this (r6 note C2) as a non-goal.
- **Carried from r6:**
  - For the TASK-003 review: `snapshot.rs:215` puts a qualified type in a signature, and `github.rs:1055` copies the PR `state` into `MalformedResponse.detail`.
  - Notes N-5 to N-8 from r4 and r5.
  - The deferred binding of a reloaded key to the caller's identity (TASK-003..005).

## Rejected

No findings or citations were rejected.

## Prior-finding closure

| ID | Status |
|---|---|
| FIND-TASK-002-1 to FIND-TASK-002-11 | CLOSED (r6); no regression |
| FIND-TASK-002-12 | CLOSED. Every constructor, serde and reload sibling routes through `is_key_segment` |

## Verification limits

- **No live GitHub or GHES run.** Provider behavior rests on the scripted fakes and the r6 domain reviews.
- **Scratch copies:** mise refuses untrusted configs in the scratch copies, so reviewers ran each lane's cargo commands directly with `--all-features`. The citation check ran through mise in the repository checkout, which was at `9fab199` and clean.
- **Probe tests** by domain-rev, task-rev A and B, and Wave 2 were temporary and reverted. They are not checked-in tests.
- **`gate` was not run.** The R5 code change is confined to the pipeline crate, which no other crate depends on. Workspace-wide clippy (`-D warnings`) and the test-placement check passed.
- **No sub-reviewer gap.** Review wall clock: 17:07–17:15.

## Orchestrator-observed commands (candidate `9fab199`, scratch copy `/tmp/cr-r7/scratch-orch`)

| Command (lane) | Observed |
|---|---|
| `cargo fmt --all -- --check` (`fmt:check`) | exit 0 |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` and `bash scripts/check-test-placement.sh` (`lints`) | exit 0, exit 0 |
| `cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` and `--doc` (`test:pipeline`) | 23 passed, 0 failed; doc-tests 0 run, exit 0 |
| `mise run review:verify-citations …/TASK-002-r7 9fab199` | exit 0; 0 entries, 0 REJECT |
