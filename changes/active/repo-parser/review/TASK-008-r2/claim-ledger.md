# Citation-checked Wave 1 claims

Subject: `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c..bdca195f6f08fda8f847efdd7c0f1d2d102a0a15`. Citation check: `citations.txt`, all final entries `OK`. Task samples A and B proposed no findings. The maintainer decision invalidates prior `FIND-008-1`; its remediation is withdrawn.

| Source ID | Reviewer | One-sentence claim | Root cause and defect class | Sibling locations | Candidate citations |
|---|---|---|---|---|---|
| `PARSER-008-1` | parser domain | Kotlin `fun interface Parser` can lose its interface declaration or emit the wrong kind from a recovered `ERROR` tree. | Kotlin's recovery-specific functional-interface interpretation is absent from the walker after whole-file deferral was removed; recovered declaration shape is misinterpreted. | Two observed `fun interface` error-tree shapes; `visit_node` and `visit_for_calls_and_structure`. | `kotlin.rs:765,847,854` |
| `SNAPSHOT-008-R2-1` | snapshot domain | A valid import-only file is counted indexed and has no `NoFacts` skip although the persisted fact has no declarations. | Shared ingest classifies raw nodes rather than retained declaration facts; mismatch between ingest outcome and exported snapshot shape. | Archive `snapshot.rs:905`, verified fallback `:981`, PR head `pull_request.rs:432`, prior-fact reuse `snapshot.rs:944,952`, publication `snapshot.rs:681`. | `snapshot.rs:1006,1012,140,905,981,944,952,681`; `tsjs/mod.rs:881`; `tsjs/extractors.rs:1120`; facade `lib.rs:148`; index `lib.rs:137,190,193`; `pull_request.rs:432` |
| `STD-008-R2-1` | repository standards | Task text and materially changed corpus rustdoc still say recovered declarations inside error regions are dropped, contrary to amended REQ-002. | Superseded written invariant was left in task and rustdoc; documentation authority drift. | Task `:12,77`; corpus rustdoc `corpus_tests.rs:1651`. | Task `:12,77`; corpus `:1651` |

Exact snippets and symbols are in each Wave 1 report's `citations` block. This ledger carries claims only; validate each independently against the source and governing authority.
