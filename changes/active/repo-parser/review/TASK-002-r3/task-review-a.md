# TASK-002 task review (round 3, sample A)

## Subject

- Spec: `changes/active/repo-parser/spec.md` (SPEC-repo-parser@10)
- Task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`
- Prior remediation: `review/TASK-002-r1/TASK-002-R1-validated-seam-values.md`, `review/TASK-002-r2/TASK-002-R2-seam-round-trip-and-host-invariant.md`
- Base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`, candidate `dbd6de34f49764b32b00628cc539e719914469ed` (HEAD), cumulative range `git diff 1842eaf..dbd6de3 -- crates Cargo.toml Cargo.lock mise.toml` (10 files, +3678/-2; new crate `crates/bohmian-code-review-pipeline`).
- No tracked file modified by this review.

## Acceptance matrix

| Requirement / AC / constraint / non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004 both hosts via configuration | `identity.rs:314-370` `GitHubHost::{Public, enterprise}`; every URL built from `host.api_base()` at `github.rs:1121` | `identity::tests::hosts_expose_api_base_and_archive_host` passes; scratch: `enterprise("https://ghe.example.com:8443/api/v3").archive_host() == "ghe.example.com:8443"`, consistent with `is_trusted_redirect` host-with-port compare (`github.rs:741-746`) | PASS |
| S1 verified identities (repo id/full_name, branch name, commit sha, tree sha, PR number + base repo id) | `github.rs:801-808, 835-840, 863-868, 893-898, 1009-1014` compare provider values to the request and fail `IdentityMismatch`; tree/PR paths through `RelativePath`, bad entries to `rejected` (`github.rs:1324-1364`) | `github::tests::verified_provider_identities_cross_and_mismatches_are_rejected` passes | PASS |
| S1 verified blob bytes | `github.rs:925-943` bounded read then `git_blob_sha` compared to requested id | same test + `helpers_encode_paths_and_hash_blobs_like_git` pass | PASS |
| REQ-014 archive redirect trusted only to archive host, credentials never forwarded | `github.rs:956-990`: one hop, `authenticated=false`, `is_trusted_redirect` requires `https://<archive_host>/`; `BoundedReader` limits stream | `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` passes | PASS |
| S2 / REQ-013 / INV-006 typed retry/terminal outcomes; cancellation and deadline honoured; no partial-as-complete | single policy `send_with_policy` (`github.rs:1163-1209`): `Execution::check` per attempt, waits capped by `max_retry_wait` and remaining deadline, `max_attempts`; `GitHubError::disposition`; `TreeListing.truncated` and `PullRequestFilesPage.has_next` explicit | `rate_limits_retry_within_bounds_or_report_typed_outcomes`, `malformed_payloads_and_interruptions_are_explicit`, `execution::tests::check_reports_cancellation_then_deadline` pass | PASS |
| Credentials fail visibly, never leak | `InstallationToken` redacted `Debug` (`identity.rs:404-409`), `HttpRequest` has no `Debug`, 401/403 typed, JSON errors carry category+position only (`github.rs:1292-1315`) | `token_debug_is_redacted` passes; `rg 'derive.*Debug' github.rs:44` none on `HttpRequest` | PASS |
| REQ-015/REQ-016/INV-007 narrow injected traits; no pool/SDK/consumer type; identity at every seam; **no unscoped object keys accepted** | traits `ObjectStorage` (`storage.rs:157-181`) and `Persistence` (`persistence.rs:131-206`) carry `Identity`/`SnapshotScope`; `cargo tree --depth 1`: facade, serde, serde_json, sha1, sha2, thiserror, ureq only | `ObjectKey`'s only reload path (`storage.rs:94-99,118-121`) checks segment alphabet only: scratch reload of `"x"` and of a foreign-tenant key both `Ok`; foreign `SnapshotPointer` reloads `Ok` | **FAIL (TASK-A-1)** |
| INV-001 no symbolic duplication, no server/worker, no source retention | crate depends on facade only; no extraction/index/select code; `rg 'std::fs|tokio|axum'` empty | as listed | PASS |
| Scoped `mise` lane | `mise.toml:54-56` `test:pipeline`, wired into `test:rust` | `mise run test:pipeline` 16 passed; but the lane runs `--all-targets` only, so the crate's sole doctest (the AC-R2-5 `compile_fail` proof) is never executed by the lane or `gate` | **FAIL (TASK-A-2)** |
| Constraint: no clone/checkout/disk retention; no server loop | `Archive`/`blob` transient bytes; no `std::fs` | rg as above | PASS |
| Non-goals (no facade, OpenDAL adapter, tar decoding, limiter, real-GitHub run) | absent | `rg -n 'opendal|tar|flate' crates/bohmian-code-review-pipeline` empty | PASS |
| AC-R1-1 `ObjectKey::context(&Identity, &ContextId)` | `storage.rs:78` | `keys_are_scoped_by_identity_and_version` passes | PASS |
| AC-R1-2 hostile segments refused on reload; constructor keys round-trip | `storage.rs:94-121` | `keys_are_revalidated_on_deserialization`, `records_revalidate_their_keys_on_deserialization` pass | PASS (segment class only; see TASK-A-1 for the scope class the same finding named) |
| AC-R1-3 `ContextId` validated, private field, re-validating `Deserialize` | `persistence.rs:63-95` | `context_ids_are_validated_on_construction_and_deserialization` passes | PASS |
| AC-R1-4 / AC-R2-4 `# Errors` on every fallible item; `cargo doc` clean | all sites | sweep from R2 (14-line lookback) printed no `MISS`; multi-line signatures read by hand (`get_json`, `request`, `send_with_policy`, `expect_status`, `parse_json`, `Deserialize for InstallationId`, trait methods) all carry `# Errors`; `cargo doc --no-deps -p bohmian-code-review-pipeline` emitted no pipeline-crate warning | PASS |
| AC-R1-5 `ObjectKind`/`kind()` removed | — | `rg -n 'ObjectKind|fn kind' crates` exit 1 | PASS |
| AC-R2-1/AC-R2-2 128-byte tenant + `EXTRACTION_VERSION` keys round-trip (ObjectKey, SnapshotPointer, ContextRecord) | `validate_key_segment` without length cap (`identity.rs:149-154`), `parse` uses it (`storage.rs:96`) | `long_constructor_segments_round_trip`, `records_revalidate_their_keys_on_deserialization` pass | PASS |
| AC-R2-3 hostile set still refused; `validate_name`/`ContextId::new` unchanged (100-byte cap) | `identity.rs:162-167` | tests above pass unchanged | PASS |
| AC-R2-5 `GitHubHost::Enterprise` not externally constructible; `enterprise("http://...")` is `Err(ApiBase)` | `#[non_exhaustive]` on variant (`identity.rs:318`), `compile_fail` doctest (`identity.rs:310-312`) | `cargo test --doc` 1 passed (compile fail ok); scratch external `match ... Enterprise { .. }` compiles; `enterprise("http://ghe.example.com")` → `Err(ApiBase)` | PASS (proof not in any lane, see TASK-A-2) |
| AC-R2-6 `archive_host` has no indexing slice | `identity.rs:364-367` `strip_prefix` + `split` | `rg 'api_base\['` exit 1; test passes | PASS |
| AC-R1-6 / AC-R2-7 write set bounded | — | cumulative `git diff --stat`: only the new crate, `Cargo.toml`, `Cargo.lock`, `mise.toml`, and change docs | PASS |
| Ponytail ladder on changed abstractions | `HttpTransport` trait: two real impls (ureq + test fake) — justified. `BoundedReader`: no stdlib equivalent that errors instead of truncating (`Take` truncates) — justified. `CallCounts`: minimal. Persisted `key` fields on `SnapshotPointer`/`ContextRecord` are derivable from `SnapshotScope`+`tree` and `Identity`+`ContextId` respectively — a redundant persisted value that is also the admission point of TASK-A-1 | — | see TASK-A-1 |

