# TASK-002 source trust, parser provenance, and import binding review

**Subject:** `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41` → `0aa3a607e770d3271f872417b535d555360b4a20`; approved `changes/active/context-relevance-eval/spec.md` revision 4; original TASK-002 and TASK-002-R1 remediation. **Result: PASS.**

## Boundary, authority, and coverage

| Boundary | Authority | Source traced | Result |
|---|---|---|---|
| Complete verified base corpus | Spec REQ-004, INV-002, INV-005; TASK-002 Scenario 2; `AGENTS.md` outer-pipeline and source ownership rules | `main.rs:220-276` takes indexed paths and blob IDs from the fixed-commit snapshot, fetches through `GitHubClient::blob` (`github.rs:1237-1269`), and passes the full base map to `evaluate`; `trial.rs:225-277` errors on a missing indexed blob and extracts every indexed file through the public facade. | PASS |
| Head input and public parser provenance | Spec REQ-004 and D rule, INV-001–INV-002; TASK-002-R1 | `main.rs:242-275` selects listing head IDs and verified blobs; `trial.rs:335-348` uses `SymbolicContext::upsert` for nodes and references; `trial.rs:529-626` prepares changed names and bindings from head facts. The evaluator does not add a parser or alter production extraction. | PASS |
| D explicit import scope and fallback | Spec variant D at `spec.md:32`, TASK-002 Scenario 2, TASK-002-R1 FIND-TASK-002-2 | `trial.rs:366-423` pairs Python import references with exactly one import node covering their line and column, then resolves a unique indexed module at `432-487`. Both base and head producers call this owner (`267`, `618-623`); `score` consumes the bindings for upstream and downstream lookup (`742-786`). Ambiguous, unresolved, wildcard, and relative imports have no module binding and retain C lookup. | PASS |
| D score after scoping | Spec variant C/D at `spec.md:30-32`, TASK-002-R1 FIND-TASK-002-1 | `trial.rs:736-763` keeps the complete `declaring.len()` denominator after filtering to the bound module; downstream remains `1/df` at `765-797`. | PASS |

The full cumulative base-to-candidate changed surface was inspected. The remediation changes only `trial.rs` behavior and inline checks; main, reporting, and task evidence were inspected for their relevant source path. The Python extractor emits import nodes and references with source coordinates at `crates/bohmian-code-review-extract/src/python.rs:620-675`; its coordinate producers are at `234-244` and `295-308`. The binding fix uses those public facts on both sides of the counterfactual.

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features trial::` passed: 5 tests, including the exact D denominator, same-line upstream/downstream imports, repeated-name fallback, and missing-base-source case. `git diff --check` for the immutable range passed. The reviewed files remain unchanged. No live GitHub run was performed in this domain review; TASK-003 owns the credentialed run and checked-in reports. No material source-trust or import-binding finding remains.

```citations
[]
```
