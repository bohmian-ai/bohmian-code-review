# Repository standards review — TASK-005 r5

Subject: base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`; candidate `866f777a1a41a03384c14c12276a5877dcac94a2`; approved `changes/active/repo-parser/spec.md` revision 13. This review covers the complete base-to-candidate diff, including earlier remediation. The r3 rejected-tree-entry and r4 clock/size maintainer decisions override their remediation tasks; FIND-TASK-005-7 is withdrawn.

## Authority coverage

| Changed surface | Applicable authority | Evidence inspected |
|---|---|---|
| Outer crate module exports, facade, source lifetime, dependency graph | `AGENTS.md:14-23,27,34`; `architecture/agent-rules.md:3,9-10`; `architecture/bohmian-code-review-design.md:5,20-22,32`; spec rev13 REQ-008–018 and INV-001,005,007 | `lib.rs`, `pull_request.rs`, `context.rs`, `Cargo.toml` and workspace manifest; only the outer crate changed, and it imports the public symbolic facade. |
| Provider tree, PR files, blob bounds, and eligibility | `AGENTS.md:23,28`; `architecture/agent-rules.md:7-9`; design `:22,26`; spec rev13 eligible-file, coverage, and trust contracts; both maintainer decisions | `github.rs:943-1015,1377-1404`, `eligibility.rs:260-313`, `snapshot.rs:743-775`, `pull_request.rs:395-448`; mode/id failures end tree listing, noncanonical paths count as unsupported, regular root attributes and bounded blob reads are used. |
| Snapshot and context records, keys, store clock, expiry, and cleanup | `AGENTS.md:23,29`; `architecture/agent-rules.md:9`; design `:22`; spec rev13 REQ-012,014–016 and INV-003,005,007; r4 clock/size decision | `persistence.rs:37-147,192-304`, `storage.rs:50-165`, `snapshot.rs:259-353,701-742`, `context.rs:288-441`, `testing.rs:211-525`; source-free snapshots and selected-text context object remain separate. |
| Rust API, errors, imports, rustdoc, tests, and tooling | `AGENTS.md:27-34,38-40`; `architecture/agent-rules.md:4-6,10-13`; `architecture/references/languages/rust-core.md:1-6`; `architecture/references/languages/testing-workflows.md:3-20` | All changed Rust modules and inline tests; manifests, lockfile, `mise.toml`, full gate, and base-to-candidate whitespace check. No grammar, golden, generated artifact, or symbolic crate changed. |
| Change packet and review process | `AGENTS.md:42-46`; `architecture/agent-rules.md:14-15`; `architecture/references/languages/spec-driven-development.md:8-18,155-174`; cr-task-review skill | Approved spec rev13, original task, r1–r4 remediation/verdict records, both maintainer decisions, and immutable commit pair. The original task's rev12 header is superseded by the approved rev13 and explicit current review instruction. |

## Applicable rule outcomes

| Rule | Result and source evidence |
|---|---|
| Only the outer pipeline may perform provider/storage IO; symbolic stages remain owned by the six crates (`AGENTS.md:14-23`; agent rules `:3,9`; design `:20-22`) | **PASS.** Changed production code is confined to `crates/bohmian-code-review-pipeline`; `pull_request.rs:518-542,451-501` calls facade ranking and candidate packing. No symbolic crate, model, server, SQL adapter, or checkout was added. |
| Canonical paths, blob identity, file bounds, honest coverage, and no source outside the selected context object (`AGENTS.md:21-23,28`; agent rules `:7-9`) | **PASS.** `github.rs:973-1015,1377-1404` validates tree entries and bounds verified blobs; `snapshot.rs:789-827,969-988,1018-1040` classifies/records omissions; `pull_request.rs:151-323` carries coverage and stores only selected spans via `context.rs:315-339`. Snapshot encoding carries facts, not source (`snapshot.rs:160-231`). |
| Durable time is store-owned; no host timestamp crosses store traits (`AGENTS.md:29`; spec rev13 REQ-015) | **PASS.** `persistence.rs:192-304` takes grace and returns store-stamped lifetime/lookup; `snapshot.rs:300-332` uses grace and zero grace; `context.rs:315-411` uses the store's expiry decision; `testing.rs:211-225,312-365,428-499` uses a manually advanced store clock. The remaining `github.rs:729` host clock is solely rate-limit wait computation, as the r4 decision permits. |
| Stateful concrete owners, narrow injected traits, typed errors, safe imports and rustdoc (`AGENTS.md:27-32`; agent rules `:10-11`; Rust core `:1-6`) | **PASS.** `PrContext`, `Contexts`, and `Snapshots` own workflows; traits are the three spec-required consumer seams. Modified fallible items document errors; no changed production input/IO path uses unchecked `unwrap()`. `getrandom` is required by the r4 context-ID decision. |
| Credential-free public journey, owner tests, and required gates; no weakened check (`AGENTS.md:38-40`; agent rules `:4-5,12`; testing workflows `:3-20`) | **PASS.** Inline provider/storage tests cover the touched owner paths; the parent orchestrator independently ran `mise run gate` at candidate HEAD (exit 0, 9.33 s), including `fmt:check`, `lints`, `test:pipeline`, `test:symbolic-context:journey`, review scripts and skill sync. `git diff --check` for base-to-candidate exited 0. No test suppression, golden edit, or absent target appears in the diff. |
| Immutable cumulative review and local Git identity (`AGENTS.md:44-46`; agent rules `:14-15`) | **PASS for this reviewer.** The candidate stayed at `866f777a1a41a03384c14c12276a5877dcac94a2`; this report changes only the r5 review directory. The orchestrator owns the final commit and verdict. |

## Material findings

None under a written repository rule. Task acceptance, domain-specific failures, and Ponytail findings belong to their independent reviewers.

## Verification limit and result

The credentialed real-GitHub run is outside the normal gate and belongs to the integrated change proof, not this candidate's repository-standards gate. Overall **PASS**.

```citations
[]
```
