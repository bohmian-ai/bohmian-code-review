# Domain review: multi-file ranking determinism and token/file budget safety (TASK-001 r2)

Reviewer: `domain-rev` (DOM-BUDGET). Subject: candidate 1108731565 vs base 9715440d, read-only.

## Reviewed boundary

- `crates/bohmian-code-review-select/src/lib.rs`: `ChangedFile::query_names`, `rank`, `rank_files`, `pack`, `pack_candidates`, `pack_candidates_into`, `candidate_spans`, `slice_lines`, `line_count`, and the owner tests added by the candidate.
- `crates/bohmian-code-review-resolve/src/lib.rs`: `resolve` (name dedup via `BTreeSet`), `reference_names`, `declaration_names`.
- `crates/bohmian-code-review-index/src/lib.rs`: `SymbolIndex::lookup` (hit order).
- `crates/bohmian-code-review-core/src/relative_path.rs`: derived `Ord` on `RelativePath(String)`.
- `crates/bohmian-code-review/src/lib.rs`: `rank`, `rank_files`, `prepare_changed`, `build_context`, `build_candidate_context`.
- `crates/bohmian-code-review/tests/symbolic_context_journey.rs`: `multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only`.

## Authority and source coverage

- `changes/active/repo-parser/spec.md` REQ-003 (line 57), INV-004 (line 88), AC-001 (line 115).
- `changes/active/repo-parser/tasks/TASK-001-symbolic-library-contract.md` Scenario 2 (lines 36-44).
- `AGENTS.md` ownership boundaries (select owns scoring/ranking/packing; facade owns no duplicate stage logic).
- Base comparison: `git diff 9715440d..1108731565 -- crates/bohmian-code-review-select/src/lib.rs crates/bohmian-code-review/src/lib.rs`.

## Questions settled from source and by execution

1. **Distinct name counts once.** `rank_files` chains every changed file's `query_names` into one iterator and hands it to `resolve`, which collects into a `BTreeSet<&str>` before lookup (resolve/src/lib.rs:47-50). Per file, `score = names.len()` on a `BTreeSet<String>` (select/src/lib.rs:213, 228-230). Scratch run: `shared` mentioned four times across two changed files scored 1 on each candidate.
2. **Every changed path excluded.** `excluded: BTreeSet<&RelativePath>` built from all `changed[].path`; hits filtered by `!excluded.contains(hit.path)` (select/src/lib.rs:208, 214). Scratch run: changed `src/b.ts` declaring `foo`, referenced from changed `src/x.ts`, did not appear as a candidate. Duplicate changed-file entries produce the same ranking.
3. **Tie-breaking is a total order.** `matched` is keyed by `&RelativePath` (unique per file), so two entries can never share a path; sort is `score desc` then derived byte-wise `RelativePath` ord (select/src/lib.rs:236-241; core relative_path.rs:21). Different-case paths are distinct keys and order bytewise (`src/A.ts` before `src/a.ts`). Reasons order derives from `BTreeSet` name order then index canonical hit order; `candidate_spans` re-keys through a `BTreeMap`. Same input yields equal output on repeated calls and on reversed changed-file order (verified).
4. **Budget arithmetic.** Candidate spans are admitted only when `tokens <= max_tokens - context.tokens` (select/src/lib.rs:446); `context.tokens` therefore never exceeds `max_tokens`, so the subtraction cannot underflow. `pack` admits hunks under the same rule before delegating. Files count only when at least one span of that file was selected and the file loop short-circuits at `context.files >= max_files` (select/src/lib.rs:429, 469). Scratch sweep over `max_tokens ∈ {0,1,3,4,5,7,8,11,12,13,15,16,17,100,usize::MAX} × max_files ∈ {0,1,2,3,usize::MAX}`: `tokens <= max_tokens`, `files <= max_files`, `files == distinct selected paths`, `tokens == sum(span.tokens)`, exact-fit (`==`) spans admitted, zero budget omits everything with `TokenLimit`, a later smaller span still fits after a larger omission, every span accounted for as selected or omitted. No separator or header tokens exist; only caller-counted span text is summed.
5. **Changed hunks excluded from candidate-only packing.** `pack_candidates` starts from `Context::default()` (files 0, tokens 0) and only `pack_candidates_into` runs (select/src/lib.rs:403-405); every emitted span is `SpanKind::Candidate` (asserted in owner test, journey test, and scratch).
6. **Single-file path preserved.** `rank` now builds one `ChangedFile` and calls `rank_files`; the base filter `hit.path != changed_path` is exactly `!{changed_path}.contains(hit.path)`. `pack`'s candidate loop body moved verbatim into `pack_candidates_into` (diff hunk @@ -329,7 +376,54). Facade `rank` keeps the base error order (path → UTF-8 → `ChangedLines::new` → extraction); the second `decode` inside `extract` is idempotent. Owner test `multi_file_ranking_unions_names_and_excludes_every_changed_path` and journey assert `rank(...).files == rank_files(&[one])`; base owner tests still pass unchanged.
7. **Sharing, not duplication.** Name collection lives once in `ChangedFile::query_names`; scoring once in `rank_files`; candidate packing once in `pack_candidates_into`. Facade delegates and duplicates no stage logic.
8. **Panics reachable from the facade.** None found on realistic input: no indexing or `unwrap` on caller data; `expect` only on the `all_native` invariant; `slice_lines`' `1u32..` counter would overflow only past 2^32 lines in one blob.

## Verification performed

- `mise run test:select` → exit 0, 10 passed.
- `mise run test:symbolic-context:journey` → exit 0, 6 passed.
- Scratch cargo project `/tmp/budget-scratch` (path deps on core/index/select/facade) exercising the cases above → `ALL OK`.

## Verification limits

- `fmt:check`, `lints`, `gate` not rerun by this reviewer (outside this domain scope; implementer reports exit 0).
- `pack_candidates` treats `max_files == 0` as all-omitted while `pack` returns `NoFileBudget`; this asymmetry is documented on both functions and matches REQ-003 ("emits explicit omissions"), so it is recorded as an observation, not a finding.
- Scenario 2 RED names "a budget failure"; candidate-only packing has no failing budget path by design, and the journey covers the starved-budget omission case plus `DeletionOnly`. Not a finding.

## Material findings

None.

```citations
[]
```

Overall: PASS
