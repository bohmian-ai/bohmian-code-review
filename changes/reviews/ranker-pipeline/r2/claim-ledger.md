# Citation-checked Wave 1 claims

Subject: `322a2b9e0984062625d54274798a9c32cb01142d..efc381f776e397ace4f43f9f4ab45fc4187436a5`. Authorities: `changes/reviews/ranker-pipeline/intent.md`, `architecture/references/domain/pipeline-consumer-ergonomics.md`, repository rules, and R1 findings/remediation. These are claims for independent Wave 2 validation, not findings or a verdict. The complete diff and authority are the reviewer's source of evidence; Wave 1 reasoning and matrices are excluded.

| Source ID | Reviewer | One-sentence claim | Root cause and defect class | Sibling locations | Verified citations |
|---|---|---|---|---|---|
| TASK-B-1 | task B | A custom-arm public eval run reports its decision as F versus C. | `live`'s report output still fixes experiment labels although its input accepts arbitrary arm names; incorrect report identity. | `live.rs:383,462,464`; `report.rs:305` supplies actual selected arms. | `live.rs:383` `record["decision"]["adopt_f"] = json!(adopt);`; `live.rs:462` `"rule": DECISION_RULE,`; `live.rs:464` `"repository_f_minus_c": verdict,`; `report.rs:305` `let verdict = verdict(&effect(&means, arms.decision, baseline, BOOTSTRAP_SEED));` |
| DOMAIN-RANK-1 | rank domain | The public eval run's report labels and rule remain F/C for arbitrary named arms. | Same root: fixed experiment metadata is emitted by generic report construction; incorrect report identity. | `live.rs:359,383,462,464`; `report.rs:28` fixed rule. | `live.rs:359` `arms: Arms<'_>,`; `live.rs:383` `record["decision"]["adopt_f"] = json!(adopt);`; `live.rs:462` `"rule": DECISION_RULE,`; `live.rs:464` `"repository_f_minus_c": verdict,`; `report.rs:28` `DECISION_RULE` constant. |

Citation check rejected at least one citation for each of `HIST-1`, `DOMAIN-PROVIDER-1`, and `STANDARD-1`; the skill requires dropping those entire claims. See `citations.txt`. Task A reported no finding.
