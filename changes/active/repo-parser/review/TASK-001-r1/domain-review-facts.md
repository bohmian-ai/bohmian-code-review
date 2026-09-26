# Domain review: source-free facts, extraction provenance, load-time trust boundary

Task: TASK-001 (candidate 34f4ae7, base 9715440). Reviewer: domain-rev (facts).

## Reviewed boundary

- `crates/bohmian-code-review-core/src/facts.rs` — `DeclarationFact`, `IndexedFileFacts`, `InvalidFact`, projection and validation.
- `crates/bohmian-code-review-index/src/lib.rs` — `SymbolIndex::export`, `SymbolIndex::load`, posting order.
- `crates/bohmian-code-review-extract/src/lib.rs` (`EXTRACTION_VERSION`) and `src/extractor.rs` (`Extractor::path_language`), `src/routing.rs` (`LanguageRouter::detect`).
- `crates/bohmian-code-review/src/lib.rs` — `SymbolicContext::{export, load, path_language}`, `Error::{ExtractionVersionMismatch, InvalidFacts}`.
- Consumers of the index: `crates/bohmian-code-review-resolve/src/lib.rs` (`index.lookup`), `crates/bohmian-code-review-select/src/lib.rs` (`hit.node.{id,start_line,end_line}`, `slice_lines`).

## Authority and source coverage

Spec REQ-001, REQ-002, INV-002, INV-003, INV-004 (`changes/active/repo-parser/spec.md` 55-56, 84-88); `architecture/agent-rules.md` 8-9; `architecture/bohmian-code-review-design.md` "Public behavior" 24-34; `architecture/references/domain/symbolic-context.md`; task contract sections of `TASK-001-symbolic-library-contract.md`. Full diff `/tmp/task001-diff.patch` read for the four crates above.

## Verification

| Lane | Exit |
|---|---|
| `mise run test:index` | 0 (4 passed) |
| `mise run test:extract` | 0 (27 + 140 passed) |
| `mise run test:symbolic-context:journey` | 0 (6 passed) |

Traced behaviors (all hold at the candidate):

- **Trust boundary before mutation.** `SymbolicContext::load` compares the version first (facade `lib.rs:193`), then `SymbolIndex::load` runs `IndexedFileFacts::validate` over every declaration (`facts.rs:124-128`: nonempty id/name/qualified_name, `start_line >= 1`, `start_line <= end_line`) before `upsert` is called (`index/lib.rs:157-163`). `RelativePath` re-validates on deserialize (`relative_path.rs:83-99`; journey asserts `../user.ts` is rejected). `upsert` has no fallible step between `remove` and the final `files.insert`, so a record is applied entirely or not at all; each `load` call is one file, so a failure in file N leaves files 1..N-1 fully loaded and file N unchanged (journey lines 452-474 cover both a malformed existing path and a malformed new path with `file_count` and lookup unchanged).
- **Source-free projection.** `DeclarationFact` carries id, kind, name, qualified_name, language, start_line, end_line, start_column only; `into_node` sets docstring/signature/decorators/return_type/type_parameters/extra to `None`. References are never retained by the index in the first place (`FileFacts.references` returned, not stored), so nothing source-bearing survives export. Journey asserts the JSON contains no bodies/signatures.
- **Round-trip fidelity.** Resolver consumes only `index.lookup(name)`; selector consumes `hit.path`, `hit.node.id`, `start_line`, `end_line` (`select/lib.rs:221-224`) and slices candidate blobs by lines. Posting order key is (path, start_line, start_column, id, slot); loading drops non-declaration nodes so slots renumber, but relative order of declarations is preserved so equal-key ambiguous hits keep extractor order. Journey asserts `rank` and `build_context` are equal between fresh parse and reloaded facts. Nothing consumed by ranking is lost.
- **Version gate bypass.** `SymbolIndex::load` deliberately does not check the version (documented, `index/lib.rs:148-150`); the index crate does not depend on extract and the facade is the consumer contract (task line 16), so the only bypass is a caller wiring the index crate directly, which is outside the public contract.
- **Path-only routing.** `path_language` = `router.detect(path, None)` plus enabled/composite check. Matches content routing for every unambiguous extension (same table); `.h` reports `C` and content may refine to C++/ObjC, which spec REQ-002 explicitly permits. Unknown extensions fall to `Language::Unknown`, not native, so `None`. Note (not a finding, unreachable through the facade which always uses `all_native`): an extract-crate caller with a config enabling `Cpp` but not `C` gets `path_language("x.h") == None` while `extract_from_source` on C++ content would succeed.
- **Edge inputs.** Duplicate paths in one batch: last wins, same as upsert. Load over an existing path: old postings removed first. Empty declarations: valid, file indexed with no postings. Huge `end_line`: `slice_lines` walks lines and returns `None` past the blob → typed `SpanBeyondSource`; `lines.end as usize` cannot overflow on 64-bit; no `end - start` arithmetic on loaded values. `byte_len` and `kind` are not validated on load but neither is consumed by lookup/ranking/packing (kind only gates posting; a non-declaration kind is silently not posted and dropped on re-export).

