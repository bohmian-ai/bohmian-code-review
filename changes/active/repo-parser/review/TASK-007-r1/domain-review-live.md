# TASK-007 domain review: real GitHub source and S20

**Subject:** `2257992f36a2aa88f33f397af1e070d6ab35961b..6e9ab0f764f9d7961924dac7fb1e5c04be02a38c`, approved specification revision 14 and `review/maintainer-decision-stateless-snapshot.md`. **Overall: PASS.** No material finding in this domain.

## Boundary and coverage

The range changes only TASK-007's record and four JSON evidence reports, and removes a superseded wyrd benchmark. I reviewed the complete range, then traced the existing S20 entrypoint through `ColdBuild::run`, `PrContext::run`, `Contexts::read`, and the `GitHubClient` repository, commit/tree, blob, archive, PR, PR-files, compare, redirect, and retry paths. Authorities: `AGENTS.md` (outer-crate source and storage IO, verified paths and source-free snapshot), `architecture/agent-rules.md` (trust boundary and credential-free gate), spec revision 14 REQ-004/005/012/013/017 and AC-006/007/011, TASK-007, and the stateless-snapshot maintainer decision.

| Obligation | Source and recorded evidence | Result |
|---|---|---|
| Verified repository and pinned source | `github.rs`: numeric repository ID and name, branch/commit/tree IDs, PR number/base repository ID, PR-file path/status, and fetched blob Git SHA are checked before extraction. `pull_request.rs`: PR base/head are read around pagination; changed-file SHA is compared with the pinned head tree. `snapshot.rs`: archive entries are matched to tree identities and missing entries fall back to verified blobs. | PASS |
| S20 real PR, useful context and read-back | `examples/live_pr.rs` invokes the production cold-build, PR-context and read facades with in-memory stores, bounds each call, prints identity/coverage/omissions/spans and fails on empty spans or unequal read-back. `s20-wyrd-pr1.json` records PR #1, base/head IDs, a published complete cold snapshot, repository scope, 50 spans, object key/digest/context ID and `read_back_equal: true`. Its eight changed files marked `NotEligible` and 277 `FileLimit` omissions remain visible. | PASS |
| Visible failure | The task records nonzero results for missing token, missing PR and wrong repository ID. `s20-wyrd-pr91.json` records zero spans and exhausted budget; `live_pr.rs` returns an error after printing that report. | PASS |
| Complete large-repository snapshots and warm reuse | The refreshed wyrd, vscode and rust JSON all report published cold snapshots, with eligible = indexed + `NoFacts` skips (1,681; 14,691; 39,496 respectively). Their context runs use repository scope; vscode honestly reports incomplete PR overlay after its change ceiling, while wyrd and rust report full context coverage. Three warm invocations are recorded per repository; the call totals include two archive requests for the one cold build's redirect and none attributable to warm PR calls. | PASS |
| Provider contract | The source restricts archive redirects to trusted HTTPS archive hosts and strips credentials on the redirect; JSON and blob reads are bounded, 403/429 handling is typed and bounded. The task records live archive redirect, recursive tree splitting, blob verification, compare and PR patch omission observations, plus local fake tests for throttling and GHES variants. | PASS with limits below |

## Verification limits

The checked-in JSON is a record of earlier credentialed runs, not a replayable live proof. I checked its internal consistency with `jq` (all three cold outcomes published; wyrd #1 has nonempty spans, repository scope, full coverage and equal read-back; wyrd #91 has zero spans and exhausted budget). I did not rerun credentialed S20, provoke a real 403/429, or access a GHES instance. The task describes a `gh api -i` header capture, but the response headers and shell exit codes are not preserved as separate artifacts. The client sends no conditional requests, so conditional-response behavior has no reachable production path to validate. The task explicitly states these bounds; they do not undermine the recorded successful S20 run or the locally tested policy paths.

## Proposed findings

None.

```citations
[]
```
