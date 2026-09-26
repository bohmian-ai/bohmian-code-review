---
id: TASK-002-R3
kind: remediation
status: ready
spec: SPEC-repo-parser@10
remediates: TASK-002
findings: [FIND-TASK-002-4, FIND-TASK-002-5, FIND-TASK-002-6, FIND-TASK-002-7, FIND-TASK-002-8]
---

## Subject

- Approved spec: `changes/active/repo-parser/spec.md` (revision 10)
- Original task: `changes/active/repo-parser/tasks/TASK-002-github-source-and-stage-contracts.md`
- Prior remediations: `../TASK-002-r1/TASK-002-R1-validated-seam-values.md`, `../TASK-002-r2/TASK-002-R2-seam-round-trip-and-host-invariant.md`
- Reviewed candidate: `dbd6de34f49764b32b00628cc539e719914469ed` (base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15`)
- Review: `changes/active/repo-parser/review/TASK-002-r3/verdict.md`; validated ledger `findings-validation.md`

## Issue diagnosis — FIND-TASK-002-4, CLASS_INCOMPLETE (host payload still externally writable)

- **Root cause:** r2 sealed *construction* of `GitHubHost::Enterprise` with `#[non_exhaustive]` (`crates/bohmian-code-review-pipeline/src/identity.rs:318`), but enum variant fields are inherently public. A crate holding `&mut GitHubHost` can bind `GitHubHost::Enterprise { api_base, .. }` and assign any `String`. The r2 task rejected "a newtype for the payload" as more surface; that rejection is what left the write path open. The first owner of the wrong decision is the variant's payload type at `identity.rs:318-322`.
- **Defect class:** validated domain value with a public writable field (second admission path not closed).
- **Sibling locations (one finding):** `identity.rs:318-322` (variant payload, owner); `identity.rs:353-358` (`api_base` returns the field verbatim); `identity.rs:361-369` (`archive_host` degrades to `""` for a non-`https://` base); consumers `github.rs:1121` (`request` builds `"{api_base}{resource}"`), `github.rs:1139` (`build` attaches `Authorization: Bearer …`), `github.rs:741-746` (`is_trusted_redirect` compares against `archive_host()`); doctest `identity.rs:310-312` (proves construction only).
- **Violated obligation:** the module's own invariant `identity.rs:307-308` ("`GitHubHost::enterprise` is the only way to obtain an `Enterprise` host, so every API base a client sees is validated"); spec REQ-014 `spec.md:77` (credentials never forwarded/exposed); REQ-004 `spec.md:61`; `AGENTS.md:29` (domain values at trust boundaries); AC-R2-5 intent.
- **Current behavior / reproduction** (external scratch crate with a path dependency on the candidate crate; reproduced independently by the source-trust reviewer at `/tmp/ghost_probe` and by Wave 2 at `/tmp/t002r3`):
  ```rust
  let mut h = GitHubHost::enterprise("https://ghe.example.com/api/v3").unwrap();
  if let GitHubHost::Enterprise { api_base, .. } = &mut h { *api_base = "http://evil.example".to_owned(); }
  assert_eq!(h.api_base(), "http://evil.example");   // passes
  assert_eq!(h.archive_host(), "");                   // passes
  ```
  Struct-expression construction fails (E0639) and exhaustive matching fails (E0638), so the existing `compile_fail` doctest passes while the mutation path compiles.
- **Observable consequence:** `GitHubClient::new(forged_host, token, limits).repository(..)` sends `GET http://evil.example/repos/o/r` with the installation token in cleartext; `archive_host()` becomes `""`, so every archive redirect is then refused (fail-closed there, but the token was already forwarded on the API call).
- **Why the candidate falls short:** `#[non_exhaustive]` addresses construction, not mutation; no test exercises the write path.

## Issue diagnosis — FIND-TASK-002-5, CONFIRMED (the AC-R2-5 proof runs under no lane)

- **Root cause:** `mise.toml:56` (`test:pipeline`) runs `cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features`. Cargo's `--all-targets` expands to lib/bins/tests/benches/examples and excludes the doctest target. The `compile_fail` doctest at `identity.rs:310-312` is the crate's only doctest and the only executable proof that `Enterprise` cannot be constructed externally.
- **Defect class:** verification target present in cargo but absent from every repository lane.
- **Sibling locations:** `mise.toml:56` (owner); inherited by `test:rust` (`mise.toml:58-67`) and `gate` (`:82-84`). `grep -n -- '--doc' mise.toml` → no match. Other crate lanes share the flag but own no doctests, so they are not siblings.
- **Violated obligation:** `AGENTS.md:40` ("An absent test target is a failing prerequisite, not a skipped pass"); `architecture/references/languages/testing-workflows.md:15` ("A missing owner target fails rather than skipping"); AC-R2-5, whose evidence in the task document cites a raw `cargo test --doc` command outside every lane.
- **Reproduction:** `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` → `16 passed`, no `Doc-tests` section; without `--all-targets` (or with `--doc`) → `Doc-tests bohmian_code_review_pipeline … identity::GitHubHost (line 310) - compile fail ... ok`.
- **Observable consequence:** removing `#[non_exhaustive]` (or, after this remediation, reopening the payload) leaves `test:pipeline`, `test:rust`, and `gate` green. The construction seal has no regression guard in CI.

## Issue diagnosis — FIND-TASK-002-6, CONFIRMED (undocumented caller-input panics)

- **Root cause:** unchecked arithmetic on caller-supplied bounds with rustdoc describing only the intended path.
- **Defect class:** undocumented panic on public input.
- **Sibling locations (one finding):** `execution.rs:58-60` — public `Execution::with_timeout` computes `Instant::now() + timeout`; `Instant: Add<Duration>` panics for `Duration::MAX` in every profile. `github.rs:1271-1284` — private `read_bounded` computes `limit as u64 + 1`; `limit` is caller-supplied through public `RequestLimits { max_json_bytes, max_blob_bytes }` (`github.rs:181-183`) via `:934`, `:1066`, `:1104`; `usize::MAX` panics under overflow-checks (the `test:pipeline` profile) and wraps to `take(0)` in release, returning `Ok(empty)` for a nonempty body.
- **Violated obligation:** `AGENTS.md:32` ("document remaining panics"); `architecture/references/languages/rust-core.md:7` ("possible panics are documented or removed"); `architecture/agent-rules.md:11` (`# Panics` where relevant).
- **Reproduction:** Wave 2 scratch crate: `Execution::with_timeout(Duration::MAX)` → `should_panic … ok`; `black_box(usize::MAX) as u64 + 1` under `overflow-checks` → `should_panic … ok`.
- **Observable consequence:** a consumer expressing "no cap" as `usize::MAX` or "no deadline" as `Duration::MAX` crashes the worker, or in release silently reads zero bytes; the API contract does not warn them.

## Issue diagnosis — FIND-TASK-002-7, CONFIRMED (test modules lack rustdoc)

- **Root cause:** only the `#[test]` fns and helpers were documented; the `mod tests` wrapper, itself a Rust item, was not.
- **Defect class:** rustdoc omission on a new item.
- **Sibling locations:** `execution.rs:87`, `identity.rs:412`, `github.rs:1367`, `persistence.rs:209`, `storage.rs:184`.
- **Violated obligation:** `AGENTS.md:32` ("Every new … Rust item, including private items and tests, requires useful rustdoc"). Every core/extract test module carries one (`crates/bohmian-code-review-core/src/facts.rs:153`, `edge.rs:236`, `language.rs:306`, `diagnostic.rs:41`, `position.rs:47`; `crates/bohmian-code-review-extract/src/buffers.rs:581`).
- **Evidence:** `grep -B1 '^mod tests' crates/bohmian-code-review-pipeline/src/*.rs` shows only `#[cfg(test)]` above each.

## Issue diagnosis — FIND-TASK-002-8, CONFIRMED (path-qualified `std::fmt` in signatures)

- **Root cause:** `persistence.rs` never imports `std::fmt`, so the `Display` impl spells `std::fmt::Display`, `std::fmt::Formatter<'_>`, and `std::fmt::Result` inline (`persistence.rs:97-99`), while sibling modules import `fmt` (`storage.rs:13`, `identity.rs:9`) and write `fmt::Formatter<'_>` / `fmt::Result` (`storage.rs:126-129`, `identity.rs:287`).
- **Defect class:** import discipline.
- **Sibling locations:** `persistence.rs:97`, `:99` only. The path-qualified `serde::Deserializer` / `serde::de::Error` sites mirror existing core code (`core/src/edge.rs`, `relative_path.rs`) and are excluded.
- **Violated obligation:** `AGENTS.md:31` ("Keep imports at module top and use imported bare types in signatures"); `architecture/agent-rules.md:11`.

## Intended correction outcome

`GitHubHost::Enterprise`'s API base cannot be constructed or overwritten from outside the crate; `enterprise()`, `api_base()`, and `archive_host()` keep their names and behaviour; the seal is proved by doctests that the owning lane and `gate` actually execute. No new fallible or panicking behaviour is undocumented; `read_bounded` cannot overflow; every `mod tests` and the `persistence.rs` `Display` impl follow the written rustdoc and import rules. Provider client behaviour, request policy, key layout, `Serialize` output, and trait method sets are unchanged.

## Decision-complete recommendation

**FIND-TASK-002-4** — owner `identity.rs`. Apply the crate's existing validated private-field newtype pattern (as `TenantId`, `ObjectSha`, `InstallationToken`, `ContextId` already do) to the `Enterprise` payload so the field type has no public constructor, no `From<String>`, and no public field; `enterprise()` remains the only producer and `api_base()` / `archive_host()` read the inner text exactly as today. Only four in-crate sites name `Enterprise` (`identity.rs:319, :347, :356, :364`) and no test pattern-matches the variant, so `hosts_expose_api_base_and_archive_host` and the existing `compile_fail` doctest survive unchanged. `#[non_exhaustive]` becomes redundant once the payload is unconstructible; keep or drop it, but the doctest must still fail to compile. Add a second `compile_fail` doctest containing the `&mut` destructure-and-assign from the reproduction. Why this closes the class: the guarantee moves from the enum (which cannot hide variant fields) to a type that can, which is the same mechanism every other identity value in the module uses. Rejected alternatives: scheme checks inside `GitHubClient::request`/`build` (guards a symptom in consumers; the invariant belongs to the value); turning `GitHubHost` into a struct with a private kind (more surface for the same guarantee). This crate has no external consumer yet, so the payload type change is not a public-API decision; if the change review disagrees, escalate `SPEC_REVISION_REQUIRED` rather than keep the open write path.

**FIND-TASK-002-5** — owner `mise.toml` `test:pipeline`. Reuse the list-form `run = [...]` already used elsewhere in `mise.toml` and add `cargo test --locked -p bohmian-code-review-pipeline --doc --all-features` as a second command. Keep `--all-targets` on the first command so the lane matches its siblings. `test:rust` and `gate` inherit the fix. Rejected alternative: dropping `--all-targets` (would also run doctests today, but diverges from sibling lanes and silently drops benches/examples if ever added).

**FIND-TASK-002-6** — `read_bounded` (`github.rs:1273`): use stdlib `(limit as u64).saturating_add(1)`; this removes both the debug panic and the release wrap without changing any in-range behaviour. `with_timeout` (`execution.rs:58-60`): there is no representable "no deadline" `Instant`, so document a `# Panics` section stating that `Instant::now() + timeout` panics when the sum is unrepresentable (e.g. `Duration::MAX`); the rule permits documented panics and inventing a far-future fallback is a semantics decision the task does not require.

**FIND-TASK-002-7** — one rustdoc line above each of the five `mod tests`, in the wording pattern the core crates use (owner tests for the module's behaviour).

**FIND-TASK-002-8** — `use std::fmt;` at the top of `persistence.rs` and spell the `Display` impl as `storage.rs:126-129` does.

## Constraints, preserved behavior, and non-goals

- Do not change `enterprise()` validation rules, `api_base()`/`archive_host()` return values, `GitHubClient` behaviour, request policy, key layout, `Serialize` output, hex encoding, or trait method sets.
- Do not add a dependency, a trait, a scheme check in the client, a whole-key length cap, or a far-future deadline fallback.
- Do not address TASK-A-1 (scope binding of reloaded keys); that is a facade-lookup obligation for TASK-003..005 recorded as a note.
- Do not add facade code, an OpenDAL adapter, conformance implementations, a concurrency limiter, or a real-GitHub run. Do not alter the six symbolic crates, grammar pins, or fixtures.
- Keep tests inline in the owning module; record the expected RED failure for each new or extended test per `architecture/references/languages/spec-driven-development.md`.

## Acceptance criteria

- **AC-R3-1 (FIND-TASK-002-4, `identity.rs:318-322`):** a `compile_fail` doctest on `GitHubHost` containing `if let bohmian_code_review_pipeline::GitHubHost::Enterprise { api_base, .. } = &mut h { *api_base = "http://evil".to_owned(); }` (or the equivalent for the chosen payload shape, assigning from a `String`) fails to compile from outside the crate; the existing construction `compile_fail` doctest still fails to compile.
- **AC-R3-2 (FIND-TASK-002-4, `identity.rs:353-369`, `github.rs:1121`, `:1139`, `:741-746`):** `identity::tests::hosts_expose_api_base_and_archive_host` and `github::tests::archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials` pass unchanged; `GitHubHost::enterprise("http://ghe.example.com")` remains `Err(InvalidIdentity::ApiBase)`.
- **AC-R3-3 (FIND-TASK-002-5, `mise.toml:56`):** `mise run test:pipeline` output contains `Doc-tests bohmian_code_review_pipeline` with both `compile fail ... ok` lines; `mise run gate` runs them.
- **AC-R3-4 (FIND-TASK-002-6, `github.rs:1273`):** `rg 'as u64 \+ 1' crates/bohmian-code-review-pipeline/src` → no match; an inline test feeding `read_bounded` a limit of `usize::MAX` over a small cursor returns the bytes (RED at the candidate under `overflow-checks`).
- **AC-R3-5 (FIND-TASK-002-6, `execution.rs:58-60`):** `with_timeout` rustdoc has a `# Panics` section; `cargo doc --no-deps -p bohmian-code-review-pipeline` emits no pipeline-crate warning.
- **AC-R3-6 (FIND-TASK-002-7):** `grep -B1 '^mod tests' crates/bohmian-code-review-pipeline/src/*.rs` shows a `///` line above each of the five modules.
- **AC-R3-7 (FIND-TASK-002-8):** `rg 'std::fmt::' crates/bohmian-code-review-pipeline/src` → no match; `mise run fmt:check` and `mise run lints` exit 0.
- **AC-R3-8:** `git diff --stat dbd6de3..HEAD` touches only `identity.rs`, `execution.rs`, `github.rs`, `persistence.rs`, `storage.rs`, `mise.toml`, and the task document's evidence section.

## Focused proof and broader verification

- Focused: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --doc` (AC-R3-1); `... --lib identity::tests::hosts_expose_api_base_and_archive_host -- --exact` and `... --lib github::tests::archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials -- --exact` (AC-R3-2); `mise run test:pipeline 2>&1 | grep -A3 Doc-tests` (AC-R3-3); the new `read_bounded` test (AC-R3-4); `cargo doc --no-deps -p bohmian-code-review-pipeline` (AC-R3-5); the two `grep`/`rg` checks (AC-R3-6, AC-R3-7).
- External-crate proof for AC-R3-1 beyond the doctest is optional; the doctest compiles from outside the crate by construction.
- Broader: `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, `mise run gate`; `git diff --stat` per AC-R3-8.
- Record observed results, including the expected RED for each new or extended test, in the task document's evidence section per `architecture/references/languages/implementation-execution.md`.

Route to `$cr-implement`. A later review reassesses the complete cumulative candidate against the original TASK-002.
