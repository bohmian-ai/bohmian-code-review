---
id: TASK-001-R4
kind: remediation
status: ready
spec: SPEC-context-relevance-eval
spec_revision: 4
requirements: [REQ-002, INV-003]
depends_on: [TASK-001]
parent_task: TASK-001
remediates: [FIND-TASK-001-7]
---

# Record patches without hunks as omissions

## Subject and diagnosis

Approved spec: `changes/active/context-relevance-eval/spec.md` revision 4. Original task: `changes/active/context-relevance-eval/tasks/TASK-001-pinned-sample.md`. Review base: `e665bc17ce91e855e4a744f3231b58c1cd9cd0ab`; cumulative candidate: `4d7115bbca74e7a4c27a16c4183f8eb5d754ae04`. This task remediates **FIND-TASK-001-7** only.

**Root cause and defect class:** At `crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs:345-355`, `Screen::new` treats any successful `parse_hunks` result as a valid present patch. The public parser in `crates/bohmian-code-review-select/src/diff.rs:137-149,243-247` can return `Ok([])` when the text contains no hunk, including an empty string or a marker-only patch. Patch presence is confused with patch validity at the sampler's shared screening boundary. REQ-002 and TASK-001 Scenario 2 require malformed per-file patches to be recorded as omissions; they supply no changed lines.

**Reachable effect and siblings:** Both empty and marker-only `Some(patch)` inputs enter the same `Screen::new` arm, set `changed_lines` false, and leave `Screen::omitted` unchanged. `Screen::plausible` and `Screen::qualifies` use that flag; `Api::qualify` and `Api::select` can retain a PR that has another valid changed-line file; `run` prints the incomplete omission list. `Api::listing` is the provider-input producer. A valid deletion-only hunk can also yield zero head-side ranges, but it contains a real hunk and must remain a valid, non-omitted patch. The current eight passing sampler tests cover absent patches and parser errors, but not no-hunk input; they cannot prove the required distinction.

## Correction outcome and recommendation

At the existing `Screen::new` classification owner, require a present patch to contain a successfully parsed hunk before considering it valid. Record present no-hunk input as `OmittedReason::MalformedPatch` and keep its changed-lines result false. Reuse `parse_hunks`, `Screen::omitted`, and the existing omission reasons. Preserve valid deletion-only hunks as parsed patches with no head-side changed lines. This closes both no-hunk sibling inputs at one ingress and preserves the command's single-listing workflow. Do not change the production parser or add a provider request, replay layer, public API, or trial source fetch.

## Acceptance and proof

1. With an otherwise qualifying in-memory listing, empty and marker-only patches each appear in the omission output as malformed and contribute no changed lines. This covers the `Screen::new` classification, `Screen::plausible`/`qualifies`, and the command's emitted omission list.
2. A valid deletion-only hunk remains non-omitted while contributing no head-side changed lines; valid added-line hunks, absent patches, and parser errors retain their existing behavior.
3. Run a focused no-network sampler test that fails on the candidate and passes after correction. Record its exact RED/GREEN command and outcome. Then run `mise run test:pipeline`, `mise run fmt:check`, `mise run lints`, and `git diff --check`. Inspect the cumulative diff for unchanged production crates, public APIs, persisted formats, gate, `PrContext`, second listing, provider replay, and trial source blobs.

Expected write set: `crates/bohmian-code-review-pipeline/examples/context_relevance/sample.rs` and this remediation task's implementation evidence. Keep the fix within the approved evaluation setup and route directly to `$cr-implement`; a later task review reassesses the complete cumulative candidate.
