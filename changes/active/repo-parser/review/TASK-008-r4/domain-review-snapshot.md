# TASK-008 r4 — snapshot and source-trust domain review

**Subject:** `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c` → `f204d3383c62c6137a603437668103187a4fd19c`; approved `SPEC-repo-parser@14`, original TASK-008, and TASK-008-R3. The maintainer's decision makes FIND-008-1 invalid.

## Boundary and authority coverage

Reviewed the cumulative relevant diff and traced indexed/no-facts outcomes from the facade's declaration projection through verified archive and blob ingest, same-blob reuse, changed-tree update, PR overlay, snapshot publication, same-tree reuse, and read-back. Governing authority: spec lines 31–34, 55–56, 62–64, and 86–87; TASK-008 scenario 3; TASK-008-R3 acceptance 1–3; `AGENTS.md` product boundaries; `architecture/agent-rules.md` complete replacement and honest coverage.

`FileOutcome::Indexed` carries `IndexedFile`, whose private fields and deserialization enforce nonempty declarations. `Ingest::settle` gets those declarations from the facade's persisted projection and otherwise records a blob-identified `NoFacts` skip, removing any no-declaration posting. `Coverage::of` derives indexed and skipped counts from the same outcome list. The extractor and provider failure categories still produce nonbenign skips, so they cannot publish a complete snapshot. `SNAPSHOT_FORMAT_VERSION = 2` changes the persisted scope and object key; `Snapshot::decode` rejects prior-format bytes even if placed at a current key. Same-tree cold build and update use the current scope, while PR snapshot read uses the same scoped read. Record, object, then pointer publication remains ordered, with identity, digest, tree, and version validation on read.

## Verification and limits

Focused commands passed: `prior_format_indexed_empty_snapshot_is_not_reused`, `indexed_outcome_without_declarations_does_not_decode`, and `import_only_file_is_no_facts_on_ingest_and_reuse`, each run with `mise exec -- cargo test --locked --all-features -p bohmian-code-review-pipeline --lib <selector> -- --exact`. `git diff --check` passed. I did not rerun the full pipeline gate or the live GitHub test. The prior-format test directly exercises cold build and current-scope read; its update assertion follows publication of the new pointer. The direct update-from-old-pointer case is supported by `Update::run` reading only `snapshot_scope` and delegating its no-pointer case to cold build, but was not separately executed here.

No material finding in this domain. The prior FIND-008-2 indexed-empty representation and same-tree reuse path are closed by the outcome shape and versioned namespace. The public `Snapshot` container remains constructible by a caller, but no pipeline facade publishes a caller-constructed snapshot; that off-path possibility is not a task finding.

**Overall: PASS** for the reviewed domain.

```citations
[]
```
