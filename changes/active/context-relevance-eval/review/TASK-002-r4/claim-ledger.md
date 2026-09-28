# Citation-checked Wave 1 claims

Subject: `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41` → `e2914e6d65a203519fefb47872040e1d2e454e20`. The citation checker exited 0; no citations were rejected.

| Source ID | Reviewer | Citations | Claim | Root cause / defect class | Sibling locations |
|---|---|---|---|---|---|
| TASK-B-1 | task-rev B | `crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs:570,581` | A parsed patch may name an old-side line beyond the verified base blob and still yield a successful trial. | `Change::new` consumes unvalidated old-side coordinates from `removed_lines`; unchecked provider coordinates used as changed-name evidence. | Modified, renamed, and removed indexed paths; deletion-only and empty head ranges; downstream query and reporting consumers at `trial.rs:690-699,762-793,1025-1048` and `main.rs:351-405`. |

The other four Wave 1 reports proposed no findings.
