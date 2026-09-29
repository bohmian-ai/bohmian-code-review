# Domain review: rankers, persisted reference facts, and candidate packing

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d` → `100249c3031d645ce8db9148b4b6fa43e71481d1`
**Result:** PASS

## Boundary and authority

Reviewed intent decisions 1, 2, 4, 5, and 6 against `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, and `architecture/references/domain/symbolic-context.md`. I traced extraction and reference-name projection through `SymbolicContext::upsert`, index export/load, the four rankers, facade ranking, candidate packing, and the pipeline's ranking and packing calls. I compared C/E/F formulas against the base experiment in `examples/context_relevance/trial.rs`.

| Obligation | Source and verification evidence | Result |
|---|---|---|
| Public IO-free `Ranker`; overlap, rarity, co-change, fusion with prior C/E/F semantics | `select/src/rank.rs` owns the trait and formulas; facade re-exports them. Formula and ordering match the base experiment. Owner ranker tests and the symbolic journey exercise them. | PASS |
| Default F; empty history follows C ordering | `DEFAULT_RANKER` and `Fused::rank`; owner fusion test and public symbolic journey cover both history states. | PASS |
| Distinct reference/candidate names persist with snapshot format 4 | Facade collects `reference_names` into a `BTreeSet`; index exports source-free names and loads them into postings; `SNAPSHOT_FORMAT_VERSION` is 4. Public journey verifies export/reload and absence of source fragments. | PASS |
| Fractional selected scores and context format 2 | `RankedFile`, `SpanKind::Candidate`, and pipeline `SelectedSpan` use `f64`; `CONTEXT_FORMAT_VERSION` is 2. | PASS |
| Co-change-only files pack declarations in location order within budgets | `candidate_spans` uses indexed declarations when reasons are empty; `pack_candidates_into` counts tokens and files before selection and records omissions. Owner packing test and public journey cover this path. | PASS |

## Verification limits

The parent orchestrator reported `mise run gate` and `git diff --check` passing at the candidate commit. I did not repeat the full gate. The domain review is source and test inspection; no live provider run was needed for this IO-free boundary.

## Material findings

None.

```citations
[]
```
