# TASK-002 review r1 — repository standards (repo-rev)

Subject: base `1842eaf` → candidate `dafe20b` (== HEAD, tree clean). Read-only. Diff: `git diff 1842eaf..dafe20b` (excl. Cargo.lock, /tmp/task002.diff).

Lanes run (observed): `mise run fmt:check` exit 0; `mise run lints` exit 0 (clippy `--locked --workspace --all-targets --all-features -D warnings` + `scripts/check-test-placement.sh`); `mise run test:pipeline` exit 0 (12 passed, 0 failed, 0 ignored); `git diff --check 1842eaf..dafe20b` exit 0; `git status --porcelain` empty; `cargo tree -p bohmian-code-review-pipeline --depth 1` = bohmian-code-review, serde, serde_json, sha1, sha2, thiserror, ureq. `mise run gate` not run (per subject).

Not covered: Cargo.lock reviewed only for source kinds (0 `git+` sources) and added package names; no per-package license/advisory audit (no repository rule requires one).

## 1. Authority coverage

| Changed surface | Governing authorities (file:line/section) |
|---|---|
| `Cargo.toml` (member + `bohmian-code-review`, `sha1`, `ureq` workspace deps), `Cargo.lock` | AGENTS.md:34 (prohibited deps); AGENTS.md:23 (outer crate depends on facade only); architecture/agent-rules.md:3, :6 (pinned deps); architecture/bohmian-code-review-design.md:22; references/languages/implementation-execution.md:99-109 (dependency = material; authorized here by TASK-002 line 52 "workspace manifest/lockfile" and spec REQ-004 spec.md:61), :265-276 (diff audit) |
| `crates/bohmian-code-review-pipeline/Cargo.toml` | AGENTS.md:23; design.md:22; agent-rules.md:3; workspace `[lints]` inheritance (Cargo.toml `[workspace.lints.rust] unsafe_code = "deny"`) |
| `src/lib.rs` | AGENTS.md:23, :32 (rustdoc), :31 (imports); agent-rules.md:3, :11; design.md:22 (no server/worker) |
| `src/identity.rs` | AGENTS.md:27-32; agent-rules.md:7 (validate at ingress), :10, :11; rust-core.md:3, :7 |
| `src/execution.rs` | AGENTS.md:27-28, :30-32; rust-core.md:5 (no runtime, caller-bounded); spec REQ-013 spec.md:76 |
| `src/github.rs` | AGENTS.md:23, :27-32, :34; agent-rules.md:5 (inline tests, credential-/network-free), :9 (no source retention), :10, :11; rust-core.md:3-7; testing-workflows.md:15; design.md:22; spec REQ-004 spec.md:61, REQ-013 :76, REQ-014 :77, INV-001 :85 |
| `src/storage.rs`, `src/persistence.rs` | AGENTS.md:27 (traits only for multiple real impls) vs spec REQ-015 spec.md:78 / REQ-016 spec.md:79 / INV-007 :91 and design.md:22 ("narrow storage/persistence traits a consumer implements"); AGENTS.md:30-32; agent-rules.md:9, :10, :11 |
| `mise.toml` `test:pipeline` + `test:rust` wiring | AGENTS.md:40; testing-workflows.md:15, :17-20; implementation-execution.md:162-179, :237-259; existing lane shape mise.toml:38-52 |
| Task document evidence section | .agents/skills/cr-implement/SKILL.md:56-75 (evidence table, commands, non-goals); spec-driven-development.md:177-223 (focused commands, RED/GREEN); testing-workflows.md:17-20; implementation-execution.md:114-137, :265-278; agent-rules.md:14 |
| Commits `1696545`, `dafe20b` | AGENTS.md:46; agent-rules.md:15 (identity, no AI trailers) |

## 2. Rule results

