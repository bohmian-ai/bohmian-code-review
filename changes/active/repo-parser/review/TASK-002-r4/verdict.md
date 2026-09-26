# TASK-002 task review r4 — verdict

## Immutable subject

- Repository: `bohmian-code-review`, branch `review-helper-setup`
- Base: `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`
- Candidate: `801d9572cf6b0364b5ab33e858388d86f54cb91e` — unchanged for the whole review (`git rev-parse HEAD` checked before Wave 1, after Wave 1, before Wave 2, and before writing this file; working tree clean apart from this review directory)
- Approved spec: `changes/active/repo-parser/spec.md` (SPEC-repo-parser revision 10); task maps REQ-004, REQ-013, REQ-015, REQ-016, INV-001, INV-003, INV-006
- Original task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`
- Prior reviews: r1 `FIX_REQUIRED` (`../TASK-002-r1/verdict.md`); r2 `FIX_REQUIRED` (`../TASK-002-r2/verdict.md`); r3 `FIX_REQUIRED` (`../TASK-002-r3/verdict.md`, remediation `../TASK-002-r3/TASK-002-R3-host-payload-seal-and-lane-doctests.md` implemented in `1958e0b`/`801d957`)
- Diff reviewed: complete cumulative `1842eaf..801d957` (39 files, +6157/−3; code: `Cargo.toml`, `Cargo.lock`, `mise.toml`, `crates/bohmian-code-review-pipeline/{Cargo.toml, src/lib.rs, identity.rs, execution.rs, github.rs, storage.rs, persistence.rs}`; the rest is task/review documentation). R3 fix diff: 7 files, +66/−7.

## Verdict

**PASS** — Wave 1 (two `task-rev` samples, `repo-rev`, `domain-rev` source trust, `domain-rev` provider bounds) proposed zero findings; the citation check passed; Wave 2 `ponytail-rev` returned an explicitly validated empty blocking ledger and independently confirmed FIND-TASK-002-4 through -8 closed with no remaining siblings. Every original-task requirement, scenario, and non-goal and every AC-R1/R2/R3 criterion passes on repository, diff, and verification evidence. Nine non-blocking notes remain. TASK-002 is complete.

## Wave 1 results

| Reviewer | Report | Result | Proposed findings |
|---|---|---|---|
| task-rev A | `task-review-a.md` | PASS | none |
| task-rev B | `task-review-b.md` | PASS | none |
| repo-rev | `standards-review.md` | PASS | none (two observations) |
| domain-rev source trust | `domain-review-source-trust.md` | PASS | none (O-1..O-3) |
| domain-rev provider bounds | `domain-review-provider-bounds.md` | PASS | none (O-1..O-3) |

## Citation check

`mise run review:verify-citations changes/active/repo-parser/review/TASK-002-r4 801d957…` → exit 0; 0 citations (every report ended with an empty `citations` block), 0 REJECT (`citations.txt`). Nothing dropped.

## Acceptance matrix

Consolidated from `task-review-a.md` and `task-review-b.md` (independent samples; all rows agree) with orchestrator-observed verification at the candidate.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004 both hosts via configuration | `identity.rs` `GitHubHost::{Public, Enterprise { api_base: ApiBase }}`; `enterprise()` validates `https://`, no trailing slash; every request URL from `api_base()` | `hosts_expose_api_base_and_archive_host` pass; `enterprise("http://ghe.example.com")` → `Err(InvalidIdentity::ApiBase)` | PASS |
| S1 / REQ-004 identities verified before source enters the library | `GitHubClient::{repository,branch_head,commit,tree,pull_request,blob}` compare ids/SHAs; paths via `RelativePath`, invalid entries to `rejected` | `verified_provider_identities_cross_and_mismatches_are_rejected` pass | PASS |
| S1 / INV-003 blob bytes verified | bounded read then git SHA-1 over `blob <len>\0` vs requested id | same test + `helpers_encode_paths_and_hash_blobs_like_git` | PASS |
| REQ-014 archive redirect trusted only to archive host, one hop, credentials never forwarded | `archive` + `is_trusted_redirect`; ureq `max_redirects(0)` returns the 3xx | `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` pass; ureq 3.4.2 source read by both domain reviewers | PASS |
| S2 / REQ-013 / INV-006 typed retry/terminal outcomes; cancellation and deadline honoured; no partial-as-complete | single `send_with_policy`; `Execution::check` before each attempt and per 100 ms sleep slice; `truncated`/`has_next` explicit | `rate_limits_…`, `malformed_…`, `check_reports_cancellation_then_deadline` pass; provider-bounds scratch: exact-N ok, N+1 → `ResponseTooLarge`, `usize::MAX` reads body | PASS |
| Credentials fail visibly, never leak | redacted `InstallationToken` Debug; `HttpRequest` no Debug; 401/403 typed; JSON errors carry category+position only | `token_debug_is_redacted` pass | PASS |
| REQ-015 / REQ-016 narrow injected traits; no pool, SDK, or consumer type | `ObjectStorage`, `Persistence` traits | pipeline deps: facade, serde, serde_json, sha1, sha2, thiserror, ureq only | PASS |
| Keys and ids validated at the durable seam; round-trip (FIND-1, AC-R1-1..3, AC-R2-1..3) | `validate_key_segment` without length cap; re-validating `Deserialize` for `ObjectKey`/`ContextId` | `long_constructor_segments_round_trip`, `records_revalidate_their_keys_on_deserialization`, `keys_are_revalidated_on_deserialization`, `context_ids_…` pass | PASS |
| INV-001 no clone/checkout, no symbolic duplication, no server/worker, no source retention | facade-only dep; no `std::fs`/`tokio`/`axum`/`std::process` | `rg` empty (both samples) | PASS |
| Scoped `mise` lane owns the crate's proof incl. doctests (AC-R3-3, FIND-5) | `mise.toml:54-59` list-form `run` with `--all-targets` then `--doc`; `test:rust` → `gate` inherit | orchestrator `mise run test:pipeline`: 17 lib passed, `Doc-tests bohmian_code_review_pipeline` 2 passed (lines 310 and 317 `compile fail ... ok`); `gate` exit 0 with the doctest section | PASS |
| Non-goals excluded (facades, OpenDAL, tar decode, conformance impls, limiter, real-GitHub run, symbolic crates untouched) | absent | `git diff --stat` lists only the pipeline crate, workspace manifests, `mise.toml`, change docs | PASS |
| AC-R1-4 / AC-R2-4 `# Errors` on every fallible item; `cargo doc` clean (FIND-2) | all sites | `cargo doc --no-deps -p bohmian-code-review-pipeline --all-features`: no pipeline warnings | PASS |
| AC-R1-5 `ObjectKind` removed (FIND-3) | — | `rg ObjectKind` empty | PASS |
| AC-R2-5 / AC-R3-1 `GitHubHost::Enterprise` neither constructible nor rewritable externally (FIND-4) | `ApiBase(String)` private field, no ctor/`From`/`Default`/`Deserialize`/`&mut` accessor; derives `Debug, Clone, PartialEq, Eq` only; `#[non_exhaustive]` retained | doctests 310 and 317 `compile fail ... ok`; task-rev A/B and source-trust scratch crates: construction E0639+E0308, `&mut` assign E0308 (scaffold without the assignment compiles, so the failure is the intended one), `ApiBase("..")` E0603, `Default` E0277 | PASS |
| AC-R3-2 accessors and client trust rules unchanged | `api_base()` returns `&api_base.0`; `archive_host()` reads `.0` with the same `strip_prefix`/`split` parsing (AC-R2-6) | both named tests pass `--exact` | PASS |
| AC-R3-4 `read_bounded` cannot overflow (FIND-6) | `github.rs:1273` `(limit as u64).saturating_add(1)` | `rg 'as u64 \+ 1'` empty; `read_bounded_accepts_the_maximum_limit` pass; RED-before-fix reproduced on the raw expression in a scratch crate | PASS |
| AC-R3-5 `with_timeout` `# Panics` (FIND-6) | `execution.rs:57-63` | `cargo doc` clean; wording accurate (`Instant + Duration::MAX`) | PASS |
| AC-R3-6 rustdoc on every `mod tests` (FIND-7) | five `///` lines | `grep -B1 '^mod tests'` shows all five | PASS |
| AC-R3-7 no path-qualified `std::fmt` (FIND-8) | `persistence.rs:12` `use std::fmt;` | `rg 'std::fmt::'` empty; `fmt:check` 0; `lints` 0 | PASS |
| AC-R3-8 / AC-R1-6 / AC-R2-7 write set bounded | — | `git diff --stat 34700ca..801d957`: `identity.rs`, `execution.rs`, `github.rs`, `persistence.rs`, `storage.rs`, `mise.toml`, task document only | PASS |
| R3 constraints: no dependency, trait, client scheme check, key length cap, far-future fallback; `Serialize`/key layout/request policy unchanged | fix diff adds only the newtype, docs, one saturating call, one import, one lane command | fix diff read in full by every reviewer | PASS |
| Tests inline (agent-rules.md:5); no `#[allow]`/`#[ignore]` | inline modules via the `HttpTransport` seam | `lints` placement check exit 0 | PASS |
| Rustdoc on every new item incl. tests and `ApiBase` (AGENTS.md:32) | documented | repo-rev sweep | PASS |
| Imports at module top, bare types in signatures (AGENTS.md:31) | — | repo-rev sweep; `rg 'std::[a-z]+::'` in signatures empty | PASS |
| Ponytail ladder on changed abstractions | `ApiBase` newtype is the minimal seal (deleting `#[non_exhaustive]` alone would not stop `&mut` rewrites; a struct still needs a sealed payload); list-form lane is the smallest way to include doctests while keeping `--all-targets` | `findings-validation.md` ladder section | PASS |
| Verification lanes | — | orchestrator at candidate: `fmt:check` 0, `lints` 0, `test:pipeline` 17 + 2 doctests, `gate` 0, `cargo doc` clean | PASS |

