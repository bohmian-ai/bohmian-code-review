# TASK-002 r3 — repository-standards review (`repo-rev`)

Subject: base `1842eafdbc884d29f45fcee1c952ed3c89dd3e15` → candidate `dbd6de34f49764b32b00628cc539e719914469ed`.
Diff audited: `git diff 1842eaf..dbd6de3 -- crates Cargo.toml Cargo.lock mise.toml` (new crate `crates/bohmian-code-review-pipeline`, workspace manifests, lockfile, `test:pipeline` lane).
Authorities read: `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md` (pipeline paragraphs), `architecture/references/languages/{rust-core,testing-workflows,implementation-execution}.md`.
Not reviewed: task acceptance, prior review directories, optional improvements.
Read-only verification run here: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p bohmian-code-review-pipeline --all-features` (clean); two standalone `rustc -C overflow-checks=on` reproductions for REPO-1. `fmt:check`, `lints`, `test:pipeline`, `gate` are the orchestrator's.

## 1. Authority coverage

| Changed surface | Governing authorities |
|---|---|
| Crate exists above the facade; depends on `bohmian-code-review` only (`crates/bohmian-code-review-pipeline/Cargo.toml:12`, `src/github.rs:14`) | `AGENTS.md:23`; `architecture/agent-rules.md:3`; `architecture/bohmian-code-review-design.md:22` |
| No symbolic-stage re-implementation; no source persistence; no clone/checkout | `AGENTS.md:21,23`; `architecture/agent-rules.md:3,9`; `design.md:5,22,32` |
| New deps `ureq` (rustls + platform-verifier), `sha1`; lockfile growth | `AGENTS.md:27,34`; `implementation-execution.md:105-107` (dependency is a material decision owned by the spec) |
| Struct-owned workflow (`GitHubClient`, `Execution`), free helpers, `HttpTransport` / `ObjectStorage` / `Persistence` traits | `AGENTS.md:27`; `agent-rules.md:10`; `rust-core.md:3` |
| Synchronous IO, no runtime | `AGENTS.md:28`; `rust-core.md:5` |
| `thiserror` errors, no unchecked `unwrap`, `expect` only for invariants | `AGENTS.md:30`; `rust-core.md:7` |
| Imports at top, bare types in signatures | `AGENTS.md:31`; `agent-rules.md:11` |
| Rustdoc on every new item incl. private/tests; `# Errors`; `# Panics` | `AGENTS.md:32`; `agent-rules.md:11`; `rust-core.md:7` |
| Domain values at trust boundaries (`TenantId`, `ObjectSha`, `RelativePath`, `ObjectKey`) | `AGENTS.md:29`; `agent-rules.md:7`; `rust-core.md:7` |
| No `#[allow]`/`#[ignore]`/weakened assertions | `AGENTS.md:40`; `agent-rules.md:12`; `implementation-execution.md:224-232` |
| Inline tests, credential/network-free | `agent-rules.md:5`; `testing-workflows.md:15` |
| `mise.toml` `test:pipeline` + `test:rust` wiring | `AGENTS.md:40`; `testing-workflows.md:15`; `implementation-execution.md:177-179` |
| Grammar pins untouched | `AGENTS.md:33`; `agent-rules.md:6` |

## 2. Rule-by-rule result

