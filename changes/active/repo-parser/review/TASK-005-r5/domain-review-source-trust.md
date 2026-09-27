# TASK-005 r5 source trust review

**Subject:** `fa4fa2101950626a9fe60b0fdff0439b04c16f01..866f777a1a41a03384c14c12276a5877dcac94a2`. **Result: PASS.**

## Boundary, authority, and source coverage

Reviewed the complete cumulative diff and traced provider identities, tree and PR-file validation, file eligibility, root attributes, blob byte bounds and SHA verification, snapshot ingest/reuse, update publication, PR pinning, changed-head reconciliation, scoped selection, candidate fetch, and stored span identities. Sources include `github.rs`, `eligibility.rs`, `snapshot.rs`, `update.rs`, `pull_request.rs`, their focused fake-provider tests, and the symbolic `parse_hunks` owner. Governing authority: approved `spec.md` revision 13 (eligible-file definition; REQ-004–006, 008–010, 013–014; INV-002–003), original TASK-005, R1–R4 remediation, `AGENTS.md`, `architecture/agent-rules.md`, and `architecture/bohmian-code-review-design.md`. The r3 maintainer decision replaces FIND-4's rejected-tree-entry correction; the r4 maintainer decision replaces FIND-8's correction shape and withdraws FIND-7. I applied those decisions ahead of the remediation text.

`GitHubClient::tree` rejects an unknown mode or malformed SHA as a provider error and counts noncanonical paths as unsupported exclusions. PR-file rejection stays file-local, lowers full coverage, and a canonical rejected changed path is excluded from secondary selection. `PrContext::pin` compares base/head before and after paginated files; each accepted changed file must match the pinned head tree's blob identity before its patch is used. Blob fetching enforces the smaller of the invocation file ceiling and provider ceiling before returning bytes, then verifies the Git blob SHA. Snapshot keys include the file ceiling, preventing reuse of facts admitted under a larger one. The shared root-policy loader reads both `100644` and `100755` `.gitattributes` blobs and feeds cold, update, warm overlay, and scoped classification. I found no material source-trust finding in this boundary.

## Verification limits

The parent review independently reported `mise run gate` and `git diff --check fa4fa2101950626a9fe60b0fdff0439b04c16f01 866f777a1a41a03384c14c12276a5877dcac94a2` passing. I inspected focused fake-provider tests for the executable root policy, understated tree size and file ceiling, provider ceiling distinction, rejected PR files, and broken tree entries. I did not run a live GitHub/GHES request or mutate the candidate. This review covers source trust only; separate reviewers own context durability, resource accounting, and task-wide acceptance.

## Material findings

None.

```citations
[]
```
