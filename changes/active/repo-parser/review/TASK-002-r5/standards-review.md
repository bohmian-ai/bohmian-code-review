# TASK-002 r5 — repository-standards review (repo-rev)

Candidate `6899a43` (base `1842eaf`), read from `/tmp/cr-r5/wt`. Scope: the code diff (`crates/bohmian-code-review-pipeline/**`, `Cargo.toml`, `Cargo.lock`, `mise.toml`) plus the TASK-002 task document. Only violations the diff introduced count.

Authorities read in full: `AGENTS.md`, `architecture/agent-rules.md`, `architecture/references/README.md`, `architecture/references/languages/rust-core.md`, `architecture/references/languages/testing-workflows.md`. I also read the pipeline sections of `architecture/bohmian-code-review-design.md` (lines 5, 22, 32) and the relevant REQs of `changes/active/repo-parser/spec.md` (REQ-015/016/017, INV-007) to confirm that the traits and dependencies are allowed.

## 1. Authority coverage

| Changed surface | Governing authorities |
|---|---|
| `src/identity.rs`, `src/storage.rs`, `src/persistence.rs` (validated values, keys, injected traits) | AGENTS.md:27, :29, :30, :31, :32; agent-rules.md:7, :10, :11; rust-core.md:3, :7; spec REQ-015, REQ-016, INV-007 |
| `src/github.rs` (client, transport seam, policy, wire parsing) | AGENTS.md:23, :27, :29, :30, :31, :32, :34; agent-rules.md:3, :10, :11; rust-core.md:3, :5, :7 |
| `src/execution.rs` | AGENTS.md:28, :30, :32; agent-rules.md:11; rust-core.md:7 |
| `src/lib.rs` | AGENTS.md:23, :32; design.md:22 |
| Inline `#[cfg(test)]` modules and doctests | agent-rules.md:5, :11, :12; testing-workflows.md:15; AGENTS.md:32, :40 |
| `crates/bohmian-code-review-pipeline/Cargo.toml`, workspace `Cargo.toml`, `Cargo.lock` | AGENTS.md:23, :27, :34; agent-rules.md:3 |
| `mise.toml` (`test:pipeline`, `test:rust`) | AGENTS.md:40; testing-workflows.md:15 |
| TASK-002 task document | AGENTS.md:44; testing-workflows.md:17-20 |

## 2. Rule results

