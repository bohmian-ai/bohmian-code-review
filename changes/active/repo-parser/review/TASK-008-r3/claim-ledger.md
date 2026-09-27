# TASK-008 r3 citation-checked claims

Subject: `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c..f56ec0e3091f1458e80c7addb3286577527f638f`. The citation check exited zero. It rejected `TASK-A-1`, `TASK-B-1`, and `PARSER-R3-1` because their qualified symbol strings did not match the verifier; their claims are excluded from Wave 2. `SNAPSHOT-008-R3-1` is the sole surviving claim.

| Source ID | Reviewer | Claim | Root cause and defect class | Sibling locations | Verified citations |
|---|---|---|---|---|---|
| `SNAPSHOT-008-R3-1` | snapshot/domain | A same-tree cold build can reuse a previously published indexed-empty record from the prior candidate, keeping false complete coverage even after R2. | `Snapshot` independently represents fact records and indexed coverage, including empty-declaration records; fresh/reuse ingest settlement does not run on the same-tree pointer path. Defect class: persisted indexed/no-facts state is not closed by shape. | `ColdBuild::run`/`build` same-tree reuse; `Update::run` same-tree delegation; `PrContext::snapshot` read before invocation-local overlay. Fresh ingest and changed-tree reuse use `Ingest::settle` and are part of closure proof. | `snapshot.rs:177` `pub facts: Vec<IndexedFileFacts>`; `:216` `if !snapshot.coverage.is_complete() {`; `:543` same-tree pointer filter; `:548` `Ok(snapshot)`; `:553` `ColdBuildOutcome::Reused(pointer.clone())`; `:948` `self.settle(path, blob.clone())`; `:1025` `if self.context.has_declarations(&path) {` |

Prior ledger: `FIND-008-1` was invalidated by the maintainer decision; `FIND-008-2` and `FIND-008-3` are in the R2 ledger. R2's shape requirement and the current maintainer direction prohibit prescribing added guards.
