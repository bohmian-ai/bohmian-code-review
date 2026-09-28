# TASK-001 independent task review B

Subject: base `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab`; candidate `4d7115bbca74e7a4c27a16c4183f8eb5d754ae04`; approved spec revision 4; original TASK-001 and R1–R3 remediation tasks. Reviewed the cumulative diff and source. The focused no-network sampler command (`mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example context_relevance --all-features sample::`) passed 8 tests.

| Requirement, acceptance criterion, constraint, or non-goal | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| REQ-001, REQ-007: two named repositories, credentialed opt-in live command | `main.rs:37-42,206-218,256-295`; `mise.toml` live task | Source inspection; live run deferred to TASK-003 | PASS |
| REQ-002: 50 newest qualifying merged default-branch PRs before cutoff, tie by number | `sample.rs:133-208,314-379`; `main.rs:219-248` | Discovery and screening focused tests pass | PASS |
| REQ-002: one complete PR file listing, rejected/incomplete input fails | `main.rs:96-118`; `sample.rs:241-279,322-331` | `listing_fails_at_provider_limits`, `listing_rejects_repeated_paths`, `screen_fails_on_rejected_or_incomplete_listing` pass | PASS |
| REQ-002: only a missing or malformed patch is recorded as an omission | `sample.rs:345-355` treats a valid patch with zero head-side added ranges as `MissingPatch` | Public `parse_hunks` contract and its deletion-only test; no sampler test covers this case | FAIL |
| REQ-003: modified path with declarations in the base snapshot is withholdable; another file has changed lines | `main.rs:180-191`; `sample.rs:371-379`; indexed files guarantee nonempty declarations | `screen_qualifies_and_records_omissions` passes | PASS |
| INV-001, AC-005: use snapshot and public path query; no production change, `PrContext`, replay, or trial blob fetch | `main.rs:126-196`; cumulative diff touches example, manifest, task and review packet, and live mise task | Diff inspection | PASS |
| INV-003: reject unusable times and inconsistent provider input | `sample.rs:140-195,211-239,263-278` | Timestamp, chronology, and duplicate tests pass; live rerun belongs to TASK-003 | PASS |
| INV-004, INV-005: source-free output; read-only in-memory sampling | `main.rs:270-295`, in-memory snapshot store at `:131` | Source inspection | PASS |
| R1: provider file ceiling and rustdoc | `sample.rs:26-28,263-277,382-425` | Cap test passes; source inspection | PASS |
| R2: duplicate paths, Gregorian timestamps, concrete IO owner | `sample.rs:160-239,263-278`; `main.rs:61-249` | Focused tests pass; source inspection | PASS |
| R3: reject merge after update before filtering and selection | `sample.rs:175-185` | `discovery_rejects_merge_after_update` passes | PASS |

## Proposed finding

**TASK-B-1 — INCORRECT — valid no-addition patch classified as missing.** Defect class: the screening owner conflates a successfully parsed, empty head-side range set with a missing patch. The owner shape is otherwise adequate; one match arm makes the wrong decision. At `sample.rs:345-354`, `Some(Ok(ranges))` only handles nonempty ranges. `Some(Ok(vec![]))`, which the public parser returns for a valid deletion-only hunk (`bohmian-code-review-select/src/diff.rs:137-148,603-604`), falls through to `OmittedReason::MissingPatch`. Sibling effects are the `Screen::omitted` record and `main.rs:283-289` JSON output; `changed_lines = false` is correct and must remain false. A PR with one modified deletion-only source file and another changed source file can qualify, then its report falsely states that the deletion-only file lacked a patch. This violates REQ-002 and TASK-001's omission boundary; it also gives TASK-002 a false omission input. At `Screen::new`, reserve `MissingPatch` for `None`, `MalformedPatch` for parser errors, and keep a valid empty range set out of omissions. Add one focused in-memory deletion-only patch assertion, preserving the existing qualification rule.

Overall: **FAIL**. Live sample completeness and repeatability are TASK-003 verification limits, not additional TASK-001 findings.

```citations
[{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":345,"snippet":"            let changed = match file.patch.as_deref().map(parse_hunks) {","symbols":["Screen::new","parse_hunks","OmittedReason::MissingPatch"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":346,"snippet":"                Some(Ok(ranges)) => !ranges.is_empty(),","symbols":["Screen::new"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs","line":350,"snippet":"                        _ => OmittedReason::MissingPatch,","symbols":["Screen::new","OmittedReason::MissingPatch"]},{"id":"TASK-B-1","path":"crates/bohmian-code-review-pipeline/examples/context_relevance/main.rs","line":283,"snippet":"                    \"omitted\": sampled.screen.omitted.iter().map(|omitted| json!({","symbols":["run","Screen::omitted"]}]
```