## Validated blocking ledger (from `findings-validation.md`)

Validated empty. No `CONFIRMED`, `REVISED`, or `CLASS_INCOMPLETE` finding.

## Notes (non-blocking, from `findings-validation.md`)

- N-1 (repo-rev): `std::hint::black_box` in `read_bounded_accepts_the_maximum_limit` is unnecessary; the plain call panics identically at RED. Cosmetic.
- N-2 (Ponytail): `#[non_exhaustive]` on `Enterprise` is now redundant for the seal; keeping or deleting it is acceptable.
- N-3 (source-trust O-2): `ApiBase` is not re-exported from `lib.rs`; no caller needs to name it.
- N-4 (source-trust O-1): `UntrustedRedirect { location }` echoes the raw `Location` header into error text; no credential is present there.
- N-5 (source-trust O-3): `ObjectDigest` derives plain `Deserialize` and fails closed at integrity check rather than at load.
- N-6 (provider-bounds O-1): `sleep_bounded` may overshoot the deadline by at most one 100 ms slice.
- N-7 (provider-bounds O-2): `max_attempts: 0` behaves as one attempt; doc wording could say so.
- N-8 (provider-bounds O-3): archive stream reads do not observe `CancelToken`; consumer-owned per spec (TASK-003+).
- N-9 (repo-rev): private struct fields lack `///`; fields are not items under AGENTS.md:32 as written.
- Carried from r3: TASK-A-1 scope binding of reloaded `ObjectKey`s is a facade-lookup obligation for TASK-003..005; `file.patch.clone()` avoidable; checked-in tests never exercise a non-zero sleep.

