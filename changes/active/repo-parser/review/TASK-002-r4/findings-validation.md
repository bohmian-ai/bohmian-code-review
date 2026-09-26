# TASK-002 r4 — Wave 2 findings validation (ponytail-rev)

## Subject

- Base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`, candidate `801d9572cf6b0364b5ab33e858388d86f54cb91e` (`git rev-parse HEAD` → matches).
- Diffs: `/tmp/task002-r4-code.diff` (cumulative), `/tmp/task002-r4-r3fix.diff` (commits `1958e0b`, `801d957`).
- Input ledger: `claim-ledger.md` (zero citation-checked findings). Wave 1 reports were not read; conclusions below are from source and commands.

## Method and commands (observed results)

| Command | Result |
|---|---|
| `git rev-parse HEAD` | `801d9572…` — subject verified |
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --doc` | `Doc-tests bohmian_code_review_pipeline`: 2 passed; `identity::GitHubHost (line 310) - compile fail ... ok`, `(line 317) - compile fail ... ok`; exit 0 |
| `mise run test:pipeline` | lib: 17 passed; Doc-tests section present with both compile-fail ok lines; exit 0 |
| Scratch crate `/tmp/scratch-apibase` (depends on the pipeline crate by path) compiling both doctest bodies plus `ApiBase(..)`, `ApiBase::default()`, `String.into()` | `E0639` (non-exhaustive variant struct expression) for the construction doctest; `E0308` mismatched types for `*api_base = String` (the `&mut` doctest); `E0603` tuple struct constructor `ApiBase` is private; `E0599` no `default`; `E0277` no `From<String>`. Both doctests fail for the intended reason, not an unrelated one. |
| `rg -n 'as u64 \+ 1\|std::fmt::' crates/bohmian-code-review-pipeline/src` | no match (exit 1) — AC-R3-4, AC-R3-7 |
| `grep -rn -A1 '#\[cfg(test)\]' crates/bohmian-code-review-pipeline/src` | all five `mod tests` (identity, execution, persistence, storage, github) carry a `///` line — AC-R3-6 |
| `grep -n 'use std::fmt' persistence.rs` | line 12 `use std::fmt;` at module top — FIND-8 |
| `grep -n 'for ApiBase\|impl ApiBase' identity.rs` | no match — no `From`/`Default`/`Deserialize`/inherent constructor or `&mut` accessor; derives are `Debug, Clone, PartialEq, Eq` only (identity.rs:339) |
| Trace of `GitHubHost::Enterprise` payload consumers | Only reads: `api_base()` (identity.rs:373 `&api_base.0`), `archive_host()` (identity.rs:381 `api_base.0.strip_prefix("https://")`), `github.rs:1121` URL build via `self.host.api_base()`, `github.rs:1139` bearer attach on that URL, `github.rs:968` → `is_trusted_redirect(&location, self.host.archive_host())`. Sole writer is `GitHubHost::enterprise` (identity.rs:350-366), which rejects non-`https://` (`InvalidIdentity::ApiBase`). No other crate-internal site constructs `ApiBase`. |
| `git diff --name-only 34700ca..HEAD` (the two fix commits) | only `identity.rs`, `execution.rs`, `github.rs`, `persistence.rs`, `storage.rs`, `mise.toml`, and the TASK-002 task doc — AC-R3-8 satisfied for the fix commits (the wider `dbd6de3..HEAD` range additionally contains the r3 review artifacts committed by the r3 review itself in `34700ca`, which is not implementation scope). |
| Scratch crate `/tmp/scratch-bb`: `limit as u64 + 1` called with `usize::MAX` with and without `std::hint::black_box` under the default test profile | both panic at runtime with `attempt to add with overflow`; neither is rejected at compile time. `black_box` is not needed for the RED behaviour. |

## Validated blocking ledger

**Validated empty.** No claim meets gates (1)-(4). My own source reading of the R3 change and its sibling paths produced no reproducible defect.

| ID | Sources | Status | Gate | Root cause | Correction owner |
|---|---|---|---|---|---|
| — | — | — | — | — | — |

## Prior-finding closure (independent, from candidate source)

