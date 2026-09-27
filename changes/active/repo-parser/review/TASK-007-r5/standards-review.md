# TASK-007-r5 repository standards review

**Subject:** `2257992f36a2aa88f33f397af1e070d6ab35961b..317b2da9a8cd45755418dca05111c96beb19eaa4`, approved `SPEC-repo-parser@14`, original TASK-007, R1–R4 remediation, and `review/maintainer-decision-stateless-snapshot.md`. No `.codegraph/` index exists. This is a review of the complete cumulative diff, not only R4. Candidate HEAD matched `317b2da` during inspection.

## Authority coverage

| Changed surface | Applicable written authority | Inspection |
|---|---|---|
| `src/snapshot.rs`: fixed-commit cold build and test | `AGENTS.md:23,27-32,38-40`; `architecture/agent-rules.md:3-5,7-11`; `architecture/bohmian-code-review-design.md:20-22,36-38`; Rust core and testing-workflows references; spec REQ-004–006, REQ-013, AC-007; stateless-snapshot decision | `run`, `run_at`, `pin`, `build`, inline test, pointer/store seam and consumers inspected. |
| `src/pull_request.rs`: fixed-pair PR sampling and test | Same Rust, architecture, test rules; spec REQ-009–013, AC-007; symbolic-context reference | `run`, `run_at`, shared `publish`, `pin`, `list`, inline test and callers inspected. |
| `examples/live_pr.rs`: S20 and capacity entrypoint | `AGENTS.md:23,27-32,38-40`; `agent-rules.md:3-5,7,9-12`; design source and IO boundaries; testing-workflows reference; spec REQ-013–014, REQ-017–018, AC-007/011 | Config validation, bounded facade calls, report and failure exits inspected. |
| `examples/provider_contract.rs` and `provider_contract_cases.json`: live contract, verdict, offline fixture | Same Rust, test, source-trust rules; spec REQ-004, REQ-013–014, AC-006; spec-driven-development lifecycle | Recording/redaction, cases and verdict, fixture path and example test inspected. |
| `mise.toml`, crate/workspace manifests, `Cargo.lock` | `AGENTS.md:9,33-34,40`; `agent-rules.md:5-6,12-13`; testing-workflows reference | Opt-in lanes, normal gate graph, crate dependencies and lack of lockfile changes inspected. |
| Task, remediation, review and JSON evidence in cumulative diff | `AGENTS.md:42-46`; `agent-rules.md:13-15`; spec-driven-development and implementation-execution references; spec AC-006/007/011 | Active-packet lifecycle, R1–R4 diagnoses, task evidence, report schemas and stateless-snapshot decision inspected. Prior review verdicts were not treated as proof. |

## Rule checks

| Written rule | Result | Candidate evidence |
|---|---|---|
| Pipeline alone owns Git/storage IO and consumes the public symbolic facade (`AGENTS.md:14-23`; `agent-rules.md:3`; design:20-22) | PASS | All changed Rust is in `bohmian-code-review-pipeline`; its manifest depends on `bohmian-code-review`. No symbolic crate changed. |
| No duplicated symbolic stage, server/worker, or persisted source (`AGENTS.md:21-23`; `agent-rules.md:3,9`; design:20-22) | PASS | `ColdBuild::run_at` calls existing `build` (`snapshot.rs:640-654`); `PrContext::run_at` calls shared `publish` (`pull_request.rs:173-182`). Examples call public facades. The evidence JSON schemas carry identities, paths, spans, counts and timings; no source/body field. |
| Concrete workflow owners, borrowing and no speculative dependency (`AGENTS.md:27-28`; `agent-rules.md:10`; Rust core:3-5) | PASS | New library behavior stays on `ColdBuild` and `PrContext`. The provider recorder implements the existing `HttpTransport` seam; manifests and lockfile did not change. |
| Validated ingress and visible incomplete/budget failures (`AGENTS.md:28,30`; `agent-rules.md:7`; design:24-34) | PASS | `live_pr.rs:80-94,106-112,197-202` validates settings and fails empty context/read mismatch; `provider_contract.rs:133-152,247-252` validates identity/commit and rejects failed cases. `PrContext::list` refuses a moved pair before publication (`pull_request.rs:385-420`). |
| Store-owned durable time and documented cancellation/partial progress (`AGENTS.md:29,32`; design:22; spec REQ-012–015) | PASS | New library entrypoints read no host time. `Instant` in the example measures elapsed time only; `bounded` drops timed-out facade futures (`live_pr.rs:61-70`). `run_at` delegates to the documented retry-safe `run`/`build` behavior. |
| Typed library errors, imported types, checked panics and rustdoc (`AGENTS.md:30-32`; `agent-rules.md:11`; Rust core:7) | PASS | New library methods return existing typed errors and use imported types. New fallible items have `# Errors`; `run_at` describes moved-PR publication behavior. Provider/example input and IO use `?`; `expect` is limited to named test/recording invariants, and no unchecked `unwrap()` was added. |
| Public journey and inline owner tests; credential-free normal lanes (`AGENTS.md:38-40`; `agent-rules.md:4-5`; testing-workflows:3-20; spec REQ-017) | PASS | `snapshot.rs` and `pull_request.rs` tests are inline. Existing public journeys remain in `test:rust`; `live:pr` and `live:provider-contract` are opt-in and absent from `gate` (`mise.toml:54-102`). Candidate `mise run gate` passed, as reported by the orchestrator for this immutable HEAD. |
| Test integrity and durable fixtures (`AGENTS.md:40`; `agent-rules.md:12`; spec-driven-development:61-71) | PASS | Provider example test loads `examples/provider_contract_cases.json` (`provider_contract.rs:337-350`), which survives active-packet completion. It still mutates valid cases to reject nine broken/missing conditions (`:351-390`). No golden, skip, suppression or test target was changed. `git diff --check 2257992 317b2da` passed. |
| No leaked source or credentials in recorded evidence (`AGENTS.md:21-23`; `agent-rules.md:9`; design:5,22; spec REQ-014) | PASS | Recorder stores selected headers, strips token query and reduces redirect location to host (`provider_contract.rs:56-113`). Live report emits span metadata rather than source (`live_pr.rs:156-195`). JSON evidence key inspection found no source/body/authorization fields. |
| Approved active-packet lifecycle and unchanged authority (`AGENTS.md:42-46`; `agent-rules.md:14-15`; spec-driven-development:17-71) | PASS | Revision 14 and stateless-snapshot decision remain authority. R4 moves only an example test fixture to durable crate ownership; the source subject was not edited in this review. |

## Findings and result

None. The cumulative changed surfaces disclose no material violation of a written repository rule. The live GitHub observations remain captured evidence; this standards pass did not rerun credentialed commands or large benchmarks.

**Overall: PASS.** Task acceptance and domain correctness belong to the other independent reviewers.

```citations
[]
```
