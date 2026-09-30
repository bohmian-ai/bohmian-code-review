# Citation-checked Wave 1 claims

Subject: `322a2b9e0984062625d54274798a9c32cb01142d..100249c3031d645ce8db9148b4b6fa43e71481d1`.

`TASK-B-2` is excluded in full because its citation block contains three `REJECT` entries. All other entries passed `mise run review:verify-citations`; see `citations.txt` for exact results and each report's `citations` block for verbatim snippets.

| Source IDs | One-sentence claim | Root cause / defect class | Citation locations and sibling locations |
|---|---|---|---|
| TASK-A-1, TASK-B-1, DOMAIN-PIPE-1 | The eval harness cannot take a repository sample and caller-named `&dyn Ranker` arms as decision 3 requires. | The exported evaluation owner fixes its arms internally; required caller input is unrepresentable. | `crates/bohmian-code-review-eval/src/trial.rs:37,273,297`; `live.rs:53,473,530`; `report.rs:116`. |
| TASK-A-2, DOMAIN-HISTORY-1 | At the ten-page discovery cap, cold build or update can publish a successful snapshot without proving the required history depth or previous-snapshot boundary. | `CoChangeHistory::read` treats exhaustion of the page bound as success; incomplete-history detection is missing at the shared owner. | `crates/bohmian-code-review-pipeline/src/history.rs:385,413,417,704`; callers `snapshot.rs:739-752`, `update.rs:120-124`. |
| DOMAIN-HISTORY-2 | A lower item can move across pagination boundaries while the first item stays fixed, yet the multi-page history read succeeds. | The stability check compares only the first entry of page one; incomplete listing detection is insufficient. | `crates/bohmian-code-review-pipeline/src/history.rs:407,409`; producer `github.rs:1494-1568`; callers `snapshot.rs:739-752`, `update.rs:120-124`. |
| DOMAIN-PIPE-2 | The eval live path makes direct authenticated provider reads outside pipeline types, contrary to the provider ownership rule. | `Api` owns an HTTP transport and direct GET method beside a pipeline client; duplicate provider access boundary. | `crates/bohmian-code-review-eval/src/live.rs:147,165,168,244,377,387`; sinks are repository discovery, PR listing, and base commit time. |

No prior `FIND-*` ledger exists for this first review.
