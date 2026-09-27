# TASK-007 r5 findings validation

Subject: `2257992f36a2aa88f33f397af1e070d6ab35961b..317b2da9a8cd45755418dca05111c96beb19eaa4`; approved `SPEC-repo-parser@14`, original TASK-007, R1–R4 remediation, and `review/maintainer-decision-stateless-snapshot.md`. Claim input: this review's `claim-ledger.md`, which has no surviving claims after a successful citation check. Recommendation: **PASS**.

## Validated ledger

**Empty.** Independent inspection of the cumulative code diff, original task, prior findings, and R4 fixture found no blocking finding under the production-path, task-contract, written-standard, or absolute-boundary gates. There are no source IDs to merge, reject, or demote, and no notes.

## Prior finding closure

| Finding | Independent closure check | Status |
|---|---|---|
| `FIND-TASK-007-1` | `live_pr.rs:106-111` selects `ColdBuild::run_at` with `BCR_COMMIT`; `snapshot.rs:640-654` resolves that commit and its tree before the existing build. `live_pr.rs:122-140` passes one PR read into every `PrContext::run_at` sample; `pull_request.rs:173-183,385-418` rejects a changed base/head before publication. The focused fake-provider tests cover a moving default head and a moved PR; the task record gives fixed cold commits and PR pairs for all three capacity rows. | **CLOSED** |
| `FIND-TASK-007-2` | `provider_contract.rs:248-250` fails the command on `failures(&cases)`; `:256-307` checks all required names, tree usability, archive redirect and gzip, omitted patch, conditional 304, and API rate headers. The captured controlled-provider report contains all seven passing cases; `:350-391` proves the retained broken and missing case mutations fail. | **CLOSED** |
| `FIND-TASK-007-3` | The fixed-commit test and original cold-journey test have separate accurate rustdoc in `snapshot.rs:1703-1743`; the provider test module has purpose and invariant documentation in `provider_contract.rs:327-333`. | **CLOSED** |
| `FIND-TASK-007-4` | `provider_contract.rs:339` now includes the crate-owned `examples/provider_contract_cases.json`, the only pipeline `include_str!` or `include_bytes!` fixture. It has no compile-time path into `changes/active/`. Its seven cases have the live report's results and exchange statuses; only unused headers and all but one omitted-patch path were removed. The exact focused example test selected one test and passed at this candidate. The R4 task also records a passing isolated-copy test with `changes/active/` absent. | **CLOSED** |

R1–R4 add no identified redundant check or parallel provider validator. The fixed-commit and stable-PR methods serve the explicitly requested reproducible measurements while ordinary default-head and PR calls retain their behavior. No aggregate snapshot state, source persistence, or consumer service appears in the cumulative code diff.

## Verification and limits

- Independently ran `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --example provider_contract --all-features tests::every_named_case_decides_the_verdict -- --exact`: **1 passed**. `git diff --check 2257992 317b2da`: **passed**; HEAD remained the candidate.
- Compared the durable fixture with the captured provider JSON: seven case names and all exchange statuses match; all result objects match except the allowed reduction of `without_patch` to one path. Inspected the complete code diff and the task's three fixed-commit capacity rows.
- I did not repeat credentialed GitHub calls, capacity measurements, an isolated-copy build, or the full gate in this validation pass. The checked-in live reports, R4 isolated-copy proof, and implementation verification record are the evidence for those operations. These limits do not reopen a reproduced claim.
