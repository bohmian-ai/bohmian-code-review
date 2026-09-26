# TASK-002 task review r3 — verdict

## Immutable subject

- Repository: `bohmian-code-review`, branch `review-helper-setup`
- Base: `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`
- Candidate: `dbd6de34f49764b32b00628cc539e719914469ed` — unchanged for the whole review (`git rev-parse HEAD` checked before Wave 1, before the citation check, before Wave 2, and before writing this file; working tree clean apart from this review directory)
- Approved spec: `changes/active/repo-parser/spec.md` (SPEC-repo-parser revision 10); task maps REQ-004, REQ-013, REQ-015, REQ-016, INV-001, INV-003, INV-006
- Original task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`
- Prior reviews: r1 `FIX_REQUIRED` (`../TASK-002-r1/verdict.md`, remediation R1 in `30b5191`/`5a8cf76`); r2 `FIX_REQUIRED` (`../TASK-002-r2/verdict.md`, remediation `../TASK-002-r2/TASK-002-R2-seam-round-trip-and-host-invariant.md` in `5cfa554`/`ebb3869`)
- Diff reviewed: complete cumulative `1842eaf..dbd6de3` (29 files, +5282/−3; code: `Cargo.toml`, `Cargo.lock`, `mise.toml`, `crates/bohmian-code-review-pipeline/{Cargo.toml, src/lib.rs, identity.rs, execution.rs, github.rs, storage.rs, persistence.rs}`; the rest is task/review documentation)

## Verdict

**FIX_REQUIRED** — Wave 2 retained five blocking findings. One reopens FIND-TASK-002-4 as `CLASS_INCOMPLETE`: `#[non_exhaustive]` sealed construction of `GitHubHost::Enterprise` but enum variant fields are inherently public, so an external `&mut` destructure can still assign an `http://` base and the bearer token is then sent to it. Four are new and mechanical: the `test:pipeline` lane never runs the crate's only doctest (the AC-R2-5 proof) because `--all-targets` excludes doctests; two caller-input panics are undocumented; the five `mod tests` lack rustdoc; and `persistence.rs` path-qualifies `std::fmt` in signatures. FIND-TASK-002-1, -2, and -3 are closed. Every scenario, requirement, and remediation acceptance criterion otherwise passes, and provider bounds pass with no findings.

Remediation task: `changes/active/repo-parser/review/TASK-002-r3/TASK-002-R3-host-payload-seal-and-lane-doctests.md`.

## Wave 1 results

| Reviewer | Report | Result | Proposed findings |
|---|---|---|---|
| task-rev A | `task-review-a.md` | FAIL | TASK-A-1 (INCORRECT), TASK-A-2 (MISSING) |
| task-rev B | `task-review-b.md` | FAIL | TASK-B-1 (DRIFT) |
| repo-rev | `standards-review.md` | FAIL | REPO-1, REPO-2, REPO-3 |
| domain-rev source trust | `domain-review-source-trust.md` | FAIL | TRUST-1 |
| domain-rev provider bounds | `domain-review-provider-bounds.md` | PASS | none (observations O1–O3) |

## Citation check

`mise run review:verify-citations changes/active/repo-parser/review/TASK-002-r3 dbd6de3…` → exit 0; 25 citations, 25 OK, 0 REJECT (`citations.txt`). No finding was dropped.

## Acceptance matrix

