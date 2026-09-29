# TASK-002 round 2: history and base-corpus boundary

**Subject:** base `3fecce95f055268359d7d1d87df6308ea504329e`, candidate `4196bbd36a150e26668bed32b2d80c90d07152a0`. **Overall: PASS.** No material proposed findings.

## Coverage

Read the complete base-to-candidate diff and the R1 correction, approved specification revision 2, TASK-002 and TASK-002-R1, contributor and architecture rules, and the surrounding `sample.rs`, `history.rs`, `main.rs`, and `trial.rs` flow. The provider file listing enters `history::changed`; `History::build` retains ten pre-cold PRs and eligible increments; `History::for_trial` removes PRs merged at or after the base commit time and the trial PR; `Api::trials` passes those paths to `evaluate`; `rank_e` limits both remaining-file anchors and credited candidates to `Corpus::declarations`. The corpus comes from the indexed base snapshot and requires a source blob for each indexed file. `rank_f` uses the resulting E ordering.

## R1 closure and checks

**FIND-TASK-002-1 closed in this domain.** At `trial.rs:420-423`, the new anchor filter prevents an added remaining path absent from the base snapshot from selecting history PRs. The existing destination check at `trial.rs:430-433` prevents absent-base history paths from becoming candidates. `absent_base_changed_path_anchors_no_cochange` tests both the absent anchor and an indexed anchor, then checks the E and F trial ranks. The existing history and co-change tests cover merge-time isolation, the cold-depth boundary, mass edits, missing or rejected listings, and excluded or absent candidate paths. The fix leaves the full remaining changes available to C and changed-path exclusion.

`git diff --check` over the immutable range passed. The parent orchestrator is running shared Cargo verification; I did not start a competing Cargo lane. No credentialed live run or independent repeat run was performed in this domain review. Those are later report-stage checks and do not limit this task's in-memory history boundary conclusion.

```citations
[]
```
