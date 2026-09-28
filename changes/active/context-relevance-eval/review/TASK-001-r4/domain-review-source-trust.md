# Source trust domain review

**Result: FAIL.** Reviewed base `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab` through candidate `4d7115bbca74e7a4c27a16c4183f8eb5d754ae04` against approved spec revision 4, original TASK-001, and R1–R3 remediation tasks. The reviewed boundary is provider input through PR discovery, one file listing, patch classification, fixed-commit snapshot, and source-free sample output.

| Boundary | Authority and source coverage | Result |
|---|---|---|
| PR identity and ordering | REQ-002, INV-003; `main.rs:206-248`, `sample.rs:83-239`. Cutoff, update and merge times are real fixed-width UTC instants; merge after update fails; settled PRs sort by merge time then number. | PASS |
| File listing and path trust | REQ-002, INV-001, AC-005; `main.rs:102-118`, `sample.rs:241-279`, `github.rs:1361-1407,1645-1671`. One logical listing follows its pages; rejected entries, duplicate paths and the 3,000-file ceiling stop the run. | PASS |
| Patch classification and qualification | REQ-002–003; `sample.rs:314-379`, `main.rs:165-197`; `diff.rs:130-150,243-248`. A present patch with no hunk is not recorded as an omission. | **FAIL: SRC-R4-1** |
| Base snapshot | REQ-002–003, INV-001, INV-005; `main.rs:120-157,180-196`, `snapshot.rs:178-224,266-310,344-376,628-655,765-815`. Fixed-commit build fails incomplete coverage, read-back verifies scope and digest, and an `IndexedFile` necessarily has declarations. | PASS |
| Output and IO | INV-004–005; `main.rs:251-305`. The command uses read-only API requests and memory stores; printed JSON contains metadata, paths and omission reasons, without patch or source text. | PASS |

## Proposed finding

**SRC-R4-1 — a no-hunk patch is silently treated as a usable patch.** Classification: INCORRECT; defect class: patch presence is confused with patch validity at `Screen::new`, the shared listing screen. REQ-002 and TASK-001 Scenario 2 require missing or malformed per-file patches to be recorded as omissions. For `PullRequestFile { patch: Some(String::new()), .. }`, `parse_hunks("")` executes no loop iterations and `Hunks::finish` returns `Ok([])`. `Screen::new` takes its `Some(Ok(ranges))` arm, sets `changed=false`, and never appends an `Omitted`. A nonempty marker-only patch such as `"\\ No newline at end of file"` follows the same path. These are the sibling inputs at the same owner. `Screen::plausible` and `qualifies` correctly decline to use either patch as changed lines, but `Api::select` can retain a PR through another file and `run` then prints an incomplete omission list. A real deletion-only hunk also yields no head-side ranges but *has a valid hunk*; it must remain distinct from these no-hunk inputs. Correct classification at `Screen::new` so a patch with no hunk is recorded as an omission, while a valid deletion-only hunk remains a parsed patch. Focused proof: screen a qualifying listing with a modified withheld path, another valid changed-line file, and a third empty or marker-only patch; assert both malformed paths are in `omitted` and the valid deletion-only hunk is not.

## Verification limits

`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::` passed all eight tests. The finding is reproduced by the cited parser and screen source path; current tests have no empty or marker-only patch case. Credentialed live sampling and a same-cutoff rerun belong to TASK-003. GitHub pagination under concurrent updates remains the approved one-listing input-stability limit. No candidate source was edited.

```citations
[{"id":"SRC-R4-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":345,"snippet":"            let changed = match file.patch.as_deref().map(parse_hunks) {","symbols":["Screen::new","parse_hunks"]},{"id":"SRC-R4-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":346,"snippet":"                Some(Ok(ranges)) => !ranges.is_empty(),","symbols":["Screen::new"]},{"id":"SRC-R4-1","path":"crates/bohmian-code-review-select/src/diff.rs","line":149,"snippet":"    hunks.finish()","symbols":["parse_hunks","finish"]},{"id":"SRC-R4-1","path":"crates/bohmian-code-review-select/src/diff.rs","line":247,"snippet":"        Ok(self.added)","symbols":["finish"]}]
```
