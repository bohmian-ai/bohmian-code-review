# TASK-005 r4 source-trust review

**Subject:** `fa4fa2101950626a9fe60b0fdff0439b04c16f01..62346cb030354596cdf3dd7596de63dbd8d99fcb`. **Result: FAIL.**

## Boundary, authority, and source coverage

Reviewed the cumulative candidate diff and the provider-to-context path: `GitHubClient::{tree,pull_request,pull_request_files,blob}`, `validate_tree_entry`, shared `root_attributes` and eligibility classification, cold/update snapshot publication, `PrContext::{pin,run,changed_head,pack}`, coverage, and selected-span identity. Authorities: approved `changes/active/repo-parser/spec.md` revision 12 (eligible-file definition, REQ-004–006 and REQ-008–010, INV-002/003), original TASK-005, R1–R3 remediation, `AGENTS.md`, `architecture/agent-rules.md`, and `architecture/bohmian-code-review-design.md`. The r3 maintainer decision overrides R3 FIND-4: malformed SHA or unknown mode now fails the entire tree listing; noncanonical paths are unsupported exclusions. Source inspection confirms both outcomes and deletion of tree-rejection consumers. PR-file rejections remain separate, lower full-coverage status, and canonical rejected changed paths are excluded from secondary candidates. The scoped index cap permits mandatory changed-head analysis while bounding index admission.

## DOMAIN-TRUST-1 — Executable root policy blob is ignored

- **Classification and obligation:** INCORRECT; violates the spec's eligible-file definition, which admits blob modes `100644` and `100755` and requires root `.gitattributes` generated/vendored overrides, plus REQ-005/006/009 and INV-002's honest completeness. This is a policy-input omission, not the malformed-entry class addressed by FIND-TASK-005-4.
- **Root cause and owner shape:** `root_attributes` selects `.gitattributes` only when its mode is `TreeEntryMode::File`. `TreeEntryMode::Executable` is a valid regular blob mode elsewhere in the eligibility owner, but the policy loader silently substitutes default rules for it. The policy loader's mode filter permits the defect; no new API is needed.
- **Reachable evidence and siblings:** A valid pinned tree with executable root `.gitattributes` containing `generated/* linguist-generated` and a valid `generated/a.ts` passes provider tree validation. `root_attributes` returns defaults without fetching the policy blob. `classify` then treats `generated/a.ts` as eligible. `ColdBuild::start` can publish a complete snapshot through `ColdBuild::complete`; `Update::run` shares that start/complete path and can advance its pointer; `PrContext::run` uses the same loader and may claim complete warm overlay coverage or index the wrong file in scoped mode. These four paths are sibling effects of the one skipped policy input.
- **Observable consequence:** Generated or vendored files can enter selected context, and the published snapshot or PR context can claim complete coverage under the wrong eligibility policy. The verified SHA and canonical path do not cure the missing policy read.
- **Testable correction:** Make the shared root-policy loader apply a verified root `.gitattributes` blob for both regular blob modes. Preserve provider failure on malformed SHA/unknown mode, unsupported exclusion for noncanonical paths, and the current behavior for symlink/submodule policy entries. One fake-provider executable-policy case should prove cold and update coverage/pointer behavior, warm PR overlay eligibility/full-coverage truth, and scoped eligibility; a normal-mode control should remain green.

## Verification limits

This is a read-only source-path reproduction. I did not mutate the candidate, run a live GitHub/GHES request, or add a test. The current root-attribute fixtures use normal mode; the passing lanes recorded in the task and R3 remediation do not demonstrate executable-mode policy loading. I found no other material source-trust gap in the inspected boundary.

```citations
[{"id":"DOMAIN-TRUST-1","path":"crates/bohmian-code-review-pipeline/src/snapshot.rs","line":746,"snippet":"        .find(|entry| entry.path.as_str() == \".gitattributes\" && entry.mode == TreeEntryMode::File)","symbols":["root_attributes"]},{"id":"DOMAIN-TRUST-1","path":"crates/bohmian-code-review-pipeline/src/eligibility.rs","line":273,"snippet":"        TreeEntryMode::File | TreeEntryMode::Executable => {}","symbols":["exclusion"]},{"id":"DOMAIN-TRUST-1","path":"crates/bohmian-code-review-pipeline/src/pull_request.rs","line":163,"snippet":"        let attributes = root_attributes(self.client, identity, &listing.entries).await?;","symbols":["run"]}]
```
