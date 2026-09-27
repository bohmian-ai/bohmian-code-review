# TASK-007 r4 citation-checked claims

Subject: `2257992f36a2aa88f33f397af1e070d6ab35961b..7f7b75d5d8645f5dd2530779fb6e146dc122efe1`.

| Source ID | Reviewer | Claim | Root cause and defect class | Siblings | Citations |
|---|---|---|---|---|---|
| TASK-A-1 | task-rev A | Compiling the provider example test after the active packet is retired fails because its captured JSON no longer exists. | `provider_contract.rs` uses a compile-time `include_str!` path into transient review evidence; test fixture ownership/lifetime mismatch. | Sole `include_str!` active-evidence site; `test:pipeline`, `test:rust`, and `gate` compile the affected example test. | `crates/bohmian-code-review-pipeline/examples/provider_contract.rs:337-338`, citation check `OK` for both. |

Task-rev B, repo-rev, capacity domain, and provider domain proposed no findings. This ledger gives claims only; reviewer reasoning and overall results remain in their separate reports.
