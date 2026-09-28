# Repository standards review

Subject: `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41` → `0aa3a607e770d3271f872417b535d555360b4a20`. Reviewed the cumulative diff, approved specification revision 4, original TASK-002 and TASK-002-R1, repository rules, applicable references, manifests, `Cargo.lock`, evaluator source and inline tests. No `.codegraph/` index exists. The candidate source was not changed.

## Authority coverage

| Changed surface | Governing authority | Source and evidence inspected |
|---|---|---|
| `examples/context_relevance/main.rs` provider orchestration and report assembly | `AGENTS.md:21-34,36-40`; `architecture/agent-rules.md:3,7,9-11`; `architecture/bohmian-code-review-design.md:5,22,32-38`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md`; approved spec REQ-004–007 and INV-001–005 | Complete changed file, `GitHubClient::blob` and facade calls, `Cargo.toml`, `Cargo.lock`, `mise.toml` |
| `examples/context_relevance/trial.rs` corpus, import bindings, scoring, packing and inline tests | Same Rust, trust, ownership and testing rules; `architecture/references/domain/symbolic-context.md:3-7`; approved spec REQ-004–005, INV-002–004; TASK-002-R1 | Complete file, `Change::new` → `Query::new` → `score`/`rank_a` → `outcome`/`trial`/`evaluate`, all inline tests |
| `examples/context_relevance/report.rs` aggregates, serialization and inline test | `AGENTS.md:27-32,36-40`; `architecture/agent-rules.md:5,10-12`; Rust/testing references; approved spec REQ-006 and INV-004 | Complete file and its inline test |
| `mise.toml` live-lane description | `AGENTS.md:36-40`; `architecture/references/languages/testing-workflows.md:15-20`; approved spec REQ-007/INV-001 | Full task graph, unchanged `gate`, pipeline manifest and lockfile |
| Task and prior review artifacts | `AGENTS.md:42-46`; `architecture/agent-rules.md:14-15`; `architecture/references/languages/spec-driven-development.md` task, TDD and review sections | Spec, original and remediation tasks, changed evidence records; prior review reports treated as artifacts, not proof of candidate behavior |

## Rule results

| Written rule | Candidate evidence | Result |
|---|---|---|
| Symbolic crates remain IO-free; the evaluator uses the public facade and verified provider blobs (`AGENTS.md:14-23`; `architecture/agent-rules.md:3,9`) | Only the opt-in pipeline example, task records and live-lane description changed. `main.rs:257-276` uses `GitHubClient::blob`, `Snapshot::into_context` and `evaluate`; no production crate, persisted format, grammar or public API changed. | PASS |
| Do not turn an omitted file into apparent success; validate effective changed-line input (`architecture/agent-rules.md:7`) | `trial.rs:590-596` converts every rank/extraction error for a valid patch and fetched head into a recorded omission; `trial.rs:694-703,828-846` then ranks without that file's names, and `main.rs:351-395` can print a successful report. | **FAIL — STD-R2-1** |
| Concrete owners, borrowed inputs, top-level imports, typed errors and meaningful rustdoc (`AGENTS.md:27-32`; `architecture/agent-rules.md:10-11`; Rust core reference) | `Api`, `Corpus`, `Change` and `Query` own their workflows; stateless transforms are free functions. Imports are at module tops; new Rust items and tests have intent docs; fallible entry points document errors. No new trait, dependency, unchecked input `unwrap`, or generated artifact. | PASS |
| Keep source transient and reports source-free (`AGENTS.md:21-23`; `architecture/agent-rules.md:9`; design `:5,32-34`) | `main.rs:257-276` holds verified blobs in a run-local map. `Trial`, `Outcome`, `VariantSummary` and `Miss` serialize metadata and counts, without blob fields. | PASS |
| Preserve inline tests, focused commands and gate integrity (`AGENTS.md:36-40`; `architecture/agent-rules.md:5,12`; testing workflows `:9-20`) | New tests are inline in `trial.rs` and `report.rs`. TASK-002 and TASK-002-R1 record exact focused RED/GREEN and broader sequential verification. Independently ran `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::`: 5 passed; `git diff --check` exited 0. No gate, test script, golden, lint allowance or ignored test changed. | PASS |
| Approved task/review packet and immutable cumulative subject (`AGENTS.md:42-44`; `architecture/agent-rules.md:14`; spec-driven reference) | The approved revision 4 spec, original and remediation tasks, and cumulative commit range are present. Review writes only under `review/TASK-002-r2/`. | PASS |

## Finding

**STD-R2-1 — Analysis failure is classified as a patch omission.** Classification: repository-rule violation. Defect class: a valid-patch input failure is converted to a successful omission at the shared `Change::new` owner; its `Result` shape already permits propagation, so the owner's shape is not the cause. `architecture/agent-rules.md:7` says to validate effective changed-line input and not turn an omitted file into apparent success. At `trial.rs:590-596`, `rank` or extraction failure after a successfully parsed nonempty patch returns `Ok(change.omit(...))`. Sibling sweep: `Change::new` is called once per listed file by `evaluate` at `trial.rs:1040-1043`; `Query::new` then omits that file's changed names and used names, while `rank_a` and B–D scoring consume the reduced change set. The valid missing-head path at `trial.rs:584-589` already fails, so this error arm is the single sibling location for this defect class. A malformed or absent patch at `trial.rs:557-565` is an intentional, separately approved omission. Observable consequence: a provider head whose bytes cannot be reconciled with a valid listing patch can produce a source-free JSON report with incomplete counterfactual inputs and apparently valid rankings. Correction: propagate rank/extraction failure as a run-level error while retaining missing and malformed patch omissions. Focused proof: supply a valid hunk with an inconsistent head and assert `evaluate` fails; retain a malformed-patch omission check. This is also a candidate contract concern under spec REQ-002/REQ-004, but the standards finding rests on the cited repository rule.

Verification limit: the focused trial tests pass but do not exercise valid-patch analysis failure. Broad lane and gate success are recorded in the task packet, not independently rerun here. Credentialed live evaluation and same-cutoff comparison belong to TASK-003.

Overall result: **FAIL**.

```citations
[{"id":"STD-R2-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":590,"snippet":"        let analyzed = corpus","symbols":["Change::new","SymbolicContext::rank","extract"]},{"id":"STD-R2-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":596,"snippet":"            Err(error) => return Ok(change.omit(format!(\"analysis failed: {error}\"))),","symbols":["Change::new","Change::omit"]}]
```
