# Citation-checked Wave 1 claims

Subject: `2257992f36a2aa88f33f397af1e070d6ab35961b..6e9ab0f764f9d7961924dac7fb1e5c04be02a38c`; approved `SPEC-repo-parser@14`, original TASK-007, and `review/maintainer-decision-stateless-snapshot.md`.

The citation check rejected `TASK-A-1` because its symbol citations do not match the candidate. Its root-cause class remains represented by independently cited `TASK-B-1` and `STD-1`. No claim from the rejected finding is passed for validation.

| Source ID | Reviewer | One-sentence claim | Root cause and defect class | Sibling locations | Surviving citations |
|---|---|---|---|---|---|
| `TASK-B-1` | task review B | The three capacity measurements cannot be rerun at their recorded commits once the default branch advances. | Capacity entrypoint invokes a cold build whose source is the moving default branch; benchmark input is not fixed. | `s20-wyrd-pr1.json`, `bench-vscode.json`, `bench-rust.json`, and TASK-007 capacity table and reproducibility note. | `examples/live_pr.rs:98`; `src/snapshot.rs:624,636`; `tasks/TASK-007-integrated-pipeline-and-real-pr.md:95` |
| `STD-1` | standards review | The refreshed benchmark claim violates the written rule requiring reproducible measurements. | The same moving default-branch benchmark input as `TASK-B-1`. | The three capacity reports and TASK-007 capacity table. | `architecture/bohmian-code-review-design.md:38`; `tasks/TASK-007-integrated-pipeline-and-real-pr.md:95`; `examples/live_pr.rs:98`; `src/snapshot.rs:636` |
| `TASK-A-2` | task review A | The recorded public-repository runs do not demonstrate the separate controlled real-provider contract required by AC-006. | Incidental S20/capacity observations were presented as the controlled contract run; missing controlled-provider evidence. | All four live reports through `examples/live_pr.rs:86`; TASK-007 provider-contract row `:86`. | `tasks/TASK-007-integrated-pipeline-and-real-pr.md:86`; `examples/live_pr.rs:86` |

The full citation result is in `citations.txt`; source report citation blocks preserve each exact snippet and symbols.
