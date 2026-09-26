# TASK-003 r1 — repository-standards review (repo-rev)

Candidate `1144845a8deadda54ee546548f43f96b4d5a284f`, base `60cc56e`. Subject: all of `crates/bohmian-code-review-pipeline/src/{snapshot,eligibility,testing}.rs` (741c049 onward), plus the `60cc56e..1144845` diff of `lib.rs`, `persistence.rs`, `storage.rs`, and the TASK-003 markdown. Read-only review; mechanical rules (fmt, clippy lints, `test:pipeline`) are covered by green lanes and are not re-argued.

## 1. Authority coverage

| Authority | Loaded | Applies to |
|---|---|---|
| `AGENTS.md` | yes | all surfaces (ownership, Rust/design rules, tests, lifecycle) |
| `architecture/agent-rules.md` | yes | owners vs free functions, rustdoc, imports, test placement |
| `architecture/bohmian-code-review-design.md` | yes | pipeline crate IO ownership (line 22), facade-only dependency |
| `references/languages/rust-core.md` | yes | owners, errors, rustdoc, unwrap |
| `references/languages/testing-workflows.md` | yes | inline tests, credential/network-free lanes |
| `references/languages/implementation-execution.md` | yes (authority/evidence slice) | task markdown evidence |
| `references/languages/spec-driven-development.md` | not needed | task markdown only records evidence; no spec/task-shape change |
| `references/domain/symbolic-context.md` | not needed | no symbolic-crate change; pipeline consumes the facade only |

## 2. Rule results

| Rule | Result | Evidence |
|---|---|---|
| Crate ownership / facade-only dependency (AGENTS.md; design.md:22) | PASS | Pipeline imports only `bohmian_code_review::{Error, EXTRACTION_VERSION, IndexedFileFacts, RelativePath, SymbolicContext}`; no symbolic stage re-implemented; no storage backend shipped. |
| No new dependencies (AGENTS.md:27) | PASS | No manifest change in the diff. |
| Traits only for multiple real implementations / intentional boundary (AGENTS.md:27; rust-core.md:3) | PASS | Only the existing consumer-implemented `SnapshotStore` boundary gained methods; that boundary is the one design.md:22 names. |
| Free functions only for stateless transformations; stateful workflows on concrete owners (AGENTS.md:27; agent-rules.md:10; rust-core.md:3) | FAIL | REPO-1. `glob_matches`, `segment_matches`, `pattern_matches`, `classify`, `snapshot_key` are pure and pass. |
| Imports at module top, bare types in signatures (AGENTS.md:31; agent-rules.md:11) | FAIL | REPO-2. The `bohmian_code_review::Error` path in `into_context` was correctly replaced by the imported `SymbolicError`. |
| thiserror errors (AGENTS.md:30) | PASS | New `ColdBuildError::ForeignKey` is a `thiserror` variant with a documented field; removed `DeadlineTooLong` leaves no dead reference. |
| No unchecked unwrap on input/IO; expect only for named invariants (AGENTS.md:30) | PASS | No `unwrap()` in production code of the three files; `unwrap_or(path)` in `pattern_matches` is total. `expect` appears only in `#[cfg(test)]` code with named invariants (locks, fixtures). |
| Rustdoc on every new/materially modified item incl. private and tests; `# Errors`; panics; partial progress (AGENTS.md:32; rust-core.md:7) | PASS | Documented: `IngestStats`+fields, `ForeignKey`, `collect_snapshots` (`# Errors`, partial progress), `snapshot_key`, `read_snapshot` (`# Errors` updated), `stream_entries` (`# Errors`, partial progress), `segment_matches`, `glob_matches`, `is_snapshot_of`, three new `SnapshotStore` methods (`# Errors`), all four `MemorySnapshotStore` impls, test helper `scope`, and all new tests. Lock-poison `expect` in the `#[cfg(test)]` doubles is judged not applicable for `# Panics`. |
| Plan/task/agent references out of production code (AGENTS.md:44) | FAIL | REPO-3. Test rustdoc labels (`S14`, `S17`, `REQ-006`) are inside `#[cfg(test)]` modules, not production code. |
| Tests inline with owning module; lanes credential/network-free (agent-rules.md:5; testing-workflows.md:15) | PASS | All new tests sit in the owning `mod tests`; doubles in `#[cfg(test)] testing.rs`; no network, disk, or clock sleep. |
| No weakened gate / deleted failing test (AGENTS.md; agent-rules.md:12) | PASS | The removed `DeadlineTooLong` and cross-identity `Corrupt` assertions track removed/replaced behavior: the deadline test now asserts a 24 h caller deadline publishes, and cross-identity reads are asserted as `ForeignKey` in `identities_cannot_read_or_advance_each_others_snapshots`. |
| Source never persisted (AGENTS.md; agent-rules.md:9) | PASS | `IngestStats::archive_error` is error text only and is not serialized; `same_tree_serializes_identically_across_ingest_paths` asserts ingest fields are absent from the object. |

## 3. Findings

### REPO-1 — Stateful storage workflows exposed as free functions