## Findings

### TASK-A-1 — INCORRECT — `ObjectKey` reload admits unscoped and foreign-identity keys (FIND-TASK-002-1 CLASS_INCOMPLETE)

- **Root cause (first owner):** `ObjectKey::parse` (`crates/bohmian-code-review-pipeline/src/storage.rs:94-99`) is the only reload path and decides validity by segment alphabet alone; it has no notion of the scope prefix the constructors emit (`storage.rs:52-59`). The R1 correction chose segment validation, which closes traversal but not the "another tenant's object" consequence R1 itself named.
- **Defect class:** validated domain value whose reload invariant is weaker than its construction invariant (scope not re-established at the seam).
- **Siblings (one finding):** `storage.rs:118-121` (`Deserialize for ObjectKey`, admission point); `persistence.rs:51` (`SnapshotPointer.key` reload); `persistence.rs:110` (`ContextRecord.key` reload); `persistence.rs:205` (`remove_repository` returns keys the library will pass to `ObjectStorage::delete`); `storage.rs:3-5` and `:44-45` (module docs asserting the invariant that does not hold).
- **Violated obligation:** spec INV-007 `spec.md:91` ("do not accept unscoped object keys"); REQ-014 `spec.md:77` ("identities scope snapshot/context keys and every read"); REQ-015 `spec.md:78` (exact-key lookup under validated identity); `AGENTS.md:29` (domain values at trust boundaries); R1 task diagnosis (`TASK-002-R1-validated-seam-values.md`, "Observable consequence": key can name another tenant's object) and its recommendation ("a foreign or traversing key is refused").
- **Evidence (scratch crate `/tmp/t002r3a`, path dependency on the candidate crate, `cargo run`):**
  ```
  foreign-tenant key reload: Ok("tenants/b/installations/9/repositories/3/contexts/x")
  unscoped key reload: Ok("x")
  foreign SnapshotPointer reload: true
  ```
- **Observable consequence:** a `Persistence` implementation (or a corrupted row) can return a `SnapshotPointer`/`ContextRecord` whose key names another tenant's object, or an unscoped key such as `x`; the type accepts it, so the cold/update/PR facades (TASK-003–005) will read it under the caller's identity, and `remove_repository` can hand the library another tenant's key to delete. The guarantee in `storage.rs:3-5` remains false at the type level, which is the exact condition R1 set out to remove.
- **Testable correction (Ponytail rung 1, delete):** the persisted `key` is fully derivable — `ObjectKey::snapshot(&scope.identity, &scope.extraction_version, scope.format_version, &pointer.tree)` and `ObjectKey::context(identity, &record.id)`. Remove `key` from `SnapshotPointer` and `ContextRecord`, have the library rebuild it, and change `remove_repository` to return the `(SnapshotScope, ObjectSha)` / `ContextId` material (or keep `Vec<ObjectKey>` and filter through a scope check). If the persisted layout must stay, the fallback is one method `ObjectKey::is_scoped_to(&self, identity: &Identity) -> bool` (prefix match on `ObjectKey::prefix(identity) + "/"`) that every facade applies to reloaded keys. Acceptance: an inline test that a `SnapshotPointer`/`ContextRecord` carrying a foreign or unscoped key cannot produce a read or delete under another identity (either the field no longer exists, or `is_scoped_to` is false and documented as mandatory at every seam); existing round-trip and hostile-segment tests stay green.

### TASK-A-2 — MISSING — the only proof of AC-R2-5 is not executed by any `mise` lane

- **Root cause (first owner):** `mise.toml:56` runs `cargo test ... --all-targets --all-features`; cargo's `--all-targets` expands to `--lib --bins --tests --benches --examples` and excludes doctests. The `compile_fail` doctest at `identity.rs:310-312` is the sole executable proof that `GitHubHost::Enterprise` cannot be constructed outside the crate.
- **Defect class:** verification target absent from the lane that claims to own it.
- **Siblings:** `mise.toml:56` only (the other crate lanes share the pattern, but this is the first crate whose acceptance depends on a doctest).
- **Violated obligation:** `AGENTS.md:40` ("Use `mise.toml` task names ... An absent test target is a failing prerequisite, not a skipped pass"); `testing-workflows.md:15` ("A missing owner target fails rather than skipping"); AC-R2-5 (`TASK-002-R2-seam-round-trip-and-host-invariant.md`, Acceptance criteria) whose evidence in the task document cites a raw `cargo test --doc` command outside every lane.
- **Evidence:** `mise run test:pipeline 2>&1 | grep -c 'Doc-tests'` → `0`; `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --doc` → `1 passed` (`identity::GitHubHost (line 310) - compile fail ... ok`). `gate` depends on `test:rust` → `test:pipeline`, so `gate` also never runs it.
- **Observable consequence:** removing `#[non_exhaustive]` from `Enterprise` leaves `test:pipeline`, `test:rust`, and `gate` green; the FIND-TASK-002-4 regression is invisible to every documented verification path.
- **Testable correction:** make `test:pipeline` a two-command lane: the existing `--all-targets` run plus `cargo test --locked -p bohmian-code-review-pipeline --doc --all-features`. Acceptance: `mise run test:pipeline` output contains `Doc-tests bohmian_code_review_pipeline` and `1 passed`; temporarily removing `#[non_exhaustive]` makes the lane fail.

## Commands run (all at HEAD `dbd6de3`; no tracked file changed)

| Command | Observed |
|---|---|
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features` | 16 passed, 0 failed |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --doc` | 1 passed (`identity::GitHubHost (line 310) - compile fail ... ok`) |
| `mise exec -- cargo doc --no-deps -p bohmian-code-review-pipeline` | no warning from the pipeline crate (only vendored-grammar C build warnings from `bohmian-code-review-extract`) |
| `mise run test:pipeline 2>&1 \| grep -c 'Doc-tests'` | `0` |
| `# Errors` sweep from the R2 task (14-line lookback) over `crates/bohmian-code-review-pipeline/src/*.rs` | no `MISS` |
| `rg -n 'ObjectKind\|fn kind' crates` | exit 1 |
| `rg -n 'api_base\[' crates/bohmian-code-review-pipeline/src` | exit 1 |
| `rg -n 'std::fs\|tokio\|axum\|unwrap\(\)' crates/bohmian-code-review-pipeline/src` (non-test) | no matches |
| `mise exec -- cargo tree -p bohmian-code-review-pipeline --depth 1 -e normal` | facade, serde, serde_json, sha1, sha2, thiserror, ureq |
| `git diff --stat 1842eaf..dbd6de3 -- crates Cargo.toml Cargo.lock mise.toml` | 10 files, +3678/-2 |
| scratch crate `/tmp/t002r3a` (`cargo run`) | output quoted under TASK-A-1; `archive_host with port: ghe.example.com:8443`; `external match with .. ok`; `http enterprise: Err(ApiBase)` |
| `git status --short` | clean |

## Verification limits

- `fmt:check`, `lints`, and `gate` were left to the orchestrator; not re-run here.
- No real-GitHub run (task marks it an opt-in integrated gate).
- Reviewed within the 15-minute budget; the github.rs test module (lines 1366-1935) was read only through its test names and observed pass results, not line by line.

## Overall result

**FAIL** — TASK-A-1 (INV-007 unscoped/foreign key admission at the persistence seam; FIND-TASK-002-1 not fully closed) and TASK-A-2 (AC-R2-5 proof outside every lane).

```citations
[
  {"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":94,"snippet":"fn parse(raw: &str) -> Result<ObjectKey, InvalidIdentity> {","symbols":["parse","validate_key_segment"]},
  {"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":118,"snippet":"fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<ObjectKey, D::Error> {","symbols":["deserialize","ObjectKey"]},
  {"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/storage.rs","line":3,"snippet":"//! The library never accepts an unscoped key: every [`ObjectKey`] is built","symbols":["ObjectKey"]},
  {"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/persistence.rs","line":51,"snippet":"pub key: ObjectKey,","symbols":["SnapshotPointer","key"]},
  {"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/persistence.rs","line":110,"snippet":"pub key: ObjectKey,","symbols":["ContextRecord","key"]},
  {"id":"TASK-A-1","path":"crates/bohmian-code-review-pipeline/src/persistence.rs","line":205,"snippet":"fn remove_repository(&self, identity: &Identity) -> Result<Vec<ObjectKey>, PersistenceError>;","symbols":["remove_repository"]},
  {"id":"TASK-A-2","path":"mise.toml","line":56,"snippet":"run = \"cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features\"","symbols":["pipeline"]},
  {"id":"TASK-A-2","path":"crates/bohmian-code-review-pipeline/src/identity.rs","line":310,"snippet":"/// ```compile_fail","symbols":["compile_fail","non_exhaustive"]}
]
```
