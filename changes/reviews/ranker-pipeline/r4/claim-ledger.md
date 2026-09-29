# Citation-checked claim ledger

Subject: `322a2b9e0984062625d54274798a9c32cb01142d..cd93bdadfa5cb17ed54752168e4a445c2e660068`.

| Source ID | Reporter | Citation locations | Claim | Root cause and defect class | Siblings |
|---|---|---|---|---|---|
| `STANDARD-1` | repository standards | `crates/bohmian-code-review-eval/src/lib.rs:15`; `src/live.rs:118,160,214,263,340,361,493` | The public evaluation live workflow returns `Box<dyn Error>` against the written typed-library-error rule. | Moving an executable workflow into a public library retained erased error types. | `Api::base_snapshot`, `qualify`, `trials`, `select`; public `live`, `run`, `print`. |

All eight citations validated at the candidate; see `citations.txt`. No other Wave 1 finding was proposed.
