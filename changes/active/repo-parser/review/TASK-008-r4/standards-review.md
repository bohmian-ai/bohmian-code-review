# TASK-008 r4 repository standards review

**Subject:** base `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c`; candidate `f204d3383c62c6137a603437668103187a4fd19c` (HEAD); spec revision 14. The r1 maintainer decision makes `FIND-008-1` invalid. This report audits repository standards only, over the complete cumulative diff. `.codegraph/` is absent. **Result: PASS.**

## Authority coverage

| Changed surface | Applicable authority | Source and result |
|---|---|---|
| Native language walkers, composite Svelte routing, extractor errors, batch parsing, corpus manifest/tests | `AGENTS.md:15,21,30-34,38-40`; `architecture/agent-rules.md:5-6,8-9,11-12`; `architecture/bohmian-code-review-design.md:14,24-26,34,38`; `architecture/references/languages/rust-core.md`; `architecture/references/languages/testing-workflows.md`; `architecture/references/domain/symbolic-context.md` | PASS: walker defer branches removed; native recovered trees supply facts; pins, vendored C, fixtures and clean goldens untouched; corpus `recover` entries keep distinct oracle goldens; errors remain typed and local. |
| Index declaration projection and public symbolic facade/journey | `AGENTS.md:16,19,21,27-32,38-40`; `architecture/agent-rules.md:3-5,7-9,11-12`; `architecture/bohmian-code-review-design.md:15,18,20,26,32,34,38`; Rust, testing, symbolic references | PASS: `SymbolIndex::export_file` reuses the same declaration projection as `export`; facade exposes it with the extraction stamp; no source bytes retained or IO introduced; public journey persists and tests prior-version rejection. |
| Pipeline snapshot outcome, ingest/reuse, cold build, update, PR overlay, live example and inline tests | `AGENTS.md:23,27-32,38-40`; `architecture/agent-rules.md:3,5,7,9-12`; `architecture/bohmian-code-review-design.md:22,32,34,38`; Rust, testing, symbolic references | PASS: provider/storage IO stays in the pipeline; `IndexedFile` requires persisted declarations on construction and deserialize, `FileOutcome` represents exclusive indexed/skipped results, and coverage derives from outcomes. Versioned snapshot scope is 2. Source-free facts remain the durable payload. |
| Spec, task, remediation/evidence records | `AGENTS.md:3,42-46`; `architecture/agent-rules.md:14-15`; `architecture/references/languages/spec-driven-development.md`; `architecture/references/languages/implementation-execution.md` | PASS: approved revision 14 and maintained withdrawal of `FIND-008-1` govern this review. Evidence includes the named live run and focused tests. |
| Manifests, lockfile, grammar/build tooling | `AGENTS.md:9,33-34,40`; `architecture/agent-rules.md:6,12`; `architecture/bohmian-code-review-design.md:38`; testing reference | PASS: `Cargo.toml`, `Cargo.lock`, build scripts, grammar C, and clean-output goldens are unchanged; no dependency, gate suppression, generated artifact edit, or forbidden runtime dependency was added. |

## Rule results and verification

| Written rule | Result and evidence |
|---|---|
| Crate ownership and dependency direction (`AGENTS.md:14-23`; design `:11-22`) | PASS. Extraction remains in extract, declaration projection in index/facade, provider and storage work in pipeline. No downstream symbolic stage is recreated. |
| Source trust and source-free facts (`AGENTS.md:21-23`; agent rules `:7-9`; design `:5,34`) | PASS. `snapshot.rs` stores blob IDs and `IndexedFileFacts`; ingest receives verified bytes transiently; format 2 separates the previous namespace. No production source read is added to a symbolic crate. |
| Rust ownership, errors, docs, imports (`AGENTS.md:27-32`; agent rules `:10-11`; Rust reference) | PASS. The new stateful outcome belongs to the snapshot owner; new methods and tests have intent rustdoc, fallible `TryFrom` has `# Errors`, and no unchecked production `unwrap()` or new trait/dependency is introduced. |
| Grammar and fixture provenance (`AGENTS.md:33`; agent rules `:6`; design `:38`) | PASS. The diff changes only malformed manifest expectations from `defer` to `recover`, as approved; `match` entries and goldens, grammar pins and vendored sources stay byte-identical. |
| Public journey and test placement (`AGENTS.md:38-40`; agent rules `:4-5,12`; testing reference `:3-20`) | PASS. Existing external journey crosses the public facade; new pipeline tests remain inline with `snapshot.rs`. No `#[allow]`, `#[ignore]`, gate weakening, golden rewrite, or removed failing target appears in the diff. |
| Verification lanes (`AGENTS.md:40`; testing reference `:15-20`) | PASS. In this review `mise run fmt:check`, `mise run lints`, `mise run test:extract`, `mise run test:index`, `mise run test:symbolic-context:journey`, and `mise run test:pipeline` all exited 0; `git diff --check` exited 0. `mise run gate` was also rerun in this review and exited 0. |
| Review scope and Git (`AGENTS.md:44-46`; agent rules `:14-15`) | PASS. The cumulative base-to-candidate diff and written authorities were inspected. No subject code was edited. |

No material repository-rule findings. This standards result does not decide task acceptance or parser/snapshot domain correctness. The mechanical lane rerun emitted pre-existing vendored C compiler warnings but no denied Rust lint.

```citations
[]
```
