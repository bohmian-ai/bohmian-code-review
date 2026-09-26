# TASK-002 r6 — repository-standards review (repo-rev)

- Candidate `7983907` (read-only at `/tmp/cr-r6/wt`), base `1842eaf`. Diff: `crates Cargo.toml Cargo.lock mise.toml`.
- In scope: pipeline `identity.rs`, `storage.rs`, `persistence.rs`, `github.rs`, `execution.rs`, `lib.rs`, crate `Cargo.toml`, workspace `Cargo.toml`/`Cargo.lock`, `mise.toml`; compile-only edits in `snapshot.rs` (b83d286 trait rename/fmt, 54232b2 `GitHubHost::public()`), `eligibility.rs` (b83d286 rustfmt), `testing.rs` (b83d286 `MemorySnapshotStore` split, `RequestHook` alias).
- Out of scope: TASK-003 behavior in `snapshot.rs`/`eligibility.rs`/`testing.rs` (including the b83d286 trailing-`**` glob change, which is TASK-003 behavior), docs-only commits, `changes/**/review/`.

## Verification run (scratch export `git archive 7983907`, `/tmp/cr-r6/scratch-reporev`)

| Command | Result |
|---|---|
| `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features` | `test result: ok. 23 passed; 0 failed` (23 = count of `#[test]` in the crate) |
| `RUSTDOCFLAGS="-D warnings" mise exec -- cargo doc --no-deps -p bohmian-code-review-pipeline --all-features --document-private-items` | builds with no warnings (broken intra-doc links would fail) |
| `mise exec -- cargo clippy --locked -p bohmian-code-review-pipeline --all-targets --all-features -- -W clippy::missing_errors_doc -W clippy::missing_docs_in_private_items -W clippy::missing_panics_doc -W clippy::unwrap_used -W missing_docs` (pipeline diagnostics only) | 69 warnings, **all** "missing documentation for a field" on private struct fields. There are zero missing-item-doc, zero `missing_errors_doc`, zero `missing_panics_doc`, and zero `unwrap_used` diagnostics. |
| Orchestrator `/tmp/cr-r6/lanes.txt` | `fmt:check` exit 0; `lints` (clippy `-D warnings` + `check-test-placement.sh`) exit 0; `test:pipeline` exit 0 (the doc step runs 0 doctests) |
| `rg -n 'non_exhaustive\|compile_fail\|validate_key_segment\|KeySegment\|<D: serde::\|&serde_json::\|#\[allow\|#\[ignore' crates/bohmian-code-review-pipeline/src` | no matches |
| Non-test `unwrap()`/`expect(`/`panic!`/`unreachable!` scan (lines before each `#[cfg(test)]`) in the five owned files | none |

## (1) Authority coverage

| Authority | Loaded | Applies to |
|---|---|---|
| `AGENTS.md` (all) | yes | every changed surface |
| `architecture/agent-rules.md` (all) | yes | every changed surface |
| `architecture/references/README.md` | yes | routing |
| `references/languages/rust-core.md` | yes | all Rust source |
| `references/languages/testing-workflows.md` | yes | inline tests, `mise.toml` lane |
| `bohmian-code-review-design.md:20-22` | yes | crate dependency direction, storage traits |
| `references/domain/symbolic-context.md` | not loaded | no symbolic-crate change |
| `spec-driven-development.md`, `implementation-execution.md` | not loaded | task-acceptance and evidence, which are out of scope for this role |

## (2) Rule-by-rule result