| Rule | Result | Evidence |
|---|---|---|
| Pipeline crate consumes only the public facade (`AGENTS.md:23`, `agent-rules.md:3`) | PASS | `Cargo.toml:12` lists `bohmian-code-review` and no other workspace crate; the only workspace import is `src/github.rs:14` (`InvalidRelativePath`, `RelativePath`); tests use `bohmian_code_review::EXTRACTION_VERSION` (`storage.rs:251`, `persistence.rs:265`). |
| No duplicate extraction/index/resolve/score/pack (`AGENTS.md:23`) | PASS | No tree-sitter, posting, scoring, or packing code; the crate holds identities, transport, storage/persistence traits. |
| Never persists source; no clone/checkout (`agent-rules.md:9`, `design.md:5`) | PASS | `ContextRecord`/`SnapshotPointer` are key+digest+shas only (`persistence.rs:45-56,106-123`); `PullRequestFile.patch` is transient in-memory data (`github.rs:535`); no storage write of bytes exists yet. |
| Forbidden deps (`AGENTS.md:34`) | PASS | Lockfile adds `ureq`, `rustls*`, `sha1`, platform verifier transitive crates; no Git-provider SDK, model SDK, tokio, sqlite, postgres, pyo3, or `openssl` (only `openssl-probe`, a rustls-native-certs path lookup). |
| Unnecessary deps/features (`AGENTS.md:27`) | PASS | `ureq` with `default-features = false` plus only `rustls`, `platform-verifier`; `sha1` needed for Git blob ids (`github.rs:701-706`). |
| Concrete owners, traits only for multiple impls (`AGENTS.md:27`, `agent-rules.md:10`) | PASS | `HttpTransport` has `UreqTransport` + test fakes; `ObjectStorage`/`Persistence` are the spec-approved consumer seams (`AGENTS.md:23` "narrow storage/persistence traits a consumer implements"). |
| Synchronous, no runtime (`AGENTS.md:28`) | PASS | Blocking `ureq`, `thread::sleep` slices (`github.rs:1218-1227`). |
| `thiserror` errors (`AGENTS.md:30`) | PASS | `Interrupted`, `InvalidIdentity`, `StorageError`, `PersistenceError`, `TransportError`, `GitHubError` all `#[derive(Error)]`. |
| No unchecked `unwrap` on input/IO (`AGENTS.md:30`) | PASS | No `.unwrap()` in the crate; `unwrap_or` fallbacks only; `expect` appears only in tests with fixture invariants. |
| Domain values at trust boundary (`agent-rules.md:7`) | PASS | Every serde entry point re-validates (`identity.rs:87-98,124-136,190-203,292-303`; `storage.rs:107-122`; `persistence.rs:84-95`). |
| Imports at module top (`AGENTS.md:31`) | PASS | All `use` lines are at module or test-module top. |
| Bare imported types in signatures (`AGENTS.md:31`, `agent-rules.md:11`) | FAIL | `persistence.rs:97,99` path-qualifies `std::fmt::{Display,Formatter,Result}` while the sibling modules import `fmt` (`identity.rs:9`, `storage.rs:13`). See REPO-3. |
| Rustdoc on every new item incl. tests (`AGENTS.md:32`) | FAIL | Every `mod tests` in the crate is undocumented (`execution.rs:87`, `identity.rs:412`, `github.rs:1367`, `persistence.rs:209`, `storage.rs:184`); all other items, including private helpers and test helpers, are documented. See REPO-2. |
| `# Errors` on every fallible fn incl. `io::Result` and multi-line signatures (`AGENTS.md:32`) | PASS | Swept every `Result`-returning fn: `Execution::check`, all `new`/`parse`/`try_from`/`deserialize`, `GitHubHost::enterprise`, `validate_key_segment`, `validate_name`, all `GitHubClient` public and private methods (`get_json`, `request`, `send_with_policy`), `sleep_bounded`, `expect_status`, `read_bounded`, `parse_json`, `validate_tree_entry`, `validate_pull_request_file`, `BoundedReader::read` (`io::Result`, `github.rs:567-571`), trait methods on `HttpTransport`, `ObjectStorage`, `Persistence`, and the test fakes (`github.rs:1435-1437,1475-1477`). None missing. |
| `# Panics` where an item can panic on input (`AGENTS.md:32`, `rust-core.md:7`) | FAIL | Two caller-input arithmetic panics are neither documented nor removed: `execution.rs:59` and `github.rs:1273`. Reproduced. See REPO-1. |
| No `#[allow]`/`#[ignore]`/weakening (`agent-rules.md:12`) | PASS | No such attributes anywhere in the crate. |
| Tests inline; credential/network-free (`agent-rules.md:5`) | PASS | Only `src/` modules; all provider tests go through `FakeTransport`; `UreqTransport::new()` is only reached via `GitHubClient::new`, which no test calls. |
| mise lane wiring (`AGENTS.md:40`) | PASS | `test:pipeline` mirrors sibling lanes (`--locked --all-targets --all-features`, `mise.toml:54-56`) and is in `test:rust` → `gate` (`mise.toml:66,84`). |
| Grammar pins untouched (`AGENTS.md:33`) | PASS | No change under `crates/bohmian-code-review-extract` or the pin block. |
| Workspace manifests (`Cargo.toml`) | PASS | Member added, facade added to `[workspace.dependencies]`, new deps pinned at workspace level; crate uses `version/edition/rust-version/license/repository/publish.workspace` and `[lints] workspace = true`. |
| Rustdoc builds clean | PASS | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p bohmian-code-review-pipeline --all-features` finished without warnings. |

## 3. Material findings

### REPO-1 — Undocumented caller-input panics (`# Panics` missing)

