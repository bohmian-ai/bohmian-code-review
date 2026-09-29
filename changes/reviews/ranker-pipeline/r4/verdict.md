# Ranker pipeline task review R4

**Verdict: FIX_REQUIRED.** Immutable subject: base `322a2b9e0984062625d54274798a9c32cb01142d`, candidate `cd93bdadfa5cb17ed54752168e4a445c2e660068`; `HEAD` remained the candidate throughout review. Authority: `changes/reviews/ranker-pipeline/intent.md`, `architecture/references/domain/pipeline-consumer-ergonomics.md`, `AGENTS.md`, applicable architecture references, and the R1–R3 remediation tasks. The maintainer directed that there be no original task file. The untracked `changes/active/impact-map/` was untouched.

## Acceptance matrix

| Obligation | Implementation and verification evidence | Result |
|---|---|---|
| 1. Public IO-free S/C/E/F `Ranker` trait and semantics | `select/src/rank.rs`, facade re-export; selector and public journey in `mise run gate` | PASS |
| 2. Fused default and empty-history rarity ordering | Default ranker and fusion tests in gate | PASS |
| 3. Feature-gated reusable named-ranker evaluation and fixed report compatibility | `eval/src/{trial,report,live}.rs`; named-arm and checked-in report tests pass, but public live errors violate a repository rule | **FAIL** (FIND-6) |
| 4. Source-free distinct references and snapshot v4 | Core validation, index/facade load, snapshot decode; focused malformed-fact and journey tests pass | PASS; FIND-4 closed |
| 5. Fractional scores and context v2 | Selector and context version; gate passes | PASS |
| 6. Co-change-only declaration packing under budgets | Selector fallback and public journey; gate passes | PASS |
| 7. Complete-or-fail history and time-eligible fused PR ranking | Shared reader, update/cold publication, PR filter; focused cap and publication tests pass | PASS; FIND-2 closed |
| 8. One consumer `ReviewPipeline` entry with shared storage/store/limits/error and per-call client | `review.rs`, public pipeline journey; gate passes | PASS |
| Provider identity and repository boundaries | Shared listed-PR base identity check, both listing operations, publication proof; focused provider tests pass | PASS; FIND-5 closed |
| Written library error rule and non-goals | `AGENTS.md:30` versus public `eval/src/live.rs:340,361,493`; no new provider, worker, dependency, or source persistence | **FAIL** (FIND-6) |

## Independent review and verification

Wave 1: `task-review-a.md` **PASS**, `task-review-b.md` **PASS**, `standards-review.md` **FAIL** (`STANDARD-1`), `domain-review-history.md` **PASS**, `domain-review-provider.md` **PASS**, and `domain-review-rank.md` **PASS**. The final `mise run review:verify-citations changes/reviews/ranker-pipeline/r4 cd93bdadfa5cb17ed54752168e4a445c2e660068` exited 0 and marked all eight `STANDARD-1` citations `OK`; no citation was rejected (`citations.txt`). The claim-only handoff is `claim-ledger.md`. Independent reproduction and prior-ID disposition are in `findings-validation.md`.

`mise run gate` and `git diff --check 322a2b9e0984062625d54274798a9c32cb01142d cd93bdadfa5cb17ed54752168e4a445c2e660068` both exited 0 in this review. Reviewers also ran focused history, update, fact, provider, selector, eval, and public journey checks, recorded in their reports. A credentialed live GitHub run was unavailable without a token; the local fake-provider and report checks cover the diagnosed paths. The green gate does not enforce typed public error signatures.

## Validated ledger and prior closure

| Stable ID | Status and gate | Root cause and consequence |
|---|---|---|
| `FIND-ranker-pipeline-6` | **CONFIRMED**; written repository standard | The eval live workflow moved into a public library with `Box<dyn Error>` in its shared owner and public functions. Callers cannot match the documented failure classes through a typed error, violating `AGENTS.md:30`. |

`FIND-ranker-pipeline-1` and `-3` remain closed by R2 and R1 respectively. R3 closes `FIND-ranker-pipeline-2` (unjoined incremental history cap), `-4` (malformed persisted reference names), and `-5` (listed-PR base identity). The prior same-tree reuse note remains nonblocking because `architecture/bohmian-code-review-design.md:22` expressly requires it. No other note or rejected claim remains.

The bounded correction is `ranker-pipeline-R4-typed-eval-errors.md`. Re-review the complete cumulative range after implementation.
