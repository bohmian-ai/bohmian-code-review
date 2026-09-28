# Citation-checked claims for Wave 2

Subject: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab..4d7115bbca74e7a4c27a16c4183f8eb5d754ae04`.

| Source ID | Reviewer | Claim | Root cause / defect class | Sibling locations | Citations |
|---|---|---|---|---|---|
| SRC-R4-1 | source-trust domain | A present patch with no hunk is not recorded as an omission. | Patch presence is confused with patch validity at the shared screening owner. | Empty and marker-only patch inputs; `Screen::plausible`, `Api::select`, output omission list. | `sample.rs:345-346`, `bohmian-code-review-select/src/diff.rs:149,247` |

The citation verifier rejected `DOMAIN-SAMPLING-1`, `DOMAIN-SAMPLING-2`, and `TASK-B-1`; they are excluded from this ledger.
