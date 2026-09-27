# Citation-checked claim ledger

Subject: `a77c64d8194ff3b8e0bc9f51f68b2cc5ae0095ec` → `f5a6f4bbeae1877083f79f4198316205c75a63e8`.

## Surviving claim

| Source ID | Reviewer | Claim | Root cause | Defect class | Sibling locations | Citations |
|---|---|---|---|---|---|---|
| `DUR-1` | durability domain | `Update::run` can report `Published` for a tree after the branch head has moved, without indicating another update is needed. | The update facade observes the branch head only before its publication workflow. | Missing post-publication branch-head observation and typed retry indication. | `update.rs:76` cold path; `update.rs:101` incremental path. | `update.rs:74,76,101,448,452,455,460` (all `OK` in `citations.txt`). |

No other Wave 1 reviewer proposed a finding. The first citation check rejected four qualified `symbols` strings; the reviewer corrected only citation metadata, and the final check accepted all seven citations.
