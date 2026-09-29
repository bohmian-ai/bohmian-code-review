# Domain review: ranking, packing, facade, and eval arms

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d` → `e52510f112b822f7f7692535b26ae81e53ac4b03`
**Result:** PASS

## Boundary and authority coverage

Reviewed intent decisions 1–3 and 5–7, R1 FIND-ranker-pipeline-1, R2 FIND-ranker-pipeline-1, `AGENTS.md`, `architecture/agent-rules.md`, `architecture/bohmian-code-review-design.md`, and the applicable pipeline consumer reference. Traced public `SymbolicContext::rank_files`/`rank_with` through all four rankers, `build_candidate_context` through candidate packing, pipeline PR use of the default ranker, and caller-named eval arms through `trial::evaluate`, `Evaluation`, `Summary`, and `live::run` report serialization. Inspected the cumulative diff and the checked-in C/E/F report regression.

| Obligation | Source and verification evidence | Result |
|---|---|---|
| Public IO-free `Ranker`, S/C/E/F semantics, deterministic order and exclusions; default F with empty-history C order | `select/src/rank.rs`, facade re-export and `rank_with`; select ranker tests and symbolic journey | PASS |
| Fractional scores and bounded candidate context, including co-change-only declaration spans in location order | `select/src/lib.rs` packing and `pipeline/src/context.rs` score; select packing tests and symbolic journey | PASS |
| Caller-supplied sample and named rankers through trials, per-PR means, paired effects, bootstrap and decision | `eval/src/{trial,report,live}.rs`; custom-arm consumer and fake-provider tests | PASS |
| R2 report identity: custom decision/baseline labels and fixed C/F compatibility | `Arms::{rule,verdict_key,adopt_key}`, `live::{run,record}`; custom-arm report test and checked-in report reconstruction | PASS |

## Verification and limits

At the immutable candidate, `mise run test:select` passed 15 tests, `mise run test:eval` passed 26 library tests and one binary test, and `mise run test:symbolic-context:journey` passed seven tests. The live authenticated command was not run; this boundary has fake-provider and checked-in report regression coverage. This review did not independently audit provider pagination, snapshot publication, or persistence; those are separate domain boundaries.

## Material findings

None. The prior report-identity defect closes at the report owner: `live::run` carries validated `Arms` into `record`, and its adoption, repository verdict, and rule use those arms. The fixed F/C binary still supplies the same arms and the report regression test checks the legacy output.

```citations
[]
```
