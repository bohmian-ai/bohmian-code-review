# Source trust, eligibility, and parser provenance review

**Subject:** base `a77c64d8194ff3b8e0bc9f51f68b2cc5ae0095ec`, candidate `f5a6f4bbeae1877083f79f4198316205c75a63e8`; `SPEC-repo-parser@12`, `TASK-004`.

**Boundary and authority.** Reviewed the complete candidate diff in `lib.rs`, `snapshot.rs`, and `update.rs`, and traced the changed update path through `github.rs`, `archive.rs`, `eligibility.rs`, the symbolic facade, and snapshot read/publication. Applied `AGENTS.md` product boundaries, `architecture/bohmian-code-review-design.md`, and spec REQ-002, REQ-004–007, REQ-012, INV-002–004, and AC-003. This report covers source identity, reuse, eligibility, versioning, failed replacement, and archive fallback; it does not decide the full task verdict.

| Boundary | Source evidence | Result |
|---|---|---|
| Pinned tree and blob trust | `ColdBuild::pin` uses the verified repository/default branch; `GitHubClient::tree` checks the requested tree SHA; `GitHubClient::blob` hashes returned bytes; `TarStream::finish_entry` releases only bytes matching the listed blob (`snapshot.rs:511–516, 565–596`; `github.rs:953–987, 997–1021`; `archive.rs:197–221`). | PASS |
| Eligibility and changed paths | `ColdBuild::start` fetches the pinned tree's verified root attributes and classifies all entries. `Ingest::reuse` carries facts/skips only where the new eligible path and blob match the prior snapshot; deleted, renamed-away, and now-ineligible paths remain absent. Pending replacements pass through verified blob or archive ingest (`snapshot.rs:565–596, 754–789, 872–963`; `update.rs:70–103`). | PASS |
| Provenance and completeness | Pointer scope includes current extraction/format versions; `Snapshots::read` checks key, digest, identity, tree, version, and complete coverage before reuse. Failed replacement is a non-benign skip, so `ColdBuild::complete` returns `Incomplete` before publication (`snapshot.rs:185–214, 259–279, 635–679`; `update.rs:72–92`). | PASS |
| Archive fallback | Updates below the changed-blob ceiling fetch pending blobs individually; above it they reuse the cold archive path, which verifies every wanted entry and applies bounded blob fallback. The archive bound produces an incomplete outcome (`update.rs:92–101`; `snapshot.rs:599–626, 643–660, 801–865, 905–936`). | PASS |

**Verification.** Source trace and `git diff --check` completed. The candidate includes fake-provider tests for fresh-parse equivalence, failed changed blob and retry, extraction-version rebuild, and archive threshold (`update.rs:236–565`). The orchestrator reported `mise run gate` exit 0 on the candidate; this domain reviewer did not start overlapping Cargo work. No separate source-trust failure was reproduced.

**Material proposed findings:** none.

**Overall domain result: PASS.** The scoped path keeps facts attached to a verified eligible path/blob and current extraction version, and incomplete replacement cannot publish stale facts.

```citations
[]
```
