# TASK-001 r3 domain review: ranking determinism and budget safety

Reviewer: `domain-rev` (read-only). Base `9715440d`, candidate `1fbcc4af`.

## Reviewed boundary

- `crates/bohmian-code-review-select/src/lib.rs`: `ChangedLines::new`, `ChangedFile::query_names`, `rank`, `rank_files`, `pack`, `pack_candidates`, shared `pack_candidates_into`, `candidate_spans`, `slice_lines`.
- `crates/bohmian-code-review-resolve/src/lib.rs`: `resolve` as consumed by `rank_files` (name dedupe and order).
- `crates/bohmian-code-review-index/src/lib.rs`: `SymbolIndex::lookup` hit order (postings are a `BTreeSet`; the outer `HashMap` is only keyed, never iterated, on the ranking path).
- `crates/bohmian-code-review/src/lib.rs`: `ChangedBlob`, `SymbolicContext::rank`, `SymbolicContext::rank_files`, `prepare_changed`, `build_candidate_context`, `build_context`.
- `crates/bohmian-code-review/tests/symbolic_context_journey.rs`: multi-file and candidate-only journey.

## Authority and source coverage

- Spec REQ-003 (line 57), INV-002 (86), INV-004 (88), AC-001 (115).
- `architecture/agent-rules.md` line 7 (validate nonempty ranges and token counts; no hidden omission/budget success).
- Diff `/tmp/task001-r3-diff.patch` for the select crate, facade, and journey test read in full; base and candidate `rank`/`pack` bodies compared hunk by hunk.

## Findings against each question

Input validation. Every changed file passes through `ChangedLines::new` (select 59-86): empty -> `DeletionOnly` (61), zero/reversed -> `InvalidRange`, non-ascending/overlap -> `UnorderedRanges`, past head -> `RangeBeyondSource`. The facade `prepare_changed` (facade 299) runs it for each `ChangedBlob` and `rank_files` collects `Result` so the first failure aborts before any ranking (facade 273). Token counts are produced only by the caller's `count_tokens` and compared against the remaining budget before any accumulation (select 361, 448); `context.tokens` can never exceed `max_tokens`, so the subtractions cannot underflow and the additions cannot overflow (probed with `count_tokens = |_| usize::MAX`, `max_tokens = usize::MAX`: one span selected, the rest omitted, no panic).

Multi-file union and distinct scoring. `rank_files` chains every file's `query_names` (declared-overlapping names plus reference/candidate names on changed lines) into one iterator; `resolve` collects them into a `BTreeSet` (resolve 49) so a name changed in two files, or both declared and referenced, is looked up once. Per candidate file, names go into a `BTreeSet<String>` (select 218) and `score = names.len()` (234), so each candidate scores once per distinct matched name regardless of how many hits or reasons it has. Probe: file A declares and calls `lookupUser` on changed lines, file B calls it too -> `src/user.ts` scores 1.

Changed-path exclusion. `excluded` is a `BTreeSet` of every changed path (select 210); every hit is filtered against it, so duplicates and a path that is both indexed and changed are dropped. Probe: passing the same `ChangedBlob` twice yields an identical ranking; journey asserts `src/report.ts` (indexed and changed) never ranks.

Ordering. Candidates are accumulated in a `BTreeMap<&RelativePath, _>` and then sorted by descending score then ascending path (select 238-243). Paths are unique map keys, so the comparator is total; reasons are appended in `resolve`'s name order then `lookup`'s posting order, so neither ranking nor reasons depend on input order or on `HashMap` iteration. Owner test (select 807) and probe confirm `[a, b] == [b, a]`.

Packing safety. `pack_candidates_into` checks `files >= max_files` before touching a file's blob and emits a `FileLimit` omission for every span of that file (select 428-437); a span whose tokens exceed the remaining budget emits `TokenLimit { tokens }` and packing continues (448-455); `files` increments only when at least one span was selected (470). Spans are whole declarations from `candidate_spans` and `slice_lines` returns `None` -> `SpanBeyondSource` rather than a truncated slice (442). Probes: `max_tokens` equal to the first span packs exactly that span (`tokens == 11`); `max_tokens = 0` selects nothing and reports every span; a short blob fails with `SpanBeyondSource`. No path drops a span silently.

Candidate-only counting. `pack_candidates` starts from `Context::default()` (`files = 0`) and `pack` starts from `files = 1`; the shared function is otherwise identical, so candidate-only `files` counts only selected candidate files. Journey (test 585) and owner test assert `(files, tokens)` accordingly. `max_files = 0` in candidate-only packing omits every candidate with an explicit `FileLimit` reason, which is honest coverage rather than hidden success; `pack` still rejects it with `NoFileBudget` because the changed file must fit.

Single-file compatibility. `rank` now builds one `ChangedFile` and calls `rank_files`; the only semantic change is `hit.path != changed_path` -> `!excluded.contains(hit.path)` with a one-element set, which is equivalent. `pack`'s hunk loop is untouched and its candidate loop was moved verbatim into `pack_candidates_into` (diff hunk `@@ -329,7 +376,54 @@` changes only the loop header and the return). The pre-existing select tests and `pr_diff_selects_user_context_deterministically` pass unchanged; probe asserts `rank(...).files == rank_files(&[one])`.

