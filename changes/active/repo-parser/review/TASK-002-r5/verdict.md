# TASK-002 task review r5: verdict

## Immutable subject

- **Repository:** `bohmian-code-review`, branch `review-helper-setup`.
- **Base:** `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`.
- **Candidate:** `6899a4323b486a1b99eaeaf1fe3af67ea8c9e429`. Every reviewer read it from a detached worktree at `/tmp/cr-r5/wt`, or with `git show 6899a43:`. The main working tree's HEAD (`169bfaf`) is past the candidate and was not in scope: `741c049` is unrelated TASK-003 work in progress, and `169bfaf` is the skill revision. `git rev-parse 6899a43` was checked before Wave 1, after Wave 1, and before this file was written, and did not change.
- **Candidate vs. `801d957` (the r4 subject):** the only difference is the r4 review documents. The code is identical.
- **Approved spec:** `changes/active/repo-parser/spec.md`.
- **Original task:** `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`.
- **Prior remediations:** `../TASK-002-r1/TASK-002-R1-validated-seam-values.md`, `../TASK-002-r2/TASK-002-R2-seam-round-trip-and-host-invariant.md`, `../TASK-002-r3/TASK-002-R3-host-payload-seal-and-lane-doctests.md`.
- **Prior verdicts:**

  | Round | Verdict |
  |---|---|
  | r1 | `FIX_REQUIRED` |
  | r2 | `FIX_REQUIRED` |
  | r3 | `FIX_REQUIRED` |
  | r4 | `PASS`, under the pre-`169bfaf` rules |

- **Why this is a re-review:** r5 is a deliberate re-review of the whole cumulative candidate under the revised skill, applying its candidate-owned-surface, unrepresentable-shape and superseded-fix-deletion rules.
- **Diff reviewed:** the complete cumulative `1842eaf..6899a43`. The code files are:
  - `Cargo.toml`, `Cargo.lock`, `mise.toml`
  - `crates/bohmian-code-review-pipeline/Cargo.toml`
  - `crates/bohmian-code-review-pipeline/src/{lib.rs, identity.rs, execution.rs, github.rs, storage.rs, persistence.rs}`

## Verdict

**FIX_REQUIRED.** Wave 2 reproduced five blocking findings:

- **Reopened as `CLASS_INCOMPLETE`:**
  - FIND-TASK-002-1: the `ObjectKey` reload accepts layouts that no constructor produces.
  - FIND-TASK-002-8: crate-qualified types appear in signatures.
- **New, `CONFIRMED`:**
  - FIND-TASK-002-9: a secondary rate limit without headers is reported as terminal `Forbidden`.
  - FIND-TASK-002-10: the redirect error text leaks the signed archive token.
  - FIND-TASK-002-11: GHES codeload archive redirects are rejected. The fix reshapes `GitHubHost` into a struct and deletes the superseded R2/R3 seal guards.

**r4's PASS is withdrawn.** Remediation: `TASK-002-R4-key-layout-rate-limits-and-host-shape.md`.

## Wave 1 results

| Reviewer | Report | Result | Proposed findings |
|---|---|---|---|
| task-rev A | `task-review-a.md` | FAIL | TASK-A-1 (DRIFT), TASK-A-2 (INCORRECT) |
| task-rev B | `task-review-b.md` | FAIL | TASK-B-1, TASK-B-2, TASK-B-3 |
| repo-rev | `standards-review.md` | FAIL | REPO-1, REPO-2 |
| domain-rev source trust | `domain-review-source-trust.md` | FAIL | DOM-ST-1, DOM-ST-2, DOM-ST-3 |
| domain-rev provider bounds | `domain-review-provider-bounds.md` | FAIL | DOM-PB-1 |