| Rule (citation) | Result | Evidence |
|---|---|---|
| Rustdoc on every new item incl. private and tests (AGENTS.md:32; agent-rules.md:11; rust-core.md:7) | PASS (items) | Every struct/enum/fn/trait/impl-method/test/type alias in all five modules carries a doc line; wire structs, `BoundedReader`, `FakeTransport`, `FakeRef`, `Scripted`, test helpers all documented. |
| `# Errors` on fallible functions (AGENTS.md:32 "Fallible functions need `# Errors`"; rust-core.md:7; agent-rules.md:11) | **FAIL** — REPO-1 | 11 new private fallible functions lack `# Errors` (list in REPO-1). All `pub` fallible methods and both injected traits have `# Errors`. Existing crates apply the section to private fallible fns (e.g. `crates/bohmian-code-review-select/src/diff.rs:185` `fn start`); scan of core/index/resolve/select/facade found zero private fallible fns without it. |
| `# Panics` documented or removed (agent-rules.md:11; rust-core.md:7) | PASS | Non-test code has no `panic!`/`unwrap()`/`expect()`; `CallCounts::record` indexes a fixed 8-array by an 8-variant enum (named invariant `Operation::ALL`, github.rs:219-233). Test-only `expect("scripted response for every request")` is documented as "an exhausted script is a test bug" (github.rs:1360). |
| No unchecked `unwrap` on input/parsing/IO; `expect` only for named invariants (AGENTS.md:30; rust-core.md:7) | PASS | Only `unwrap_or(...)` fallbacks (identity.rs:286, :314; github.rs:571, :701, :722, :955); `expect` confined to `#[cfg(test)]`. |
| `thiserror` for library errors (AGENTS.md:30; rust-core.md:7) | PASS | `InvalidIdentity`, `Interrupted`, `TransportError`, `GitHubError`, `StorageError`, `PersistenceError` all `#[derive(Error)]`. |
| Imports at module top; bare imported types in signatures (AGENTS.md:31; agent-rules.md:11) | PASS | All `use` at module/test-module top. Qualified `serde::Deserializer`, `serde_json::Error`, `ureq::Agent`, `std::fmt::Display` (persistence.rs:70-72) match the established pattern in core (`relative_path.rs:94-96`, `node.rs:180` `serde_json::Value`), so not a diff-introduced deviation. |
| Concrete owners; traits only for multiple real impls / intentional boundary; no zero-sized utility structs or speculative layers (AGENTS.md:27; agent-rules.md:10; rust-core.md:3) | PASS | `GitHubClient`, `Execution`, `CancelToken`, `UreqTransport` are stateful concrete owners. `HttpTransport` has two real impls (`UreqTransport` github.rs:130; `FakeTransport` :1359) and is the documented seam (github.rs:8-9) — "intentional boundary" per rust-core.md:3. `ObjectStorage`/`Persistence` have no impl in this task but are explicitly required by spec REQ-015 (spec.md:78 "The library defines a narrow trait for snapshot metadata/pointers...") and REQ-016 (spec.md:79 "The library defines a narrow trait for immutable snapshot/context objects...") and design.md:22. No zero-sized structs. |
| Prohibited dependencies (AGENTS.md:34) | PASS | Added: `ureq` (generic HTTP client, not a Git provider SDK), `sha1` (Git blob id, REQ-004 blob verification), `sha2` (already in workspace). No Node/NAPI/SQLite/PostgreSQL/PyO3/provider SDK/model SDK. Cargo.lock: 0 `source = "git+"` entries. Dependency addition is authorized by the task write set (TASK-002 line 52) rather than a material stop (implementation-execution.md:105). |
| Outer crate depends on facade only; no consumer type; no server/worker loop (AGENTS.md:23; agent-rules.md:3; design.md:22; spec INV-001 spec.md:85) | PASS | `cargo tree` above; only `bohmian_code_review::{InvalidRelativePath, RelativePath}` imported; no `tokio`/`axum`/`thread::spawn`/`async`; `thread::sleep` is a bounded, cancellable retry wait (github.rs:1177-1186), not a loop. |
| Synchronous, no runtime, caller-bounded (AGENTS.md:28; rust-core.md:5) | PASS | `Execution::check` before every attempt (github.rs:1138); per-request timeout capped by remaining deadline (:1139). |
| No source retention/persistence outside selected context object; no disk (agent-rules.md:9; spec REQ-014 spec.md:77) | PASS | No `std::fs`; `blob` returns transient `Vec<u8>`; `Archive` streams through `BoundedReader`; `ObjectStorage` stores only snapshot/context objects by scoped key. Token redacted in `Debug`; `HttpRequest` has no `Debug` (github.rs:42-44). |
| Validate at ingress; domain values at trust boundaries (agent-rules.md:7; rust-core.md:7) | PASS | `TenantId`, `InstallationId`, `RepositoryIdentity`, `ObjectSha`, `GitHubHost::enterprise`, `InstallationToken` validate on construction and re-validate on deserialize; tree/PR paths go through `RelativePath`. |
| Tests inline; external tests only in facade (agent-rules.md:5; `scripts/check-test-placement.sh`) | PASS | No `crates/bohmian-code-review-pipeline/tests/`; lints lane (placement script) exit 0. |
| Fast lane credential-free and network-free (agent-rules.md:5; testing-workflows.md:15) | PASS | Tests use only `FakeTransport`; `UreqTransport::new()` is constructed solely in `GitHubClient::new` (github.rs:751), which no test calls; `test:pipeline` finished in 0.00s. |
| mise lane shape consistent (`--locked --all-targets --all-features`) and wired into `test:rust` (AGENTS.md:40; testing-workflows.md:15; mise.toml:38-52) | PASS | mise.toml:54-56 mirrors `test:index`/`test:select`; added to `test:rust` depends (mise.toml:66). |
| No `#[allow]`/`#[ignore]`/weakened checks (agent-rules.md:12) | PASS | `grep` finds none in the crate. |
| Grammar pins untouched; generated artifacts untouched (AGENTS.md:33; agent-rules.md:6) | PASS | Cargo.toml diff touches only members and three non-grammar deps. |
| Evidence format: AC table, commands, non-goals, focused `mise exec -- cargo test --locked -p <crate> --lib <sel> -- --exact` per named test (cr-implement/SKILL.md:64-72; spec-driven-development.md:183-200; testing-workflows.md:17-20) | PASS | Task lines 70-80 table; lines 86-105 non-goals, commands (fmt/lints/gate/diff --check), 12 focused selectors matching source test names; all 12 reproduced by `test:pipeline` (12 passed). |
| RED per scenario for expected missing behavior, not compilation (spec-driven-development.md:204-208; implementation-execution.md:215-217) | PARTIAL, not a finding | Task line 82 self-reports compile-failure RED for a new crate plus one observed behavioral RED. Same pattern was accepted for TASK-001 (task doc line 68, review r3 PASS); no retroactive testable correction exists; GREEN closure reproduced. Recorded for the orchestrator, not counted. |
| Plan/task/agent references out of production code (AGENTS.md:44) | PASS | `rg "TASK-|REQ-|cr-" src/` — only "Scenario 1/2" in test rustdoc, none in production items. |
| Git identity; no AI co-author trailers (AGENTS.md:46; agent-rules.md:15) | PASS | Both commits authored `Thorrester <sjforrester32@gmail.com>`; no `Co-Authored-By` trailer. |

