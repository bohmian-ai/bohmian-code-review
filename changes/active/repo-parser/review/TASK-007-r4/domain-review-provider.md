# Provider and live PR domain review

## Subject and reviewed boundary

Base `2257992f36a2aa88f33f397af1e070d6ab35961b`; candidate `7f7b75d5d8645f5dd2530779fb6e146dc122efe1`. Reviewed the cumulative TASK-007/R1/R2/R3 implementation at the real GitHub provider and source-trust boundary, including S20 and the fixed-workload capacity path. No `.codegraph/` directory exists.

## Authority and source coverage

Read `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, the applicable spec-driven, Rust, and testing references, approved spec revision 14, original TASK-007, all three remediation tasks, and `review/maintainer-decision-stateless-snapshot.md`. Inspected the full base-to-candidate changed source and evidence, especially `examples/provider_contract.rs`, `examples/live_pr.rs`, `src/pull_request.rs`, `src/snapshot.rs`, `mise.toml`, the provider report, and S20/capacity reports. Traced their calls into `GitHubClient`, `ReqwestTransport`, and the existing provider verification path.

The controlled command reaches the production `GitHubClient` and `ReqwestTransport`. Its repository, commit, tree, and blob requests are checked by the client; the blob is SHA verified. Archive redirects are accepted only on trusted archive hosts and followed without the bearer token. The command's success check requires all seven named cases, a nonempty untruncated tree with no invalid paths, redirect then gzip archive, omitted PR patch, conditional 304 observation, and rate headers on every public API response. The conditional probe is expressly labeled a provider observation, since the production client does not issue conditional requests. The captured Wyrd report records passing values at commit `2855eac00a15581cede6e29ec5a0dde0b8bc4d48`. Existing fake transport tests cover 403/429 and GHES variants.

The S20 entrypoint uses `ColdBuild::run_at` for an optional fixed commit, reads one real PR, and sends that same PR base/head to every `PrContext::run_at` call. That method lists files and re-reads the PR before publishing; a moved base/head fails without publication. The checked-in passing PR #1 report records nonempty spans, complete coverage, read-back equality, one base/head pair, provider calls, and timing. The PR #91 report correctly fails on an empty context.

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example provider_contract tests::every_named_case_decides_the_verdict -- --exact` passed (1 test). `mise exec -- cargo test --locked -p bohmian-code-review-pipeline samples_at_one_read_pr_refuse_a_moved_head` passed (1 test). I inspected the checked-in live reports and their invocations but did not repeat a credentialed GitHub run; current remote behavior is therefore a verification limit. Live quota-exhaustion and GHES tests are not claimed; their local fake coverage is the recorded proof.

## Material proposed findings

None. The reviewed source and available evidence establish no provider trust or real PR contract failure introduced by this candidate.

**Result: PASS** for this domain.

```citations
[]
```
