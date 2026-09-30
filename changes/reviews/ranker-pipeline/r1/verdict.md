# Ranker pipeline task review R1

**Verdict: FIX_REQUIRED.** Reviewed immutable base `322a2b9e0984062625d54274798a9c32cb01142d` through candidate `100249c3031d645ce8db9148b4b6fa43e71481d1` (candidate remained HEAD throughout). The human-approved authority is [`intent.md`](../intent.md), decisions 1–8, plus `architecture/references/domain/pipeline-consumer-ergonomics.md` and repository rules. There is no original task file by maintainer direction. The untracked `changes/active/impact-map/` is outside this review and the diff.

## Acceptance matrix

| Decision | Candidate evidence | Verification evidence | Result |
|---|---|---|---|
| 1. Public IO-free Ranker and S/C/E/F semantics | `select/src/rank.rs`; facade re-exports | Ranker tests and symbolic journey in gate; rank domain review | PASS |
| 2. F default, C order with empty history | `DEFAULT_RANKER`, `Fused::rank` | Fusion test and symbolic journey in gate | PASS |
| 3. Feature-gated reusable evaluation over a supplied sample and named rankers; thin live binary and preserved report | Eval crate exists, but `trial::evaluate` fixes arms and `live` fixes sampling and report arms | Eval lane passes only fixed S/C/E/F; FIND-ranker-pipeline-1 and -3 | FAIL |
| 4. Source-free distinct persisted reference/candidate names; snapshot v4 | Index facts export/load and snapshot version | Index and pipeline tests in gate; rank domain review | PASS |
| 5. Fractional selected scores; context v2 | `SelectedSpan.score: f64`; context format version | Selector and pipeline tests in gate | PASS |
| 6. Co-change-only candidate declarations pack by location under budgets | Selector candidate span fallback | Selector and public journey tests in gate | PASS |
| 7. Complete-or-fail bounded history in both passes, persisted, always used with time eligibility | `CoChangeHistory::read`, snapshot/update and PR context | Gate passes, but its cap test accepts incomplete success; FIND-ranker-pipeline-2 | FAIL |
| 8. One consumer ReviewPipeline with shared storage/store/limits/error, per-call client and public stage facades | `review.rs`, facade exports, README | Pipeline public journey in gate; pipeline domain review | PASS |

The unchanged constraints and non-goals were checked through the standards and domain reviews: no source persisted in snapshot facts or history, no symbolic-crate IO, store-clock durable time, no server or background worker, and the public stage facades remain available. No unrelated tracked change entered the base-to-candidate diff.

## Review and verification

Wave 1: `task-review-a.md` FAIL, `task-review-b.md` FAIL, `standards-review.md` PASS, `domain-review-history.md` FAIL, `domain-review-rank.md` PASS, and `domain-review-pipeline.md` FAIL. Each report and its citations are preserved here. `mise run review:verify-citations changes/reviews/ranker-pipeline/r1 100249c3031d645ce8db9148b4b6fa43e71481d1` exited 0; its output is `citations.txt`. It rejected three citations under `TASK-B-2`, so that entire Wave 1 finding was dropped. The separately cited `DOMAIN-PIPE-2` claim survived and was independently reproduced. Wave 2's `findings-validation.md` merges the surviving claims by root cause.

`mise run gate` passed at candidate HEAD, covering format, lints, all component and public journey lanes, review-script self-check, and skill sync. `git diff --check` over the base-to-candidate range passed. The live GitHub evaluation and its exact checked-in report reproduction were not rerun because they require a token; the local fake-provider and eval tests passed. The moving-pagination case is established by a traced source path and needs the focused regression in remediation. There are no prior findings to close.

## Validated blocking ledger

| ID | Status and gate | Root cause and consequence |
|---|---|---|
| FIND-ranker-pipeline-1 | CONFIRMED; task contract | The eval API has no caller-supplied sample or named ranker inputs, so a consumer cannot evaluate its own sample and rankers as decision 3 requires. |
| FIND-ranker-pipeline-2 | REVISED, merging cap and pagination claims; task contract and history integrity | The shared history reader can report success before proving a stable, complete discovery window, so cold build or update may persist missing PR history. |
| FIND-ranker-pipeline-3 | CONFIRMED; repository standard | The eval live path performs authenticated REST reads outside the pipeline owner, duplicating Git provider access and validation. |

Full citations, caller and sibling sweeps, independent reproductions, correction boundaries, and closure proofs are in `findings-validation.md`. There are no nonblocking notes. The bounded remediation is [`ranker-pipeline-R1-complete-contracts.md`](ranker-pipeline-R1-complete-contracts.md).