| Rule (file:line) | Surfaces | Evidence | Result |
|---|---|---|---|
| Imports at module top; imported bare types in signatures (AGENTS.md:31; agent-rules.md:11) | all `.rs` | `Deserializer` imported at `identity.rs:8`, `storage.rs:11`, `persistence.rs:12`; `JsonError` alias at `github.rs:16` used at `:1343`. `fmt::`/`io::` signatures use module imports (`use std::fmt;`, `use std::io::{self, Read}`), the form r3/r5 accepted, and `fmt::Result`/`io::Result` cannot be bare without shadowing `Result`. **The `UreqTransport` field type `ureq::Agent` (`github.rs:107`) is a crate-root path with no import.** | **FAIL — REPO-1** |
| Rustdoc on every new/materially modified item incl. private items and tests (AGENTS.md:32; agent-rules.md:11; rust-core.md:7) | all `.rs` | The clippy `missing_docs_in_private_items` run reports no item, only private fields. Fields are not items, and the crate's convention (e.g. `Execution`, `RepositoryIdentity`) leaves private fields undocumented. The R4 items `canonical`, `parse`, `SECONDARY_RATE_LIMIT_WAIT`, `RATE_LIMIT_BODY_BYTES`, `names_rate_limit`, `trusts_archive_host`, `GitHubHost` fields, `hostile_keys`, and every test have rustdoc. | PASS |
| `# Errors` on fallible functions; document panics (AGENTS.md:32; rust-core.md:7) | all `.rs` | `missing_errors_doc`/`missing_panics_doc` clean. A manual check of the `io::Result`/`Result<_, String>` returns at `github.rs:584,1306,1359,1381` and the test fakes `:1474,:1514` found `# Errors` present. `send_with_policy` `# Errors` (`github.rs:1188-1200`) describes the new 403/429 classification. `Execution::with_timeout` has `# Panics` (`execution.rs:59-62`). | PASS |
| `thiserror` for library errors (AGENTS.md:30; rust-core.md:7) | errors | `InvalidIdentity`, `StorageError`, `PersistenceError`, `GitHubError`, `Interrupted`, `TransportError` derive `Error`. `UntrustedRedirect` is now a unit variant (`github.rs:343-345`). | PASS |
| No unchecked `unwrap()`; `expect()` only for named invariants (AGENTS.md:30; rust-core.md:7) | non-test code | The scan and `clippy::unwrap_used` are clean. The intentional discard `let _ = …read_to_end` (`github.rs:763-765`) is documented as "a read error counts as no signal". | PASS |
| Concrete owners; traits only for multiple real implementations or an intentional boundary; no future-proofing (AGENTS.md:27; agent-rules.md:10; rust-core.md:3) | `GitHubHost`, traits | `GitHubHost` is a struct with private fields; `#[non_exhaustive]`, `ApiBase` newtype, and `compile_fail` doctests are gone. `ObjectStorage`/`SnapshotStore`/`ContextStore` are the consumer-implemented seams design.md:22 names. `HttpTransport` has `UreqTransport` plus test fakes. The split `Persistence` → two traits follows design.md:22. | PASS |
| Free functions only for stateless transforms (AGENTS.md:27) | `canonical`, `names_rate_limit`, `rate_limit_wait`, `is_trusted_redirect` | `canonical`/`parse` are associated functions. The helpers are stateless over their arguments; `names_rate_limit` consumes the response body it is given. | PASS |
| Dependency limits: no Node/NAPI/SQLite/PostgreSQL/PyO3/Git-provider SDK/model SDK/remote source; no unnecessary features (AGENTS.md:27,34) | `Cargo.toml`, crate manifest, `Cargo.lock` | New direct deps are `ureq` (`default-features = false`, `rustls`, `platform-verifier`), `flate2`, `tar` (`default-features = false`), and `sha1`. `Cargo.lock` adds only transitive TLS/HTTP/compression crates. There is no provider SDK or database. R4 added no dependency. | PASS |
| Pipeline depends only on the public facade (AGENTS.md:23; agent-rules.md:3; design.md:22) | crate `Cargo.toml:11-20` | The only workspace dep is `bohmian-code-review`. | PASS |
| Use domain values at trust boundaries (AGENTS.md:29; agent-rules.md:7) | `ObjectKey`, `GitHubHost` | `ObjectKey::parse` validates through `TenantId::new`, `InstallationId::new`, `NonZeroU64`, `ObjectSha::new`, `ContextId::new`, and checks byte-exact re-format (`storage.rs:97-145`). | PASS |
| Tests inline with owning module; credential- and network-free (agent-rules.md:5; testing-workflows.md:15) | tests | `check-test-placement.sh` passes in `lints`. Tests use `FakeTransport`/`RoutedTransport`; there is no network. | PASS (lane) |
| Never hide a failing check with `#[allow]`/`#[ignore]`/weakened gate (AGENTS.md:40; agent-rules.md:12) | all | No `#[allow]` or `#[ignore]` in the crate. The `test:pipeline` lane keeps `--all-targets` and `--doc`. | PASS |
| Formatting and lint (enforced by mise `fmt:check`, `lints`) | all | lanes exit 0 | PASS (lane) |
| No plan/task/agent references in production code (AGENTS.md:44) | `src/` | `rg 'TASK-\|FIND-\|REQ-\|INV-\|AC-\|spec\.md'` on `src/` finds nothing. | PASS |
| `mise.toml` lane naming (AGENTS.md:40) | `mise.toml` | `test:pipeline` is added and aggregated into `test:rust`. R4 left `mise.toml` unchanged. | PASS |

