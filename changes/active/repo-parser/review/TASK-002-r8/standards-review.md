# TASK-002 Revision 12: repository standards review

**Subject:** `ab2fa3a..3979b427ed52e298f5fa88ef0c00bb324f0acc6b`; approved spec revision 12; TASK-002 Revision 12 delta only. TASK-002-r7 passed the preceding work. `snapshot.rs` cold-build internals receive only the requested compile, bound, and buffering-honesty check. No source was edited.

**Overall: FAIL** (one material rule finding).

## Authority coverage

| Changed surface | Applicable authority | Coverage |
|---|---|---|
| `Cargo.toml`, `Cargo.lock`, pipeline manifest | `AGENTS.md:25-33,35-39`; `architecture/bohmian-code-review-design.md:20-22`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md` | Dependency direction, prohibited dependencies, pinned lanes and test dependencies inspected. |
| `github.rs`, deleted `execution.rs`, `lib.rs` | `AGENTS.md:23,25-33`; `architecture/agent-rules.md:3,9-13`; `architecture/bohmian-code-review-design.md:22`; Rust core and testing references | Transport, cancellation removal, bounds, error handling, rustdoc, inline tests and public exports inspected. |
| `storage.rs`, `persistence.rs`, `testing.rs` | `AGENTS.md:23,25-33`; `architecture/agent-rules.md:3,7-9`; `architecture/bohmian-code-review-design.md:22`; Rust core reference | Async traits, `Send` futures, in-memory implementations, owner boundaries, rustdoc inspected. |
| `snapshot.rs` | `AGENTS.md:23,25-33`; `architecture/bohmian-code-review-design.md:22`; Rust core reference | Only compile closure, compressed and expanded byte ceilings, and the `ponytail:` buffering disclosure assessed, per user scope. |
| Task packet and verification | `architecture/references/languages/spec-driven-development.md`; `AGENTS.md:35-43`; `architecture/agent-rules.md:10-13`; testing reference | Revision 12 task criteria and named lanes inspected; orchestrator owns execution results. |

## Rule results

| Written rule | Evidence | Result |
|---|---|---|
| Pipeline alone owns provider/storage IO, consumes public symbolic facade, and has bounded ingest (`AGENTS.md:21-23`; design `:20-22`) | Pipeline manifest and `github.rs`; archive chunk checks limit before returning; `snapshot.rs:656-675` accumulates only successful bounded chunks, then uses `BoundedReader` for expansion. `names_rate_limit` copies a whole untrusted chunk before truncating it. | **FAIL**, S-1 |
| Concrete stateful owners and justified traits (`AGENTS.md:27`; agent rules `:8`) | `GitHubClient`, `ReqwestTransport`, `ColdBuild` own state; transport/body traits have production and test implementations; storage traits are approved injection seams. | PASS |
| Borrowed values and domain paths (`AGENTS.md:28`; Rust core) | Provider paths continue through `RelativePath`; changed trait signatures borrow keys and byte slices. | PASS |
| Typed errors, no unchecked input/IO `unwrap`, and imports at top (`AGENTS.md:29-30`; agent rules `:9`) | Changed production paths use `GitHubError`, `StorageError`, `PersistenceError`; imports remain module-top. `BoundedReader`'s `unwrap_or` has an explicit fallback, and test `expect`s name invariants. | PASS |
| Rustdoc on changed items and `# Errors` (`AGENTS.md:31`; agent rules `:9`) | New transport/body API, async methods, trait methods and changed tests have intent docs; fallible items document errors. Snapshot buffering is explicitly disclosed at `snapshot.rs:650-656`. | PASS |
| No prohibited runtime dependency, grammar/fixture change or symbolic IO (`AGENTS.md:21,32-33`; agent rules `:3-6`) | Diff changes only the outer crate's HTTP/runtime dependencies and lockfile; no grammar pins, fixtures or symbolic crates changed. | PASS |
| Inline owner tests; no weakening (`AGENTS.md:35-39`; agent rules `:4,10`; testing reference) | `github.rs` and `snapshot.rs` owner tests remain inline; no `#[allow]`, `#[ignore]` or altered golden in the changed source. Focused and lane command results are pending orchestrator verification. | PASS on source; execution pending |
| No detached task; drop-controlled async IO (design `:22`) | No `tokio::spawn`; retry wait is `tokio::time::sleep` inside the caller future. Async store methods return `Send` futures. | PASS |

## Finding

### S-1 — Prefix buffer copies an entire untrusted chunk before enforcing its limit

- **Classification:** VIOLATION. **Defect class:** bound applied after allocation. The owner's local buffer logic permits it; no public API shape causes it.
- **Rule:** `AGENTS.md:23` assigns bounded ingest to the pipeline; `architecture/bohmian-code-review-design.md:22` repeats bounded ingest. The changed function also promises a 4096-byte read at `github.rs:813-814`.
- **Root cause and location:** `crates/bohmian-code-review-pipeline/src/github.rs:819` extends the prefix buffer with all bytes of one `HttpBody::chunk()`, then truncates at line 823. `HttpBody` permits any chunk length. At the base commit, `Read::take(4096).read_to_end` bounded this copy before allocation; the async port removed that limit at the copy point.
- **Reachable consequence:** A 403/429 response without rate-limit headers can deliver a large first chunk. The transport has already allocated that chunk, and the client makes an additional full-size copy of it while classifying the response, despite its 4096-byte prefix budget. A fake transport can reproduce this by returning one large body chunk. The request body transfer has a time bound, but no byte bound at this additional copy point.
- **Sibling sweep:** `read_bounded` at `github.rs:1357-1371` checks the chunk against remaining capacity before extending. `Archive::chunk` at `github.rs:594-605` rejects an over-limit chunk before returning it; `snapshot.rs:656-675` only appends accepted archive chunks. No same-pattern sibling found in the Revision 12 delta.
- **Testable correction:** Extend only the first `RATE_LIMIT_BODY_BYTES - body.len()` bytes of a chunk. Add a focused fake-body test where the first 403 chunk exceeds the prefix ceiling and verify classification using only that prefix. This restores the existing bound without a new abstraction.

## Verification limits

The orchestrator is running the cargo and mise checks; this report does not claim their results. The review did not assess TASK-003 cold-build behavior beyond the user's stated bounds. No live GitHub or GHES call was made.

```citations
[{"id":"S-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":819,"snippet":"            Ok(Some(chunk)) => body.extend_from_slice(&chunk),","symbols":["names_rate_limit","RATE_LIMIT_BODY_BYTES"]}]
```
