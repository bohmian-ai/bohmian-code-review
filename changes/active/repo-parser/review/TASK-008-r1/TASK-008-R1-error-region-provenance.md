---
id: TASK-008-R1
kind: remediation
status: ready
spec: SPEC-repo-parser@14
parent_task: TASK-008
remediates: [FIND-008-1]
---

# Keep error-region declarations out of recovered facts

## Authority and subject

Approved spec: `changes/active/repo-parser/spec.md` revision 14, especially REQ-002, INV-003, and the Complete snapshot term. Original task: `changes/active/repo-parser/tasks/TASK-008-recovered-tree-extraction.md`. Reviewed base: `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c`. Cumulative candidate: `caad6d05711e9e98a992d712331bc18a1fdb78ca`. Review: `changes/active/repo-parser/review/TASK-008-r1/`; validated finding: `FIND-008-1` in `findings-validation.md`.

## Diagnosis: FIND-008-1

The candidate removes whole-file `has_error()` deferral but leaves declaration walkers receiving unqualified tree-sitter `Node` values. Generic descent passes a declaration beneath an `ERROR` ancestor into the same dispatch as a declaration outside that region. The defect class is lost parser-error provenance at declaration input, not a missing check at the pipeline or facade. REQ-002 requires a syntax error to drop declarations inside its error region while retaining those outside it.

At `crates/bohmian-code-review-extract/src/rustlang.rs:247,556-604`, the root enters `visit_node`, `function_item` is emitted, and generic child descent traverses an `ERROR` child. With pinned Rust grammar, `fn before() {}\n{ fn inside() {}\nfn after() {}\n` places `inside` and `after` inside `ERROR` yet public extraction emits them. Independently, pinned TypeScript grammar parses `function before() {}\n@ function ghost() {} }\nfunction after() {}` with `before` below `ERROR`; public `Extractor::extract_file` still emits `before`. These are reachable through the facade and can pollute exported facts, snapshot postings, candidate lookup, and ranking. The existing recovered-tree and corpus tests require expected names to be present but do not reject these extra names, so a passing gate does not close the gap.

The same raw-node declaration-dispatch shape appears in `tsjs/mod.rs:834,918`, Rust's nested visits at `rustlang.rs:707,737,775`, and `kotlin.rs:836,899` (whose `resolve_body` uses an `ERROR` body at lines 450-465). The candidate also removes whole-file rejection in the `cfquery`, `cfscript`, `cobol`, `csharp`, `dart`, `erlang`, `go`, `java`, `lua`, `nix`, `objc`, `pascal`, `php`, `python`, `rlang`, `ruby`, `scala`, `solidity`, `swift`, `terraform`, and `vbnet` walkers; ArkTS and composite routing consume those paths. Each is a sibling surface to check for the same class. The existing C/C++ `ERROR` recovery path is an adjacent behavior to preserve. These are not separate fixes or claims that every grammar was independently reproduced. `findings-validation.md` gives the inspected dispatch locations and proof limits.

## Required correction

Change the candidate-owned recovered-tree traversal contract so declaration dispatch can receive only syntax whose ancestry excludes parser `ERROR` regions. Keep error-containing trees available for recovery of valid sibling declarations and for existing non-declaration behavior that the task still requires. Make the invalid declaration input unrepresentable at the traversal/dispatch boundary; do not add repeated guards at callers, declaration emitters, the facade, or pipeline. Use the existing language walkers and tree-sitter dependency. No approved public interface or persistence decision needs revision.

The correction must preserve clean-file extraction and frozen goldens, the pinned grammars and vendored C, the new extraction identity, source-free facts, recovered declarations outside error regions, and the shared `NoFacts` content skip with path/blob identity and honest completeness. Preserve Kotlin's separate error-body range behavior and any existing non-declaration error recovery rather than discarding whole files or all error-bearing ancestors. Do not change provider, storage, snapshot publication, ranking, or model behavior. Do not create a new dependency or public configuration surface.

## Acceptance and proof

1. Through the public extractor, the reproduced Rust and TypeScript inputs emit declarations outside their `ERROR` subtrees and emit none whose syntax is within those subtrees. The exact pinned-grammar tree shape and emitted names are asserted, so the regression fails on the cumulative candidate.
2. The same declaration-provenance rule holds at every candidate-changed walker and composite route listed above; representative malformed tests cover Rust, TypeScript, and Kotlin's error-body path, while C/C++ recovery remains intact. Existing valid-region corpus expectations remain green without changing clean fixtures or goldens.
3. Through the facade and pipeline, an error-only file with no remaining declarations is a `NoFacts` skip carrying its verified blob, while a file with valid recovered declarations is indexed. Fetch, verification, and extractor failures remain nonbenign; an incomplete attempt does not advance the snapshot pointer.
4. Run the focused regressions using exact selected test commands, then `mise run test:extract`, `mise run test:symbolic-context:journey`, `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Record RED on the cumulative candidate, GREEN after correction, and the final results. Do not edit a golden to obtain green.

The write set is the extractor traversal and any affected language walker or composite module, its focused tests, and the public facade/pipeline tests needed to prove the correction across their existing consumer paths. It is not limited to the two reproduced languages; close the shared defect class before returning to `$cr-task-review` against the original task and a new cumulative candidate.