Consolidated from `task-review-a.md` and `task-review-b.md` (independent samples; agreeing rows) with orchestrator-observed verification. Rows overridden by Wave 2 are marked.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004 both hosts via configuration | `identity.rs:314-370` `GitHubHost::{Public, enterprise}`; every URL from `api_base()` at `github.rs:1121` | `hosts_expose_api_base_and_archive_host`; scratch: GHES host-with-port consistent with `is_trusted_redirect` | PASS via `enterprise()`; **FAIL for the mutation path — FIND-TASK-002-4** |
| S1 / REQ-004 identities verified before source enters the library (repo id/full_name, branch, commit, tree, PR number + base repo) | `github.rs:801-808, 835-840, 863-868, 893-898, 1009-1014`; paths via `RelativePath`, bad entries to `rejected` | `verified_provider_identities_cross_and_mismatches_are_rejected` 1 passed | PASS |
| S1 / INV-003 blob bytes verified | bounded read then git SHA-1 over `blob <len>\0` compared to requested id (`github.rs:925-943`) | same test + `helpers_encode_paths_and_hash_blobs_like_git` | PASS |
| REQ-014 archive redirect trusted only to the archive host, one hop, credentials never forwarded | `github.rs:956-990`, `is_trusted_redirect` `:741-746` | `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` 1 passed | PASS (trust host degrades to `""` only on the FIND-TASK-002-4 mutation path) |
| S2 / REQ-013 / INV-006 typed retry/terminal outcomes; cancellation and deadline honoured; no partial-as-complete | single `send_with_policy` `github.rs:1163-1209`; `TreeListing.truncated`, `PullRequestFilesPage.has_next` explicit | `rate_limits_…`, `malformed_…`, `check_reports_cancellation_then_deadline`; provider-bounds scratch: mid-sleep cancel 324 ms, no sleep past deadline, exact-limit archive/blob succeed and +1 byte fails | PASS |
| REQ-013 concurrency ceilings | not in this task (caller worker pool) | — | N/A |
| Credentials fail visibly, never leak | redacted `InstallationToken` Debug; `HttpRequest` no Debug; 401/403 typed; JSON errors carry category+position only | `token_debug_is_redacted`; ureq 3.4.2 URL-echo paths unreachable for `https://` URLs | PASS via constructor; **bearer to cleartext via mutation — FIND-TASK-002-4** |
| REQ-015 / REQ-016 narrow injected traits; no pool, SDK, or consumer type | `ObjectStorage` `storage.rs:157-181`; `Persistence` `persistence.rs:131-206` | `cargo tree --depth 1`: facade, serde, serde_json, sha1, sha2, thiserror, ureq | PASS (contract only) |
| Keys and ids validated at the durable seam; constructor keys round-trip (FIND-TASK-002-1, AC-R1-1..3, AC-R2-1..3) | `validate_key_segment` without length cap (`identity.rs:149-154`); `parse` uses it (`storage.rs:96`); `ContextId` private field, re-validating Deserialize | `long_constructor_segments_round_trip`, `records_revalidate_their_keys_on_deserialization` (128-byte tenant + `EXTRACTION_VERSION`), `keys_are_revalidated_on_deserialization` (hostile set), `context_ids_…` all pass | PASS / FIND-TASK-002-1 CLOSED (scope binding at reload is a facade obligation — see Notes, TASK-A-1) |
| INV-001 no clone/checkout, no symbolic duplication, no server/worker, no source retention | facade-only dep; `rg 'std::fs|tokio|axum|std::process'` → none | observed by both samples | PASS |
| Scoped `mise` lane owns the crate's proof | `mise.toml:54-56` `test:pipeline` wired into `test:rust` and `gate` | `mise run test:pipeline` 16 passed; **no `Doc-tests` section: the AC-R2-5 `compile_fail` doctest runs only under `cargo test --doc`** | **FAIL — FIND-TASK-002-5** |
| Non-goals excluded (facades, OpenDAL, tar decode, conformance impls, limiter, real-GitHub run, symbolic crates untouched) | absent | `rg 'opendal|tar|flate'` empty; `git diff --stat -- crates` lists only the pipeline crate | PASS |
| AC-R1-4 / AC-R2-4 `# Errors` on every fallible item; `cargo doc` clean | all sites incl. `BoundedReader::read` (`io::Result`) | R2 sweep → no `MISS`; multi-line signatures read by hand; `cargo doc --no-deps` no pipeline warnings | PASS |
| AC-R1-5 `ObjectKind`/`kind()` removed (FIND-TASK-002-3) | — | `rg -n 'ObjectKind|fn kind' crates` → none | PASS / CLOSED |
| AC-R2-5 `GitHubHost::Enterprise` not externally constructible; `enterprise("http://…")` → `Err(ApiBase)` | `#[non_exhaustive]` `identity.rs:318`; `compile_fail` doctest `:310-312` | `cargo test --doc` 1 passed (compile fail ok); scratch: E0639 on construction, E0638 on exhaustive match; **`&mut` destructure assignment compiles and forges the base** | **FAIL — FIND-TASK-002-4 CLASS_INCOMPLETE** (and proof not in any lane — FIND-TASK-002-5) |
| AC-R2-6 `archive_host` has no indexing slice | `strip_prefix` + `split` (`identity.rs:364-367`) | `rg 'api_base\['` → none; test passes | PASS |
| AC-R1-6 / AC-R2-7 write set bounded | — | cumulative `git diff --stat`: new crate, `Cargo.toml`, `Cargo.lock`, `mise.toml`, change docs only | PASS |
| Tests inline (agent-rules.md:5); no `#[allow]`/`#[ignore]` | inline modules through the `HttpTransport` seam | `lints` placement check exit 0 | PASS |
| Rustdoc on every new item incl. tests (AGENTS.md:32) | fns and helpers documented; **five `mod tests` wrappers undocumented** | `grep -B1 '^mod tests'` | **FAIL — FIND-TASK-002-7** |
| No undocumented panic on input (AGENTS.md:32; rust-core.md:7) | **`execution.rs:59` `Instant::now() + timeout`; `github.rs:1273` `limit as u64 + 1`** | Wave 2 scratch: both `should_panic … ok` | **FAIL — FIND-TASK-002-6** |
| Imports at module top, bare imported types in signatures (AGENTS.md:31) | **`persistence.rs:97-99` `std::fmt::…` path-qualified** while siblings import `fmt` | `rg 'std::fmt::'` | **FAIL — FIND-TASK-002-8** |
| Ponytail ladder on changed abstractions | `HttpTransport` two real impls; `BoundedReader` no stdlib equivalent that errors instead of truncating; `CallCounts` minimal | — | PASS |
| Verification lanes | — | orchestrator at candidate: `fmt:check` 0, `lints` 0, `test:pipeline` 16 passed, `cargo test --doc` 1 passed, `gate` 0 | PASS |