## Verification limits

- No probe test was needed; every edge above is either covered by an existing test or determined by reading a non-branching code path. Did not run `mise run gate`.

## Findings

### DOM-FACTS-1 — `EXTRACTION_VERSION` is a free constant; nothing ties it to the grammar pins it names

- **Classification:** Medium. Obligation gap, not a present-state defect.
- **Violated obligation:** spec REQ-002 (`spec.md:56`): the version "changes when extracted facts or grammar behavior changes" and "Version changes cannot silently reuse incompatible snapshots"; `architecture/agent-rules.md:9`.
- **Root cause / defect class:** provenance stamp maintained by convention only. `crates/bohmian-code-review-extract/src/lib.rs:119` hard-codes `tree-sitter-0.25.10` and `572d22bf`; the only test is `assert!(!EXTRACTION_VERSION.is_empty())` (`tests/symbolic_context_journey.rs:508`). A bump of any `tree-sitter*` pin at `Cargo.toml:35-...` leaves the stamp unchanged, so a snapshot exported under the old grammar loads under the new one with no error — exactly the silent reuse INV-003 forbids. The task's own evidence declares this as a limit (`TASK-001-symbolic-library-contract.md:80`).
- **Judgment:** the walker/projection portion (`facts-1`) can only be maintained by hand and the fixtures are reasonable evidence for that. The grammar portion is mechanically checkable and is the part the spec names explicitly ("grammar behavior"). Current stamp matches current pins, so no live incorrect behavior exists; this does not block the task on its own.
- **Testable correction:** in `bohmian-code-review-extract`, add one unit test that `include_str!`s the workspace `Cargo.toml`, extracts the `tree-sitter = "=X"` pin, and asserts `EXTRACTION_VERSION` contains `tree-sitter-X`; optionally extend to each `tree-sitter-<lang>` pin joined into the stamp. Any pin bump then fails the extract lane until the stamp is bumped.
- **Siblings:** none; single definition site.

## Overall: PASS

One Medium obligation gap (DOM-FACTS-1) recorded for follow-up; the load-time trust boundary, source-free projection, atomic per-file replacement, and round-trip fidelity all hold and are covered by passing owner and journey tests.

```citations
[
  {"id": "DOM-FACTS-1", "path": "crates/bohmian-code-review-extract/src/lib.rs", "line": 119, "snippet": "pub const EXTRACTION_VERSION: &str = \"codegraph-kernel-572d22bf.tree-sitter-0.25.10.facts-1\";", "symbols": ["EXTRACTION_VERSION"]},
  {"id": "DOM-FACTS-1", "path": "Cargo.toml", "line": 35, "snippet": "tree-sitter = \"=0.25.10\"", "symbols": ["tree-sitter"]},
  {"id": "DOM-FACTS-1", "path": "crates/bohmian-code-review/src/lib.rs", "line": 193, "snippet": "if facts.extraction_version != EXTRACTION_VERSION {", "symbols": ["extraction_version", "EXTRACTION_VERSION"]},
  {"id": "DOM-FACTS-1", "path": "crates/bohmian-code-review/tests/symbolic_context_journey.rs", "line": 508, "snippet": "assert!(!EXTRACTION_VERSION.is_empty());", "symbols": ["EXTRACTION_VERSION"]}
]
```
