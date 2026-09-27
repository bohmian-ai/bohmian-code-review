# TASK-008 parser provenance domain review

**Subject:** `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c` → `caad6d05711e9e98a992d712331bc18a1fdb78ca`; approved `SPEC-repo-parser` revision 14; original TASK-008. **Result: FAIL.**

## Boundary and coverage

Reviewed the complete base-to-candidate diff for the extractor, corpus manifest, public facade, and journey, plus the parser-to-walker-to-facade path. The governing behavior is spec REQ-002: tree-sitter recovery keeps declarations outside an error region and drops declarations inside it; INV-003 requires changed extraction identity. TASK-008 requires this for every ported language without changing valid-file goldens or grammar pins. I inspected the changed language entrypoints, representative traversal and emission paths (`rustlang`, `tsjs`, Kotlin, C/C++), extractor dispatch/decoding, malformed fixture assertions, version constant, and public journey. The version changed from `…facts-1` to `…recovered-tree.facts-2`, and the journey rejects old facts. I found no separate version finding.

## Verification and limits

`mise exec -- cargo test --locked -p bohmian-code-review-extract --lib recovered_tree -- --nocapture` passed both focused tests (2 passed). The candidate's recorded `test:extract`, public journey, and gate results were reviewed as evidence, not rerun. I independently compiled a read-only probe outside the repository against the candidate's public `Extractor` and pinned `tree-sitter-rust` grammar. The probe parsed `fn before() {} ??? fn hidden() {} { fn after() {}` and printed:

```
(source_file (function_item name: (identifier) parameters: (parameters) body: (block)) (expression_statement (try_expression (try_expression (try_expression (MISSING identifier)))) (MISSING ";")) (function_item name: (identifier) parameters: (parameters) body: (block)) (ERROR (function_item name: (identifier) parameters: (parameters) body: (block))))
File: x.rs 1 1
Function: before 1 1
Function: hidden 1 1
Function: after 1 1
```

The `after` declaration is inside the parser's `ERROR` node but is emitted. The probe did not cover every grammar's possible recovery shape; the confirmed Rust path is sufficient for the named REQ-002 boundary.

## Proposed finding

### DOMAIN-PARSER-1 — Declaration provenance is lost when recovered `ERROR` subtrees enter ordinary walker traversal

- **Classification:** INCORRECT; task contract and source-trust boundary.
- **Defect class and owner shape:** The language walkers receive raw tree-sitter `Node`s for both valid and error regions. `rustlang::Walker::visit_node` recursively passes every named child back through declaration dispatch, including `ERROR` descendants. Its `function_item` branch then emits a declaration without the child's error-region provenance. The raw-node traversal shape permits the invalid declaration to reach emission.
- **Violated obligation:** `changes/active/repo-parser/spec.md` REQ-002, line 56, and TASK-008 Scenario 1/acceptance: a syntax error drops declarations inside its error region while retaining recovered declarations outside it.
- **Evidence and reachable consequence:** The public `Extractor::extract` dispatches the Rust walker. The probe above shows `after` emitted although its `function_item` lies under `ERROR`. The exported declaration may enter index postings and affect file choice, ranking, and context. The two new focused tests assert retained names around errors but no rejected name within an error region; the malformed corpus likewise asserts only presence of expected names.
- **Sibling sweep:** The same provenance question applies at every native walker entrypoint whose whole-tree rejection was removed: `cfquery`, `cfscript`, `cobol`, `csharp`, `dart`, `erlang`, `go`, `java`, `kotlin`, `lua`, `nix`, `objc`, `pascal`, `php`, `python`, `rlang`, `ruby`, `rustlang`, `scala`, `solidity`, `swift`, `terraform`, `tsjs` (also ArkTS), and `vbnet`. `kotlin::resolve_body` explicitly returns an `ERROR` node for normal body traversal; C/C++ also intentionally examines `ERROR` children, so a correction must preserve intended non-declaration recovery while enforcing declaration provenance. Rust is the reproduced emission; I do not claim the same emitted name has been reproduced in every sibling grammar.
- **Testable correction:** Make error-region provenance part of the declaration traversal/emission shape so a declaration under an `ERROR` ancestor cannot become a fact, while ordinary recovered siblings remain available. Cover the reproduced Rust case and representative recovery shapes that currently route through an `ERROR` body; retain valid-file fixtures and goldens. This is a candidate-owned extraction boundary and requires no change to the approved spec.

```citations
[{"id":"DOMAIN-PARSER-1","path":"crates/bohmian-code-review-extract/src/rustlang.rs","line":562,"snippet":"        if matches!(kind, \"function_item\" | \"function_signature_item\") {","symbols":["visit_node","extract_fn_or_method"]},{"id":"DOMAIN-PARSER-1","path":"crates/bohmian-code-review-extract/src/rustlang.rs","line":603,"snippet":"                if let Some(c) = node.named_child(i) {","symbols":["visit_node"]},{"id":"DOMAIN-PARSER-1","path":"crates/bohmian-code-review-extract/src/kotlin.rs","line":457,"snippet":"            if child.kind() == \"ERROR\"","symbols":["resolve_body"]},{"id":"DOMAIN-PARSER-1","path":"crates/bohmian-code-review-extract/src/corpus_tests.rs","line":1795,"snippet":"        for (kind, name) in expectation.recovered {","symbols":["recover_fixtures_keep_valid_regions_per_fixture"]}]
```
