# Ranker pipeline R4 remediation — typed evaluation errors

**Kind:** remediation. **Status:** ready for `$cr-implement`. **Approved authority:** `changes/reviews/ranker-pipeline/intent.md` decision 3, `architecture/references/domain/pipeline-consumer-ergonomics.md`, `AGENTS.md:30`, and `architecture/references/languages/rust-core.md:7`. There is no original task file by maintainer direction. **Reviewed subject:** base `322a2b9e0984062625d54274798a9c32cb01142d`, candidate `cd93bdadfa5cb17ed54752168e4a445c2e660068`. **Prior remediation tasks:** `changes/reviews/ranker-pipeline/r1/ranker-pipeline-R1-complete-contracts.md`, `r2/ranker-pipeline-R2-report-identity.md`, and `r3/ranker-pipeline-R3-complete-boundaries.md`. **Remediates:** `FIND-ranker-pipeline-6`. Re-review the complete cumulative range afterward.

## Diagnosis and correction

### FIND-ranker-pipeline-6 — public evaluation workflow erases library errors

The feature-gated evaluation crate is a reusable public library under intent decision 3. `crates/bohmian-code-review-eval/src/lib.rs:15` exports its `live` module, but its public `live::live`, `live::run`, and `live::print` return `Box<dyn Error>` (`live.rs:340,361,493`). The same erased result appears in their shared workflow owner, `Api::base_snapshot`, `qualify`, `trials`, and `select` (`live.rs:118,160,214,263`), and the history-read closure fixes its error to `Box<dyn Error>` (`:322`). This shape came from the former executable example when it moved into the public crate. It violates the written rule in `AGENTS.md:30` and `architecture/references/languages/rust-core.md:7` requiring typed `thiserror` library errors.

The failure is reachable: a public `live::run` calls `Api::select` and `Api::trials`; provider listing or blob failures, incomplete snapshots, history failures, trial failures, and report errors travel through those erased results. A consumer cannot match the documented failure class from the signature and must downcast or parse text. Existing eval tests in `live.rs:967,990,1015,1037,1053` downcast errors, demonstrating the current contract. The green gate checks behavior but does not enforce the public error type. The source citations and independent trace are in `citations.txt` and `findings-validation.md`.

Correct this once in the existing eval live owner: expose a typed `thiserror` error for its public workflow and carry it through the `Api` operations and history read that feed those public functions. Preserve the underlying sample, trial, provider, history, snapshot, symbolic, token, and serialization failure distinctions as typed causes where applicable; represent token and repository configuration failures explicitly. Reuse the crate's installed `thiserror` and existing typed errors rather than adding a second evaluation path or dependency. Keep the binary's failure display and exit behavior, sample selection, ranking, report data and keys, and fixed C/E/F compatibility. This is a candidate-owned error surface; the approved product behavior and spec-named API do not change.

## Scope and acceptance

**Write set:** `crates/bohmian-code-review-eval/src/live.rs`, including its inline tests and rustdoc. Include `crates/bohmian-code-review-eval/src/bin/context_relevance.rs` only if its call site needs adjustment for the typed result. No pipeline, symbolic, provider, storage, ranking, report-schema, or dependency change is required. Preserve the closed `FIND-ranker-pipeline-1` through `-5` behavior and the untracked `changes/active/impact-map/`.

| Finding and siblings | Required observable result and focused proof |
|---|---|
| `FIND-6`: public `live`, `run`, `print`; `Api::base_snapshot`, `qualify`, `trials`, `select`; history read | All library-facing workflow failures use the one typed error contract, with no `Box<dyn Error>` in these siblings. A public fake-provider `live::run` failure can be matched by its typed provider or history cause without downcasting or message inspection. A configuration failure is typed. |
| Preserved evaluation and command behavior | The existing custom-arm fake-provider report remains unchanged; the checked-in fixed C/E/F report regression passes; the binary still reports failure and exits nonzero without partial reports. |

Record the exact focused test command and observed result for the typed failure and report-preservation cases. Run `mise run test:eval`, `mise run fmt:check`, `mise run lints`, `mise run gate`, and `git diff --check` for the cumulative range. A credentialed live run remains conditional on a supplied token; the fake-provider proof is required regardless.
