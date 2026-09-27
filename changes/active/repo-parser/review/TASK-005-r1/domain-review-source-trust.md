# Source trust domain review — TASK-005

**Subject:** `fa4fa2101950626a9fe60b0fdff0439b04c16f01` → `e68bcdb65b44095989b5f42dfda5c7d87295582b`
**Result:** FAIL

## Boundary and coverage

Reviewed the complete task diff for PR pinning, paginated file validation, head commit/tree/blob provenance, snapshot reuse and overlay, scoped fallback, candidate fetch, selected span identity, and shared read integrity. Authorities: approved `changes/active/repo-parser/spec.md` REQ-004, REQ-009–REQ-011 and INV-002; original TASK-005 scenarios 1–3; `AGENTS.md` product boundary; `architecture/agent-rules.md` honest coverage and source rules; `architecture/bohmian-code-review-design.md` outer pipeline ownership. Traced `PrContext::run`, `pin`, `changed_head`, `pack`, `Contexts::publish/read`, `GitHubClient::pull_request/tree/blob/pull_request_files`, `Ingest::new/reuse/fallback/finish`, and their nearest fake-provider tests.

The positive provenance chain is present: `pin` rereads base/head after pagination; the head commit and tree are fetched by SHA; a PR file SHA must equal the head tree's eligible entry before its patch is used; blob fetch verifies Git object SHA; reused facts require matching path/blob identity; selected spans receive their head-tree blob SHA. Shared reads check exact identity/key, expiry, digest, and context identity. No live-provider run was performed in this domain review; conclusions about malformed provider entries follow the validated parser and facade control flow.

## Finding SRC-1 — rejected PR files can still produce a full-coverage claim

- **Classification:** INCORRECT; violates REQ-009 and INV-002, which permit full PR-head coverage only when every changed file is accounted for.
- **Defect class and owner:** `ContextCoverage::is_full` uses the pagination ceiling flag but has no completeness signal for PR file entries rejected during page validation. Its shape permits a full-coverage result with unprocessed changed files.
- **Evidence and reachable path:** `GitHubClient::pull_request_files` places a file with an invalid SHA, status, path, or previous path in `rejected` and omits it from `files` (`github.rs:1144–1155`, `1395–1420`). `PrContext::pin` retains that rejection while still accepting an unchanged base/head (`pull_request.rs:310–328`). `run` records it as `NotAnalyzed` (`pull_request.rs:199–208`), but sets `pr_files_truncated` only from the page ceiling (`pull_request.rs:222–232`). With an otherwise complete snapshot and overlay, `ContextCoverage::is_full` returns true (`context.rs:60–65`). A PR with one valid changed file and one rejected `src/missing.rs` SHA therefore publishes a context that says full coverage while never analyzing the rejected file or excluding it from secondary candidates.
- **Sibling sweep:** All `validate_pull_request_file` rejection causes share this path: invalid canonical filename, unknown status, invalid SHA, and invalid previous filename. The `listing.rejected` tree-entry path is a related completeness risk: `run` passes its count to `Ingest::new` (`pull_request.rs:171–176`), while `Coverage::is_complete` checks only truncation and skips (`snapshot.rs:130–134`). The correction should ensure both rejected PR-file and rejected head-tree entries cannot yield `is_full()`, without changing treatment of ordinary excluded/unsupported files.
- **Observable consequence:** Consumers can treat a partial PR-head context as repository-complete and infer absence of matching definitions despite unprocessed provider entries. An invalid previous filename alone triggers the gap even when the head path and SHA are valid.
- **Testable correction:** Carry rejection completeness into the context coverage decision at the PR facade/coverage owner. A focused fake-provider case with one rejected PR entry, and another with a rejected tree entry, must retain the explicit rejection reason and report `is_full() == false`; valid entries must still produce selected context. Preserve the existing base/head retry and blob SHA checks.

## Verification limits

This was a read-only domain audit. The repository's existing fake-provider tests cover moving base/head, blob-local omissions, private overlays, and shared read, but do not exercise a rejected PR-file or tree entry reaching `is_full()`.

```citations
[{"id":"SRC-1","path":"crates/bohmian-code-review-pipeline/src/context.rs","line":60,"snippet":"    pub fn is_full(&self) -> bool {","symbols":["is_full"]},{"id":"SRC-1","path":"crates/bohmian-code-review-pipeline/src/pull_request.rs","line":199,"snippet":"        let mut not_analyzed: Vec<NotAnalyzed> = pinned","symbols":["run"]},{"id":"SRC-1","path":"crates/bohmian-code-review-pipeline/src/pull_request.rs","line":230,"snippet":"            pr_files_truncated: pinned.truncated,","symbols":["run"]},{"id":"SRC-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1145,"snippet":"            match validate_pull_request_file(&file) {","symbols":["pull_request_files","validate_pull_request_file"]},{"id":"SRC-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":132,"snippet":"    pub fn is_complete(&self) -> bool {","symbols":["is_complete"]}]
```