## Verification run

- `mise run test:select`: 10 passed, 0 failed.
- `mise run test:symbolic-context:journey`: 6 passed, 0 failed.
- Scratch crate `/tmp/budget-probe` (path dependency on the public facade): order independence, duplicate changed blob, declared-and-referenced name scoring once, single-file vs one-element multi-file equality, exact token budget, zero token budget, `usize::MAX` token counts, `max_files = 1` changed-hunk-only packing, short candidate blob, empty range, zero-start range, range past head. All assertions held; error variants observed: `InvalidRange`, `RangeBeyondSource`, `SpanBeyondSource`.

## Verification limits

- `mise run gate`, `fmt:check`, and `lints` were not run here (owned by the repo-wide reviewer).
- `SymbolIndex::load`/`export` equivalence was not re-derived beyond the journey assertion `reloaded == fresh`; that belongs to the facts domain reviewer.
- Observation, not a finding: `rank_files(&[])` returns `Ok(vec![])`. REQ-003 defines input as "multiple changed head-side blobs"; per-file deletion-only input is already rejected, so an empty set is the caller's explicit decision, not a hidden omission.

## Findings

None.

Overall: PASS

```citations
[
  {"id": "select-deletion-only", "path": "crates/bohmian-code-review-select/src/lib.rs", "line": 61, "snippet": "            return Err(SelectError::DeletionOnly);", "symbols": ["ChangedLines::new", "SelectError::DeletionOnly"]},
  {"id": "select-excluded", "path": "crates/bohmian-code-review-select/src/lib.rs", "line": 210, "snippet": "    let excluded: BTreeSet<&RelativePath> = changed.iter().map(|file| file.path).collect();", "symbols": ["rank_files"]},
  {"id": "select-distinct-names", "path": "crates/bohmian-code-review-select/src/lib.rs", "line": 218, "snippet": "            names.insert(found.name.clone());", "symbols": ["rank_files"]},
  {"id": "select-score", "path": "crates/bohmian-code-review-select/src/lib.rs", "line": 234, "snippet": "            score: names.len(),", "symbols": ["rank_files", "RankedFile"]},
  {"id": "select-tiebreak", "path": "crates/bohmian-code-review-select/src/lib.rs", "line": 242, "snippet": "            .then_with(|| left.path.cmp(&right.path))", "symbols": ["rank_files"]},
  {"id": "select-hunk-remaining", "path": "crates/bohmian-code-review-select/src/lib.rs", "line": 361, "snippet": "        let remaining_tokens = limits.max_tokens - context.tokens;", "symbols": ["pack"]},
  {"id": "select-file-limit", "path": "crates/bohmian-code-review-select/src/lib.rs", "line": 428, "snippet": "        if context.files >= limits.max_files {", "symbols": ["pack_candidates_into"]},
  {"id": "select-span-beyond", "path": "crates/bohmian-code-review-select/src/lib.rs", "line": 442, "snippet": "            let text = slice_lines(source, lines).ok_or_else(|| SelectError::SpanBeyondSource {", "symbols": ["pack_candidates_into", "slice_lines", "SelectError::SpanBeyondSource"]},
  {"id": "select-token-limit", "path": "crates/bohmian-code-review-select/src/lib.rs", "line": 448, "snippet": "            if tokens > limits.max_tokens - context.tokens {", "symbols": ["pack_candidates_into"]},
  {"id": "select-files-count", "path": "crates/bohmian-code-review-select/src/lib.rs", "line": 470, "snippet": "        context.files += usize::from(selected);", "symbols": ["pack_candidates_into"]},
  {"id": "select-order-test", "path": "crates/bohmian-code-review-select/src/lib.rs", "line": 807, "snippet": "        assert_eq!(rank_files(&index, &[other, handler]), ranked);", "symbols": ["multi_file_ranking_unions_names_and_excludes_every_changed_path"]},
  {"id": "resolve-dedupe", "path": "crates/bohmian-code-review-resolve/src/lib.rs", "line": 49, "snippet": "        .collect::<BTreeSet<_>>()", "symbols": ["resolve"]},
  {"id": "facade-collect", "path": "crates/bohmian-code-review/src/lib.rs", "line": 273, "snippet": "            .collect::<Result<Vec<_>, Error>>()?;", "symbols": ["rank_files"]},
  {"id": "facade-validate", "path": "crates/bohmian-code-review/src/lib.rs", "line": 299, "snippet": "        let changed = ChangedLines::new(changed, decode(&path, head)?)?;", "symbols": ["prepare_changed", "ChangedLines::new"]},
  {"id": "facade-pack-candidates", "path": "crates/bohmian-code-review/src/lib.rs", "line": 336, "snippet": "    Ok(bohmian_code_review_select::pack_candidates(", "symbols": ["build_candidate_context", "pack_candidates"]},
  {"id": "journey-candidate-count", "path": "crates/bohmian-code-review/tests/symbolic_context_journey.rs", "line": 585, "snippet": "    assert_eq!((packed.files, packed.tokens), (1, 11));", "symbols": ["multi_file_pr_ranks_union_of_changed_names_and_packs_candidates_only"]}
]
```