- **Timing:** all five reports were written 16:25–16:31. The longest reviewer, repo-rev, took about 6 minutes.
- **Wave 2:** `ponytail-rev` ran 16:32–16:38. The harness denied it write access to the review directory, so it returned the ledger in full and the orchestrator saved it verbatim as `findings-validation.md`. The orchestrator re-read the key source sites at the candidate: `storage.rs:90-100`, `github.rs:1180-1192`, `identity.rs:376-388`, and spec REQ-013, REQ-014 and INV-007.

## Citation check

`mise run review:verify-citations changes/active/repo-parser/review/TASK-002-r5 6899a43` exited 0, with 43 entries and 7 REJECT (`citations.txt`).

**What the rejects are:** every REJECT is a qualified symbol name in the entry's `symbols` list, which the whole-word matcher does not resolve. Each unqualified name does occur in the cited file:

| Symbol | Occurrences |
|---|---|
| `max_retry_wait` | 8 |
| `archive` | 21 |
| `deserialize` | 2 |
| `archive_host` | 3 |
| `remove_repository` | 1 |

Every rejected entry's line and snippet also matched.

**What was dropped:** the 7 rejected citation entries. Every finding kept at least one OK citation, so no finding was dropped. The claim ledger (`claim-ledger.md`) passes only OK citations.

## Acceptance matrix

This matrix consolidates `task-review-a.md` and `task-review-b.md` with the Wave 2 results. Rows the two samples agree on and Wave 2 did not overturn are carried as PASS.

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004 / AC-006: both hosts supported through configuration | `identity.rs:305-388` `GitHubHost`; `archive_host` returns only the API host for GHES | scratch C4: a GHES 302 to `codeload.<host>` becomes `UntrustedRedirect`, Terminal | **FAIL** (FIND-11) |
| S1 / INV-003: identities and blob bytes verified | `GitHubClient` id/SHA checks; git SHA-1 over `blob <len>\0` | `verified_provider_identities_…`, `helpers_encode_paths_and_hash_blobs_like_git` pass | PASS |
| REQ-014: redirect trust, one hop, credentials never forwarded | `archive`, `is_trusted_redirect`, ureq `max_redirects(0)` | archive redirect test passes; source-trust reviewer read the ureq 3.4.2 source | PASS |
| REQ-014: credentials never appear in logs | `UntrustedRedirect { location }` Display echoes the raw URL | scratch C3: `?token=SECRET-SIGNED` appears in `to_string()` | **FAIL** (FIND-10) |
| REQ-013 / S2: primary and secondary limits give typed retry or terminal outcomes | `send_with_policy` 403/429 arm reads headers only | scratch C1a: a secondary-limit 403 gives `Forbidden`/Terminal. C1b: a bare 429 reports `max_retry_wait` as the provider wait | **FAIL** (FIND-9) |
| REQ-013 / INV-006: cancellation, deadline, bounded reads, no partial result reported as complete | `Execution::check`, `sleep_bounded`, `read_bounded` with `saturating_add`, `truncated`/`has_next` | provider-bounds reviewer; `check_reports_cancellation_then_deadline`, `read_bounded_accepts_the_maximum_limit` pass | PASS |
| REQ-015 / REQ-016: narrow injected traits; no pool, SDK or consumer type | `ObjectStorage`, `Persistence` | repo-rev dependency sweep | PASS |
| INV-007 / R1 AC-R1-2: unscoped or foreign keys refused on reload | `storage.rs:94-99` checks only the segment alphabet | scratch C2: `"x"`, `"foo/bar"`, installation `0` and a non-hex version all deserialize | **FAIL** (FIND-1) |
| R2 AC-R2-1..3: long constructor segments round-trip | `validate_key_segment`, no length cap | `long_constructor_segments_round_trip` passes | PASS (must be preserved) |
| R3 AC-R3-1: `Enterprise` payload sealed | private `ApiBase.0` | the seal holds; the doctests fail with E0308 whatever the privacy, which AC-R3-1 allowed as written | PASS; superseded guards are DRIFT (FIND-11) |
| R3 AC-R3-3 / FIND-5: the lane runs doctests | `mise.toml:54-59` `--doc` | `test:pipeline`: 2 doctests run | PASS |
| R3 AC-R3-4/5 / FIND-6: overflow guard, `# Panics` | `saturating_add`, `execution.rs:59` | `rg 'as u64 \+ 1'` finds nothing | PASS |
| R3 AC-R3-6 / FIND-7: rustdoc on `mod tests` | five `///` lines | grep | PASS |
| AGENTS.md:31: imports at top, bare types in signatures | six `serde::Deserializer` / `serde_json::Error` signature sites | `rg` | **FAIL** (FIND-8) |
| AGENTS.md:27: no future-proofing | `#[non_exhaustive]` redundant after R3 | a scratch deletion keeps 17 unit tests and 2 doctests green | FAIL, as DRIFT folded into FIND-11 |
| INV-001: no clone, no symbolic duplication, no server, no source retention | facade-only dependency | `rg` for `std::fs`/`tokio`/`std::process` finds nothing | PASS |
| Non-goals excluded | only the pipeline crate, workspace manifests, `mise.toml` and docs changed | `git diff --stat` | PASS |
| Tests inline; no `#[allow]`/`#[ignore]`; rustdoc and `# Errors` | inline `mod tests` | `lints` exit 0; `cargo doc` clean | PASS |
| Verification lanes | none | orchestrator at the candidate: `fmt:check` 0, `lints` 0, `test:pipeline` 17 lib tests and 2 doctests pass, `cargo doc` has no pipeline warnings | PASS (the lanes are green; none of them exercises the five defects) |