## Validated blocking ledger (from `findings-validation.md`)

| ID | Sources | Status | Gate | Root cause | Correction owner |
|---|---|---|---|---|---|
| FIND-TASK-002-4 | TRUST-1 | CLASS_INCOMPLETE | 4 (bearer to cleartext host); 3 (AGENTS.md:29) | `#[non_exhaustive]` seals construction only; the variant's `String` field remains externally assignable through `&mut` destructuring | `identity.rs:318-322` `Enterprise` payload |
| FIND-TASK-002-5 | TASK-A-2, TASK-B-1 | CONFIRMED | 2 (AC-R2-5 proof); 3 (AGENTS.md:40; testing-workflows.md:15) | `test:pipeline` uses `--all-targets`, which excludes the doctest target; no lane runs `--doc` | `mise.toml:56` |
| FIND-TASK-002-6 | REPO-1 | CONFIRMED | 3 (AGENTS.md:32; rust-core.md:7; agent-rules.md:11) | unchecked arithmetic on caller-supplied bounds, undocumented | `execution.rs:58-60`; `github.rs:1271-1284` |
| FIND-TASK-002-7 | REPO-2 | CONFIRMED | 3 (AGENTS.md:32) | five `mod tests` items lack rustdoc | `execution.rs:87`, `identity.rs:412`, `github.rs:1367`, `persistence.rs:209`, `storage.rs:184` |
| FIND-TASK-002-8 | REPO-3 | CONFIRMED | 3 (AGENTS.md:31; agent-rules.md:11) | `std::fmt` path-qualified in signatures instead of the module-top import used by sibling modules | `persistence.rs:97-99` |

## Notes (non-blocking)