| ID | Status at r4 | Evidence / remaining siblings |
|---|---|---|
| FIND-TASK-002-1 | CLOSED (carried from r3) | not re-examined; no R3 change touches key validation |
| FIND-TASK-002-2 | CLOSED (carried from r3) | not re-examined |
| FIND-TASK-002-3 | CLOSED (carried from r3) | `ObjectKind` absent from the pipeline crate |
| FIND-TASK-002-4 | CLOSED | `ApiBase(String)` private field, no public constructor/`From`/`Default`/`Deserialize`/`&mut` accessor; both compile_fail doctests fail with the intended codes (E0639, E0308); all consumers (`api_base()`, `archive_host()`, `github.rs:1121`, `:1139`, `:741-746` via `:968`) read only through `enterprise()`-validated text. No remaining siblings. |
| FIND-TASK-002-5 | CLOSED | `mise.toml` `test:pipeline` list form runs `--all-targets` then `--doc`; observed Doc-tests section with both compile-fail ok; exit 0 |
| FIND-TASK-002-6 | CLOSED | `read_bounded` uses `(limit as u64).saturating_add(1)` (github.rs:1273); `read_bounded_accepts_the_maximum_limit` exercises `usize::MAX`; `with_timeout` has an accurate `# Panics` (`Instant::now() + timeout` unrepresentable). No remaining siblings. |
| FIND-TASK-002-7 | CLOSED | five `mod tests` documented |
| FIND-TASK-002-8 | CLOSED | `use std::fmt;` at persistence.rs:12; no `std::fmt::` path in crate |

## Notes (non-blocking)

- N-1 (repo-rev, black_box): `std::hint::black_box` in `read_bounded_accepts_the_maximum_limit` is unnecessary — reproduced that the plain call panics identically at RED (`/tmp/scratch-bb`). Dropping it also removes the path-qualified call. Cosmetic; no written rule violated (the imports rule concerns signatures).
- N-2 (Ponytail): with `ApiBase`'s private field sealing construction, `#[non_exhaustive]` on `GitHubHost::Enterprise` is now redundant for the seal (E0603 would block construction on its own). Keeping it is harmless and still documents field-addition freedom; deleting it is the smaller surface. Either is acceptable.
- N-3 (source-trust O-2): `ApiBase` is not re-exported from `lib.rs`. Fine as-is: no caller needs to name it (only `GitHubHost::enterprise` produces it, `api_base()` returns `&str`). Re-export only when a consumer needs the type in a signature.
- N-4 (source-trust O-1): `UntrustedRedirect { location }` echoes the raw `Location` header into error text. Diagnostic value outweighs risk; no credential is present in that header. Note.
- N-5 (source-trust O-3): `ObjectDigest` derives plain `Deserialize` and fails closed at the integrity check rather than at load. Behaviourally safe; tightening to validate-on-deserialize is optional consistency with `ObjectSha`/`ContextId`.
- N-6 (provider-bounds O-1): `sleep_bounded` slices by `min(left, 100ms)` and re-checks the deadline between slices; overshoot is bounded by one slice. Within the documented bounded-policy contract.
- N-7 (provider-bounds O-2): `max_attempts: 0` behaves as one attempt (`attempt >= max_attempts` checked after the first try). Doc wording could say "at least one attempt". Note.
- N-8 (provider-bounds O-3): archive stream reads do not observe `CancelToken`; consumer-owned per spec. Note.
- N-9 (repo-rev): private struct fields lacking `///` — fields are not items under the AGENTS.md rustdoc rule as written. Not a violation.

## Rejected

- None. No Wave 1 observation was presented as a claim, and none meets a blocking gate; all are classified as notes above.

## Ponytail ladder assessment (R3 changes)

- `ApiBase` newtype: minimal safe seal. Alternatives ranked: (a) deleting `#[non_exhaustive]` alone would not stop `&mut` rewrites — insufficient; (b) converting `Enterprise` to a struct-carrying or tuple variant still exposes the payload for `&mut` unless the payload itself is sealed — same newtype needed; (c) the newtype with a private field is the one-line-of-type solution and every consumer stays a `&str` read. Rung 7, minimum code. Correct choice.
- `test:pipeline` list form: minimal. `--all-targets` excludes doctests by cargo design, so a second invocation is the smallest way to include them without dropping `--all-targets`. Alternative `cargo test -p … --all-features` (no `--all-targets`) would run lib+doc in one command but drop benches/examples; none exist today, so either is defensible. Note only.
- `black_box`: not needed (N-1).
- `saturating_add` and the `# Panics` section: exact, minimal.

## Verification limits

- FIND-1..3 closure carried from r3 without re-derivation (out of the assigned 4..8 scope, and no R3 change touches their code).
- `mise run gate`, `fmt:check`, `lints`, and `cargo doc --no-deps` were not re-run within the 12-minute budget; `mise run test:pipeline` and the doctest lane were run directly and passed.
- Scratch crates compiled with `--offline` against the path dependency; no network.

## Recommendation

**PASS.** Validated empty blocking ledger; FIND-TASK-002-4..8 independently confirmed closed with no remaining siblings.
