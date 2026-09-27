# Repository standards review — TASK-005 r2

Subject: base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`, candidate `31e71a2e8867215e0c3f8958bdcee5f94da17ad4` (HEAD at inspection). Reviewed the cumulative diff, not only the remediation commits. No `.codegraph/` index exists. The approved spec and task inform scope; findings here are limited to written `AGENTS.md` and `architecture/` rules.

## Authority coverage

| Changed surface | Applicable authority | Result and evidence |
|---|---|---|
| `context.rs`, `persistence.rs`, `testing.rs` context owner and stores | `AGENTS.md:23,27–31,37–39`; `architecture/agent-rules.md:3,5,9–11`; design, crate direction and public behavior; Rust core and testing workflow references | PASS. Contexts owns publication/read/cleanup; storage and persistence stay injected; source is stored in selected context object only. Identity-scoped, expiry, corrupt-object, retry and cleanup tests are inline. Public symbolic journey remains present. |
| `pull_request.rs`, `lib.rs`, `github.rs` PR facade and provider seam | `AGENTS.md:23,27–33,37–39`; `architecture/agent-rules.md:3–5,7,9–11`; design, crate direction and public behavior; Rust core and testing workflow references | PASS. Outer pipeline owns provider IO and calls the public symbolic facade for ranking/packing. Validated paths and SHAs precede source use. The inline PR journey covers concurrent heads, scoped fallback, pinning, budgets, publication and read-back with fake provider and stores. |
| `snapshot.rs` ingest/coverage and cold publication | `AGENTS.md:23,28–32,37–39`; `architecture/agent-rules.md:7–9,11–12`; design, source-free snapshot and coverage rules; Rust core and testing workflow references | PASS. Rejected provider entries are recorded as `InvalidEntry`; `Coverage::is_complete` prevents their unknown facts from producing a full-coverage pointer. Existing eligible facts still proceed. The implementation has a focused cold-build test at `snapshot.rs:2310–2344`. |
| `TASK-005` and r1 change packet evidence | `AGENTS.md:41–45`; `architecture/agent-rules.md:13–14`; spec-driven development and implementation execution references | PASS. Approved spec, original task and remediation are tracked; candidate source has no task/agent references. No generated mirror or fixture edit appears in the diff. |
| Manifest, grammar, tooling and generated boundaries | `AGENTS.md:31–33,39`; `architecture/agent-rules.md:6,12–13`; testing workflow reference | PASS. `Cargo.toml`, `Cargo.lock`, grammar pins and generated artifacts are untouched. `git diff --check base candidate` exits 0. Task evidence records `mise run test:pipeline`, `test:symbolic-context:journey`, `fmt:check`, `lints` and `gate` passing; this review did not rerun Cargo lanes. |

## Applicable rules

| Rule | Result | Evidence |
|---|---|---|
| Pipeline-only Git/storage IO; symbolic crate ownership and no duplicate symbolic stage (`AGENTS.md:14–23`; `architecture/agent-rules.md:3,9`) | PASS | All production edits are in `bohmian-code-review-pipeline`; `pull_request.rs:415–459,471–494` delegates ranking and packing through `bohmian_code_review`. |
| Validated boundaries, source lifetime and honest coverage (`AGENTS.md:23,28`; `architecture/agent-rules.md:7–9`) | PASS | `github.rs:946–987,1374–1393` validates tree entries; `snapshot.rs:130–151,637–674` blocks incomplete publication; `context.rs:60–70` requires every coverage axis for `is_full`. |
| Concrete owners, borrowed inputs, errors, imports and rustdoc (`AGENTS.md:27–31`; `architecture/agent-rules.md:10–11`) | PASS | `PrContext`, `Contexts`, `ColdBuild` and `Ingest` own workflows. No new production unchecked `unwrap`; added fallible methods have `# Errors`. Existing `expect` calls inspected in touched files are test/invariant uses. |
| Inline tests, real journey, fast lane and no weakened gate (`AGENTS.md:37–39`; `architecture/agent-rules.md:4–5,12`; testing workflows `:3–15`) | PASS | PR owner tests invoke fake GitHub and real symbolic facade, compare with a fresh parse oracle, and cover negative flows. Symbolic public journey and mise targets remain intact; no new `#[allow]` or `#[ignore]`. |
| Lifecycle and Git constraints (`AGENTS.md:41–45`; `architecture/agent-rules.md:13–15`) | PASS | Candidate adds task/review evidence without changing generated mirror, identity, or publication surface. |

## Findings

None under written repository rules. The rule to avoid reporting partial coverage as full supports conservatively treating an unclassified rejected entry as incomplete. Whether every rejected entry should instead be classified against the approved eligibility contract is a task/spec acceptance question for the other reviewers, not a repository-standards violation by itself.

Overall: **PASS**. Verification limit: this report inspected source and recorded gate evidence; it independently ran only `git diff --check`.

```citations
[]
```
