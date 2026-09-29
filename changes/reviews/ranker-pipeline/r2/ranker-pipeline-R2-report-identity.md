# Ranker pipeline R2 remediation — report decision identity

**Kind:** remediation for `FIND-ranker-pipeline-1` (`CLASS_INCOMPLETE`). **Status:** ready for `$cr-implement`. **Approved authority:** `changes/reviews/ranker-pipeline/intent.md` decision 3, `architecture/references/domain/pipeline-consumer-ergonomics.md`, and repository rules. There is no original task file by maintainer direction. **Reviewed subject:** base `322a2b9e0984062625d54274798a9c32cb01142d`, candidate `efc381f776e397ace4f43f9f4ab45fc4187436a5`. Re-review the complete cumulative base-to-new-candidate range after correction.

## Diagnosis

The public `live::run` accepts validated caller-named `Arms` (`crates/bohmian-code-review-eval/src/live.rs:356-360`), and `Summary::new` computes effects and the decision verdict for the selected baseline and decision (`report.rs:291-305`). But `live::record` receives only the summary (`live.rs:392`) and writes the fixed `DECISION_RULE` and `repository_f_minus_c` (`live.rs:462-464`); `run` writes `adopt_f` (`live.rs:383`). These three output sinks are siblings of one defect: the generic report serialization boundary has lost the arm identities and retained the former fixed experiment's labels. The fixed rule itself is defined in `report.rs:28` and explicitly names F/C.

A public call with `oracle` as decision and `silent` as baseline can therefore produce an `oracle-silent` effect and numeric verdict, yet describe that verdict as F versus C. The existing named-arm consumer test stops at `Evaluation::finish`; the fake-provider live report test uses C/F, so both pass without exposing the mismatch. R1's correction made the trial and numeric summary generic but left this report owner fixed, reopening the original finding's class. The exact source-path reproduction, callers, and citation-checked evidence are in `findings-validation.md`.

## Correction outcome and boundary

The existing live report owner must carry the already validated `Arms` decision and baseline identities into every decision field it emits: the repository verdict, aggregate adoption, and decision rule. Select the smallest correction in that owner using `Arms::decision()` and `Arms::baseline()`; avoid a second evaluation path, duplicate label state, or caller-side rewriting. For the fixed S/C/E/F binary, preserve the checked-in C/E/F report's exact keys, rule, values, and command. For other caller-named arms, emit truthful decision labels and rule that identify the selected pair. The candidate-owned report output shape may be adjusted for custom arms; no approved public surface or product decision changes.

Preserve existing sample selection, named trial means/effects, seeded bootstrap, numeric decision rule and threshold, provider ownership, source-free facts/history, and current pipeline/ranker contracts. Do not add a ranker, provider, dependency, background worker, algorithm configuration, or unrelated refactor. The correction is confined to the eval report owner and its focused tests; include the live binary or shared report owner only if required to preserve fixed-output compatibility.

## Acceptance and proof

| Finding and sibling | Required observable result | Focused proof |
|---|---|---|
| FIND-ranker-pipeline-1, `live.rs:462-464` | Custom-arm repository verdict and rule identify the actual decision and baseline, while the fixed F/C report retains its exact legacy fields and text. | Run the existing fake-provider public `live::run` with two non-C/F caller-named arms; assert effect key, verdict identity, and rule. Reconstruct/compare the checked-in fixed reports. |
| FIND-ranker-pipeline-1, `live.rs:383` | Aggregate adoption identifies the same custom decision pair and retains `adopt_f` for the fixed binary. | Assert the custom-arm aggregate output and fixed-binary output in the same focused report tests. |

Record exact focused commands and outcomes. Run `mise run test:eval`, `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check`. A credentialed live run is conditional on a supplied token; local fake-provider and report regression proof is required regardless. **Write set:** `crates/bohmian-code-review-eval/src/live.rs`, affected report owner or binary under `crates/bohmian-code-review-eval/src/` if needed, and focused inline tests. Leave `changes/active/impact-map/` untouched.
