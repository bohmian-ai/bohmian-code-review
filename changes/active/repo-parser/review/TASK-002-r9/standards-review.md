# Repository standards review — TASK-002 r9

## Subject and scope

Base `ab2fa3a`; candidate `e38c3ae165f8972037c6b823aa9c725782b4237c` (the checked-out HEAD). I reviewed the cumulative diff, but judged TASK-002 only against its Revision 12 delta and TASK-002-R6. The rest passed r7. For TASK-003's `snapshot.rs` cold-build internals I checked compilation, bounds, and the `ponytail:` buffering disclosure only. The `.codegraph/` index is absent.

## Authority coverage

| Changed surface | Applicable written authority | Evidence inspected |
|---|---|---|
| Workspace and pipeline manifests, `Cargo.lock` | `AGENTS.md` product/dependency and verification rules; `architecture/bohmian-code-review-design.md:20-22`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md` | `Cargo.toml`, pipeline manifest, lockfile, `mise.toml`, base-to-candidate manifest diff |
| Provider and transport (`github.rs`, removed `execution.rs`) | `AGENTS.md` Rust, outer-pipeline and source-lifetime rules; `architecture/agent-rules.md:3-8`; design `:20-22`; Rust core | Full changed provider flow, request policy, body readers, inline tests |
| Async injected traits and test adapters (`storage.rs`, `persistence.rs`, `testing.rs`, `lib.rs`) | `AGENTS.md` concrete-owner, trait, rustdoc, error and test rules; agent rules `:3-8`; design `:20-22`; Rust core and testing workflows | Trait methods, generic facade consumers, in-memory adapters, re-exports, tests |
| `snapshot.rs` consumer seam | User's narrow TASK-003 scope; `AGENTS.md` outer-pipeline and bounded-source rules; design `:20-22`; Rust core | `ColdBuild::run`, `publish`, `Ingest::consume_archive`, `Archive::chunk`, limits and `ponytail:` comment |
| Task and remediation evidence | `architecture/references/languages/spec-driven-development.md` review authority and immutable-subject rules; `architecture/agent-rules.md:13`; `AGENTS.md` lifecycle | Revision 12 task delta, R6 remediation, prior r7/r8 verdicts, candidate diff and observed checks |

## Rule results

| Written rule | Result | Source and verification evidence |
|---|---|---|
| Outer pipeline alone owns Git/provider and injected storage IO; symbolic crates remain IO-free (`AGENTS.md`, product boundaries; design `:20-22`) | PASS | Diff changes only the pipeline code and manifests; pipeline still depends on the public `bohmian-code-review` facade. No symbolic crate changed. |
| Async tokio pipeline with caller-owned drop cancellation and no detached tasks (design `:22`; task Revision 12) | PASS | `github.rs:1283-1323` awaits transport and `tokio::time::sleep`; no `tokio::spawn`, `Execution`, `CancelToken`, or `Interrupted` remains. Exact dropped-call test passed (1 test). |
| Source trust, bounded provider bodies, and no credential forwarding (`AGENTS.md`, pipeline boundary; `architecture/agent-rules.md:7`; design `:22`) | PASS | `github.rs:147-162` disables redirects and strips URLs from reqwest errors; `:1092-1104` checks archive redirect before the unauthenticated hop; `:1369-1384` bounds JSON/blob copies. `:827-839` slices a chunk before copying into the 4096-byte rate-limit prefix. Exact prefix test passed (1 test). The transport may materialize one chunk first, as R6 explicitly excludes. |
| State on cohesive concrete owners; traits only for real multiple implementations or injected boundaries (`AGENTS.md`, Rust rules; agent rules `:3`; Rust core) | PASS | `GitHubClient` owns provider policy, `ReqwestTransport` and test fake implement `HttpTransport`, and storage/persistence traits are the approved injection seams. `ColdBuild` remains a concrete generic facade. |
| Borrow and validate at trust boundaries; typed library errors and no unchecked input/IO unwrap (`AGENTS.md`, Rust rules; agent rules `:4-5`; Rust core) | PASS | Provider validates identities and paths in existing flow; async port retains `GitHubError`, `TransportError`, `PersistenceError`, and `StorageError`. Changed production paths use typed errors; no new unchecked `unwrap` on input or IO was found. |
| Useful rustdoc, `# Errors` for fallible items, imports at top and bare types (`AGENTS.md`, Rust rules; agent rules `:9`; Rust core) | PASS | New transport/body methods, async trait methods, prefix helper and tests have intent docs; fallible methods include `# Errors`. Imports remain at module tops. The pre-existing qualified signature in TASK-003 `snapshot.rs` is outside this review's scoped judgment. |
| No prohibited production dependency or generated/grammar change (`AGENTS.md`, Rust rules; agent rules `:6`) | PASS | Manifest replaces `ureq` with requested `reqwest` and `tokio`; no prohibited SDK, database, generated artifact, or grammar edit. Forbidden-symbol sweep found no `ureq`, `async_trait`, cancellation types, or `tokio::spawn`. |
| Test placement and required lanes (`AGENTS.md`, tests; agent rules `:4,10`; testing workflows) | PASS | New provider tests are inline; the orchestrator observed `mise run gate` exit 0, including `test:pipeline`, formatting, lints, the symbolic journey, and skill sync. `git diff --check` exited 0. |
| Review immutable cumulative candidate and preserve prior passed scope (`architecture/agent-rules.md:13`; spec-driven development, review and remediation) | PASS | Candidate HEAD stayed `e38c3ae`; base-to-candidate diff was inspected. r7's non-delta TASK-002 pass was respected while R6 and Revision 12 were checked directly. |
| TASK-003 seam stays bounded and buffering is honestly marked (user scope; `AGENTS.md`, source lifetime; design `:22`) | PASS | `snapshot.rs:653-675` discloses the in-memory compressed archive buffer; `Archive::chunk` rejects after `max_archive_bytes` at `github.rs:594-605`, while `BoundedReader` limits expansion at `snapshot.rs:668-673`. Gate compiled this seam. Streaming behavior itself belongs to TASK-003. |

## Material findings

None.

## Overall result

**PASS** for repository standards in the requested scope. Verification used fake provider tests and the orchestrator's gate; no live GitHub/GHES contract run was performed.

```citations
[]
```
