# TASK-002 task review r4 — sample B

## Subject

- Repository: `bohmian-code-review`, branch `review-helper-setup`
- Base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`; candidate `801d9572cf6b0364b5ab33e858388d86f54cb91e` (verified `git rev-parse HEAD`)
- Task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`; spec `changes/active/repo-parser/spec.md`
- Cumulative review over base..candidate (`/tmp/task002-r4-code.diff`), including remediation tasks R1, R2, R3 and prior ledger FIND-TASK-002-1..8
- Reviewer: task-rev B, read-only; own conclusions from source and reproduced commands

## Acceptance matrix

| Requirement / AC / constraint / non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004 both GitHub hosts through configuration | `identity.rs:325-333` `GitHubHost::{Public, Enterprise}`; `enterprise()` `:349-368` validates `https://host[/path]`; `api_base()`/`archive_host()` `:370-386` | `identity::tests::hosts_expose_api_base_and_archive_host` passed (17/17 lib run) | PASS |
| S1 verified identities before symbolic parsing (repo id, branch, commit, tree, PR); mismatch rejected | `GitHubClient::{repository, branch_head, commit, tree, pull_request}` compare provider ids/shas; tree and PR paths via `RelativePath`, rejects to `rejected` | `github::tests::verified_provider_identities_cross_and_mismatches_are_rejected` passed | PASS |
| S1 blob bytes verified (INV-003) | `blob` `github.rs:925` bounded read + git SHA-1 compare | same test + `helpers_encode_paths_and_hash_blobs_like_git` passed | PASS |
| S1 untrusted archive redirect rejected; credentials not forwarded (REQ-014) | `archive` `github.rs:956`; one hop, `archive_host()` only, hop unauthenticated, `BoundedReader` `:558-589` | `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` passed | PASS |
| S2 bounded provider failure: 403/429, retry-after, malformed, cancellation, deadline typed; no partial-as-complete | one policy `send_with_policy` `github.rs:1163-1209`: `execution.check()` per attempt, wait capped by `max_retry_wait` and remaining deadline (`too_long`), `max_attempts`; `sleep_bounded` `:1218` cancellable; `TreeListing.truncated`, `PullRequestFilesPage.has_next` explicit | `rate_limits_retry_within_bounds_or_report_typed_outcomes`, `malformed_payloads_and_interruptions_are_explicit`, `execution::tests::check_reports_cancellation_then_deadline` passed | PASS |
| Credentials fail visibly, never leak | `InstallationToken` redacted `Debug` `identity.rs:421-426`; 401/403 typed `status_error` `github.rs:1231`; JSON errors position-only | `token_debug_is_redacted` passed | PASS |
| REQ-015/REQ-016 narrow injected storage/persistence traits; no pool, SDK, consumer type | `ObjectStorage` (`storage.rs`), `Persistence` (`persistence.rs`), every method carries `Identity` | `cargo tree --depth 1`: facade, serde, serde_json, sha1, sha2, thiserror, ureq only | PASS |
| INV-001: no clone/checkout, no disk source, no symbolic duplication, no server/worker | facade-only dependency; transient `Archive`/blob bytes | `rg 'std::fs\|tokio\|axum\|std::process\|opendal\|flate\|tar::'` → no match (exit 1) | PASS |
| Scoped `mise` lane, wired into `test:rust` | `mise.toml:54-59` list form `--all-targets` + `--doc`; `:69` in `test:rust` | lane commands reproduced individually: 17 lib tests + 2 doctests passed | PASS |
| Write set: new crate, workspace manifest/lockfile, tests, mise lane; symbolic crates untouched | `git diff --stat 1842eaf..HEAD`: pipeline crate, `Cargo.toml`, `Cargo.lock`, `mise.toml`, change docs | observed | PASS |
| Non-goals: no facades, OpenDAL, tar decode, limiter, real-GitHub run | absent | `rg` above; crate list | PASS |
| AC-R1-1 `ObjectKey::context(&Identity, &ContextId)` | `storage.rs:80` | `keys_are_scoped_by_identity_and_version` passed | PASS |
| AC-R1-2 `ObjectKey` re-validated on deserialize; constructor keys round-trip | `ObjectKey::parse` `storage.rs:96-101` via `validate_key_segment`; manual `Deserialize` `:110-122` | `keys_are_revalidated_on_deserialization`, `long_constructor_segments_round_trip`, `records_revalidate_their_keys_on_deserialization` passed | PASS |
| AC-R1-3 `ContextId` validated, private field, re-validating `Deserialize` | `persistence.rs:66-96` | `context_ids_are_validated_on_construction_and_deserialization` passed | PASS |
| AC-R1-4 / AC-R2-4 `# Errors` on every fallible item; `cargo doc` clean | present on all `Result`-returning items read (`identity.rs`, `storage.rs`, `persistence.rs`, `github.rs` incl. `BoundedReader::read` `:566-572`, `read_bounded` `:1264-1270`, `sleep_bounded` `:1213-1217`) | `cargo doc --no-deps --locked -p bohmian-code-review-pipeline --all-features` → no pipeline-crate warnings (only vendored C build noise from extract) | PASS |
| AC-R1-5 `ObjectKind`/`kind()` removed | — | `rg` in prior rounds; not present in `storage.rs`/`lib.rs` at candidate | PASS |
| AC-R1-6 / AC-R2-7 write sets bounded | — | per-commit `git log --stat` consistent with the evidence tables | PASS |
| AC-R2-1/2 keys with `EXTRACTION_VERSION` and 128-byte tenant round-trip | `validate_key_segment` no length cap `identity.rs:149-154` | `long_constructor_segments_round_trip`, `records_revalidate_their_keys_on_deserialization` passed | PASS |
| AC-R2-3 hostile strings still refused; `validate_name` 100-byte cap kept | `validate_name` `identity.rs:162-167` | hostile-set tests passed | PASS |
| AC-R2-5 external `Enterprise { .. }` construction fails; `enterprise("http://…")` → `Err(ApiBase)` | `#[non_exhaustive]` `identity.rs:327`; payload `ApiBase` `:331` | doctest line 310 `compile fail ... ok`; reproduced unmarked via `rustdoc --test` against the built rlib: **E0639 (non-exhaustive variant) + E0308 (mismatched types)** — intended reasons, no unrelated error | PASS |
| AC-R2-6 `archive_host` no indexing slice | `strip_prefix` + `split` `identity.rs:380-385` | test passed | PASS |
| AC-R3-1 `&mut` destructure-and-assign fails; construction doctest still fails | `pub struct ApiBase(String)` `identity.rs:340`: private field, derives only `Debug, Clone, PartialEq, Eq`; no `From`, `Default`, `Deserialize`, `DerefMut`, or `&mut` accessor (`rg ApiBase` → 5 hits, all in `identity.rs`); `GitHubHost` has no `Deserialize` | doctest line 317 `compile fail ... ok`; unmarked reproduction: **only E0308 mismatched types**; positive control (same scaffold without the assignment) compiles, so the failure is the assignment alone. Swapping in another `enterprise()`-validated payload via `Clone` compiles — benign, still validated | PASS |
| AC-R3-2 accessors and client trust rules unchanged | `api_base()`/`archive_host()` read `api_base.0`; `is_trusted_redirect` untouched | `hosts_expose_api_base_and_archive_host`, `archive_redirect_…` passed | PASS |
| AC-R3-3 lane runs doctests; gate runs them | `mise.toml:56-59` second command `cargo test --locked -p bohmian-code-review-pipeline --doc --all-features` | `--doc` reproduced: 2 passed; lane/gate execution is the orchestrator's run (see limits) | PASS |
| AC-R3-4 `read_bounded` cannot overflow | `github.rs:1273` `saturating_add(1)` | `rg 'as u64 \+ 1'` → no match; `read_bounded_accepts_the_maximum_limit` `:1939` (`black_box(usize::MAX)`) passed under debug overflow checks | PASS |
| AC-R3-5 `with_timeout` `# Panics`; docs clean | `execution.rs:59-62` | `cargo doc` clean | PASS |
| AC-R3-6 rustdoc on every `mod tests` | `///` line under `#[cfg(test)]` in all five modules | `grep -B1 '^mod tests' … \| grep -c '///'` → 5 | PASS |
| AC-R3-7 no path-qualified `std::fmt` | `persistence.rs:12` `use std::fmt;`; `:98-100` bare `fmt::` | `rg 'std::fmt::'` → no match | PASS |
| AC-R3-8 write set bounded | — | `git log --stat dbd6de3..HEAD`: `34700ca` = r3 review record only; `1958e0b` = `execution.rs`, `github.rs`, `identity.rs`, `persistence.rs`, `storage.rs`, `mise.toml`; `801d957` = task doc evidence only | PASS |
| Rust rules: imports at top, no unchecked `unwrap` on input, `thiserror`, `expect` only for invariants | production `expect`/`unwrap` absent outside tests (`rg` hits are all in `mod tests` or `unwrap_or` fallbacks) | `git diff --check 1842eaf..HEAD` exit 0 | PASS |
| Ponytail ladder on changed abstractions | `ApiBase` newtype is the smallest seal available (enum fields cannot be private; alternative is a private-field struct wrapper on the whole host, which is more code); no new dependency, helper, or trait | — | PASS |

## Proposed findings

None. Every falsification attempt on the r3 fix succeeded in the intended direction: the payload is unconstructible and unassignable from outside the crate (compile errors are exactly E0639/E0308), the lane now includes the doctest target, the overflow site is gone and covered by a RED-then-GREEN test, and the mechanical rustdoc/import obligations hold.

## Verification limits

- `mise run test:pipeline`, `fmt:check`, `lints`, and `gate` were not re-run here (orchestrator lanes); each lane command was reproduced individually with identical results (17 lib tests, 2 doctests, clean `cargo doc`).
- The 1944-line `github.rs` was read at the policy, archive, bounded-reader, and helper sites plus grep sweeps, not line by line; the r1–r3 reviews covered it in full and nothing in the r3 fix diff touches other regions.
- TASK-A-1 (unscoped-key reload) was recorded in r3 as a facade obligation for TASK-003..005, not a TASK-002 acceptance criterion; not re-litigated.

## Overall

**PASS**

```citations
[]
```