- Rule: `AGENTS.md:32` ("document remaining panics"); `architecture/agent-rules.md:11` ("`# Panics` where relevant"); `architecture/references/languages/rust-core.md:7` ("possible panics are documented or removed").
- Defect class: rustdoc/panic contract on public input.
- Locations:
  1. `crates/bohmian-code-review-pipeline/src/execution.rs:59` — `Execution::with_timeout(timeout)` computes `Instant::now() + timeout`. `Instant: Add<Duration>` panics when the sum is unrepresentable; `Execution::with_timeout(Duration::MAX)` panics in every profile. Public API, no `# Panics`.
  2. `crates/bohmian-code-review-pipeline/src/github.rs:1273` — `read_bounded` computes `limit as u64 + 1`. `limit` is the caller-supplied `RequestLimits::max_json_bytes` / `max_blob_bytes` (`github.rs:934,1066,1104`); `usize::MAX` overflows `u64` on 64-bit targets and panics under `overflow-checks` (the profile `test:pipeline` runs in). `RequestLimits` is a public struct with public fields, so the value is caller input. No `# Panics` on `read_bounded`, `blob`, `pull_request_files`, or `get_json`.
- Root cause: unchecked arithmetic on caller-supplied bounds; the rustdoc describes only the intended path.
- Evidence: standalone reproduction with `rustc -C overflow-checks=on` — `black_box(usize::MAX) as u64 + 1` → `panicked at ... attempt to add with overflow`; `Instant::now() + Duration::MAX` → `panicked at library/std/src/time.rs:429:33`.
- Consequence: a consumer that expresses "no cap" as `usize::MAX` or "no deadline" as `Duration::MAX` crashes the worker instead of receiving a typed error; the API contract does not warn them.
- Testable correction: remove the panics rather than document them — `body.take((limit as u64).saturating_add(1))` at `github.rs:1273`; `Instant::now().checked_add(timeout).unwrap_or(<far-future>)` or an explicit `# Panics` on `with_timeout` at `execution.rs:57-60`. RED tests: `read_bounded(&mut Cursor::new(b""), usize::MAX, "r")` returns `Ok(vec![])`; `Execution::with_timeout(Duration::MAX).check()` returns `Ok(_)`.

### REPO-2 — Test modules lack rustdoc

- Rule: `AGENTS.md:32` ("Every new ... Rust item, including private items and tests, requires useful rustdoc"); `architecture/agent-rules.md:11`; `architecture/references/languages/rust-core.md:7`.
- Defect class: rustdoc omission on a new item (a `mod` is an item).
- Locations (all five test modules of the crate): `src/execution.rs:87`, `src/identity.rs:412`, `src/github.rs:1367`, `src/persistence.rs:209`, `src/storage.rs:184`.
- Root cause: only the `#[test]` fns and helpers were documented; the module wrapper was not.
- Evidence: each `mod tests {` is preceded only by `#[cfg(test)]`. Existing owners document the wrapper (`crates/bohmian-code-review-core/src/facts.rs:153`, `edge.rs:236`, `language.rs:306`, `limits.rs:7`, `diagnostic.rs:41`).
- Consequence: rule violation; inconsistent with every other crate in the workspace.
- Testable correction: add a one-line `///` intent doc above each `mod tests` (e.g. "Owner tests for bounded execution: cancellation precedence and deadline expiry."). Verify by `rg -B1 '^mod tests' crates/bohmian-code-review-pipeline/src` showing `///` on every preceding line.

