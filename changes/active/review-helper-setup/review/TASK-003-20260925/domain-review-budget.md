# TASK-003 budget safety and deterministic packing review

**Subject:** `be64610028dc896936d6ce18f285a8e73bb40346` → `fa62d9740a3824213c19318e3f8de27547a42fb4` (`review-helper-setup`). HEAD matched the candidate during review. **Result: PASS.**

## Boundary and authority

Reviewed changed-line validation, distinct-name scoring, candidate order and reasons, changed-hunk priority, whole-span selection, distinct-file and caller-token accounting, explicit omissions, and typed budget/input failures. The governing obligations are approved `spec.md` REQ-005–007, INV-003–004, AC-004–005; original TASK-003 scenarios 1, 2, and 4; `AGENTS.md`; `architecture/agent-rules.md`; `architecture/bohmian-code-review-design.md`; `architecture/references/domain/symbolic-context.md`; and `architecture/references/languages/testing-workflows.md` and `rust-core.md`.

## Source and verification coverage

Inspected the complete cumulative diff, then traced `SymbolicContext::rank` → `ChangedLines::new` → selector `rank` → resolver `resolve` and index lookup, and `build_context` → selector `pack` → `candidate_spans`/`slice_lines`/`blob_text`. The resolver deduplicates query names with a `BTreeSet`; selector scores each file from distinct names, orders score ties by canonical path, and groups matched declaration reasons before packing. `pack` initializes the changed file count to one, counts each selected whole span with the supplied callback, uses remaining token budget for admission, increments file count only when a candidate contributes a span, and emits an omission for each excluded candidate span. Empty, malformed, overlapping, and out-of-source changed ranges fail before ranking; missing or unusable candidate blobs and unfittable changed hunks return typed errors.

`mise run test:select` passed 4/4 and `mise run test:symbolic-context:journey` passed 3/3 at the candidate. These cover repeated names, path ties, range validation, exact selected text and token counts, file and token omissions, deterministic repeat output, and budget/missing-blob errors through the public facade.

## Verification limits

The journey uses a deterministic whitespace token callback. The contract provides spans as separate text values and does not define a serialized combined prompt, so accounting is tested per selected span. Blob identity is supplied and verified by the future service under the approved trust boundary; this library checks candidate UTF-8 and indexed line bounds.

## Material proposed findings

None.
