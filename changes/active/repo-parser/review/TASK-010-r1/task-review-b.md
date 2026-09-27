# TASK-010 independent implementation review B

Subject: `97fff9feaffa2ec46fef512f5f476133830f1ac3` → `da81a598c10441b3b2e1231ce674d89003017f26`. Authority: approved `SPEC-repo-parser` revision 14 (`63d6bd0`), original TASK-010, and `review/maintainer-decision-stateless-snapshot.md`. I read the complete eight-file diff and the relevant pipeline owners. Result: **PASS**.

| Obligation | Implementation evidence | Verification evidence | Result |
|---|---|---|---|
| Compare from pointer commit to pinned default head; no recursive tree listing on incremental path | `update.rs` `Update::run` calls `GitHubClient::compare`, then `Directories::entries`; `github.rs` `directory` uses `tree_wire(..., false)` | `change_delete_and_rename_match_a_fresh_parse` asserts compare and non-recursive directory calls; focused update lane passed | PASS |
| Changed paths include addition, modification, removal, rename, copy and mode change; no stale changed fact survives | `github.rs` `Comparison` splits statuses into removed and added paths; `snapshot.rs` `Ingest::carry` excludes removed paths before carrying prior outcomes; `Ingest::new` classifies head entries by mode | Compare owner test; update fresh-build oracle test includes add, modify, delete, rename, symlink and excluded deletion | PASS |
| Changed-path mode and blob identity come from verified head tree; invalid/missing paths do not silently publish | `update.rs` walks verified subtree IDs and looks up each added path; `github.rs` `directory` validates tree ID, path, mode and SHA; `snapshot.rs` verified blob fallback settles outcomes | `broken_tree_entry_keeps_the_pointer`, failed-blob retry test, compare identity test | PASS |
| Root `.gitattributes`, compare ceiling, changed-blob bound and non-descendant head rebuild from archive | `update.rs` trigger branch calls `cold.build`; `github.rs` marks 300-file compare response truncated | `rebuild_triggers_use_the_archive` and `changes_past_the_ceiling_rebuild_from_the_archive` | PASS |
| Complete snapshot is a tree/version function with one outcome per eligible path and no persisted aggregate counts | `snapshot.rs` removes `Snapshot.excluded`, increments format version to 3, derives coverage from outcomes; `Ingest` counts only exclusions observed by the invocation | Snapshot reuse test distinguishes build-time exclusions from persisted outcomes; update and cold build objects compare byte-for-byte | PASS |
| Failed blob, incomplete listing, interrupted update, pointer race and running review retain complete prior snapshot; retry converges | `update.rs` returns incomplete before `ColdBuild::complete`, and `ColdBuild::complete` publishes only complete ingest; publication still uses record, object, then pointer CAS | Existing failed-blob, dropped-update, moving-head, version-change tests; focused update lane passed | PASS |
| No new runtime owner, source persistence, full-tree listing on the incremental branch, or unsupported rebuild trigger | Diff touches pipeline crate and task documents only; only `cold.build` trigger branch uses full listing/archive | Complete diff inspection; `git diff --check 97fff9f da81a59` clean | PASS |

Verification: `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --all-features --lib update` ran 8 tests, all passed, 67 filtered. The task's scenario text says blob fetches only for changed paths, but the approved rev-14 stateless snapshot requires reading unchanged root `.gitattributes` to classify changed paths without persisting derived state. The implementation test explicitly counts that attributes fetch. I applied the later approved specification and maintainer decision to this tension; no additional changed source blob is fetched.

No proposed findings. The focused test and source trace support this sample's result; aggregate pipeline, journey, format and lint results remain for the orchestrator's final verification.

```citations
[]
```
