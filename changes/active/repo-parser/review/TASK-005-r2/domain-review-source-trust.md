# Source trust domain review — TASK-005 r2

**Subject:** `fa4fa2101950626a9fe60b0fdff0439b04c16f01` → `31e71a2e8867215e0c3f8958bdcee5f94da17ad4`
**Result:** PASS

## Boundary, authority, and source coverage

Reviewed the cumulative diff and revision 12 spec, original TASK-005, R1 remediation, `AGENTS.md`, `architecture/agent-rules.md`, and the pipeline boundary in `architecture/bohmian-code-review-design.md`. Traced provider tree and PR-file validation through `GitHubClient::{tree,pull_request_files}`, eligibility classification, `ColdBuild::{start,complete}`, `Ingest::{new,reuse,finish}`, `Update::run`, `PrContext::{run,pin,changed_head}`, `ContextCoverage::is_full`, snapshot read/publication, selected head-blob spans, and context read integrity. The relevant contracts are REQ-004–006, REQ-008–011, INV-002–003, and R1 FIND-TASK-005-4.

The FIND-4 change is within those contracts. A rejected tree entry has no fully validated path, mode, and object identity, so the remaining valid entries cannot establish that every eligible file was indexed. `Coverage::unindexed` counts `InvalidEntry`; `ColdBuild::complete` returns `Incomplete` before publication; `Update::run` uses the same completion boundary. This can keep a stable malformed default-branch tree on scoped fallback indefinitely, but REQ-006 permits publication only after completeness is proved, and REQ-008 provides that fallback. Treating an unknown entry as a benign exclusion would risk a false full-coverage claim under INV-002. The R1 remediation explicitly requires rejected tree entries to lower snapshot or overlay coverage unless validated facts prove ineligibility. Validated unsupported paths continue as benign exclusions.

For PR context, rejected PR-file records retain `NotAnalyzed::Rejected` and increment `pr_files_rejected`; `is_full` then fails even if the snapshot and overlay are otherwise complete. In repository scope, rejected head-tree entries increment `InvalidEntry`, making `overlay_complete` false. The focused tests cover both paths and a valid-source continuation. In scoped mode, `PrContext::run` indexes nearby validated entries and does not carry the head listing's global rejection count into its *scoped-set* exclusion totals; `Scope::Scoped` and `snapshot_complete=false` still preclude full PR-head coverage. I found no source-trust or named acceptance failure from that scoped-set accounting choice.

## Verification limits

This is a read-only source trace. I did not run a live GitHub/GHES call or add a new test. The candidate's fake-provider tests exercise rejected PR-file and head-tree entries; the original and remediation task records report the pipeline, journey, format, lint, and gate lanes passing. This report does not treat those recorded commands as independently rerun.

## Material findings

None.

```citations
[]
```
