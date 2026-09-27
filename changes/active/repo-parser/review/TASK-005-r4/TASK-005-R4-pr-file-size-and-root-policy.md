---
id: TASK-005-R4
kind: remediation
status: proposed
spec: SPEC-repo-parser@12
parent_task: TASK-005
remediates: [FIND-TASK-005-8, FIND-TASK-005-9]
---

# Enforce PR file admission and executable root policy

Approved specification: `changes/active/repo-parser/spec.md` revision 12. Original task: `changes/active/repo-parser/tasks/TASK-005-pr-context-and-shared-read.md`. Review subject: base `fa4fa2101950626a9fe60b0fdff0439b04c16f01`, candidate `62346cb030354596cdf3dd7596de63dbd8d99fcb`. Validated findings and reproduction: `changes/active/repo-parser/review/TASK-005-r4/findings-validation.md`. The R3 maintainer decision replaces the FIND-4 portion of R3 and remains authoritative.

## FIND-TASK-005-8 — actual PR source exceeds its file ceiling

**Diagnosis.** The PR facade classifies tree entries using optional `TreeEntry::size` (`eligibility.rs:267–281`) but does not apply `PrContextLimits::max_file_bytes` to actual verified bytes after `PrContext::changed_head` fetches a blob (`pull_request.rs:396–428`) or after `PrContext::pack` fetches a selected candidate (`pull_request.rs:441–487`). A compatible snapshot built under a looser limit can also donate unchanged facts through `Ingest::reuse` (`snapshot.rs:910–934`) when the current tree omits size. These are siblings of one PR admission gap: the current invocation's configured limit is not established by optional tree metadata or by the independent provider transport ceiling. The defect class is missing actual-size validation at the PR source/fact admission boundary. With a canonical supported path, `size: None` or an understated size, a valid SHA and a blob above the PR cap but below the provider cap, a changed head can enter analysis and indexing; a matching candidate can enter stored spans. Warm reuse can retain its oversized fact. The pipeline gate has no focused test for this combination. This violates the spec's eligible-file definition, REQ-004/013 and TASK-005's bounded PR/scoped context; ranking, indexed coverage, and the published context can be wrong.

**Correction outcome and recommendation.** Enforce the current PR facade's file ceiling on verified changed-head bytes, selected candidate bytes, and reused facts before each can contribute to analysis, the private index, ranking, or selected spans. Reuse the existing `PrContextLimits`, `Ingest` size/skip coverage, and per-file `not_analyzed` reporting. A too-large changed head remains a file-local omission; other valid heads continue. A too-large candidate is removed from selection with an explicit reason, allowing other ranked candidates to be considered. A reused fact whose recorded byte length exceeds the current cap must not enter the PR index. Apply the rule at the PR admission owners once for each distinct source form (verified bytes and source-free reused facts), rather than adding caller-specific transport limits or a new abstraction. Keep Git SHA verification and honest partial coverage.

**Closure proof.** Use a public PR-facade fake-provider case with missing or understated tree sizes, `max_file_bytes` below the provider `max_blob_bytes`, one oversized changed head, one oversized matching candidate, and an under-limit control. Assert provider calls, file-local diagnostics, indexed/eligible coverage, ranking inputs, and selected spans. Repeat with a compatible warm snapshot built under a larger file cap to show reused facts cannot reintroduce an oversized candidate. The proof must distinguish the facade's eligibility ceiling from provider transport failure.

## FIND-TASK-005-9 — executable root `.gitattributes` is ignored

**Diagnosis.** The shared `root_attributes` loader selects root `.gitattributes` only at `TreeEntryMode::File` (`snapshot.rs:739–752`). Executable mode `100755` is a valid Git blob mode accepted by provider validation and ordinary eligibility (`eligibility.rs:267–281`). Thus a valid executable root policy blob is not fetched or parsed; default rules silently replace its generated/vendored overrides. Cold build, update, warm PR overlay and scoped PR fallback all consume this same policy loader (`snapshot.rs:574–589`; `pull_request.rs:155–195`). A tree with executable `.gitattributes` containing `generated/* linguist-generated` and a valid `generated/a.ts` can index and select that file, publish or advance a supposedly complete snapshot, or report misleading PR coverage. Current fixtures use ordinary mode and do not prove this case. The defect class is an overly narrow accepted-mode check at the shared policy producer. It violates the spec's eligible-file definition, REQ-005/006/009 and honest coverage.

**Correction outcome and recommendation.** Have the existing shared `root_attributes` owner use verified root policy blobs in both admitted regular-blob modes, `100644` and `100755`. Reuse its existing fetch and parse path. Preserve provider failure for malformed SHA/unknown mode, unsupported exclusion of noncanonical tree paths, and PR-file rejection exactly as the R3 maintainer decision requires. Keep symlink/submodule policy entries excluded. One correction at the shared loader closes cold, update, warm overlay and scoped siblings.

**Closure proof.** A focused fake-provider executable root policy with a generated/vendored source path proves the policy is fetched and that the source is excluded for the policy reason through cold build and public PR context. Cover update pointer behavior and both warm and scoped PR paths; retain ordinary-mode and absent-policy controls. Assert coverage and selected paths, not only that a blob request occurred.

## Scope, preserved behavior, and verification

The correction write set includes the outer crate's existing PR, ingest/eligibility, root-policy owners and their nearest tests as required: `crates/bohmian-code-review-pipeline/src/pull_request.rs`, `snapshot.rs`, `eligibility.rs`, and fake-provider test support where needed. Do not change the six symbolic crates, approved public behavior, Git identity rules, provider transport ceiling, context expiry/cleanup, scoped index count, R3 maintainer tree-entry decision, or unrelated storage contracts. No model, comment publication, consumer scheduling, dependency, or new backend belongs here.

Acceptance is closure of both findings across every sibling above, including explicit file-local and coverage results. Run exact focused new tests and their controls, then `mise run test:pipeline`, `mise run test:symbolic-context:journey`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check`. Record RED/GREEN and final results. Route this remediation directly to `$cr-implement`; the next task review reassesses the complete cumulative candidate.