## 3. Findings

### REPO-1 — Private fallible functions omit the required `# Errors` section

- Classification: rustdoc contract violation (written rule), non-behavioral.
- Root cause: the implementer applied `# Errors` only to `pub` items and trait methods; the repository rule and existing crates apply it to every fallible function including private helpers. First site in dependency order: `crates/bohmian-code-review-pipeline/src/identity.rs:125` `validate_name`.
- Defect class: missing mandatory rustdoc section on new fallible items.
- Sibling locations (one finding):
  - `src/identity.rs:125` `validate_name`
  - `src/identity.rs:162` `TryFrom<RawRepository>::try_from` (doc line present, no `# Errors`)
  - `src/github.rs:1075` `GitHubClient::get_json`
  - `src/github.rs:1089` `GitHubClient::request`
  - `src/github.rs:1127` `GitHubClient::send_with_policy`
  - `src/github.rs:1177` `sleep_bounded`
  - `src/github.rs:1204` `expect_status`
  - `src/github.rs:1217` `read_bounded`
  - `src/github.rs:1232` `parse_json`
  - `src/github.rs:1258` `validate_tree_entry`
  - `src/github.rs:1273` `validate_pull_request_file`
  - Out of scope: `HttpTransport::get` impls (trait doc github.rs:96-100 carries `# Errors`), std `Read`/`Deserialize` impls.
