# Ranker pipeline task review R2

**Verdict: FIX_REQUIRED.** Immutable subject: base `322a2b9e0984062625d54274798a9c32cb01142d` through candidate `efc381f776e397ace4f43f9f4ab45fc4187436a5`; HEAD remained the candidate during source review. The human-approved authority is `changes/reviews/ranker-pipeline/intent.md` decisions 1–8, `architecture/references/domain/pipeline-consumer-ergonomics.md`, and repository rules. No original task file exists by maintainer direction. R1's verdict, findings, and `ranker-pipeline-R1-complete-contracts.md` were included. `changes/active/impact-map/` is untracked and outside the subject.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| 1. Public IO-free Ranker with S/C/E/F semantics | `select/src/rank.rs`, facade re-exports | Ranker tests, public symbolic journey, full gate | PASS |
| 2. Fused default and rarity order with empty history | `DEFAULT_RANKER`, `Fused::rank` | Selector and journey tests in gate | PASS |
| 3. Feature-gated named-ranker evaluation, decision, thin live binary, preserved fixed report | `eval/src/{trial,report,live}.rs`; report identity is fixed at `live.rs:383,462-464` | Named-arm tests and report regression pass, but custom-arm live report is wrong; FIND-ranker-pipeline-1 | FAIL |
| 4. Source-free reference/candidate names, snapshot v4 | `index/src/lib.rs`, `snapshot.rs` | Index and pipeline tests in gate | PASS |
| 5. Fractional selected scores, context v2 | Selector and `context.rs` | Selector and pipeline tests in gate | PASS |
| 6. Co-change-only candidate declaration packing under budgets | Selector candidate fallback | Selector and symbolic journey tests in gate | PASS |
| 7. Co-change history in both passes, bounded and complete-or-fail, persisted and time-eligible | `history.rs`, snapshot/update and PR context | Pipeline tests in gate; history domain claim excluded by citation check | PASS on admissible evidence; limit below |
| 8. One ReviewPipeline with one storage/store/limits/error and per-call client; public stage facades | `review.rs`, facade exports, README | Public pipeline journey and gate | PASS |
| Repository boundaries and non-goals | Cumulative diff, manifests, stage ownership and tests | Full gate and source review; rejected standards/provider claims excluded | PASS on admissible evidence; limit below |

## Review and verification

Wave 1 produced `task-review-a.md` PASS, `task-review-b.md` FAIL, `standards-review.md` FAIL, `domain-review-history.md` FAIL, `domain-review-provider.md` FAIL, and `domain-review-rank.md` FAIL. The standards and history/provider proposals were **excluded**, not accepted as findings: `mise run review:verify-citations changes/reviews/ranker-pipeline/r2 efc381f776e397ace4f43f9f4ab45fc4187436a5` exited 0 but printed `REJECT` for one or more citations under each of `STANDARD-1`, `HIST-1`, and `DOMAIN-PROVIDER-1`. The complete output is in `citations.txt`. It accepted every citation under `TASK-B-1` and `DOMAIN-RANK-1`. The citation-checked claim set is in `claim-ledger.md`; independent Wave 2 reproduction and deduplication are in `findings-validation.md`.

`mise run gate` and `git diff --check 322a2b9e0984062625d54274798a9c32cb01142d efc381f776e397ace4f43f9f4ab45fc4187436a5` both exited 0 in this review. The gate covered format, Clippy, all component/eval and public journeys, review-script checks, and skill sync. The authenticated live command was unavailable without a token; local fake-provider and checked-in report regression tests passed. Citation rejection leaves the three excluded proposals unadjudicated by this verdict; green tests do not settle their claims.

## Validated blocking ledger and prior closure

| ID | Status and gate | Root cause and consequence |
|---|---|---|
| FIND-ranker-pipeline-1 | CLASS_INCOMPLETE; task contract | The generic live report loses caller-selected baseline/decision identities and emits the fixed F/C rule and verdict/adoption keys. A custom-ranker consumer receives a numerically correct verdict under false labels. |

The R1 input-shape correction is only partially closed: named rankers now reach trials, per-PR means, effects, and numeric verdict, but not report decision identity. R1 FIND-ranker-pipeline-2 and FIND-ranker-pipeline-3 had no **admissible** reopened claims in this review; their proposed domain claims were citation-rejected as stated above. No nonblocking notes survived. The bounded correction is `ranker-pipeline-R2-report-identity.md`.
