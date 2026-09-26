# TASK-002 task review r3 — sample B

## Subject

- Base: `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`
- Candidate: `dbd6de34f49764b32b00628cc539e719914469ed` (HEAD; cumulative range `1842eaf..dbd6de3`)
- Spec: `changes/active/repo-parser/spec.md` (SPEC-repo-parser@10)
- Task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`
- Prior remediation: `review/TASK-002-r1/TASK-002-R1-validated-seam-values.md`, `review/TASK-002-r2/TASK-002-R2-seam-round-trip-and-host-invariant.md`
- Code write set at candidate: `Cargo.toml`, `Cargo.lock`, `mise.toml`, `crates/bohmian-code-review-pipeline/{Cargo.toml,src/{execution,github,identity,lib,persistence,storage}.rs}` (3678 insertions, 2 deletions). No symbolic crate, grammar pin, or fixture touched.

## Acceptance matrix

| Requirement / AC / constraint / non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Both GitHub hosts through configuration (REQ-004) | `identity.rs:314-370` `GitHubHost::{Public, enterprise()}`, `api_base()`, `archive_host()`; `github.rs:1121` builds every URL from `host.api_base()` | `identity::tests::hosts_expose_api_base_and_archive_host`; GHES half of `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` (`github.rs:1722-1744`) — both pass | PASS |
| S1 verified identities: repository, commit, tree, blob, PR verified against request; mismatched repo/path/SHA rejected before symbolic parsing (REQ-004, INV-003) | `github.rs:801-808` (id + full_name), `:835` (branch name), `:863` (commit sha), `:893` (tree sha), `:935-941` (blob SHA-1 recompute), `:1008-1014` (PR number + base repo id); paths via `RelativePath::new` at `:1326`, `:1347`, `:1354` with invalid entries in `rejected` | `verified_provider_identities_cross_and_mismatches_are_rejected` (`github.rs:1507-1654`): 13 requests, 2 Repository, 2 Blob; `../escape`, `/abs`, unknown mode/status rejected; five mismatch cases each `IdentityMismatch` — pass | PASS |
| Untrusted archive redirect rejected; credentials never forwarded (REQ-014) | `github.rs:964-982` one hop only, `is_trusted_redirect` requires `https://<archive_host>` (`:741-746`), hop sent with `authenticated=false` (`:971-977`), stream bounded by `BoundedReader` (`:984-988`) | `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` asserts unauthenticated hop, `evil.example.com` never contacted, over-limit read errors, GHES same-host rule — pass | PASS |
| S2 bounded failure: 403/429, retry-after/reset, cancellation, deadline yield typed retry/terminal; no partial-as-complete (REQ-013, INV-006) | Single policy `send_with_policy` (`github.rs:1163-1209`): `Execution::check` before each attempt, wait bounded by `max_retry_wait` and remaining deadline, `max_attempts`, `sleep_bounded` slices on cancellation (`:1218-1227`); `GitHubError::disposition` (`:350-360`) | `rate_limits_retry_within_bounds_or_report_typed_outcomes`, `malformed_payloads_and_interruptions_are_explicit`, `execution::tests::check_reports_cancellation_then_deadline` — pass; zero requests sent once interrupted (`:1893-1897`) | PASS |
| Malformed payload terminal, never echoes body (REQ-014) | `parse_json` (`:1292-1305`) reports only serde category + position; `read_bounded` (`:1271-1283`) caps bytes | malformed case `detail.starts_with("unexpected json shape")`, 65-byte body over 64 limit → `ResponseTooLarge` — pass | PASS |
| Credentials fail visibly, never leak | `InstallationToken` redacted `Debug` (`identity.rs:404-409`), `HttpRequest` has no `Debug` (`github.rs:42-51`), 401→`Unauthorized`, bare 403→`Forbidden` (`:1185`, `:1232-1233`) | `token_debug_is_redacted`; 401/403 cases pass | PASS |
| Narrow injected storage/persistence traits; every method carries `Identity`; no pool/SDK/consumer type (REQ-015, REQ-016, INV-007) | `storage.rs:157-181` `ObjectStorage`; `persistence.rs:131-206` `Persistence`; keys only via `ObjectKey::snapshot/context` from `&Identity` | `keys_are_scoped_by_identity_and_version`, `digest_is_sha256_hex` pass; `cargo tree --depth 1` → facade, serde, serde_json, sha1, sha2, thiserror, ureq only | PASS |
| INV-001: no symbolic duplication, no server/worker, no source retention; no clone/checkout/disk | Crate depends on `bohmian-code-review` facade only; `Archive`/`blob` return transient bytes | `rg 'std::fs|tokio|axum|std::env' crates/bohmian-code-review-pipeline` → no match (exit 1); `cargo tree` as above | PASS |
| Constraint: no production Node/NAPI/SQLite/PostgreSQL/PyO3/Git-SDK/model SDK | manifests | `Cargo.toml` diff adds `sha1`, `ureq` (rustls, platform-verifier) only | PASS |
| Scoped `mise` lane wired into `test:rust`/`gate` | `mise.toml:54-56`, `:66` | `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` → 16 passed. Lane excludes doctests — see TASK-B-1 | PASS with finding |
| Material stop conditions (no consumer crate import, no raw SQL pool) | — | `cargo tree`; no `sqlx`/`tokio-postgres` | PASS |
| Non-goals: no facades, OpenDAL adapter, tar/gzip decoding, limiter, real-GitHub run | — | `rg -n 'opendal|flate2|tar' crates/bohmian-code-review-pipeline` → none; no facade module | PASS |
| AC-R1-1 `ObjectKey::context(&Identity, &ContextId)` | `storage.rs:78` | `keys_are_scoped_by_identity_and_version` expects `.../contexts/ctx1` — pass | PASS |
| AC-R1-2 `ObjectKey` re-validated on deserialize; constructor keys round-trip; siblings `SnapshotPointer.key`, `ContextRecord.key` | `storage.rs:94-99` `parse` → `validate_key_segment` per segment; `:107-122` manual `Deserialize`; `persistence.rs:51`, `:110` inherit through field type | `keys_are_revalidated_on_deserialization` (7 hostile strings), `records_revalidate_their_keys_on_deserialization` (hostile substitution in both records) — pass | PASS |
| AC-R1-3 `ContextId` validated, private field, re-validating `Deserialize`; sibling `AlreadyPublished { id }` | `persistence.rs:63-95`; `AlreadyPublished` at `:20-23` carries the validated type | `context_ids_are_validated_on_construction_and_deserialization` — pass | PASS |
| AC-R1-4 / AC-R2-4 `# Errors` on every fallible item incl. `BoundedReader::read`; `cargo doc` clean | `github.rs:565-571` and all sites | Sweep (`-> (io::)?Result<`, 14-line lookback) → no `MISS`; multi-line signatures (`repository`, `branch_head`, `commit`, `tree`, `blob`, `archive`, `pull_request`, `pull_request_files`, `get_json`, `request`, `send_with_policy`, `expect_status`, `parse_json`, `Deserialize for InstallationId`) read by hand → all documented; `cargo doc --no-deps -p bohmian-code-review-pipeline` → 0 pipeline warnings | PASS |
| AC-R1-5 `ObjectKind`/`kind()` removed | — | `rg -n 'ObjectKind\|fn kind' crates` → exit 1 | PASS |
| AC-R2-1/2 keys with `EXTRACTION_VERSION` and 128-byte tenant round-trip (snapshot, context, pointer, record) | `identity.rs:149-154` `validate_key_segment` (no cap); `validate_name` keeps 100-byte cap (`:162-167`) | `long_constructor_segments_round_trip`, `records_revalidate_their_keys_on_deserialization` — pass | PASS |
| AC-R2-3 hostile strings still refused; `ContextId::new`/`validate_name` unchanged | as above | hostile-set tests unchanged and passing | PASS |
| AC-R2-5 external `GitHubHost::Enterprise { .. }` fails to compile; `enterprise("http://...")` → `Err(ApiBase)`; siblings `github.rs:1121` URL, `:1138-1140` bearer, `:968` redirect host | `identity.rs:318` `#[non_exhaustive]` on the variant; `:310-312` `compile_fail` doctest; only in-crate constructor at `:347-349` | `cargo test --doc -p bohmian-code-review-pipeline --all-features` → `identity::GitHubHost (line 310) - compile fail ... ok`; `hosts_expose_api_base_and_archive_host` covers `http://ghe.example.com` → `Err(ApiBase)` | PASS (proof not lane-wired: TASK-B-1) |
| AC-R2-6 `archive_host` has no indexing slice | `identity.rs:364-367` `strip_prefix` + `split('/').next()` | `rg 'api_base\[' crates` → exit 1 | PASS |
| AC-R1-6 / AC-R2-7 write set bounded | — | `git diff --stat 93b2542..dbd6de3` code files: `github.rs`, `identity.rs`, `persistence.rs`, `storage.rs` + task evidence + r2 review record (from `e54fa2b`) | PASS |
| Ponytail ladder on changed abstractions | `HttpTransport` trait: two real impls (`UreqTransport`, test fake) — justified; `validate_key_segment` reuses `is_key_safe`; no new dep, helper struct, or trait in remediations | — | PASS |
| `git diff --check` | — | exit 0 | PASS |

