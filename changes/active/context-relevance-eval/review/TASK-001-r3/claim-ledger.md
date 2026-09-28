# Citation-checked Wave 1 claims

Subject: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab..c88bea58b62df3854b28a082fc7dd969ed8cfd6a`. Citation check exited 0; all eight cited locations were accepted. Wave 1 full reports remain in this directory. This ledger gives claims only, not reviewer reasoning or verdicts.

| Source ID / reviewer | One-sentence claim | Root cause / defect class | Cited locations | Siblings |
|---|---|---|---|---|
| `TASK-A-1` / task A | A valid deletion-only patch is reported as missing despite parsing successfully. | `Screen::new` maps a successful empty changed-line result to `MissingPatch`; successful parse conflated with absent input. | `sample.rs:339,340,344`; `main.rs:283`; `crates/bohmian-code-review-select/src/diff.rs:620` (full paths and exact snippets in task A report) | JSON omission output in `main.rs:283-288`. |
| `TASK-B-R3-1` / task B | Discovery accepts a merged PR whose merge time exceeds its update time. | `Discovery::offer` validates timestamps separately but not their chronological relation; inconsistent provider pair. | `sample.rs:178,198`; `main.rs:227` (full paths and exact snippets in task B report) | `Discovery::settled` and `Api::select` page-boundary settlement. |

Standards, source-trust, and sampling reports contain no proposed findings.
