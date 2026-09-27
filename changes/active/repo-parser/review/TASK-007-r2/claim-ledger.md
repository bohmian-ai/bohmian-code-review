# Citation-checked Wave 1 claims

Subject: `2257992f36a2aa88f33f397af1e070d6ab35961b..f820d662a841f7e13773223b2f93cf90e82b6274`. Citation check: `citations.txt`; all entries OK.

| Source / reviewer | One-sentence claim | Root cause and defect class | Candidate citations | Siblings |
|---|---|---|---|---|
| `TASK-B-1` / task review B | Fixed-commit capacity runs still measure the current PR separately for first and warm samples. | `live_pr` pins only the cold commit; mutable PR-number input permits mixed or later changed warm workload. | `examples/live_pr.rs:117,121`; `src/pull_request.rs:330` | First and each warm call; Wyrd, VS Code and Rust reports and TASK-007 capacity rows. |
| `CAP-1` / capacity domain | A fixed cold tree does not make warm p50/p95 or API calls reproducible when the PR base/head can move. | Same root cause and defect class as `TASK-B-1`. | `examples/live_pr.rs:104,117,121`; `src/pull_request.rs:330,338` | First and each warm call; the three capacity reports and table. |
| `PROV-1` / provider domain | The new provider contract command can exit successfully while named compatibility cases fail. | `provider_contract::run` records several case outcomes but only checks for a PR file without a patch before returning success; missing contract-result validation. | `examples/provider_contract.rs:155,172,213,225` | Recursive-tree completeness/paths, archive gzip, conditional 304, and required rate headers. |

All abbreviated source paths in this ledger are under `crates/bohmian-code-review-pipeline/`. Wave 1 task review A and standards review proposed no findings.
