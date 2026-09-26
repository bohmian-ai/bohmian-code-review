# Domain review: multi-file ranking determinism and budget-safe candidate packing (TASK-001-r1)

## Reviewed boundary

- `crates/bohmian-code-review-select/src/lib.rs`: `ChangedFile` + `query_names`, `rank_files`, `pack_candidates`, the shared `pack_candidates_into`, and the delegating `rank` / `pack`; `candidate_spans`, `blob_text`, `slice_lines`.
- `crates/bohmian-code-review/src/lib.rs`: `ChangedBlob`, `SymbolicContext::rank_files`, `prepare_changed`, `build_candidate_context`, `build_context`.
- `crates/bohmian-code-review/tests/symbolic_context_journey.rs`: `multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only`.
- Upstream determinism dependencies read for evidence: `resolve` (resolve crate, BTreeSet name dedupe), `SymbolIndex::lookup` and `Posting` ordering (index crate).

## Authority and source coverage

- Spec REQ-003, INV-004, INV-006 (`changes/active/repo-parser/spec.md` lines 57, 88, 90).
- `architecture/agent-rules.md` line 7 (validate effective inputs; no hidden omission/budget success).
- `architecture/bohmian-code-review-design.md` lines 28, 30; `architecture/references/domain/symbolic-context.md` line 7.
- Full base→candidate diff of the select crate (`/tmp/sel.diff`, 342 lines) and facade/journey diffs compared against `git show 9715440d:...`.

## Trace results (no defects found)

- Name union / dedupe: `rank_files` flattens every changed file's `query_names` into one iterator; `resolve` collects them into a `BTreeSet<&str>` before lookup, so a name mentioned in two changed files resolves once and scores once per candidate (`BTreeSet<String>` per file at `select/src/lib.rs:213`).
- Exclusion of every changed path: `excluded: BTreeSet<&RelativePath>` built from all `changed[*].path` (`select/src/lib.rs:209`) and applied to every hit (`:214`). Covered by the select unit test where the second changed file `src/d.ts` declares and references `offRange` and is absent from the ranking in both input orders; the same test proves `rank_files(&[x]) == rank(...).files`.
- Determinism: `resolve` iterates names in BTreeSet order; `lookup` iterates `BTreeSet<Posting>` (path, line, col, id, slot); `matched` is a `BTreeMap` keyed by path; final `sort_by` is stable on (score desc, path asc) with unique path keys, so no ties remain. `candidate_spans` orders spans by `(LineRange, node_id)` via BTreeMap. No HashMap/HashSet iteration reaches output (the only HashMap is `postings`, which is keyed lookup, never iterated for ranking). Journey asserts a repeat `build_candidate_context` call yields an identical `Context`.
- Candidate-only packing: `Context::default()` starts `files: 0`; `files` increments only when at least one span of a candidate is selected (`:471`); `max_files` compared before reading the blob (`:428`), so file-limited candidates never require a blob. Token check `tokens > limits.max_tokens - context.tokens` cannot underflow because `context.tokens` only grows by accepted spans. An over-budget span is pushed to `omissions` with `TokenLimit { tokens }` and packing continues; text is only ever the full `slice_lines` of the declaration (`:442`), never cut. `max_tokens == 0` omits every span with a token reason; `max_files == 0` omits every span with `FileLimit` (documented on `pack_candidates`, unit-tested). Missing blob for a non-file-limited candidate is `SelectError::MissingBlob` (`blob_text`, `:490`), surfaced through the facade as `Error::Select`.
- Regression of `rank` / `pack`: the base bodies were moved verbatim into `rank_files` / `pack_candidates_into` with only the exclusion predicate generalized (`hit.path != changed_path` → `!excluded.contains(hit.path)`, equivalent for one file). `pack` still rejects `max_files == 0` with `NoFileBudget`, still starts `files: 1`, still raises `ChangedHunkOverBudget`. The select diff contains no removed test lines; the journey diff removes only an import line that was widened. Public signatures of `rank`, `pack`, `ContextLimits`, `Context` are unchanged.
- Facade validation: `SymbolicContext::rank_files` runs `prepare_changed` for every blob before ranking (canonical path via `RelativePath::new`, UTF-8 decode, `ChangedLines::new` for nonempty/ordered/in-source ranges, extraction), failing on the first error. Journey asserts `DeletionOnly` for an empty range list. Empty changed list returns `Ok(vec![])`; duplicate changed paths dedupe through the `BTreeSet` and union names — neither is prohibited by REQ-003 or the rules, so not reported. A hand-built `files` list containing a changed path is outside the facade's contract (`rank_files` already excludes them) and is not an obligation.

## Verification

- `mise run test:select` → exit 0 (10 passed).
- `mise run test:symbolic-context:journey` → exit 0 (6 passed).

## Verification limits

- No scratch probe project was built; every questioned edge (zero limits, underflow, exclusion of the non-first changed path, order independence) is already asserted by the candidate's own unit/journey tests, which I read and ran rather than re-deriving.
- Nested-declaration double counting in `candidate_spans` (a method inside a matched class packs as two overlapping spans) is pre-existing base behavior with a documented ceiling comment; out of task scope.

## Findings

None.

Overall: PASS

```citations
[]
```