### REPO-3 — Path-qualified types in signatures despite module-top imports

- Rule: `AGENTS.md:31` ("use imported bare types in signatures"); `architecture/agent-rules.md:11` ("use bare imported types in signatures").
- Defect class: style rule / import discipline.
- Primary location: `crates/bohmian-code-review-pipeline/src/persistence.rs:97` (`impl std::fmt::Display for ContextId`) and `:99` (`&mut std::fmt::Formatter<'_>) -> std::fmt::Result`). The sibling modules in the same crate import `fmt` (`identity.rs:9`, `storage.rs:13`) and write `fmt::Formatter<'_>` / `fmt::Result` (`identity.rs:287`, `storage.rs:126`), so this is an inconsistency introduced by the diff, not a workspace convention.
- Sibling locations of the same form: `serde::Deserializer<'de>` in `identity.rs:94,131,299`, `storage.rs:118`, `persistence.rs:91`; `serde::de::Error::custom` at `identity.rs:96,134,301`, `storage.rs:120`, `persistence.rs:93`. These mirror pre-existing `bohmian-code-review-core` code (`edge.rs:167`, `relative_path.rs:96`), so they are recorded for completeness only; the correction is mechanical if applied.
- Consequence: minor; no behavioral effect.
- Testable correction: `use std::fmt;` in `persistence.rs` and write `impl fmt::Display for ContextId` / `fmt::Formatter<'_>` / `fmt::Result`. Verify with `rg 'std::fmt::' crates/bohmian-code-review-pipeline/src` returning nothing and `mise run fmt:check lints`.

## Notes (not findings)

- Private struct fields (`Execution`, `GitHubClient`, `BoundedReader`, `RepositoryIdentity`, wire structs, `SeenRequest`) carry no field docs; fields are not Rust items, so `AGENTS.md:32` does not literally require them, though `bohmian-code-review-index` documents its private fields. Reported for the orchestrator's judgement; not counted.
- `testing-workflows.md:15` enumerates the scoped mise tasks without `test:pipeline`; the reference is descriptive and outside the audited diff paths, so not counted.
- `mode` index in `CallCounts::record`/`get` (`github.rs:244,249`) cannot panic: `Operation` has exactly eight variants and the array is `[AtomicU64; 8]`.

## 4. Overall

**FAIL** — three rule violations, all mechanically correctable: REPO-1 (undocumented, reproducible input-driven panics; the one with runtime consequence), REPO-2 (undocumented `mod tests` × 5), REPO-3 (path-qualified `std::fmt` signature types in `persistence.rs`). Crate boundary, dependency policy, error typing, `# Errors` coverage, test placement, and mise wiring all pass.

```citations
[
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/execution.rs", "line": 59, "snippet": "Execution::new(Instant::now() + timeout, CancelToken::new())", "symbols": ["with_timeout", "Execution", "CancelToken"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1273, "snippet": "body.take(limit as u64 + 1)", "symbols": ["read_bounded", "RequestLimits", "max_json_bytes", "max_blob_bytes"]},
  {"id": "REPO-2", "path": "crates/bohmian-code-review-pipeline/src/execution.rs", "line": 87, "snippet": "mod tests {", "symbols": ["tests"]},
  {"id": "REPO-2", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 412, "snippet": "mod tests {", "symbols": ["tests"]},
  {"id": "REPO-2", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1367, "snippet": "mod tests {", "symbols": ["tests"]},
  {"id": "REPO-2", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 209, "snippet": "mod tests {", "symbols": ["tests"]},
  {"id": "REPO-2", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 184, "snippet": "mod tests {", "symbols": ["tests"]},
  {"id": "REPO-3", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 97, "snippet": "impl std::fmt::Display for ContextId {", "symbols": ["ContextId", "Display"]},
  {"id": "REPO-3", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 99, "snippet": "fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {", "symbols": ["fmt", "Formatter"]}
]
```