- Violated rule: AGENTS.md:32 "Every new or materially modified Rust item, including private items and tests, requires useful rustdoc ... Fallible functions need `# Errors`"; architecture/agent-rules.md:11; architecture/references/languages/rust-core.md:7 "including private helpers ... fallible methods explain `# Errors`".
- Evidence: awk scan of the crate (fn signature containing `Result<` with no `# Errors` in the preceding 13 doc lines) lists exactly the sites above; the same scan over the five existing symbolic crates and the facade returns none (e.g. private `fn start` at `crates/bohmian-code-review-select/src/diff.rs:185` has the section).
- Observable consequence: `send_with_policy` is the single request policy whose retry/terminal outcomes (REQ-013, INV-006) are not documented at the function; readers must infer which `GitHubError` variants it can return from the body. No runtime effect; not caught by `lints` (clippy `missing_errors_doc` is pedantic and pub-only).
- Testable correction: add an `# Errors` section to each listed function naming the returned error variants (e.g. `send_with_policy`: `Interrupted` via `Execution::check`, `RateLimited`, `Forbidden`, `Unavailable`, `Transport`); verify with `mise run lints` (exit 0) and re-run the awk scan (empty output). Doc-only; no behavior change.

## 4. Citations

```citations
[
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 125, "snippet": "fn validate_name(raw: &str, what: &'static str) -> Result<String, InvalidIdentity> {", "symbols": ["validate_name"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/identity.rs", "line": 162, "snippet": "    fn try_from(raw: RawRepository) -> Result<RepositoryIdentity, InvalidIdentity> {", "symbols": ["RepositoryIdentity", "RawRepository"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1075, "snippet": "    fn get_json<T: for<'de> Deserialize<'de>>(", "symbols": ["GitHubClient", "get_json"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1089, "snippet": "    fn request(", "symbols": ["GitHubClient", "request"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1127, "snippet": "    fn send_with_policy(", "symbols": ["GitHubClient", "send_with_policy"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1177, "snippet": "fn sleep_bounded(execution: &Execution, wait: Duration) -> Result<(), Interrupted> {", "symbols": ["sleep_bounded"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1204, "snippet": "fn expect_status(", "symbols": ["expect_status"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1217, "snippet": "fn read_bounded(body: &mut dyn Read, limit: usize, resource: &str) -> Result<Vec<u8>, GitHubError> {", "symbols": ["read_bounded"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1232, "snippet": "fn parse_json<T: for<'de> Deserialize<'de>>(", "symbols": ["parse_json"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1258, "snippet": "fn validate_tree_entry(entry: &TreeEntryWire) -> Result<TreeEntry, String> {", "symbols": ["validate_tree_entry"]},
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 1273, "snippet": "fn validate_pull_request_file(file: &PullRequestFileWire) -> Result<PullRequestFile, String> {", "symbols": ["validate_pull_request_file"]}
]
```

## 5. Overall

FAIL — REPO-1 (doc-only remediation; all mechanical lanes green).