- **Rule:** `AGENTS.md:27` "Use free functions only for stateless transformations"; `architecture/agent-rules.md:10` "Use concrete owners and inherent methods for stateful workflows … Pure deterministic helpers may remain free functions."; `architecture/references/languages/rust-core.md:3` "Use a free function only when its result is stateless and deterministic from arguments alone."
- **Location:** `crates/bohmian-code-review-pipeline/src/snapshot.rs:278` `pub fn collect_snapshots(store, storage, before, limit)`, which is new. It lists records, deletes storage objects, and deletes persistence rows: a multi-step mutating IO workflow.
- **Root cause and sibling:** the snapshot read/cleanup surface was built as free functions over injected `&dyn` stores and not as methods on a concrete owner of those stores. Sibling: `snapshot.rs:239` `pub fn read_snapshot(storage, identity, pointer)`. That function was materially modified in this diff (the exact-key `ForeignKey` gate was added) and performs storage IO, so its result is not "deterministic from arguments alone". `ColdBuild` already owns `storage` and `store` as a concrete struct.
- **Consequence:** the public API now has two IO workflows as free functions, and each caller has to re-thread the storage/store pair. Cleanup becomes a third public shape next to the `ColdBuild` owner, which the written owner rule forbids and later stages (PR overlay, context read-back) would copy.
- **Correction (testable):** move both onto a concrete owner of the injected stores, either inherent methods on `ColdBuild` or a small struct holding `&dyn ObjectStorage` + `&dyn SnapshotStore`, and re-export that owner instead of the functions. Check: `rg -n '^pub fn (read_snapshot|collect_snapshots)' crates/bohmian-code-review-pipeline/src/snapshot.rs` returns nothing, `lib.rs` re-exports no free IO function, and the existing tests (`collection_removes_only_expired_unreferenced_snapshots`, `identities_cannot_read_or_advance_each_others_snapshots`, the reuse/corruption tests) pass through the owner.

### REPO-2 — Path-qualified external type in a new signature

- **Rule:** `AGENTS.md:31` "Keep imports at module top and use imported bare types in signatures."; `architecture/agent-rules.md:11`.
- **Location:** `crates/bohmian-code-review-pipeline/src/snapshot.rs:646` `fn stream_entries<R: Read>(&mut self, tar: &mut tar::Archive<R>) -> io::Result<()>`, a new item.
- **Root cause and sibling:** `tar::Archive` collides by name with the imported `crate::github::Archive`, so the signature falls back to a crate path. Sibling: the body use `tar::Archive::new(expanded)` at the `consume_archive` call site. `io::Result` in the same signature uses the already-imported `io` module. That is a narrower question and is not required by this finding.
- **Consequence:** it breaks the written signature convention, and at the signature it is ambiguous which `Archive` is meant.
- **Correction (testable):** add a top-level import alias (e.g. `use tar::{Archive as TarArchive, EntryType};`) and use `&mut TarArchive<R>` in the signature and `TarArchive::new` at the call site. Check: `rg -n 'tar::Archive' crates/bohmian-code-review-pipeline/src/snapshot.rs` matches only the `use` line.

### REPO-3 — Agent-mode marker in production code and public rustdoc

- **Rule:** `AGENTS.md:44` "Keep plan/task/agent references out of production code."
- **Location:** `crates/bohmian-code-review-pipeline/src/snapshot.rs:268`. This is a new `/// ponytail: …` paragraph in the public rustdoc of `collect_snapshots`, so it renders in published docs.
- **Root cause and sibling:** `ponytail:` is the marker of an agent coding persona/plugin mode, not a domain term, and it was carried into production doc comments. In-scope sibling: `crates/bohmian-code-review-pipeline/src/eligibility.rs:151` (`/// ponytail: no [class] support…`, from 741c049). Out-of-scope siblings exist in `bohmian-code-review-select/src/lib.rs:477` and `src/diff.rs:388`; they are not findings of this review. The TASK-003 markdown also cites the marker, which is fine because it is not production code.
- **Consequence:** an agent-tool reference ships in the crate's public API docs. Readers get a limitation labelled with tooling jargon when it should read as a stated contract.
- **Correction (testable):** keep the substance and drop the agent marker. In the rustdoc, restate the race as a plain limitation (e.g. a "Concurrency" paragraph) and the glob gap as "`[class]` patterns are not supported". Check: `rg -n 'ponytail' crates/bohmian-code-review-pipeline/src` returns nothing.

Note: REPO-3 depends on reading `ponytail:` as an agent reference under `AGENTS.md:44`. If the maintainer has adopted it as a repository comment convention, record that decision in `AGENTS.md` and withdraw the finding.

## 4. Overall

**FAIL.** There are three standards findings (REPO-1, REPO-2, REPO-3). None affects runtime correctness. REPO-1 is the most significant because it fixes the public API shape.

```citations
[
  {"id": "REPO-1", "path": "crates/bohmian-code-review-pipeline/src/snapshot.rs", "line": 278, "snippet": "pub fn collect_snapshots(", "symbols": ["collect_snapshots", "read_snapshot", "ColdBuild"]},
  {"id": "REPO-2", "path": "crates/bohmian-code-review-pipeline/src/snapshot.rs", "line": 646, "snippet": "    fn stream_entries<R: Read>(&mut self, tar: &mut tar::Archive<R>) -> io::Result<()> {", "symbols": ["stream_entries", "consume_archive"]},
  {"id": "REPO-3", "path": "crates/bohmian-code-review-pipeline/src/snapshot.rs", "line": 268, "snippet": "/// ponytail: a build adopting an object between the store's listing and the", "symbols": ["collect_snapshots"]}
]
```
