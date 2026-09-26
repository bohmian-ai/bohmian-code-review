# TASK-002 task review r2 — repository standards (repo-rev)

Subject: `/tmp/t002r2-subject.md`. Range `1842eaf..93b2542` (cumulative), read-only. Finding prefix `REPO-`.

Method: read AGENTS.md, `architecture/agent-rules.md`, `architecture/references/README.md`, `rust-core.md`, `testing-workflows.md`, `implementation-execution.md` (rules sections), `spec-driven-development.md` (test-command and TDD sections), `bohmian-code-review-design.md`; read every changed source file in full (`lib.rs`, `execution.rs`, `identity.rs`, `storage.rs`, `persistence.rs`, `github.rs` 1-1929), the manifest/lane/lockfile diffs, and the task document evidence sections. Mechanical sweeps run: `# Errors` 14-line lookback over every `-> Result<` in `crates/bohmian-code-review-pipeline/src/*.rs`, multi-line signature list, `unwrap|expect`, `#[allow|ignore]`, non-top `use`, `clone|to_owned|to_vec`, `async|tokio|std::fs|std::env`, `tracing|println`, `git+` lockfile sources, symbolic-crate manifests for a `pipeline` dependency. One focused Cargo command run (below). Prior r1 standards finding REPO-1 (= FIND-TASK-002-2) rechecked for closure across every sibling.

## 1. Authority coverage

| Changed surface | Governing authorities (file:line) |
|---|---|
| `Cargo.toml` (member, `bohmian-code-review` path dep, `sha1`, `ureq`) | AGENTS.md:23 (facade-only dependency), AGENTS.md:27 (no unnecessary deps/features), AGENTS.md:33 (grammar pins intact), AGENTS.md:34 (prohibited deps), agent-rules.md:3, agent-rules.md:6, design.md:22, implementation-execution.md:105 (dependency addition needs task authority) |
| `Cargo.lock` | AGENTS.md:34 (no remote source dep), testing-workflows.md:18 (`--locked`) |
| `mise.toml` `test:pipeline` + `test:rust` wiring | AGENTS.md:40, testing-workflows.md:15, agent-rules.md:5 (credential/network-free fast lane), spec-driven-development.md:178-181 |
| `crates/bohmian-code-review-pipeline/Cargo.toml` | AGENTS.md:23, AGENTS.md:34, design.md:22, spec.md:19 |
| `src/lib.rs` | AGENTS.md:23 (no consumer imports, facade only), AGENTS.md:32 (rustdoc), AGENTS.md:44 (no plan/task refs in production code) |
| `src/identity.rs` | AGENTS.md:29-32, agent-rules.md:7 (validate at ingress), agent-rules.md:10-11, rust-core.md:7 |
| `src/execution.rs` | AGENTS.md:27-28 (concrete owners, synchronous), AGENTS.md:32, rust-core.md:3,5 |
| `src/github.rs` | AGENTS.md:21,23 (only crate with provider IO; no persistence of source), AGENTS.md:27-32, AGENTS.md:34 (no provider SDK), agent-rules.md:3,5,9,10,11, rust-core.md:3,5,7, design.md:5,22 |
| `src/storage.rs`, `src/persistence.rs` | AGENTS.md:23 (narrow traits, no consumer type), AGENTS.md:27 (traits only for multiple real impls / intentional boundary), AGENTS.md:29-32, agent-rules.md:7,9,10,11, rust-core.md:3,7, spec.md:79,91 |
| Inline test modules (all five files) | agent-rules.md:5 (inline by default), agent-rules.md:12 (no hidden failing checks), AGENTS.md:32 (rustdoc on tests), testing-workflows.md:15 |
| Task document evidence sections (`tasks/TASK-002-...md:66-122`) | spec-driven-development.md:183-200 (exact focused command per named test), :204-218 (RED/GREEN record), implementation-execution.md:236-275 (focused verification, `git diff --check`, diff audit), testing-workflows.md:17-20 |
| Commits `1696545..93b2542` | AGENTS.md:46, agent-rules.md:15 (identity, no AI co-author trailers) |

## 2. Rule-by-rule result

