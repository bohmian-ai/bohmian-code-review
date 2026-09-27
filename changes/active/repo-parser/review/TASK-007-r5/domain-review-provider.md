# Provider and real PR domain review

## Subject and boundary

Reviewed immutable `2257992f36a2aa88f33f397af1e070d6ab35961b..317b2da9a8cd45755418dca05111c96beb19eaa4` against approved SPEC-repo-parser revision 14, TASK-007, R1–R4 remediation, and the stateless-snapshot maintainer decision. This review covers the production GitHub client and transport reached by the controlled provider command, the fixed-commit and fixed-PR live entrypoint, provider evidence, and the R4 offline fixture's lifetime. No `.codegraph/` index exists.

## Authority and source coverage

Read `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, the applicable spec-driven development reference, the approved spec and tasks, prior provider findings and verdicts, and the complete cumulative source diff. Inspected `examples/provider_contract.rs`, its durable `provider_contract_cases.json`, `examples/live_pr.rs`, `src/pull_request.rs`, `src/snapshot.rs`, `src/github.rs`, `mise.toml`, and the captured provider and PR reports. The large capacity reports were inspected for recorded identities, coverage, calls, and measurements, not line by line for every span.

The controlled command uses production `GitHubClient` and `ReqwestTransport` for repository, commit/tree, blob, archive, and PR-file requests. The client verifies repository, commit/tree and blob identities, handles archive redirects through trusted hosts without forwarding the bearer token, and bounds/retries provider calls. The command requires all seven named cases, an untruncated nonempty tree without invalid paths, redirect and gzip shape, a patchless PR file, a 304 conditional observation, and rate headers on public API exchanges before success. Its conditional request is explicitly an observation outside the client path. Local fake transport tests cover rate-limit and GHES variants; the live report records the controlled Wyrd request outcomes.

`live_pr.rs` reads one PR after the cold build and sends that base/head to every context sample. `PrContext::run_at` lists files, re-reads the same PR and refuses a changed pair before publication. `ColdBuild::run_at` resolves the caller's fixed commit and verified tree before using the ordinary snapshot build. The passing Wyrd PR #1 report records nonempty spans and equal read-back; the PR #91 report records an empty-context failure. The capacity rows record fixed cold commits and one base/head pair per PR. R4 moved the offline verdict fixture into the crate, so normal completion can remove `changes/active/` without invalidating its compile-time include; the real provider report remains review evidence.

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example provider_contract --all-features tests::every_named_case_decides_the_verdict -- --exact` passed one selected test at this candidate. Source inspection confirms that its only compile-time include resolves to `examples/provider_contract_cases.json`, and the test still breaks or removes every required case in turn. Prior focused moved-PR and fixed-commit tests and the checked-in gate/live results are supporting evidence. I did not rerun credentialed GitHub calls or large capacity measurements, so current remote behavior remains unverified. The live report does not claim actual quota exhaustion or GHES access.

## Material proposed findings

None. The provider trust, fixed-workload evidence, real PR entrypoint, and durable fixture satisfy the obligations within this boundary. Prior `FIND-TASK-007-1` (moving benchmark inputs), `FIND-TASK-007-2` (unchecked provider outcome), and `FIND-TASK-007-4` (transient test fixture) are closed here; `FIND-TASK-007-3` is outside this domain except that the example test now has item rustdoc.

**Result: PASS** for this domain.

```citations
[]
```
