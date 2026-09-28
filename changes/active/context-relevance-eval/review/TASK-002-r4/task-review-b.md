# TASK-002 Wave 1 task review B

Subject: `ab8ac7745f7a9240cd9360fe1a1cdcf66a1e5e41` → `e2914e6d65a203519fefb47872040e1d2e454e20`; approved spec revision 4. I read the cumulative diff, original task, R1–R3 remediation tasks, repository authorities, and evaluator source. The candidate was unchanged during this review.

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-004 / AC-003: withhold one modified declared file; retain its base candidate, exclude other changed paths, use full indexed corpus | `trial.rs:233-277,825-872,961-1048`; `main.rs:227-276` | `withholding_keeps_base_file_rankable_and_a_is_production`; focused `trial::` 7 passed | PASS |
| Variant A uses public ranking and separate production packing | `trial.rs:825-862,976-1006` | Baseline test compares exact public `rank_files` result | PASS |
| B directional names; C exact rarity and ambiguity formula; D public-fact import scope and fallback | `trial.rs:378-487,683-816` | Scoring, exact-credit, same-line binding, resolved-without-declaration tests passed | PASS |
| Changed names include head and base declarations, and provider inconsistency fails the run | `trial.rs:489-623` | R3 head-range test passed; base-side range has no corresponding check | FAIL |
| REQ-005: whole and enclosing-span counts, common 100,000-token pack, diff and reserve separate, production A observation | `trial.rs:874-1009`; `main.rs:186-191,385-399` | `packing_records_whole_spans_omitted_and_unranked` passed | PASS |
| REQ-006: source-free trials, aggregates, test split, distributions, misses | `trial.rs:85-149`; `report.rs:11-185`; `main.rs:378-405` | `report::` 1 passed; serialization test passed | PASS |
| INV-001/004/005 and AC-005: opt-in, read-only, transient verified blobs; no production or gate change | Cumulative production diff limited to example files and live-lane description in `mise.toml`; `main.rs:227-276,351-406` | Diff inspection and `git diff --check` passed | PASS |
| R1–R3 closures | `trial.rs:390-406,590-594,740-758` | Focused `trial::` 7 passed, including each regression | PASS |

## Proposed finding

**TASK-B-1 — INCORRECT; base-side patch coordinates are not validated against verified base source.** The root cause is at `Change::new`: a syntactically parsed patch is passed to `removed_lines`, and its old-side line numbers are compared only with declaration spans. Neither that owner nor `removed_lines` checks that those lines exist in the indexed base blob. The defect class is *unchecked provider coordinates used as changed-name evidence*. It is an incomplete sibling of prior `FIND-TASK-002-4`'s provider-consistency class, although R3's head-side symbolic failure propagation itself works.

**Reachable reproduction:** For a modified indexed two-line Python file, use a syntactically well-formed patch `@@ -99 +2 @@\n-old\n+    return 2` and a verified two-line head with the change on line 2. `parse_hunks` checks hunk syntax and body counts, so it accepts the patch and yields head range 2; `rank` accepts that head. `removed_lines` yields old line 99. `Change::new` finds no base declaration overlapping it and completes successfully. A second modified file makes this a real trial. The report then omits the base declaration's old name from downstream B–D lookup, potentially changing a withheld caller's rank, score, and sent result. A deletion-only hunk is an additional sibling because `Change::new` returns before any head analysis. This violates the spec's requirement that provider inconsistency fail the run (`REQ-002`, `INV-003`) and that removed declarations contribute old names (`REQ-004`); `architecture/agent-rules.md:8` also bars apparent success after omitted source facts.

**Sibling sweep:** The same owner handles modified, renamed, and removed listed Python/Rust paths when their base path is indexed (`trial.rs:566-582`). Its wrong `changed` set flows through `Query::new` (`trial.rs:690-699`), downstream scoring for B–D (`trial.rs:762-793`), and every successful trial/report (`trial.rs:1025-1048`, `main.rs:351-405`). R3's head-side check at `trial.rs:590-594` does not cover removed files, empty head ranges, or old coordinates.

**Testable correction:** At the shared change-preparation boundary, reject a parsed patch whose old-side changed coordinates disagree with the verified indexed base blob, while preserving missing/malformed-patch omissions and the existing head-side failure. Prove it with a no-network `evaluate` trial for an old line beyond a short verified base, including a deletion-only or removed-file case; preserve R1–R3 tests. No new public API or parser is needed.

The focused `trial::` selector passed 7 tests, `report::` passed 1, and the base-to-candidate `git diff --check` passed. I did not run a live GitHub evaluation. Ponytail review found no other necessary abstraction or dependency change; the evaluator remains private. **Overall: FAIL.**

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":570,"snippet":"            let removed = removed_lines(patch);","symbols":["Change::new","removed_lines"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/trial.rs","line":581,"snippet":"        if file.status == FileStatus::Removed || ranges.is_empty() {","symbols":["Change::new"]}]
```
