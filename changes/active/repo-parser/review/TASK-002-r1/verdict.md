# TASK-002 task review r1 — verdict

## Immutable subject

- Repository: `bohmian-code-review`, branch `review-helper-setup`
- Base: `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`
- Candidate: `dafe20bd6112b17aed345278d85025fe1a0aaeb2` — unchanged for the whole review (`git rev-parse HEAD` checked before Wave 1, before the citation check, before Wave 2, and before writing this file; the working tree carried only this untracked review directory)
- Approved spec: `changes/active/repo-parser/spec.md` (SPEC-repo-parser revision 10); task maps REQ-004, REQ-013, REQ-015, REQ-016, INV-001, INV-003, INV-006
- Original task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`
- Diff reviewed: complete `1842eaf..dafe20b` (11 files, +3415/−3: workspace `Cargo.toml`/`Cargo.lock`, `mise.toml`, new crate `crates/bohmian-code-review-pipeline` with `lib.rs`, `identity.rs`, `execution.rs`, `github.rs`, `storage.rs`, `persistence.rs`, and the task document's evidence section)
- Prior reviews of this task: none (r1)

## Verdict

**FIX_REQUIRED** — the Wave 2 validation confirmed three blocking findings, all under gate 3 (written repository standard). The provider client itself satisfies every task scenario and mapped requirement; the defects sit in the injected storage/persistence contract (unvalidated key-bearing values), rustdoc completeness, and one uncalled public item.

Remediation task: `changes/active/repo-parser/review/TASK-002-r1/TASK-002-R1-validated-seam-values.md`.

## Wave 1 results

| Reviewer | Report | Result | Proposed findings |
|---|---|---|---|
| task-rev A | `task-review-a.md` | PASS | TASK-A-1 (DRIFT, non-blocking) |
| task-rev B | `task-review-b.md` | PASS | TASK-B-1 (DRIFT, non-blocking) |
| repo-rev | `standards-review.md` | FAIL | REPO-1 |
| domain-rev source trust | `domain-review-source-trust.md` | FAIL | TRUST-1 |
| domain-rev provider bounds | `domain-review-provider-bounds.md` | PASS | none |

## Citation check

`mise run review:verify-citations changes/active/repo-parser/review/TASK-002-r1 dafe20b` → exit 0; 26 citations, 23 OK, 3 REJECT on form only (`citations.txt`):

- TRUST-1 `changes/active/repo-parser/spec.md:77` and `AGENTS.md:29` — rule-line snippets paraphrased; the rule lines exist and were re-read by the orchestrator and Wave 2. All six TRUST-1 code citations OK.
- TASK-B-1 symbol `ObjectKey::kind` — symbol spelled in path form; the sibling citation with `kind` OK.

No finding was dropped: every finding retained at least one OK code citation for its root-cause location (same treatment as TASK-001 r2/r3).

## Acceptance matrix

Consolidated from `task-review-a.md` and `task-review-b.md` (independent samples; identical rows) with orchestrator-observed verification. Rows overridden by Wave 2 are marked.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004 both hosts via configuration | `identity.rs:264-318` `GitHubHost::{Public, enterprise}`; `github.rs:1097` every API URL from `host.api_base()`; archive trust host from `archive_host()` | `identity::tests::hosts_expose_api_base_and_archive_host`; GHES half of `archive_redirect_…` | PASS |
| REQ-004 identity validation before source enters library (repository, commit/tree, path, blob) | `github.rs:789-796` id + full_name; `:823` branch; `:851` commit sha; `:881` tree sha; `:923-929` blob SHA-1 recomputed; `:997` PR number + base repo id; tree/PR paths through `RelativePath` (`:1260`, `:1274`) | `verified_provider_identities_cross_and_mismatches_are_rejected` 1 passed; task-rev B mutation probe: disabling the blob-hash or redirect-trust check makes the scenario tests fail | PASS |
| REQ-004 unauthorized / oversize / unsupported PR state fail explicitly | `status_error` 401/403/404 typed; `read_bounded` → `ResponseTooLarge`; unknown PR state → `MalformedResponse` | `rate_limits_…`, `malformed_…` | PASS |
| REQ-004 credentials handled visibly, never leak | redacted `InstallationToken` `Debug` (`identity.rs:352`); `HttpRequest` has no `Debug`; header attached only when `authenticated` | `identity::tests::token_debug_is_redacted`; archive hop asserted unauthenticated | PASS |
| REQ-013 retry-after / reset / bounded retry time | single `send_with_policy` (`github.rs:1127-1173`); waits capped by `max_retry_wait` and remaining deadline; `max_attempts` | `rate_limits_retry_within_bounds_or_report_typed_outcomes` 1 passed | PASS |
| REQ-013 cancellation and deadline reported honestly | `Execution::check` (`execution.rs:78-83`); wait ≥ remaining → typed error, no sleep; per-request timeout = min(remaining, request_timeout) | `malformed_…`; `execution::tests::check_reports_cancellation_then_deadline` | PASS |
| REQ-013 concurrency ceilings | not in this task; client is synchronous per-call; scenarios do not name it | open obligation for TASK-003–005 / change review | N/A (later task) |
| REQ-013 size bounds (blob, JSON, archive) | `RequestLimits`; `read_bounded` reads `limit+1` before trusting any length; `BoundedReader` fails rather than truncates | `ResponseTooLarge` and archive over-limit cases observed | PASS |
| REQ-014 (supporting) archive redirect: trusted host only, one hop, no credential forwarding | `archive` (`:944-978`) + `is_trusted_redirect` (`:729-734`); `ureq` `max_redirects(0)` verified in vendored source so the hop logic is reachable in production | `archive_redirect_…` 1 passed | PASS |
| REQ-014 / REQ-015 keys and ids carry validated identity across the durable seam | `ObjectKey::snapshot` scoped; **but** `ObjectKey::context(&str)`, transparent `Deserialize` on `ObjectKey`, and `ContextId(pub String)` admit raw text | Wave 2 scratch reproduction: traversing key built, foreign key minted by serde, `ContextId("../x")` constructs while `TenantId::new("../b")` is rejected | **FAIL — FIND-TASK-002-1** |
| REQ-015 narrow persistence trait, no pool, no consumer type | `persistence.rs:104-179`; every method carries `Identity`/`SnapshotScope` | compiles under lane; conformance impl is TASK-006 | PASS (contract only, as tasked) |
| REQ-016 narrow storage trait, exact-key, create-only, delete | `storage.rs:139-163` | `storage::tests::*` | PASS (contract only; key validation gap under FIND-TASK-002-1) |
| INV-001 no clone/checkout, no symbolic duplication, no server/worker | `cargo tree --depth 1`: facade, serde, serde_json, sha1, sha2, thiserror, ureq; no `std::fs`/`tokio`/`axum` | observed by both samples | PASS |
| INV-003 blob integrity | Git SHA-1 over `blob <len>\0` | `helpers_encode_paths_and_hash_blobs_like_git` | PASS |
| INV-006 no hidden budget success | `TreeListing.truncated`, `PullRequestFilesPage.has_next`, `rejected` lists surfaced; rate exhaustion typed | scenario assertions | PASS |
| Scenario 1 / Scenario 2 RED-GREEN-REFACTOR | fake transport drives real client through public `GitHubClient`; identities centralized in `identity.rs`; one policy for every operation including the archive hop | four scenario tests 1 passed each | PASS |
| Constraints: no consumer import, no server/worker loop, never retain source on disk | lib.rs deps; no `std::fs` | `cargo tree`; rg | PASS |
| Write set bounded | exactly the 11 files | `git diff --stat` | PASS |
| Tests inline (agent-rules.md:5) | inline modules exercise the public client through the `HttpTransport` seam | `scripts/check-test-placement.sh` in `lints` | PASS |
| Rustdoc on every new item with `# Errors` (AGENTS.md:32; agent-rules.md:11; rust-core.md:7) | public items documented; **17 fallible private/impl items lack `# Errors`** | Wave 2 sweep | **FAIL — FIND-TASK-002-2** |
| No unchecked `unwrap()` on input/IO | only `unwrap_or*` in production; `expect` only in tests | rg | PASS |
| No speculative abstraction / unnecessary features (AGENTS.md:26; agent-rules.md:10) | `HttpTransport` has two real impls; storage/persistence traits are the task-mandated seam; **`ObjectKind`/`ObjectKey::kind()` have zero non-test callers** | rg | **FAIL — FIND-TASK-002-3** |
| Non-goals excluded (facades, OpenDAL, tar decoding, concurrency limiter, real-GitHub run) | absent | read | PASS |
| Verification lanes | — | orchestrator: `fmt:check` 0, `lints` 0, `test:pipeline` 12 passed, `gate` 0 | PASS |

