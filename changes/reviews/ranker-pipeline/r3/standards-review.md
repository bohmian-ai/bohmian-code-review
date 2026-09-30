# Repository standards review

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d` → `e52510f112b822f7f7692535b26ae81e53ac4b03` (HEAD checked at the latter commit). **Result: PASS.** This is a repository-rule audit of the complete cumulative diff; it does not decide task acceptance. The user's review authority is `changes/reviews/ranker-pipeline/intent.md` because this review has no task file.

## Authority coverage

| Changed surface | Applicable authority |
|---|---|
| Core facts, index postings, resolver, selector rankers and packing, public facade | `AGENTS.md:14-21,27-34,38-40`; `architecture/agent-rules.md:3-11`; `architecture/bohmian-code-review-design.md` (Crate direction, Public behavior, Verification contract); `architecture/references/languages/rust-core.md`; `architecture/references/domain/symbolic-context.md`; `architecture/references/languages/testing-workflows.md` |
| Pipeline provider, history, snapshots, PR overlay, review wrapper, storage traits, pipeline journey | `AGENTS.md:21-23,27-34,38-40`; `architecture/agent-rules.md:3-11`; `architecture/bohmian-code-review-design.md` (Crate direction, Public behavior, Verification contract); `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md`; `architecture/references/domain/pipeline-consumer-ergonomics.md` |
| New evaluation crate, its binary and report/sample/trial/live modules | `AGENTS.md:23,27-34,40`; `architecture/agent-rules.md:3,5,10-11`; `architecture/bohmian-code-review-design.md` (Crate direction, Verification contract); Rust and testing references above |
| Workspace manifests, lockfile, mise tasks, public journeys, README and architecture changes | `AGENTS.md:3,7-9,33-34,38-40,44`; `architecture/agent-rules.md:4-6,12-14`; `architecture/references/languages/spec-driven-development.md`; testing reference above; the repository reference router |
| Prior review/remediation records and approved intent added in this range | `AGENTS.md:42-44`; `architecture/agent-rules.md:14`; `architecture/references/languages/spec-driven-development.md` (Authority; Review and remediation) |

## Rule results

| Written rule | Source evidence and verification | Result |
|---|---|---|
| Six symbolic owners remain IO-free; pipeline alone owns provider and storage IO; evaluation uses the public ranker and pipeline access. (`AGENTS.md:14-23`; `architecture/agent-rules.md:3,9`) | `crates/bohmian-code-review-select/src/rank.rs:34-48` owns `Ranker` and input; `crates/bohmian-code-review/src/lib.rs:347` composes it; pipeline manifests depend only on the facade; `crates/bohmian-code-review-eval/src/trial.rs:266` calls facade `rank_with`, and its feature-gated manifest has no reverse production dependency. The changed symbolic modules have no provider/storage/source IO imports. | PASS |
| Source-free indexed and persisted facts; canonical paths, versioned snapshot loads, and no implied full coverage. (`AGENTS.md:21`; `architecture/agent-rules.md:7-9`; design Public behavior) | `crates/bohmian-code-review-core/src/facts.rs:39-70` drops source fragments; `crates/bohmian-code-review-pipeline/src/snapshot.rs:258-301` stores facts/history with format and extraction versions and rejects mismatch; pipeline context retains explicit coverage. | PASS |
| Stateful work has concrete owners, with only real boundary traits and typed errors; imports and rustdoc follow Rust rules. (`AGENTS.md:27-32`; `architecture/agent-rules.md:10-11`; Rust core reference) | `ReviewPipeline` in `crates/bohmian-code-review-pipeline/src/review.rs:82-173`, `History` in `history.rs:237-310`, and rankers in `select/src/rank.rs` own their workflows. Existing store/provider traits are consumer boundaries; new fallible APIs document errors and use `thiserror`. A scan of changed Rust declarations found no newly undocumented item. Host time at `github.rs:958-964` is used for rate-limit waiting, not a durable store timestamp. | PASS |
| Consumer entry path stays one wrapper, shared storage/store, per-call client, public stage facades, and no background worker or algorithm-selection config. (`architecture/references/domain/pipeline-consumer-ergonomics.md`, Review checklist; design Crate direction) | `review.rs:82-173` has `context`, `refresh`, and `read` over one `O` and one `S`, with client passed to calls; `pipeline/src/lib.rs:44-78` reexports wrapper and stage facades; `README.md:4` leads with the wrapper. | PASS |
| Keep grammar pins/fixtures, banned dependencies, and generated output rules. (`AGENTS.md:33-34`; `architecture/agent-rules.md:6`) | The diff changes no extractor fixture, grammar pin, build script, or generated artifact. New eval manifest uses existing workspace dependencies and is feature gated. | PASS |
| Public two-pass journey and relevant boundary tests remain; broad change runs gate with no suppressed checks. (`AGENTS.md:38-40`; `architecture/agent-rules.md:4-5,12`; testing reference) | `crates/bohmian-code-review/tests/symbolic_context_journey.rs` and `crates/bohmian-code-review-pipeline/tests/pipeline_journey.rs:617-711` are changed and exercise public facade/wrapper paths. `mise.toml:16-111` includes owner, eval, journey, format, lint, and gate lanes. Root orchestrator ran `mise run gate` on this candidate: exit 0, including format, lints, all Rust tests, workspace check, review scripts, and skill sync. `git diff --check` on the complete range exits 0. | PASS |
| Review records use the approved authority hierarchy; no production task/agent metadata. (`AGENTS.md:44`; `architecture/agent-rules.md:14`; spec-driven development Authority) | The current user's explicit instruction substitutes `intent.md` for a task file. Prior records are under `changes/reviews/ranker-pipeline/`; changed production modules contain no task/agent references. | PASS |

**Findings:** none. The full gate is local and credential-free; this review did not run live provider experiments. Those experiments are outside this repository-rule audit.

```citations
[]
```
