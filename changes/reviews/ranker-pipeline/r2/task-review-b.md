# Ranker pipeline R2 — independent task review B

**Subject:** `322a2b9e0984062625d54274798a9c32cb01142d` through `efc381f776e397ace4f43f9f4ab45fc4187436a5`. **Authority:** `changes/reviews/ranker-pipeline/intent.md` decisions 1–8, `architecture/references/domain/pipeline-consumer-ergonomics.md`, repository rules, and the R1 remediation task. No original task file exists by maintainer direction. I inspected the cumulative diff and candidate source without another R2 reviewer's conclusions. Candidate remained HEAD during this review.

## Acceptance matrix

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| 1. Public IO-free `Ranker`; S/C/E/F experiment semantics | `select/src/rank.rs`, `bohmian-code-review/src/lib.rs` re-export | Selector and public symbolic journey tests in the candidate; `mise run test:eval` passed | PASS |
| 2. Default F; empty history follows C order | `DEFAULT_RANKER`, `Fused::rank` | Selector and symbolic journey cases | PASS |
| 3. Feature-gated reusable named-arm hit@10 harness, thin binary, preserved report | `eval/src/trial.rs`, `report.rs`, `live.rs`, binary; report still claims F/C for arbitrary public arms | `mise run test:eval` passed, including named-arm consumer and checked-in report regression; no arbitrary-arm live-report assertion; TASK-B-1 | FAIL |
| 4. Persist source-free distinct reference/candidate names; snapshot v4 | `index/src/lib.rs`, `pipeline/src/snapshot.rs` | Index and pipeline tests; `mise run test:pipeline` passed | PASS |
| 5. Fractional score; context v2 | `select/src/lib.rs`, `pipeline/src/context.rs` | Selector and pipeline tests | PASS |
| 6. Co-change-only candidates pack indexed declarations under budgets | `select/src/lib.rs` | Symbolic journey co-change case | PASS |
| 7. History in both passes, bounded and complete-or-fail, source-free, time-eligible | `pipeline/src/history.rs`, `snapshot.rs`, `update.rs`, `pull_request.rs` | R1 cap and movement regressions; `mise run test:pipeline` passed | PASS |
| 8. One `ReviewPipeline` with one storage/store/limits/error, per-call client; stage facades public | `pipeline/src/review.rs`, `lib.rs`, README | Public pipeline journey; `mise run test:pipeline` passed | PASS |
| Repository boundaries and non-goals: no symbolic IO or source persistence, store-clock durable time, no worker, unchanged grammar pins and fixtures | Crate manifests, exports, snapshot/context records, cumulative diff | `git diff --check` passed; source inspection | PASS |

## Proposed finding

**TASK-B-1 — INCORRECT; prior FIND-ranker-pipeline-1 class remains incomplete.** Defect class: the public evaluation runner accepts caller-named arms but its report owner serializes a fixed F-versus-C decision identity. The root cause is `live::record`'s fixed decision field and rule (`live.rs:461–465`); the sibling is `live::run`'s fixed `adopt_f` field (`live.rs:380–384`). `Arms` accepts arbitrary decision and baseline names (`report.rs:200–204`) and `Summary::new` computes their actual verdict (`report.rs:291–310`), so a consumer calling public `live::run` with, for example, `oracle` against `silent` receives correctly named effects but a report saying `repository_f_minus_c`, `adopt_f`, and a rule about F/C. This makes the emitted report misidentify the evaluated decision, violating intent decision 3's reusable named-ranker harness and report output. The existing named-arm test stops at `Evaluation::finish`; the live-report test uses fixed F/C arms, leaving this path unproved. Correct the report serialization at the shared `record`/`run` owner using the already validated `Arms` names, while retaining the exact existing F/C keys and rule for the fixed binary. A fake-provider public `run` test with non-F/C names should assert that decision labels and rule name the supplied arms and the fixed binary report regression remains exact. This is a candidate-owned output shape, not a new approved product decision.

## Verification and result

`mise run test:eval`, `mise run test:pipeline`, and `git diff --check` all exited 0. The live credentialed command was not run; its fixed report is covered by checked-in report regression, but arbitrary-arm live output is not. No source was modified.

**Overall: FAIL.**

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":462,"snippet":"            \"rule\": DECISION_RULE,","symbols":["record","DECISION_RULE"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":464,"snippet":"            \"repository_f_minus_c\": verdict,","symbols":["record"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-eval/src/live.rs","line":383,"snippet":"            record[\"decision\"][\"adopt_f\"] = json!(adopt);","symbols":["run","record"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-eval/src/report.rs","line":305,"snippet":"        let verdict = verdict(&effect(&means, arms.decision, baseline, BOOTSTRAP_SEED));","symbols":["Summary::new","Arms"]}]
```
