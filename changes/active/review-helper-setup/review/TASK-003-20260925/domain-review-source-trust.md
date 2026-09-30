# Domain review: caller-supplied source trust

**Subject:** `be64610028dc896936d6ce18f285a8e73bb40346` → `fa62d9740a3824213c19318e3f8de27547a42fb4` (TASK-003). **Result: PASS.**

## Boundary and authority

| Boundary | Authority | Source inspected | Result |
|---|---|---|---|
| Canonical caller paths and UTF-8 source | Approved spec REQ-003, REQ-005, INV-003; `AGENTS.md`; `architecture/agent-rules.md`; `architecture/bohmian-code-review-design.md`; Rust-core reference | Facade `relative_path`, `decode`, `upsert`, `rank`, `build_context`; core `RelativePath::new`; extractor `extract_from_source` | PASS: ingress paths use the validated core type; source decode is typed and precedes parsing or mutation. |
| Changed head-side ranges | Approved spec REQ-005, INV-003, deletion-only non-goal; symbolic-context reference | `ChangedLines::new`, facade `rank`, selector `pack`, `slice_lines` | PASS: empty, zero, reversed, unordered, overlapping, and out-of-source ranges fail; packing rejects a shorter head instead of silently cutting a hunk. |
| Transactional one-file replacement | Approved spec REQ-003, REQ-010, INV-001, INV-003; agent rules | Facade `upsert`; extractor routed entry; index `upsert`, `remove`, `lookup`, `byte_len`, `nodes`; public journey transactional test | PASS: extraction errors return before `SymbolIndex::upsert`; the index holds facts and byte length, not the input blob. |
| Candidate blob supply and provenance | Approved spec REQ-006, INV-001, INV-003; design and symbolic-context reference | Selector `pack`, `blob_text`, `slice_lines`; facade `build_context`; journey missing-blob test | PASS: a visited selectable file without bytes fails with `MissingBlob`; invalid UTF-8 and too-short blobs fail. The approved trust boundary assigns repository, commit, path, and blob identity verification to the future service. |

Reviewed the complete base-to-candidate diff, the relevant design and routed Rust/testing/symbolic-context references, and the public journey. No `.codegraph/` index exists. The working tree's pre-existing `IMPLEMENTATION_PLAN.md` edit is outside the immutable candidate.

## Verification limits

Implementation evidence records passing `test:extract`, `test:index`, `test:resolve`, `test:select`, `test:symbolic-context:journey`, `fmt:check`, `lints`, and `git diff --check`. Source inspection confirms the paths those checks exercise; this review did not independently rerun Cargo. The journey proves failed malformed and non-UTF-8 replacements preserve existing postings, and missing selected bytes produce a typed error. It does not prove that caller-supplied candidate bytes are the same commit blobs as the indexed facts; the specification explicitly places that verification in the future service, so it is not a TASK-003 finding.

## Material findings

None.
