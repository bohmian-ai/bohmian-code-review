# TASK-009 domain review: provider bounds and incomplete coverage

**Subject:** `597f5180808b78e1b104c10c1a375d5604b89a2e` → `ef48e308838fd081561754059e78a3814f03cf8a`; `SPEC-repo-parser@14`. **Result: PASS** within this boundary; no material proposed findings.

## Boundary and authority

I traced `GitHubClient::tree` through `list_into` and `tree_wire` to `get_json`, `send_with_policy`, and both callers: `ColdBuild::start` and `PrContext::run`. I then traced cold-build publication through `ColdBuild::complete`/`publish`, and PR-context coverage through `Ingest::reuse`, `Ingest::fallback`, `ContextCoverage::is_full`, and context publication. The controlling obligations are spec REQ-005, REQ-006, REQ-008, REQ-009, REQ-013, INV-002/003; TASK-009 scenarios and its material stop conditions; and the repository's outer-pipeline ownership and caller cancellation rules.

The recursion is sequential: each subtree call is awaited in `list_into` (`github.rs:1024-1050`), with no detached work. Every tree request uses the existing bounded JSON reader and per-request retry/rate-limit policy (`github.rs:1062-1079,1263-1273,1332-1372`). Dropping the facade future also drops the outstanding tree request or retry wait. The changed path adds no concurrent request fanout. The number of calls grows with the number of subtrees that must be split; it has no new aggregate call ceiling, consistent with REQ-005's complete-tree requirement and REQ-013's caller-owned invocation duration. Whether that fits the real large-repository provider budget remains unmeasured here.

If a non-recursive listing is truncated, `list_into` marks the joined listing incomplete (`github.rs:1031-1035`). Cold build returns `Incomplete(TruncatedTree)` before archive ingest or any snapshot write (`snapshot.rs:680-706`); the only pointer publication path is later in `complete` (`snapshot.rs:752-789`). PR context may still publish useful context, but sets `files.truncated_tree` from the listing, making `overlay_complete` and `is_full()` false (`pull_request.rs:241-255`; `context.rs:64-74`). With a loaded snapshot, `Ingest::reuse` carries only paths and blob identities actually present in the head listing (`snapshot.rs:1051-1075`), so omitted subtree facts do not survive as apparent current facts. Without a snapshot, the scoped path remains explicitly scoped and incomplete.

## Verification and limits

I ran `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib truncated`: **2 passed**, covering split root/nested listings against an untruncated snapshot oracle and an unsplittable listing with no snapshot publication. Existing PR scoped-fallback behavior is source-traced, not independently rerun. No live GitHub/GHES or `rust-lang/rust` capacity run was available in this review. TASK-009 assigns that live row to TASK-007's later rerun, so actual subtree call count, rate-limit behavior at large scale, and provider compatibility remain verification limits rather than inferred successes.

```citations
[]
```