- **TASK-A-1 (task-rev A):** `ObjectKey` reload accepts well-formed unscoped or foreign-tenant keys (`"x"`, another tenant's prefix). Reproduced by Wave 2 but not a reopen of FIND-TASK-002-1: R1/R2 acceptance criteria cover traversal and round-trip only, and r2 recorded scope binding as a facade-lookup obligation. Closure for TASK-003..005 with zero new code: the facade rebuilds the expected key via the existing `ObjectKey::snapshot`/`context` and requires equality with `pointer.key`/`record.key` before any read; an unequal key is a persistence fault. Type-level binding would need identity at `Deserialize` (public-API/persisted-data decision → `SPEC_REVISION_REQUIRED` if the change review wants it). Doc drift to tidy then: `storage.rs:3-5`, `:44-45` claim the invariant for reload, which today holds for constructors only.
- provider-bounds O1: `archive` rustdoc omits that ureq's `timeout_global` also bounds the stream by `min(remaining deadline, request_timeout)`; TASK-003 should size `request_timeout` for archive streaming.
- provider-bounds O2: header-less 429 yields `Disposition::Retry { after: Some(max_retry_wait) }` while the doc describes `after` as the provider's wait; doc-only.
- provider-bounds O3: HTTP-date `retry-after` on 403 classifies as `Forbidden`; GitHub documents integer seconds, so not a defect.
- repo-rev observation: private struct fields carry no field docs; fields are not items, so AGENTS.md:32 does not literally apply.
- source-trust: `ObjectDigest` does not re-validate on deserialize but fails closed on read-back; not material.
- Carried from r2: `file.patch.clone()` at `github.rs` avoidable (preference); checked-in tests never exercise a non-zero sleep (proved by reviewer scratch runs only).

## Rejected

None. Every Wave 1 finding was reproduced by Wave 2; TASK-A-1 was classified as a note, not rejected.

## Prior-finding closure

| ID | Status | Remaining siblings |
|---|---|---|
| FIND-TASK-002-1 | CLOSED | none (`long_constructor_segments_round_trip`, `records_revalidate_their_keys_on_deserialization` with 128-byte tenant + `EXTRACTION_VERSION` pass; hostile set still refused) |
| FIND-TASK-002-2 | CLOSED | none (`# Errors` sweep incl. `io::Result` → no `MISS`; `cargo doc` clean) |
| FIND-TASK-002-3 | CLOSED | none |
| FIND-TASK-002-4 | CLASS_INCOMPLETE | `identity.rs:318-322` (writable payload); consumers `identity.rs:353-358`, `:361-369`; `github.rs:1121`, `:1139`, `:741-746`. The `archive_host` slice sibling is closed (AC-R2-6). |

## Verification limits

- No live GitHub/GHES contract run (opt-in integrated gate outside CI); redirect, header, and timeout behaviour proved against the scripted fake and vendored `ureq` 3.4.2 source only.
- Wave 2 reproduced the `read_bounded` overflow on the extracted expression, not through a fake transport (private fn, single call path); release-profile wrap traced from source, not executed.
- Wave 2 did not re-run `lints`/`fmt:check`/`gate`; the orchestrator ran them at the candidate (all exit 0).
- task-rev A read the `github.rs` test module through test names and pass results, not line by line.
- Every reviewer finished within budget (Wave 1 longest 5 min 9 s; Wave 2 ~9 min); no report was missing; no sub-reviewer gap was recorded. Review wall clock 14:40–15:03.

## Orchestrator-observed commands (candidate `dbd6de3`)

| Command | Observed |
|---|---|
| `mise run fmt:check` | exit 0 |
| `mise run lints` | exit 0 |
| `mise run test:pipeline` | 16 passed, 0 failed; no `Doc-tests` section |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --doc` | 1 passed (`identity::GitHubHost (line 310) - compile fail ... ok`) |
| `mise run gate` | exit 0 (8 `test result: ok` lines) |
| `mise run review:verify-citations <dir> dbd6de3` | exit 0; 25 OK / 0 REJECT |
