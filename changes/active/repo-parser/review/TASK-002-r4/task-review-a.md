# TASK-002 task review r4 — sample A

## Subject

- Repository: `bohmian-code-review`, branch `review-helper-setup`
- Base: `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`; candidate: `801d9572cf6b0364b5ab33e858388d86f54cb91e` (verified `git rev-parse HEAD`)
- Spec: `changes/active/repo-parser/spec.md` (rev 10); task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`
- Cumulative review over the original task plus R1, R2, R3 remediation obligations. Diffs read: `/tmp/task002-r4-code.diff` (cumulative), `/tmp/task002-r4-r3fix.diff` (r3 fix). Conclusions formed from source at the candidate, not from the evidence sections.

## Commands run (all at the candidate, read-only)

| Command | Observed |
|---|---|
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --doc` | 2 doctests: `GitHubHost (line 310) - compile fail ... ok`, `GitHubHost (line 317) - compile fail ... ok` |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib` | 17 passed, 0 failed (incl. `read_bounded_accepts_the_maximum_limit`, `hosts_expose_api_base_and_archive_host`, `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials`) |
| External scratch crate `/tmp/t002r4` (path dep on the pipeline crate) compiling both doctest bodies plus sibling probes (`ApiBase(..)` construction, `Default::default()`) | Construction: `E0639` (non-exhaustive struct expression) and `E0308`; `&mut` assign from `String`: `E0308` mismatched types; `ApiBase("..")`: `E0603` tuple struct constructor private; `Default`: `E0277`. Both doctests fail for the intended reason, not for an unrelated error. |
| `mise exec -- cargo doc --no-deps --locked -p bohmian-code-review-pipeline --all-features` | Finished, 0 pipeline-crate warnings |
| `rg -n '&mut GitHubHost|&mut ApiBase|impl ApiBase|pub fn.*-> &mut' crates/bohmian-code-review-pipeline/src` | no match |
| `rg -n 'std::fmt::|as u64 \+ 1' crates/bohmian-code-review-pipeline/src` | no match |
| `grep -B1 '^mod tests' crates/bohmian-code-review-pipeline/src/*.rs` | `///` line above all five modules |
| `git diff --stat 34700ca..HEAD` | `execution.rs`, `github.rs`, `identity.rs`, `persistence.rs`, `storage.rs`, `mise.toml`, task document only |
| `rg -n 'as u64|\+ 1|saturating|checked_' crates/bohmian-code-review-pipeline/src/github.rs` | remaining arithmetic: `BoundedReader::read` (`try_from(...).unwrap_or`, `remaining -= read`), `reset.saturating_sub(...)`, `read_bounded` saturating; no other overflowable caller-input site |

## Acceptance matrix

| Requirement / AC / constraint / non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Original AC: both GitHub hosts via configuration (REQ-004) | `identity.rs:323-340` `GitHubHost::{Public, Enterprise{api_base: ApiBase}}`, `enterprise()` validates `https://` and rejects trailing slash; `github.rs` `request` builds URLs from `api_base()` | `identity::tests::hosts_expose_api_base_and_archive_host` pass; `http://ghe.example.com` -> `Err(InvalidIdentity::ApiBase)` (`identity.rs:498`) | PASS |
| Original S1: verified identities; mismatched repo/path/SHA rejected before parsing | `GitHubClient::{repository,branch_head,commit,tree,pull_request,blob}` compare ids/shas; tree/PR paths via `RelativePath`, invalid into `rejected` | `github::tests::verified_provider_identities_cross_and_mismatches_are_rejected` pass (in 17) | PASS |
| Original S1: untrusted archive redirect rejected, credentials never forwarded (REQ-014) | `GitHubClient::archive` one hop, only to `archive_host()`, unauthenticated, `BoundedReader` | `archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` pass | PASS |
| Original S2: 403/429, malformed, cancellation, deadline yield typed retry/terminal outcomes (REQ-013, INV-006) | `send_with_policy`, `GitHubError::disposition`, `Execution::check`, `CallCounts` | `rate_limits_retry_within_bounds_or_report_typed_outcomes`, `malformed_payloads_and_interruptions_are_explicit`, `execution::tests::check_reports_cancellation_then_deadline` pass | PASS |
| Original: narrow injected storage/persistence traits, no consumer type, no pool/SDK (REQ-015/016) | `storage.rs` `ObjectStorage`, `ObjectKey`, `ObjectDigest`; `persistence.rs` `Persistence`, `SnapshotPointer`, `ContextRecord`, `ContextId` | `storage::tests::*`, `persistence::tests::*` pass; pipeline `Cargo.toml` deps: facade, serde, serde_json, sha1, sha2, thiserror, ureq only | PASS |
| Original: no clone/checkout/source retention, no server/worker, no symbolic duplication (INV-001) | crate depends on facade only; transient bytes; no `std::fs`/`tokio`/`axum` | `rg` no match in `src` (non-test) | PASS |
| Original: workspace manifest/lockfile, scoped `mise` lane | `Cargo.toml` member + `sha1`, `ureq` (rustls, no default features); `mise.toml:54-59` `test:pipeline`; wired into `test:rust` (`:61-70`) -> `gate` | lane inspected; orchestrator runs `test:pipeline`, `gate` | PASS |
| Original non-goals: no facade, OpenDAL, tar/gzip, limiter, real-GitHub run; symbolic crates untouched | cumulative diff touches only the pipeline crate, workspace manifests, `mise.toml`, change docs | `git diff --stat 1842eaf..HEAD` | PASS |
| AC-R1-1..6 (`ObjectKey::context(&ContextId)`, re-validating `Deserialize` for `ObjectKey`/`ContextId`, `# Errors` sweep, `ObjectKind` removed, write set) | present in `storage.rs`/`persistence.rs`/`identity.rs` at candidate | `keys_are_revalidated_on_deserialization`, `records_revalidate_their_keys_on_deserialization`, `context_ids_are_validated_on_construction_and_deserialization` pass; `cargo doc` clean; `rg ObjectKind` no match (prior-round verified, spot-checked) | PASS |
| AC-R2-1..4, R2-6..7 (`validate_key_segment` no length cap, 128-byte tenant round trip, hostile strings refused, `# Errors` on `BoundedReader::read`, no indexing slice in `archive_host`) | `identity.rs:379-385` uses `strip_prefix`/`split`; `long_constructor_segments_round_trip` present | tests pass in 17; `cargo doc` clean | PASS |
| AC-R2-5: external `Enterprise { .. }` construction fails to compile; `enterprise("http://...")` still `Err(ApiBase)` | `#[non_exhaustive]` + `ApiBase` payload | doctest line 310 `compile fail ... ok`; scratch crate `E0639`+`E0308` | PASS |
| AC-R3-1: `&mut` destructure-and-assign from `String` fails to compile; construction doctest still fails | `identity.rs:317-322` doctest; `ApiBase(String)` private tuple field, no ctor, no `From<String>`, no `Default`, no `&mut` accessor (`rg` no match); derives only `Debug, Clone, PartialEq, Eq`; no `Deserialize` on `GitHubHost`/`ApiBase` | doctest line 317 `compile fail ... ok`; scratch crate `E0308`; sibling probes `E0603`, `E0277` | PASS |
| AC-R3-2: accessors and client trust rules unchanged | `api_base()` returns `&api_base.0`; `archive_host()` reads `.0` with same parsing | both named tests pass `--exact` within the lib run; `enterprise` rules untouched in diff | PASS |
| AC-R3-3: lane runs doctests; `gate` inherits | `mise.toml:56-59` list form adds `--doc` command; `test:rust` depends on `test:pipeline`; `gate` depends on `test:rust` | lane definition read; orchestrator lane runs pending (see limits); direct `--doc` invocation reproduces the two lines | PASS |
| AC-R3-4: `read_bounded` cannot overflow; `usize::MAX` test | `github.rs:1273` `(limit as u64).saturating_add(1)` | `rg 'as u64 \+ 1'` no match; `read_bounded_accepts_the_maximum_limit` (black_box `usize::MAX`, 5-byte cursor) pass | PASS |
| AC-R3-5: `with_timeout` `# Panics`; docs clean | `execution.rs:57-63` | `cargo doc` 0 pipeline warnings | PASS |
| AC-R3-6: rustdoc on every `mod tests` | five `///` lines | `grep -B1` shows all five | PASS |
| AC-R3-7: no path-qualified `std::fmt` | `persistence.rs:12` `use std::fmt;`, `impl fmt::Display for ContextId` | `rg` no match; fmt/lints by orchestrator | PASS |
| AC-R3-8: write set bounded | — | `git diff --stat 34700ca..HEAD` = the six code/lane files + task document; `review/TASK-002-r3/` files belong to the review-record commit `34700ca` | PASS |
| R3 constraints: no dependency, trait, client scheme check, key length cap, far-future fallback; `Serialize`/key layout/request policy unchanged | r3 fix diff adds only the `ApiBase` newtype, docs, one saturating call, one import, lane command | fix diff read in full | PASS |
| Ponytail ladder on changed abstractions | `ApiBase` newtype is the only new item; the alternatives (scheme check in client, struct-with-private-kind) are larger; `#[non_exhaustive]` retained is harmless and documented as permitted by the R3 task; `ApiBase` not re-exported from `lib.rs` — reachable via `pub mod identity`, callers never name it; not a defect | — | PASS |

## Proposed findings

None. Every path to an invalid `GitHubHost::Enterprise` from outside the crate that I could construct fails to compile with the intended error class, the lane and gate now execute the doctests, the overflow site is removed with a regression test, and the documentation/import obligations are met.

## Verification limits

- `mise run fmt:check`, `lints`, `test:pipeline`, and `gate` were run by the orchestrator, not re-run here; the lane definition was inspected and the doctest command reproduced directly.
- The cumulative 3277-line diff was reviewed by targeted reads of the r3 fix, `identity.rs` host block, `read_bounded`, `with_timeout`, manifests, and `mise.toml`; the `github.rs` request policy body was not re-read line by line in this round (it is unchanged since r2 per the fix diff, and its tests pass).
- No real-GitHub run (explicitly outside normal CI per the task).

## Overall

PASS

```citations
[]
```