| Rule | Result | Evidence |
|---|---|---|
| Facade-only dependency, no symbolic duplication (AGENTS.md:23; agent-rules.md:3) | PASS | `cargo tree -p bohmian-code-review-pipeline -e normal --depth 1` shows `bohmian-code-review`, serde, serde_json, sha1, sha2, thiserror, ureq. The only facade import is `RelativePath`/`InvalidRelativePath` (github.rs:14). |
| No provider SDK / DB / Node / PyO3 dependency; no unnecessary deps (AGENTS.md:27, :34) | PASS | `ureq` is a plain HTTP client with `default-features = false`, `rustls`, `platform-verifier`. `sha1` is needed for Git blob ids (github.rs:701). `--locked` builds succeed. |
| Cohesive concrete structs; traits only for multiple real impls or an intentional boundary (AGENTS.md:27; agent-rules.md:10; rust-core.md:3) | PASS | `GitHubClient` owns host, token, transport, limits, and counts. `ObjectStorage`/`Persistence` are required by the spec (REQ-015/016). `HttpTransport` has `UreqTransport` plus scripted fakes and is a documented injection boundary (github.rs:766); rust-core.md:3 allows an "intentional boundary". |
| No future-proofing (AGENTS.md:27) | **FAIL** | REPO-2 |
| Domain values at trust boundaries (AGENTS.md:29; rust-core.md:7) | PASS | Provider paths go through `RelativePath::new` (github.rs:1326, :1347). Shas are `ObjectSha` and ids are `TenantId`/`InstallationId`/`RepositoryIdentity`/`ContextId`/`ObjectKey`, all re-validated on deserialize. `extraction_version` is `&str` because the facade exports `EXTRACTION_VERSION` as text, and it is hex-encoded into the key (storage.rs:73). |
| `thiserror` library errors (AGENTS.md:30) | PASS | `Interrupted`, `InvalidIdentity`, `TransportError`, `GitHubError`, `StorageError`, `PersistenceError` all derive `Error`. |
| No unchecked `unwrap()`; `expect()` only for named invariants (AGENTS.md:30; rust-core.md:7) | PASS | Non-test code has no `unwrap()`, `expect()`, or `panic!`. It uses only `unwrap_or*` fallbacks (identity.rs:354, :385; github.rs:583, :713, :734, :967). `expect` appears only in `#[cfg(test)]` modules and one doctest. |
| Imports at module top; bare imported types in signatures (AGENTS.md:31; agent-rules.md:11) | **FAIL** | REPO-1. Every `use` is at module top (checked per file); signatures still carry qualified types. |
| Rustdoc on every new item, `# Errors`, `# Panics` (AGENTS.md:32; agent-rules.md:11; rust-core.md:7) | PASS | `cargo clippy -p bohmian-code-review-pipeline --lib` and `--tests` with `-W missing_docs -W clippy::missing_docs_in_private_items -W clippy::missing_errors_doc -W clippy::missing_panics_doc` report no item warnings, only struct fields, which are not items. `Execution::with_timeout` documents `# Panics` (execution.rs:59). |
| Synchronous, no runtime (AGENTS.md:28; rust-core.md:5) | PASS | No async or runtime. Sleeps are cancellable slices (github.rs:1218). |
| Inline owner tests; credential- and network-free lane (agent-rules.md:5; testing-workflows.md:15) | PASS | `scripts/check-test-placement.sh` passes. Tests drive `FakeTransport` and make no network calls. |
| No `#[allow]`/`#[ignore]`/weakened gate (agent-rules.md:12; AGENTS.md:40) | PASS | `rg '#\[allow|#\[ignore'` over the crate finds nothing. |
| Scoped mise lane wired into the gate (AGENTS.md:40; testing-workflows.md:15) | PASS | `test:pipeline` runs `--all-targets` and `--doc`, and `test:rust` depends on it (mise.toml:54-69). Ran at candidate: 17 unit tests and 2 doctests pass. `fmt:check` and `lints` (clippy `-D warnings` plus placement) pass. |
| No plan/task/agent references in production code (AGENTS.md:44) | PASS | `rg 'TASK-|REQ-|AC-|INV-|spec\.md'` over the crate source and manifest finds nothing. |

## 3. Findings

### REPO-1 — VIOLATION: qualified crate paths in new function signatures

- **Violated rule:** AGENTS.md:31 ("Keep imports at module top and use imported bare types in signatures"), repeated at agent-rules.md:11. No mise lane enforces it.
- **Root cause and defect class:** the candidate names external types by crate path inside the signatures of its new functions instead of importing them. It is one pattern, found in every manual `Deserialize` impl and in one private helper.
- **Locations (all siblings):**
  - identity.rs:94 (`TenantId`)
  - identity.rs:131 (`InstallationId`)
  - identity.rs:299 (`ObjectSha`)
  - persistence.rs:92 (`ContextId`)
  - storage.rs:118 (`ObjectKey`)

  Each of these declares `D: serde::Deserializer<'de>`. Separately, github.rs:1308 declares `json_error_category(error: &serde_json::Error)`.
- **Consequence:** these signatures break the written import convention the reviewer is told to enforce. Each module mixes an imported `Deserialize` with an unimported `serde::Deserializer`. Older crates (core `edge.rs`, `relative_path.rs`) have the same pattern, but that is pre-existing debt and does not waive the new sites.
- **Testable correction:**
  - In identity.rs, persistence.rs, and storage.rs, add `Deserializer` (and `de` for `de::Error::custom`) to the top-level `use serde::{...}`, and write `D: Deserializer<'de>`.
  - In github.rs, add `use serde_json::Error as JsonError;` (or an equivalent import) and write `error: &JsonError`.
  - Check: `rg -n 'fn [^(]*(<[^>]*)?\(.*\b(serde|serde_json|ureq)::' crates/bohmian-code-review-pipeline/src` and `rg -n 'D: serde::' crates/bohmian-code-review-pipeline/src` both return no matches. `mise run fmt:check`, `mise run lints`, and `mise run test:pipeline` stay green.

