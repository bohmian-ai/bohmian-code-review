# TASK-002 r7 — repository-standards review (repo-rev)

Subject: `/tmp/cr-r7/wt`, HEAD `9fab199dfb282d07186e9f66ec3b3686328421e7`, checked at start and end. Base `1842eaf`. Scope: the cumulative in-scope diff (`crates`, `Cargo.toml`, `Cargo.lock`, `mise.toml`), with priority on R5 (`6c0e386`). TASK-003 behavior in `snapshot.rs`, `eligibility.rs`, and `testing.rs` is out of scope.

## 1. Authority coverage

| Changed surface | Applicable authorities |
|---|---|
| `crates/bohmian-code-review-pipeline/src/identity.rs` (R5: `is_key_segment`, `TenantId::new`, `validate_name`, `InvalidIdentity::Tenant`, owner test) | AGENTS.md:27, 29, 30, 31, 32; agent-rules.md:7, 10, 11, 12; references `languages/rust-core.md` |
| `.../src/storage.rs` (R5: `hostile_keys` fixture rows) | AGENTS.md:32, 40; agent-rules.md:5, 11, 12 |
| `.../src/{execution,github,persistence,storage,lib,testing}.rs` (cumulative) | AGENTS.md:23, 27–32, 34; agent-rules.md:3, 5, 9–12 |
| `crates/bohmian-code-review-pipeline/Cargo.toml`, root `Cargo.toml`, `Cargo.lock` | AGENTS.md:23 (facade-only dependency), 27 (no unnecessary deps), 34 (banned dependency classes) |
| `mise.toml` (`test:pipeline`, `test:rust`) | AGENTS.md:40; agent-rules.md:5; references `languages/testing-workflows.md` |

## 2. Rule results

| Rule | Result | Evidence |
|---|---|---|
| Formatting (AGENTS.md:40, `fmt:check`) | PASS | Mise refused the untrusted scratch config, so I ran the lane command `cargo fmt --all -- --check` in `/tmp/cr-r7/scratch-std`. Exit 0. |
| No suppressed lints; clippy clean (AGENTS.md:40, agent-rules.md:12, `lints`) | PASS | `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` finished clean. `scripts/check-test-placement.sh` exited 0. The added lines in `full.diff` contain no `#[allow`, `#![allow`, or `#[ignore`. |
| Owner lane passes (AGENTS.md:40, `test:pipeline`) | PASS | `cargo test --locked -p bohmian-code-review-pipeline --all-targets --all-features`: 23 passed, 0 failed, 0 ignored, including `identity::tests::identities_validate_key_safe_spellings`. The doc lane ran with 0 failures. The `test:pipeline` target exists and is wired into `test:rust`. |
| Rustdoc on new or modified items, including private items and tests (AGENTS.md:32, agent-rules.md:11) | PASS | A scripted scan of every `fn/struct/enum/trait/const/static/type` in the in-scope pipeline files found only one item without a preceding `///`: the trait-associated `type Error` at identity.rs:179. Every module has `//!` docs. R5's `is_key_segment` doc states the shared rule, bounds, and the no-traversal invariant. Its claim that the rule is shared by tenant, owner, repository, and context ids is true: callers are identity.rs:83, identity.rs:151, identity.rs:207–208, and persistence.rs:77 via `validate_name`. The R5-modified test keeps its `///` summary. |
| `# Errors` on fallible functions (AGENTS.md:32) | PASS | Every in-scope `-> Result` fn documents `# Errors`, including `TenantId::new` (updated in R5 to list `.`/`..`) and `validate_name`. The only fns without it are the std `fmt` trait implementations (identity.rs:275, 393; persistence.rs:101; storage.rs:172), whose error contract std defines. |
| `thiserror` library errors (AGENTS.md:30) | PASS | `InvalidIdentity` derives `Error`. R5 updated its `#[error]` text to match the new rejection. |
| No unchecked `unwrap`; `expect` only for invariants (AGENTS.md:30) | PASS | No added `.unwrap()`. `.expect(` and `panic!` appear only inside `#[cfg(test)]` modules in the in-scope files. |
| Imports at module top; bare imported types in signatures (AGENTS.md:31, agent-rules.md:11) | PASS | No nested `use` inside function bodies. The only module-qualified signature types are `fmt::Formatter`/`fmt::Result`/`io::Result` from top-level `use std::{fmt, io}` in trait implementations. |
| Domain values at trust boundaries (AGENTS.md:29) | PASS | R5 moves tenant, owner, repository, and context segment validation onto one shared predicate. `serde` deserialization of `TenantId` still routes through `TenantId::new`, which the new test asserts for `"."` and `".."`. |
| Concrete owners; traits only with real multiple implementations (AGENTS.md:27, agent-rules.md:10) | PASS | `ObjectStorage`, `SnapshotStore`, and `ContextStore` are the consumer-implemented traits the spec mandates (spec REQ-016, fixed decisions at spec.md:93). `HttpTransport` has a production `UreqTransport` and the provider fakes that drive the required fake-provider evidence. R5 added no trait or abstraction and replaced two ad-hoc checks with one free, stateless predicate. |
| Dependency limits (AGENTS.md:23, 27, 34) | PASS | `cargo tree -p bohmian-code-review-pipeline --depth 1` shows `bohmian-code-review` (the facade only), `flate2`, `serde`, `serde_json`, `sha1`, `sha2`, `tar`, `thiserror`, and `ureq` (a generic HTTP client, not a Git provider SDK). `Cargo.lock` has no sqlite, postgres, pyo3, napi, octocrab, git2, gix, reqwest, or tokio crate. R5 changes no manifest. |
| No plan, task, or agent references in production code (AGENTS.md:44) | PASS | `grep -rn "TASK-\|FIND-\|REQ-0\|INV-0"` over `crates/bohmian-code-review-pipeline/src` returns nothing. |
| Inline owner tests; fast lane has no credentials or network (agent-rules.md:5) | PASS | The R5 tests are inline in `identity.rs` and in the shared `storage.rs` fixture. The placement script passed, and the lane uses in-memory fakes only. |
| No weakened assertion or changed golden (agent-rules.md:12) | PASS | R5 only adds assertions: the dot-segment tenant cases and two hostile-key rows. No existing assertion was removed or loosened. |

## 3. Findings

None.

## 4. Overall

**PASS**

```citations
[]
```