## Rejected

None. No Wave 1 report proposed a finding; no citation was rejected.

## Prior-finding closure

| ID | Status | Remaining siblings |
|---|---|---|
| FIND-TASK-002-1 | CLOSED (r3) | none |
| FIND-TASK-002-2 | CLOSED (r3) | none |
| FIND-TASK-002-3 | CLOSED (r3) | none |
| FIND-TASK-002-4 | CLOSED | none — payload sealed by `ApiBase`; `api_base()`, `archive_host()`, request URL build, bearer attach, `is_trusted_redirect` can no longer receive a non-`https://` base; both `compile_fail` doctests fail for the intended reason |
| FIND-TASK-002-5 | CLOSED | none — `test:pipeline` and `gate` run the doctests |
| FIND-TASK-002-6 | CLOSED | none — `saturating_add`; `# Panics` on `with_timeout`; no other unchecked arithmetic on caller bounds |
| FIND-TASK-002-7 | CLOSED | none |
| FIND-TASK-002-8 | CLOSED | none |

## Verification limits

- No live GitHub/GHES contract run (opt-in integrated gate outside CI); redirect, header, and timeout behaviour proved against the scripted fake and pinned `ureq` 3.4.2 source only.
- `read_bounded` RED-before-fix reproduced on the extracted expression in scratch crates, not by checking out the pre-fix tree.
- `github.rs` request-policy regions unchanged since r2 were read at the policy, archive, bounded-reader, and helper sites plus grep sweeps by task-rev A/B, not line by line this round.
- `sleep_bounded` overshoot not measured on wall clock.
- Every reviewer finished within budget (Wave 1 longest 4 min 52 s; Wave 2 2 min 52 s); no report missing; no sub-reviewer gap recorded. Review wall clock 15:24–15:37.

## Orchestrator-observed commands (candidate `801d957`)

| Command | Observed |
|---|---|
| `mise run fmt:check` | exit 0 |
| `mise run lints` | exit 0 |
| `mise run test:pipeline` | 17 passed; `Doc-tests bohmian_code_review_pipeline` 2 passed (`line 310` and `line 317` `compile fail ... ok`) |
| `mise run gate` | exit 0 (pipeline 17 + doctests 2 included) |
| `mise exec -- cargo doc --no-deps -p bohmian-code-review-pipeline --all-features` | no pipeline-crate warnings |
| `rg 'as u64 \+ 1\|std::fmt::' crates/bohmian-code-review-pipeline/src` | no match |
| `grep -B1 '^mod tests' crates/bohmian-code-review-pipeline/src/*.rs` | `///` above all five |
| `mise run review:verify-citations <dir> 801d957` | exit 0; 0 citations / 0 REJECT |