## Validated blocking ledger

The full diagnosis, reproduction and correction for each finding are in `findings-validation.md` and the remediation task.

| ID | Wave 1 sources | Status | Gate | Class | Root-cause owner |
|---|---|---|---|---|---|
| FIND-TASK-002-1 | TASK-B-1, DOM-ST-2 | CLASS_INCOMPLETE | 2 (R1 `:63,65`; INV-007 `spec.md:91`) | INCORRECT | `ObjectKey::parse` (`storage.rs:94-99`) validates the segment alphabet, not the constructor layout. The R1 and R2 per-segment rules are both replaced. `validate_key_segment` and `InvalidIdentity::KeySegment` are deleted |
| FIND-TASK-002-8 | REPO-1 | CLASS_INCOMPLETE | 3 (`AGENTS.md:31`) | VIOLATION | The R3 sweep searched for one path (`std::fmt::`), not the rule. Six `serde::`/`serde_json::` signature sites remain |
| FIND-TASK-002-9 | TASK-A-2, DOM-PB-1 | CONFIRMED | 1, 2 (REQ-013 `spec.md:76`; S2) | INCORRECT | The `send_with_policy` 403/429 arm (`github.rs:1182-1191`) classifies from headers only |
| FIND-TASK-002-10 | DOM-ST-1 | CONFIRMED | 4 (REQ-014 `spec.md:77`) | INCORRECT | The `UntrustedRedirect { location }` payload (`github.rs:339-345`, `:969`) |
| FIND-TASK-002-11 | DOM-ST-3; absorbs TASK-A-1, TASK-B-2, TASK-B-3, REPO-2 | CONFIRMED | 1, 2 (REQ-004 `spec.md:61`; AC-006) | INCORRECT + DRIFT | `GitHubHost` enum-with-payload (`identity.rs:305-388`): a single re-derived archive host, plus the layered seal guards (`#[non_exhaustive]`, `ApiBase`, and two doctests that prove nothing about privacy) |

## Notes (non-blocking)

