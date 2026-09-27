# Snapshot and source-trust domain review

**Subject:** `cea61000ac2d57a0c8f9dd9037c7fb1fd7d41e5c` → `caad6d05711e9e98a992d712331bc18a1fdb78ca`, TASK-008, SPEC-repo-parser revision 14.

**Boundary reviewed:** verified Git blobs entering the symbolic facade, content skips, complete snapshot publication, update reuse, and PR overlay reconciliation. I inspected the complete base-to-candidate diff and the surrounding `snapshot.rs`, `update.rs`, `pull_request.rs`, facade `SymbolicContext`, and index replacement code. Governing obligations are the spec's Complete snapshot term, REQ-002, REQ-005, REQ-006, INV-002, INV-003, and AC-004, plus `AGENTS.md` and `architecture/agent-rules.md` source and coverage rules.

| Path | Observed result |
|---|---|
| Cold build | Archive entries reach ingest only after tar/blob verification; fallback uses pinned blob identity. A successful upsert with only file nodes is removed from the context and recorded as `NoFacts` with its listed blob. `Coverage::is_complete` permits this content skip; publication follows only complete coverage. |
| Update | The update constructs a new ingest for the head tree. `reuse` carries facts or skips only for unchanged path/blob pairs; changed paths remain pending. The shared ingest path gives changed no-facts content the same result as a cold build. Fetch and parse failures remain nonbenign, so an incomplete attempt cannot advance the pointer. |
| PR overlay | The head tree creates the eligible path/blob set; unchanged snapshot facts and skips are reused only on identical identities. Changed paths are fetched by their head blob identity and use the shared ingest path. A failed changed blob remains pending/skipped, without old facts in the private index. Full coverage requires the overlay's complete coverage. |
| Durable snapshot | The serialized object contains source-free indexed facts and eligible path/blob identities, plus ordered skips. Decode checks identity, tree, versions, and completeness; the existing record/object/pointer sequence remains unchanged by this task. |

**Verification:** `mise exec -- cargo test --locked -p bohmian-code-review-pipeline --lib no_facts_file_is_a_recorded_skip_in_a_published_build` passed (1 test). The first attempt with `-- --exact` selected zero tests; it is not counted as verification. The task records passing `mise` extract, journey, pipeline, format, lint, gate, and live-Wyrd runs, but I did not independently rerun those broad lanes. I did not exercise every archive corruption, update, or overlay case in this domain review; source tracing and existing owner tests support the corresponding conclusions.

**Proposed findings:** None. The changed `NoFacts` classification is reached only after successful extraction of verified bytes, and `ParseFailed`, `FetchFailed`, and `NotFetched` remain outside the benign set. The same ingest owner is used by cold build, update, and overlay, so I found no sibling path that records an extraction or fetch failure as a complete content skip.

**Result: PASS** for this domain.

```citations
[]
```
