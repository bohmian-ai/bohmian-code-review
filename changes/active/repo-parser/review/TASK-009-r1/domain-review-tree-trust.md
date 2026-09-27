# Tree listing and source trust domain review

**Subject:** `597f5180808b78e1b104c10c1a375d5604b89a2e` → `ef48e308838fd081561754059e78a3814f03cf8a`; `SPEC-repo-parser@14`; `TASK-009`. **Result: FAIL.**

## Boundary and coverage

Reviewed the complete candidate diff in `github.rs`, `snapshot.rs`, and `pull_request.rs`; `GitHubClient::tree`/`tree_wire`/`list_into`, path and SHA validation, `ColdBuild::start`/publication, `PrContext::run`/head verification, `Ingest::reuse`, and eligibility. Authorities: spec REQ-004, REQ-005, REQ-008, REQ-009, REQ-013, INV-002, INV-003; task scenarios and material stop conditions; `AGENTS.md` product boundary. The requested tree SHA is checked on every response. Split subtree paths are prefixed and validated. Cold build refuses a non-recursively truncated listing before publication. PR context marks such a listing's overlay incomplete. I found no separate path/mode/blob substitution finding.

## Proposed finding

### TREE-1 — Split listing has no invocation-wide resource budget

- **Classification / defect class:** VIOLATION; unbounded aggregate resource use in a newly introduced recursive traversal. The owner shape is a growing `TreeListing { entries: Vec<_> }` filled by `list_into` with no traversal-wide budget or bounded output representation. This permits the defect across both facade callers.
- **Obligation:** REQ-013 requires bounded GitHub calls and memory; TASK-009's material stop condition explicitly calls out subtree-call feasibility within that budget. REQ-005 needs complete listing without treating repository size alone as a scoped-fallback trigger.
- **Root owner and siblings:** `GitHubClient::tree` creates the accumulating listing (`github.rs:1000-1007`); `list_into` appends each untruncated subtree and recursively requests every subtree of each truncated response (`github.rs:1024-1049`). The per-response JSON byte ceiling in `get_json` (`github.rs:1269-1273`) and per-request retries bound neither the number of successful tree calls nor the total entries retained. The resulting listing is consumed in cold build (`snapshot.rs:680`) and PR context (`pull_request.rs:163`), so both inherit the same fanout and memory exposure.
- **Reachable consequence:** A root that truncates and contains many subtrees can cause at least two calls per truncated node and one per other subtree, with aggregate entries spanning many bounded JSON responses. A caller timeout can end the operation, but it does not bound memory or provider calls before the drop; completion of a large tree therefore depends on unbounded resources despite the explicit task stop condition.
- **Testable correction outcome:** Make the tree traversal's resource ownership and completed/incomplete result shape enforce one invocation-wide budget while retaining the task's complete-listing behavior for supported large trees. Prove a broad/deep split tree cannot exceed that budget and reports honest incomplete coverage when it cannot finish. Do not add scattered caller guards.

## Verification limits

Source-path reproduction above; no focused resource-exhaustion test was run. The candidate's fake provider verifies a five-call split only (`snapshot.rs:2618-2641`). Its task evidence says the required live `rust-lang/rust` capacity rerun was not performed, so the stated real-repository subtree-call feasibility is unverified. No live GitHub call was made in this review.

```citations
[{"id":"TREE-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1000,"snippet":"        let mut listing = TreeListing {","symbols":["GitHubClient::tree","TreeListing"]},{"id":"TREE-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1024,"snippet":"        let (resource, wire) = self.tree_wire(repository, tree, true).await?;","symbols":["GitHubClient::list_into","GitHubClient::tree_wire"]},{"id":"TREE-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1049,"snippet":"                Box::pin(self.list_into(repository, &sha, &subprefix, listing)).await?;","symbols":["GitHubClient::list_into"]},{"id":"TREE-1","path":"crates/bohmian-code-review-pipeline/src/github.rs","line":1272,"snippet":"            read_bounded(response.body.as_mut(), self.limits.max_json_bytes, resource).await?;","symbols":["GitHubClient::get_json"]}]
```
