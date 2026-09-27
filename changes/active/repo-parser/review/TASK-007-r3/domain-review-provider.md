# Provider domain review

## Subject and boundary

Base `2257992f36a2aa88f33f397af1e070d6ab35961b`, candidate `b721917420113ab2e3530966e9e456eecc89dcff`. Reviewed the GitHub provider contract and source trust path for TASK-007 and TASK-007-R2 against approved spec revision 14, especially REQ-004, REQ-017, and AC-006. The stateless-snapshot maintainer decision does not change this provider boundary.

## Authority and source coverage

Read `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, the applicable spec-driven development reference, TASK-007, TASK-007-R2, and the maintainer decision. Inspected the complete base-to-candidate changed provider surfaces (`examples/provider_contract.rs`, `mise.toml`, captured provider report and task evidence), their production `GitHubClient`/`ReqwestTransport` path in `src/github.rs`, and the local scripted provider tests. No `.codegraph/` index exists.

The opt-in command uses the production client and transport. Repository ID and name, commit SHA, tree SHA, and blob bytes are checked by the client before the command records success. Archive redirects stay on trusted hosts and carry no forwarded bearer token. The contract command makes each named observed case decisive for exit status, including tree completeness, gzip archive redirect/shape, patch omission, conditional 304, and five rate headers on API responses. The captured report has seven cases and records their passing outcomes at the pinned Wyrd commit. Local fake tests cover 403/429 retry and header variants plus GHES API and archive hosts.

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example provider_contract tests::every_named_case_decides_the_verdict -- --exact` passed: 1 test. I inspected the captured live report and its documented invocation; I did not repeat a credentialed GitHub call, so current remote behavior is a verification limit. The live 403/429 and GHES cases remain local fake transport evidence, as the task allows.

## Material proposed findings

None. No source-trust or AC-006 failure was established from the reviewed candidate and evidence.

**Result: PASS** for the provider domain.

```citations
[]
```