## (3) Findings

### REPO-1 — CLASS_INCOMPLETE of FIND-TASK-002-8: a crate-root type path remains in a declared type position

- **Classification:** CLASS_INCOMPLETE. This is a sibling the R4 closure missed, and it is the same class as FIND-TASK-002-8.
- **Root cause:** The R4 closure again swept a fixed pattern (`<D: serde::|&serde_json::`, AC-R4-3) instead of applying the rule. As a result, the `UreqTransport` struct declares its field type as `ureq::Agent` (`github.rs:107`), and `ureq` is never imported at module top.
- **Defect class:** A type in a declaration or signature is written as an unimported crate path instead of an imported bare type. The owner's shape permits it because no lint enforces the rule, so every closure depends on a manual sweep.
- **Sibling locations:** `github.rs:107`. A full-crate sweep for `[a-z_]+::[A-Z]` outside `use` lines found no other declaration or signature site. The remaining hits are body-level paths (`github.rs:113`, `:150`, `:589`, `:1345-1348`, test `:2010`) or `fmt::`/`io::` paths through imported modules, which prior rounds accepted.
- **Violated rule:** AGENTS.md:31 "Keep imports at module top and use imported bare types in signatures."; agent-rules.md:11 "Keep imports at module top; use bare imported types in signatures."
- **Evidence:** `github.rs:106-108` reads `pub struct UreqTransport {` / `agent: ureq::Agent,` / `}`. `github.rs:11-23` contains no `use ureq`.
- **Observable consequence:** Code the diff introduced violates a written repository rule. FIND-8's own closure criterion is pattern-based, so the rule is still not fully applied after two rounds.
- **Testable correction:** Add `use ureq::Agent;` at the top of `github.rs`. Declare `agent: Agent` and, optionally, `Agent::config_builder()`. `ureq::Error` in the body can stay as it is. A sweep of field and signature types (`rg -n '^\s+[a-z_]+: [a-z_]+::[A-Z]|-> [a-z_]+::[A-Z]|: [a-z_]+::[A-Z][^(]*[,)]' crates/bohmian-code-review-pipeline/src` excluding `fmt::`/`io::`) should return nothing.

## (4) Overall

**FAIL.** One finding, REPO-1: an incomplete sibling of FIND-TASK-002-8 (AGENTS.md:31). Every other applicable repository rule passes on the in-scope diff.

Note (not a finding): AC-R4-7's literal `rg -n 'ApiBase|…' identity.rs` still matches, because `InvalidIdentity::ApiBase` is kept on purpose (R4 constraint "`InvalidIdentity` keeps its other variants"). The task reviewers should read that AC as referring to the deleted newtype.

```citations
[
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/github.rs", "line": 107, "snippet": "    agent: ureq::Agent,", "symbols": ["UreqTransport", "agent"]}
]
```
