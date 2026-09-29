# Domain review: ranking, packing, persisted facts, facade, and evaluation

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d` → `cd93bdadfa5cb17ed54752168e4a445c2e660068`
**Result:** PASS

## Boundary and authority coverage

Reviewed the cumulative diff against `changes/reviews/ranker-pipeline/intent.md` decisions 1–6 and 8, `architecture/references/domain/pipeline-consumer-ergonomics.md`, `architecture/bohmian-code-review-design.md`, `AGENTS.md`, `architecture/agent-rules.md`, and R1/R2/R3 remediation. Traced the public `SymbolicContext` upsert, export, load, rank, and pack paths through the core fact validator, index, resolver, four rankers, and select packer. Traced caller-named eval arms through trials, per-PR means, effects, verdict, and report serialization. Checked the pipeline snapshot decode and reuse sinks for malformed fact handling. The history reader and provider listing boundaries are assigned to their separate domain reviews.

| Obligation | Evidence | Result |
|---|---|---|
| IO-free public `Ranker`; S/C/E/F semantics, deterministic ties and exclusions; default F equals C order with empty history | `select/src/rank.rs`, facade re-exports and `rank_with`; select unit tests and public journey | PASS |
| Fractional scores and bounded context packing; reasonless co-change candidates use indexed declaration location order | `select/src/lib.rs` `RankedFile`, `candidate_spans`, `pack_candidates_into`; select tests and public journey | PASS |
| Source-free, distinct persisted reference names and explicit malformed-fact rejection before replacement (FIND-ranker-pipeline-4) | `core/src/facts.rs` `IndexedFileFacts::validate`; index `load`; facade `load`; snapshot `decode`; core, index, facade journey, and snapshot regression tests | PASS |
| Reusable caller-supplied eval sample and named rankers through trial, mean, effect, bootstrap, decision, and compatible report shape | `eval/src/{trial,report,live}.rs`; named-arm and checked-in report regression tests | PASS |
| Consumer default remains fixed; public facade supplies two-pass symbolic journey | `SymbolicContext::rank_files`, `rank_with`, `build_candidate_context`; `ReviewPipeline` uses the default; public journey | PASS |

## Verification and limits

At the immutable candidate, `mise run test:select` passed 15 tests, `mise run test:eval` passed 26 library and one binary test, and `mise run test:symbolic-context:journey` passed seven tests. The authenticated live evaluation was unavailable without a token; fake-provider and checked-in report tests cover this domain. One initial combined `mise run` command failed because `mise` passed subsequent task names to Cargo; the three tasks then passed individually. No source was edited.

## Material findings

None. FIND-ranker-pipeline-4 closes at the shared fact validator: empty, repeated, and unordered names fail before index mutation; the public facade preserves prior postings on rejection, and snapshot decode rejects malformed persisted facts. No ranker, packing, or eval regression was reproduced in the cumulative range.

```citations
[]
```