- C6 (REPO-2): the claim that `ApiBase` is "unnameable" is wrong. `pub mod identity` makes it reachable. This becomes moot under FIND-11.
- C5 (TASK-B-2): the doctests met AC-R3-1 as written, and FIND-4's seal held at the candidate. They are deleted as DRIFT, not reopened.
- r3 deferral, carried forward: binding a reloaded, well-formed `ObjectKey` to the caller's identity remains a facade-lookup obligation for TASK-003..005. FIND-1 is about keys that have no scope at all.
- r4 notes N-5 to N-8 (`ObjectDigest` plain `Deserialize`, 100 ms sleep overshoot, `max_attempts: 0`, archive stream ignoring `CancelToken`): Wave 1 did not re-raise them, and they stay non-blocking.

## Rejected

- No finding was rejected. Seven citation entries were dropped for qualified symbol names; see the citation check.
- Two claims were `REVISED` into DRIFT under FIND-11 instead of standing as separate findings: C5 / TASK-B-2 and C6 / TASK-A-1, TASK-B-3, REPO-2.

## Prior-finding closure

| ID | Status | Remaining siblings |
|---|---|---|
| FIND-TASK-002-1 | **CLASS_INCOMPLETE** (reopened) | `storage.rs:94-99`, `:107-122`, `:3-5`, `:39-45`; `persistence.rs:52`, `:111`, `:206`; `identity.rs:35-38`, `:138-154`; tests `storage.rs:221-244`, `persistence::tests::records_revalidate_their_keys_on_deserialization` |
| FIND-TASK-002-2 | CLOSED | none |
| FIND-TASK-002-3 | CLOSED | none |
| FIND-TASK-002-4 | CLOSED | none. The seal holds. Its superseded guards are removed under FIND-11, which replaces the enum with a private-field struct |
| FIND-TASK-002-5 | CLOSED | none. `--doc` stays in the lane |
| FIND-TASK-002-6 | CLOSED | none |
| FIND-TASK-002-7 | CLOSED | none |
| FIND-TASK-002-8 | **CLASS_INCOMPLETE** (reopened) | `identity.rs:94`, `:131`, `:299`; `persistence.rs:92`; `storage.rs:118`; `github.rs:1308` |

## Verification limits

- **No live GitHub or GHES run.** Three findings rest on public documentation, not a live instance, and were reproduced against the scripted `HttpTransport` fake:
  - FIND-9: GitHub's documented secondary-limit responses and the one-minute minimum wait.
  - FIND-11: GHES serving archives from `codeload.<host>` under subdomain isolation, and from `<host>/_codeload/` without it.
  - FIND-10: the signed-token query on archive redirects.
- **Wave 2 reproductions** ran in the scratch crate `/tmp/cr-r5/scratch-ponytail/repro`, not in checked-in tests. Each finding's closure proof is a test that fails at the candidate.
- **`gate` was not run this round.** `fmt:check`, `lints`, `test:pipeline` and `cargo doc` were run at the candidate. No crate outside the pipeline crate changed since r4, which ran `gate` green on identical code.
- **Wave 2 write denial.** The harness blocked `ponytail-rev` from writing `findings-validation.md`. The orchestrator saved the agent's returned ledger verbatim; the agent's conclusions were not modified.
- **Timing.** Every reviewer finished within budget, and no sub-reviewer gap was recorded. Review wall clock: 16:24–16:45.

## Orchestrator-observed commands (candidate `6899a43`, worktree `/tmp/cr-r5/wt`)

| Command | Observed |
|---|---|
| `mise run fmt:check` | exit 0 |
| `mise run lints` | clippy finished; test-placement check exit 0 |
| `mise run test:pipeline` | 17 lib tests passed; `Doc-tests bohmian_code_review_pipeline` 2 passed (lines 310 and 317, `compile fail ... ok`) |
| `mise exec -- cargo doc --no-deps -p bohmian-code-review-pipeline --all-features` | no pipeline-crate warnings |
| `mise run review:verify-citations …/TASK-002-r5 6899a43` | exit 0; 43 entries, 7 qualified-symbol REJECTs, no finding dropped |
