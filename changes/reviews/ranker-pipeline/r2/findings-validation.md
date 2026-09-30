# Wave 2 findings validation

Subject: `322a2b9e0984062625d54274798a9c32cb01142d..efc381f776e397ace4f43f9f4ab45fc4187436a5`. Authority: `changes/reviews/ranker-pipeline/intent.md`, `architecture/references/domain/pipeline-consumer-ergonomics.md`, repository rules, and the R1 remediation task. There is no original task file. The candidate source was not edited.

## Blocking ledger

### FIND-ranker-pipeline-1 — CLASS_INCOMPLETE

- **Wave 1 sources:** TASK-B-1, DOMAIN-RANK-1, merged as one root cause. **Gate:** task contract. **Class:** INCORRECT. **Defect class:** the caller-selected decision and baseline arm identities are lost at the report serialization boundary, leaving fixed F/C decision metadata.
- **Obligation:** decision 3 of `changes/reviews/ranker-pipeline/intent.md:9` requires a reusable named-ranker harness through the decision rule and existing report shape. The R1 correction explicitly requires named rankers through report output and a public consumer proof of named decision output (`changes/reviews/ranker-pipeline/r1/ranker-pipeline-R1-complete-contracts.md:11`).
- **Root owner and siblings:** `crates/bohmian-code-review-eval/src/live.rs:356-376` accepts `Arms` and computes a `Summary`, but `record` receives only that summary (`live.rs:392`), whose `verdict` has no arm identity (`report.rs:241-253`). `record` inserts the fixed `DECISION_RULE` (`live.rs:462`, defined at `report.rs:28`) and `repository_f_minus_c` (`live.rs:464`); `run` inserts `adopt_f` (`live.rs:383`). These are three outputs of the same identity loss. The named effect keys are already correct (`report.rs:291-305`, `live.rs:422-426`), so this finding does not include effect computation.
- **Independent reproduction:** `Arms::new` accepts `("silent", "oracle")` and exposes both identities (`report.rs:193-231`); `Evaluation::finish` computes the `oracle-silent` effect and its verdict (`report.rs:282-311`). The existing public consumer test uses those exact arms and observes `oracle-silent` plus `Verdict::Met` (`report.rs:629-665`). The public `live::run(api, repos, arms)` passes any such `Arms` through evaluation, then calls `record` and writes the fixed labels above. Thus a successful `run` with `oracle`/`silent` reports a true `oracle-silent` verdict under keys and a rule claiming F/C, regardless of the numerical verdict. `live::print` is a direct caller of `run` (`live.rs:482-491`); the binary supplies C/F (`src/bin/context_relevance.rs:46-49`). The fake live test supplies only C/F (`live.rs:793-859`), so it cannot catch this case. This is a complete source-path reproduction from the public runner; no credentials are needed.
- **Observable consequence:** a consumer reading a custom-arm report attributes the adoption decision to F over C. The fixed experiment remains correctly named, which hid the defect in its tests.
- **Why R1 remained incomplete:** R1 made `Arms` the validated identity source for trials and effects, but the candidate-owned report owner still accepts a `Summary` without those identities and retained fixed experiment labels. Repeating guards at callers would leave the report owner capable of the same mismatch.
- **Minimal correction:** make the existing report construction use the validated `Arms` identities already passed to `run` for all three decision labels and its rule. Keep the exact F/C keys and rule text for the fixed C/F binary and checked-in reports; for other named arms, make the repository verdict, aggregate adoption, and rule identify the actual decision/baseline pair. Preserve evaluation, sampling, history, provider ownership, and numeric decision semantics. No new ranker, harness, provider, dependency, or product decision is needed.
- **Focused closure proof:** run the existing fake-provider `live::run` with two caller-named arms distinct from C/F and assert that `effects`, repository verdict label, aggregate adoption label, and rule all identify that pair; assert the fixed C/F run retains its exact legacy labels and rule. The public consumer journey continues to prove named hit@10 and numeric verdict.

## Notes and rejected claims

- No nonblocking note from the surviving claims.
- HIST-1, DOMAIN-PROVIDER-1, and STANDARD-1 were rejected in full by the citation check and are excluded. No rejected surviving claim remains.

## Validation limit

Reproduction used the complete public call path and the existing named-arm consumer test, rather than a live authenticated request. The correction proof needs a custom-arm fake-provider report assertion because the current fake-provider report test supplies only C/F.