### REPO-2 — DRIFT: `#[non_exhaustive]` on `GitHubHost::Enterprise` is superseded and now only future-proofs

- **Violated rule:** AGENTS.md:27 ("Avoid … future-proofing, and unnecessary features"). This also falls under the round rule that superseded remediation code is DRIFT.
- **Root cause and defect class:**
  - r2 (commit `ebb3869`) added `#[non_exhaustive]` at identity.rs:328 to seal outside construction.
  - r3 then changed the payload to `ApiBase`, a newtype with a private field and no public constructor, `Default`, `From`, or `Deserialize` (identity.rs:339-340). That makes outside construction impossible on its own.
  - The attribute therefore no longer seals anything. Its only remaining effect is its native meaning: reserving the right to add fields, which forces every downstream `match` to write `..`.
- **Evidence:** in a scratch copy (`/tmp/cr-r5/scratch-repo-rev/ws`), deleting the attribute leaves `cargo test -p bohmian-code-review-pipeline --all-targets --all-features` at 17 passed and `--doc` at 2 passed. Both `compile_fail` doctests still pass. No outside code can obtain an unvalidated `ApiBase`, so no outside code can build an unvalidated `Enterprise`.
- **Locations:** identity.rs:328. The doctest at identity.rs:319 uses `..` only because of this attribute.
- **Consequence:** a redundant attribute left over from an earlier guard. It adds a pattern-matching burden on consumers and misleads maintainers about which mechanism holds the invariant.
- **Testable correction:** delete `#[non_exhaustive]` at identity.rs:328 and let the `ApiBase` newtype be the single seal. `mise run test:pipeline`, `fmt:check`, and `lints` must stay green.
- **Out-of-role note for the task and domain reviewers (not counted here):** the two `compile_fail` doctests (identity.rs:310, :317) fail to compile because a `String` is not an `ApiBase`, not because of the seal. In the scratch copy, making the field `pub struct ApiBase(pub String)` and removing `#[non_exhaustive]` still leaves both doctests passing. They do not guard what their rustdoc claims.

## 4. Overall

**FAIL** — REPO-1 and REPO-2.

```citations
[
  {"id": "REPO-1", "path": "AGENTS.md", "line": 31, "snippet": "- Keep imports at module top and use imported bare types in signatures.", "symbols": []},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 94, "snippet": "fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<TenantId, D::Error> {", "symbols": ["TenantId"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 131, "snippet": "fn deserialize<D: serde::Deserializer<'de>>(", "symbols": ["InstallationId"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 299, "snippet": "fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<ObjectSha, D::Error> {", "symbols": ["ObjectSha"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/persistence.rs", "line": 92, "snippet": "fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<ContextId, D::Error> {", "symbols": ["ContextId"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/storage.rs", "line": 118, "snippet": "fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<ObjectKey, D::Error> {", "symbols": ["ObjectKey"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1308, "snippet": "fn json_error_category(error: &serde_json::Error) -> &'static str {", "symbols": ["json_error_category"]},
  {"id": "REPO-2", "path": "AGENTS.md", "line": 27, "snippet": "- Keep stateful capabilities and workflows on cohesive concrete structs. Use free functions only for stateless transformations; use traits only for multiple real implementations. Avoid generic storage/provider interfaces, future-proofing, and unnecessary features or dependencies.", "symbols": []},
  {"id": "REPO-2", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 328, "snippet": "#[non_exhaustive]\nEnterprise {", "symbols": ["GitHubHost", "Enterprise"]},
  {"id": "REPO-2", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 339, "snippet": "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ApiBase(String);", "symbols": ["ApiBase"]}
]
```
