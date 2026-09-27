# Source trust domain review

**Subject:** `c74871cbc78140820c9b0a5fda60e0952c4265e4` → `3ada5c59931f236716ed027d773a2746ccf3c3af`; approved `changes/active/repo-parser/spec.md` revision 12; original TASK-003 and TASK-003-R4.

**Reviewed boundary:** GitHub tree and blob identities, root attributes, archive entry path and blob verification, fallback classification, bounded ingest, snapshot coverage, and source-free read-back. I read the full cumulative diff and the current `archive.rs`, `eligibility.rs`, relevant `github.rs`, `snapshot.rs`, `persistence.rs`, and in-memory test-store paths. Authorities: `AGENTS.md` product boundary and input-validation rules; `architecture/agent-rules.md` source lifetime and effective-input rules; `architecture/bohmian-code-review-design.md`; spec REQ-004–006, REQ-013–014, INV-002–003 and TASK-003 Scenarios 1, 2, 5, plus R4 FIND-8, FIND-10, FIND-11.

| Trust obligation | Source trace | Result |
|---|---|---|
| Archive content is tied to a listed blob before indexing | `GitHubClient::tree` validates entry path/mode/SHA; `Ingest::new` selects eligible paths; `TarStream::finish_entry` hashes wanted file bytes and releases only a match; `Ingest::ingest` passes them to the public symbolic facade. | PASS |
| Fallback cannot turn a provider ceiling below the eligible-file ceiling into a benign exclusion | `GitHubClient::blob` hashes returned bytes; `Ingest::fallback` classifies `ResponseTooLarge` as `Oversize` only when its limit is at least `max_file_bytes`; otherwise it records `FetchFailed`, preventing complete coverage. | PASS |
| Exhausted compressed or expanded archive bounds cannot publish a complete pointer | `Archive::chunk` reports the compressed limit; `TarStream::write` reports the expansion limit; `Ingest::consume_archive` returns either bound state; `ColdBuild::run` reports `Incomplete(ArchiveTooLarge)` after recording fallback coverage and before `publish`. | PASS |
| Root attribute reset retains Git's unspecified versus explicit-false distinction | `RootAttributes::resolve` carries `Option<Option<bool>>`; `exclusion` applies built-in path exclusions only for unspecified state. | PASS |
| Durable snapshot has no source and read-back is identity-bound | `ColdBuild::run` serializes only paths/blob IDs/facts/coverage; `Snapshots::read` checks identity-scoped key, object digest, identity, tree, versions and complete coverage before returning it. | PASS |

**Verification:** Exact focused tests `snapshot::tests::blob_ceiling_below_file_bound_is_not_oversize`, `snapshot::tests::incomplete_attempts_report_coverage_and_keep_the_prior_pointer`, and `eligibility::tests::attribute_resets_read_as_unset` each selected one test and passed. The candidate includes archive substitution, read-back and source-free tests. I did not run a live GitHub/GHES contract or memory profiler; those limits do not undermine the reviewed local trust paths.

**Material findings:** None. No sibling location with the same source-trust defect class was found in the reviewed paths.

**Overall: PASS** for the source trust domain.

```citations
[]
```