## Validated blocking ledger (from `findings-validation.md`)

| ID | Sources | Status | Gate | Root cause | Correction owner |
|---|---|---|---|---|---|
| FIND-TASK-002-1 | TRUST-1 | CONFIRMED | 3 (AGENTS.md:29; rust-core.md:7; spec.md:77-78); consequence class 4 | `ObjectKey` and `ContextId` skip the crate's own re-validating construction pattern (`is_key_safe`/`validate_name`, re-validating `Deserialize`) and admit raw text | `identity.rs` validators reused by `storage.rs` / `persistence.rs` |
| FIND-TASK-002-2 | REPO-1 | CONFIRMED (siblings 11→17) | 3 (AGENTS.md:32; agent-rules.md:11; rust-core.md:7) | `# Errors` applied only to public inherent methods and trait declarations | rustdoc on each of the 17 fallible items |
| FIND-TASK-002-3 | TASK-A-1 (ObjectKind part), TASK-B-1 | CONFIRMED | 3 (AGENTS.md:26; agent-rules.md:10) | public key-kind classifier with no production consumer | delete `ObjectKind`, `ObjectKey::kind`, re-export, two asserts |

## Notes (non-blocking)

None retained by Wave 2. Observations recorded by reviewers for later tasks, not findings:

- REQ-013 concurrency ceilings are not implemented in this task; the change review must confirm they land in TASK-003–005 or the caller's pool before the integrated journey.
- `retry-after` HTTP-date form is not parsed (only seconds); in-flight deadline surfaces as `Transport(Timeout)` with the retry disposition; archive stream is not cancel-aware (TASK-003 consumer's concern).
- Facade tasks must call `repository()` before `branch_head()` in the same execution because the branch endpoint carries no repository id.
- RED for the crate was compile failure (crate absent at base), self-reported; same pattern accepted in TASK-001 r3.

## Rejected

- TASK-A-1 second sibling (`Operation::ALL` `github.rs:219`, `CallCounts::snapshot()` `github.rs:247`): consumed by the Scenario 2 accounting test as the task's required request-accounting evidence; no gate applies.

## Verification limits

- No live GitHub/GHES contract run (opt-in integrated gate outside CI per the task); redirect and header behavior proved against the scripted fake and vendored `ureq` source only.
- Wave 2 did not re-run `lints`/`fmt:check`/`gate`; the orchestrator ran them at the candidate (all exit 0) and Wave 2's corrections are doc, deletion, and validation changes that the same lanes will re-prove.
- No reviewer exceeded its budget; no report was missing; no gap was recorded.

## Orchestrator-observed commands (candidate `dafe20b`)

| Command | Observed |
|---|---|
| `mise run fmt:check` | exit 0 |
| `mise run lints` | exit 0 |
| `mise run test:pipeline` | 12 passed, 0 failed; exit 0 |
| `mise run gate` | exit 0 (7 `test result: ok` lines) |
| `mise run review:verify-citations <dir> dafe20b` | exit 0; 23 OK / 3 REJECT (form only) |

## Prior-finding closure

Not applicable (r1).
