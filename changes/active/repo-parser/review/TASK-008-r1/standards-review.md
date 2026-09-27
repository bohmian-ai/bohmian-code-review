# TASK-008 repository standards review

**Subject:** base `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c`, candidate `caad6d05711e9e98a992d712331bc18a1fdb78ca`, approved `changes/active/repo-parser/spec.md` revision 14. Reviewed the complete 39-file diff. No `.codegraph/` index exists.

## Authority coverage

| Changed surface | Applicable authority |
|---|---|
| Native walkers in 25 language modules, composite Svelte dispatch, `extractor.rs`, `error.rs`, `lib.rs`, parse pool | `AGENTS.md:14-21,25-34`; `architecture/agent-rules.md:3,6-11`; `architecture/bohmian-code-review-design.md` Purpose, Crate direction, Public behavior; `architecture/references/languages/rust-core.md`; `architecture/references/domain/symbolic-context.md` |
| Corpus tests and manifest expectations | `AGENTS.md:32-40`; `architecture/agent-rules.md:4-6,12`; `architecture/references/languages/testing-workflows.md`; `architecture/references/languages/spec-driven-development.md` |
| Public facade re-export and journey | `AGENTS.md:19-23,38-40`; `architecture/agent-rules.md:3-4,9`; design Public behavior and Verification contract; testing workflows |
| Pipeline snapshot ingest, skip reason, example | `AGENTS.md:23,27-32,40`; `architecture/agent-rules.md:3,7-11`; design Crate direction and Public behavior; Rust core and testing workflows |
| Task evidence and live run JSON | `AGENTS.md:42-46`; `architecture/references/languages/spec-driven-development.md` Task contract, TDD execution, Review and remediation; `architecture/references/languages/testing-workflows.md` |

## Rule results

| Rule | Result and source evidence |
|---|---|
| Crate ownership and IO boundary (`AGENTS.md:14-23`; `agent-rules.md:3,9`) | **PASS.** Walkers and version remain in extract; public facade only re-exports `NodeKind`; pipeline owns ingest and the live provider example. No new IO or dependency in symbolic crates. |
| Recovered-tree extraction, typed failure, source-free facts, version mismatch (`design` Public behavior; `agent-rules.md:8-9`; Rust core) | **PASS.** The changed walkers remove the whole-tree `has_error()` rejection; `Extractor::extract` keeps typed walker failures; `EXTRACTION_VERSION` changes; the public journey rejects prior-version facts and checks recovered Rust/TypeScript declarations. |
| Honest completeness and file-local replacement (`AGENTS.md:23`; `agent-rules.md:7-8`; design Public behavior) | **PASS.** `Ingest::ingest` records a verified file with only its file node as `NoFacts` with path and blob; `Coverage::is_complete` counts only the three approved benign content reasons. Fetch and parse errors remain non-benign. The context removes the file-only upsert before recording the skip. |
| Grammar pins, fixtures, generated artifacts (`AGENTS.md:33`; `agent-rules.md:6,12`) | **PASS.** The diff contains no grammar manifest, vendored C, build script, fixture source, or golden change. Only malformed fixture manifest classifications change from `defer` to `recover` as revision 14 requires. Existing `match` differential assertions remain. |
| Concrete owners, borrowing, errors, imports, rustdoc (`AGENTS.md:27-32`; `agent-rules.md:10-11`; Rust core) | **PASS.** The pipeline retains its existing `Ingest` owner and one `SkipReason::is_benign` definition. No new trait, dependency, unchecked production `unwrap`, or misplaced import. New tests and variants have rustdoc; fallible changed functions retain `# Errors`. |
| Public journey and verification (`AGENTS.md:38-40`; `agent-rules.md:4-5,12`; testing workflows) | **PASS.** The existing external public-facade journey is updated. Task evidence records RED/GREEN, `test:extract`, `test:pipeline`, journey, format, lints, full gate, and a live read-only run. I independently ran `git diff --check` on the immutable range; it passed. The named two files are absent from the live run's skipped list in the checked-in JSON. |
| Lifecycle and Git (`AGENTS.md:42-46`; `agent-rules.md:14-15`; spec-driven development) | **PASS.** Candidate task evidence references approved revision 14. No production code contains task or agent references; this review changes no candidate source. |

## Material findings

None. A corpus expectation list covers 19 of 21 malformed fixtures; the other two still pass the generic recovered-tree success and file-node assertion. Their inputs have no intact declaration outside their malformed region that this review can prove missing, so this is not a written-rule violation or blocking finding.

**Overall: PASS.** Verification limit: I relied on the candidate's recorded `mise` outputs for Cargo-backed lanes and did not rerun the full gate independently. The complete diff, rule sources, manifest, journey, pipeline tests, and live JSON were inspected; `git diff --check` was independently repeated.

```citations
[]
```
