# TASK-003 repository standards review

Subject: `be64610028dc896936d6ce18f285a8e73bb40346` → `fa62d9740a3824213c19318e3f8de27547a42fb4` (complete cumulative diff). Reviewed source remains unchanged. The unrelated working-tree edit to `IMPLEMENTATION_PLAN.md` is outside this committed range.

## Authority coverage

| Changed surface | Applicable authority inspected |
|---|---|
| `bohmian-code-review-select/src/lib.rs`, manifest, lockfile | `AGENTS.md` ownership, Rust, trust, and dependency rules; `architecture/agent-rules.md`; `architecture/bohmian-code-review-design.md`; `architecture/references/languages/rust-core.md`; `architecture/references/domain/symbolic-context.md`; approved `spec.md` revision 2; `implementation-execution.md` |
| `bohmian-code-review/src/lib.rs`, manifest, lockfile | Same Rust, ownership, source-lifetime, and dependency authorities; `architecture/references/languages/testing-workflows.md` for the public API contract |
| `bohmian-code-review-index/src/lib.rs` | Index ownership, declaration-posting rule, Rust documentation and dependency direction in `AGENTS.md`, agent rules, design, Rust core, and symbolic-context reference; adjacent selector consumer inspected |
| `tests/symbolic_context_journey.rs` | Public-journey and test-integrity rules in `AGENTS.md`, agent rules, design, testing-workflows, spec-driven-development, and approved `spec.md` |
| `mise.toml` and task evidence file | Scoped lane, exact focused command, execution-evidence, and change-packet rules in `AGENTS.md`, testing-workflows, implementation-execution, spec-driven-development, and approved `spec.md` |

The reference router was consulted; Rust core, testing workflows, symbolic context, implementation execution, and spec-driven development are the complete applicable reference set. No grammar or generated-artifact surface changed.

## Rule results

| Applicable rule | Result | Source and verification evidence |
|---|---|---|
| Six crate owners and one-way dependencies | PASS | Selector depends on core/index/resolve and facade on stage crates; manifests and `Cargo.lock` add only existing workspace `thiserror` edges. Facade delegates ranking and packing to selector (`crates/bohmian-code-review/src/lib.rs:177`, `:203`). |
| In-memory caller-supplied paths, bytes, ranges, and counts; no source retention, IO, checkout, or repository-coverage claim | PASS | Facade validates `RelativePath` and UTF-8 (`:110-120`, `:166-183`, `:217-230`); index receives facts and byte length only. Selector accepts head text, supplied blob map, and tokenizer (`crates/bohmian-code-review-select/src/lib.rs:290-296`). No source-fetching or storage dependency enters the diff. |
| Concrete state owners, synchronous execution, and no duplicate stage algorithm | PASS | `SymbolicContext` owns extractor/index (`facade:68-75`); stateless selector functions operate on borrowed facts and caller input. Existing resolver and index behavior are reused (`select:9-11`, `:161`). |
| Boundary validation, typed errors, and no unchecked input panic | PASS | Canonical path and UTF-8 checks at facade ingress; `ChangedLines::new` checks empty, invalid, unordered, and past-source ranges (`select:43-81`); `pack` has explicit file, budget, blob, and span errors (`select:297-374`, `:427-493`). The only new `expect` in production names the `all_native` invariant (`facade:83-96`). |
| Exact ambiguity, distinct-name ranking, deterministic order, and budget accounting | PASS | Selector uses resolver hits and a per-file `BTreeSet`, excludes the changed file, sorts score/path (`select:148-195`); changed hunks precede candidates and whole spans are selected or omitted under callback-counted limits (`select:300-374`). |
| Rust style and useful rustdoc on new/materially changed items, including errors and panic contracts | PASS | Imports are at module tops; public and private types, functions, fields, and tests carry intent comments. Fallible functions document `# Errors` (`select:43-53`, `:273-290`, `:395-409`; `facade:99-110`, `:158-166`, `:187-201`); invariant `expect` and test helper panics are documented. |
| Real public journey, supporting owner tests, and no weakened fixture/gate | PASS | External test imports only `bohmian_code_review`, drives four in-memory TypeScript files, asserts ranking, selected text/tokens, repeat determinism, and typed failures (`tests/symbolic_context_journey.rs:99-296`). Selector has owner tests (`select:500-849`). Existing grammar pins and goldens are untouched. |
| Scoped verification lanes and execution evidence | PASS | `mise.toml:42-49` adds owner and external journey targets with `cargo test --locked`; task evidence records RED/GREEN, upstream lanes, owner/journey lanes, `fmt:check`, `lints`, and `git diff --check` as passing. The exact named tests and package/target can be identified from source; reported broad checks are available evidence, not independently rerun in this review. |
| Approved packet and Git identity/publication rules | PASS | Only task implementation evidence changed under `changes/active`; commit authors use the local identity and no AI trailer appears. The reviewed range contains no push, tag, or publication action. |

## Material findings

None.

Overall: **PASS**. Verification limit: this is an independent static standards audit of the immutable committed range using the recorded check results; tests were not rerun here.