| Rule (authority) | Result | Evidence |
|---|---|---|
| Outer crate depends on the public facade only; no consumer imports; no symbolic stage duplication (AGENTS.md:23; agent-rules.md:3; design.md:22) | PASS | `crates/bohmian-code-review-pipeline/Cargo.toml` deps: `bohmian-code-review`, serde, serde_json, sha1, sha2, thiserror, ureq. Only facade import is `github.rs:14` `use bohmian_code_review::{InvalidRelativePath, RelativePath};` (both re-exported by `crates/bohmian-code-review/src/lib.rs:20-21`). No extraction/index/resolve/select logic in the crate; `git_blob_sha`/`encode_segment` are provider-level helpers. No symbolic crate manifest names `pipeline` (grep: none). |
| Six symbolic crates gain no IO (AGENTS.md:21; design.md:22) | PASS | Diff touches no `crates/bohmian-code-review-{core,extract,index,resolve,select}` or facade source. |
| No source retention/persistence; source only transient (agent-rules.md:9; design.md:5) | PASS | No `std::fs`/`std::env` (grep: none). `blob` returns `Vec<u8>` to caller; `Archive` streams through `BoundedReader`; `ObjectStorage`/`Persistence` are traits with no implementation here; `ContextRecord`/`SnapshotPointer` carry keys and digests only. |
| Synchronous, no runtime, no server/worker loop (AGENTS.md:28; rust-core.md:5; spec INV-001) | PASS | No `async`/`tokio` (grep: none). `thread::sleep` in `sleep_bounded` (github.rs:1212-1221) is a cancellable, deadline-bounded wait inside one call, not a loop owner. |
| Concrete owners; traits only for multiple real impls or intentional boundary (AGENTS.md:27; agent-rules.md:10; rust-core.md:3) | PASS | `GitHubClient`, `Execution`, `CancelToken`, `UreqTransport`, `CallCounts` are concrete stateful owners. `HttpTransport` has two real impls (`UreqTransport` github.rs:130, test `FakeTransport` :1426) and is the documented seam (:8-9). `ObjectStorage`/`Persistence` are the spec-mandated injected boundary (spec.md:79; design.md:22). No zero-sized utility structs. |
| Prohibited dependencies; no unnecessary deps/features (AGENTS.md:34; AGENTS.md:27) | PASS | Added `ureq` (generic HTTP; not a provider SDK) with `default-features = false` — drops ureq's default `gzip` (archive is consumed compressed) — plus `rustls` and `platform-verifier` (system trust store, needed for GHES private CAs under REQ-004). `sha1` is required for Git blob ids (REQ-004/INV-003). `Cargo.lock`: 0 `git+` sources. Grammar pins untouched. Dependency addition authorized by TASK-002 write set (task line 52). |
| Ownership: references where ownership unnecessary; `.clone()` as a decision (AGENTS.md:29) | PASS (one observation, §3-O1) | All public methods take `&Execution`, `&RepositoryIdentity`, `&ObjectSha`, `&str`. `to_owned()` sites are constructors storing validated text or error payloads. |
| `thiserror` for library errors; no unchecked `unwrap`; `expect` only for named invariants (AGENTS.md:30; rust-core.md:7) | PASS | Six error enums derive `Error`. Production code: zero `unwrap()`/`expect()`; only `unwrap_or` fallbacks (identity.rs:312,340; github.rs:577,707,728,961). All `expect` are in `#[cfg(test)]` with names. |
| `# Panics` documented or removed (agent-rules.md:11; rust-core.md:7) | PASS | Remaining implicit panics are named invariants: `CallCounts` indexes an 8-array by an 8-variant enum (`Operation::ALL`, github.rs:225-234); `archive_host` slices past `"https://"` only after `enterprise` validated the prefix (identity.rs:309-311, :340); `BoundedReader::read` subtracts `read <= want <= remaining` (github.rs:577-579). |
| Imports at module top; bare imported types in signatures (AGENTS.md:31; agent-rules.md:11) | PASS | Every `use` is at module top or test-module top. Path-qualified `serde::Deserializer`, `serde_json::Error`, `std::fmt::*` (persistence.rs:97-99, github.rs:1302) are un-imported types, matching the core precedent (`core/src/relative_path.rs:94-96`); not a diff-introduced deviation. |
| Rustdoc on every new item incl. private items and tests (AGENTS.md:32; agent-rules.md:11; rust-core.md:7) | PASS | Every struct, enum, trait, fn, impl method, const, type alias, and `#[test]` in all five modules has an intent doc line, including wire structs, `BoundedReader`, `FakeTransport`, `FakeRef`, `SeenRequest`, `Scripted`, and test helpers. Undocumented private struct fields follow the existing-crate pattern (`select/src/diff.rs:37-61`). |
| `# Errors` on every fallible item (AGENTS.md:32; agent-rules.md:11; rust-core.md:7) — r1 REPO-1 / FIND-TASK-002-2 closure | PASS (closed) | Sweep over all `-> Result<` fns: one lookback miss at `github.rs:1164`, which is the multi-line `send_with_policy` signature (starts :1157) whose `# Errors` section is at :1146-1156 — present. All 17 r1 sibling sites now carry `# Errors`: identity.rs:86,123,136,178,278; github.rs:133,1082,1104,1146,1208,1240,1260,1281,1313,1334, tests :1429,1469. New fallible items from R1 also covered: `ObjectKey::parse` storage.rs:88, `Deserialize for ObjectKey` :112, `ContextId::new` persistence.rs:70, `Deserialize for ContextId` :87. Trait methods: storage.rs:158,167,175; persistence.rs:134,148,160,172,183,194,202. |
| Validate at public ingress; domain values at trust boundaries (agent-rules.md:7; rust-core.md:7; spec INV-007) — r1 FIND-TASK-002-1 closure | PASS (closed) | `ObjectKey::context` takes `&ContextId` (storage.rs:78); `ObjectKey` re-validated per segment on deserialize (storage.rs:92-97,116-119); `ContextId` is a private newtype validated by `validate_name` (persistence.rs:65-76,91-94). Tests storage.rs:218-241 and persistence.rs:216-232,237-281 prove hostile reloads fail. |
| Uncalled public classifier removed — r1 FIND-TASK-002-3 closure | PASS (closed) | `rg 'ObjectKind|fn kind' crates` → no matches (recorded in task doc :117; confirmed by reading `storage.rs`/`lib.rs`). |
| Tests inline with owning module; external tests only via facade (agent-rules.md:5; testing-workflows.md) | PASS | Five `#[cfg(test)] mod tests`; no `crates/bohmian-code-review-pipeline/tests/`; `scripts/check-test-placement.sh` runs inside `lints`. |
| Fast lane credential-free, network-free (agent-rules.md:5; testing-workflows.md:15) | PASS | Tests use only `FakeTransport`; `UreqTransport::new` is reached solely through `GitHubClient::new` (github.rs:756-758), which no test calls. Focused run finished in 0.00s. |
| Lane shape and wiring (AGENTS.md:40; testing-workflows.md:15) | PASS | `mise.toml:54-56` `test:pipeline` = `cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features`, mirroring sibling lanes; added to `test:rust` depends (:66). `lints` and `check:default` are `--workspace` so the new crate is covered mechanically. |
| No `#[allow]`/`#[ignore]`/weakened assertions/changed goldens (agent-rules.md:12) | PASS | grep: none. No fixture or golden touched. |
| Plan/task/agent references out of production code (AGENTS.md:44) | PASS | Only "Scenario 1/2" appear, in test rustdoc (github.rs:1497,1650,1741,1826). |
| Exact focused command per named test; selector proven to run a test (spec-driven-development.md:183-200; testing-workflows.md:17-18) | PASS | Original 12 selectors listed verbatim (task doc :94-105). Remediation evidence (:122) states the four new/changed selectors were run with the same template; I reproduced one: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib storage::tests::keys_are_revalidated_on_deserialization -- --exact` → `1 passed; 14 filtered out`, exit 0. |
| RED/GREEN record per scenario (spec-driven-development.md:204-218; implementation-execution.md:215-217) | PARTIAL, not counted (§3-O2) | Original evidence (:82) records one behavioral RED and honestly states the rest were compile-level. Remediation section (:107-122) records GREEN, lanes, and `git diff --check` but no RED for the three new behavioral tests. Same class r1 recorded as "PARTIAL, not a finding" (no retroactive testable correction). |
| Diff audit: `git diff --check`, tracked/untracked inspection, bounded write set (implementation-execution.md:247-275) | PASS | Task doc :88 and :122 list `git diff --check`; :118 `git diff --stat dafe20b..HEAD` bounded to the five source files plus the two documents. Cumulative diff stat matches the subject's file list. |
| Git identity; no AI co-author trailers (AGENTS.md:46; agent-rules.md:15) | PASS | All six commits `Thorrester <sjforrester32@gmail.com>`; no `Co-Authored-By` in any body. |

## 3. Findings

None material.

Observations (not findings; recorded for the orchestrator, no rule text makes them violations):

- **O1 — avoidable clone of PR patch text.** `validate_pull_request_file(file: &PullRequestFileWire)` clones `file.patch` (github.rs:1355) although the caller owns `file` and needs only `file.filename` on the `Err` path (github.rs:1064-1071). AGENTS.md:29 asks that `.clone()` be an ownership decision; it does not forbid one. Sibling `validate_tree_entry` has no equivalent clone. A by-value signature would remove it.
- **O2 — remediation RED not recorded.** Task doc :107-122 records no expected RED failure for `storage::tests::keys_are_revalidated_on_deserialization`, `persistence::tests::context_ids_are_validated_on_construction_and_deserialization`, or `persistence::tests::records_revalidate_their_keys_on_deserialization` (spec-driven-development.md:218 "Record the expected RED failure"). r1 declined to count the identical class for the original evidence because no retroactive correction exists; the same reasoning applies here, and the tests are proven to run and to refuse the hostile inputs the r1 finding named.

Not covered: I did not rerun `fmt:check`, `lints`, or `gate` myself (orchestrator runs them in parallel; only one focused Cargo command was run to respect sequential Cargo use). I did not build a scratch worktree to reproduce the remediation RED.

## 4. Overall

PASS

## 5. Citations

```citations
[]
```
