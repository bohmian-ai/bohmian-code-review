# Citation-checked Wave 1 claims

Subject: `2257992f36a2aa88f33f397af1e070d6ab35961b..b721917420113ab2e3530966e9e456eecc89dcff`. Citation verification exited 0; all three `STD-R3-1` citations passed. Task A, task B, capacity, and provider proposed no findings.

| Source ID | Reviewer | Claim | Root cause and defect class | Candidate citations and siblings |
|---|---|---|---|---|
| `STD-R3-1` | standards | The inserted fixed-commit test now owns the prior cold-journey rustdoc, leaving that journey test undocumented; the new provider example test module also has no rustdoc. | Source-order insertion detached an existing item's documentation; missing module documentation is the alleged sibling under the written all-items rule. | `crates/bohmian-code-review-pipeline/src/snapshot.rs:1703` (`    /// Scenario 1: the cold build verifies archive entries, falls back to`), `:1743` (`    #[tokio::test]`); `crates/bohmian-code-review-pipeline/examples/provider_contract.rs:328` (`mod tests {`). Rule: `AGENTS.md:32`, `architecture/agent-rules.md:11`. |

Prior finding IDs for closure audit: `FIND-TASK-007-1` (stable fixed workload) and `FIND-TASK-007-2` (provider cases decide success), from `review/TASK-007-r2/findings-validation.md`.