## Findings

### TASK-B-1 — DRIFT — the AC-R2-5 regression guard is not executed by any `mise` lane

- **Root cause:** `test:pipeline` (`mise.toml:56`) runs `cargo test ... --all-targets --all-features`. Cargo's `--all-targets` excludes doctests, so the `compile_fail` doctest on `GitHubHost` (`crates/bohmian-code-review-pipeline/src/identity.rs:310-312`), which is the only proof that `Enterprise` cannot be constructed outside the crate, never runs under `test:pipeline`, `test:rust`, or `gate`.
- **Defect class:** verification target present in cargo but absent from every repository lane (test exists, gate never executes it).
- **Siblings:** `mise.toml:56` (owner); `crates/bohmian-code-review-pipeline/src/identity.rs:310` (the orphaned doctest). No other doctest exists in the crate (`rg -n '^\s*///\s*```' crates/bohmian-code-review-pipeline/src` → only line 310). Other lanes use the same flag but own no doctests, so they are not siblings.
- **Violated obligation:** `AGENTS.md:40` ("An absent test target is a failing prerequisite, not a skipped pass"); TASK-002-R2 AC-R2-5 (`TASK-002-R2-seam-round-trip-and-host-invariant.md`, "Acceptance criteria", AC-R2-5) names the doctest as the proof; task "Verification and Evidence" requires the focused lane to carry the crate's proof.
- **Evidence:** `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` prints one `test result: ok. 16 passed` and no `Doc-tests` section; `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features` (no `--all-targets`) additionally prints `Doc-tests bohmian_code_review_pipeline ... identity::GitHubHost (line 310) - compile fail ... ok`. `gate` depends on `test:rust` → `test:pipeline` (`mise.toml:60-67`, `:84`); no lane passes `--doc`.
- **Observable consequence:** removing `#[non_exhaustive]` from `identity.rs:318` (re-opening FIND-TASK-002-4: cleartext bearer token, wrong archive trust host) leaves `gate` green. The invariant holds at the candidate (I ran the doctest; it passes) but has no gate-level guard.
- **Testable correction:** make `test:pipeline` also run doctests, e.g. `run = ["cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features", "cargo test --locked -p bohmian-code-review-pipeline --doc --all-features"]` (or drop `--all-targets` for this crate, which has no integration/bench targets). Proof: `mise run test:pipeline` output includes `identity::GitHubHost (line 310) - compile fail ... ok`; temporarily deleting `#[non_exhaustive]` makes the lane fail.

No other findings. Prior findings FIND-TASK-002-1, -2, -3, -4 are closed across every sibling listed in the r1 and r2 remediation tasks (see matrix rows AC-R1-*, AC-R2-*).

## Commands run (all at candidate `dbd6de3`, working tree clean)

| Command | Observed |
|---|---|
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features` | 16 unit tests passed; Doc-tests: 1 passed (`identity::GitHubHost (line 310) - compile fail ... ok`); exit 0 |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` | 16 passed; no Doc-tests section (TASK-B-1) |
| `mise exec -- cargo doc --no-deps -p bohmian-code-review-pipeline` | Finished; no warnings from the pipeline crate (only vendored-grammar C build warnings from `bohmian-code-review-extract`) |
| `# Errors` sweep from TASK-002-R2 "Focused proof" over `crates/bohmian-code-review-pipeline/src/*.rs` | no `MISS` line |
| `rg -n 'std::fs\|tokio\|axum\|std::env' crates/bohmian-code-review-pipeline` | no match (exit 1) |
| `mise exec -- cargo tree -p bohmian-code-review-pipeline --depth 1` | bohmian-code-review, serde, serde_json, sha1, sha2, thiserror, ureq |
| `rg -n 'ObjectKind\|fn kind\|api_base\[' crates` | no match (exit 1) |
| `git diff --check 1842eaf..dbd6de3` | exit 0 |
| `git diff --stat 1842eaf..dbd6de3 -- crates Cargo.toml Cargo.lock mise.toml` | 10 files, +3678 / -2 |
| `git diff --stat 93b2542..dbd6de3` | code: `github.rs`, `identity.rs`, `persistence.rs`, `storage.rs`; docs: task evidence, r2 review record |

## Verification limits

- `fmt:check`, `lints`, `test:pipeline`, and `gate` were run by the orchestrator, not re-run here; my `cargo test` runs stand in for `test:pipeline`.
- No scratch crate was built; the external-construction guarantee (AC-R2-5) is taken from the passing `compile_fail` doctest, which compiles from outside the crate by construction.
- The real-GitHub contract run is out of scope per the task; `UreqTransport::get` was read but not exercised against a live host.
- Reviewed within the 15-minute budget; no other file under `review/TASK-002-r1` or `review/TASK-002-r2` was read beyond the two remediation tasks.

## Overall result

**FAIL** — one low-severity DRIFT finding (TASK-B-1). Every task requirement, scenario, constraint, non-goal, and remediation AC is implemented and passes when its proof is run; the one gap is that the AC-R2-5 doctest is not executed by `test:pipeline`/`gate`. A one-line `mise.toml` correction closes it.

```citations
[
  {"id": "TASK-B-1", "path": "mise.toml", "line": 56, "snippet": "run = \"cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features\"", "symbols": ["pipeline"]},
  {"id": "TASK-B-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 310, "snippet": "/// ```compile_fail\n/// let _ = bohmian_code_review_pipeline::GitHubHost::Enterprise { api_base: \"http://x\".to_owned() };", "symbols": ["compile_fail", "Enterprise", "GitHubHost"]}
]
```
