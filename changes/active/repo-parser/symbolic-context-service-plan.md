# Symbolic review-context pipeline library plan

Status: proposal supporting [revision 11 of the specification](spec.md). The specification is the authority. This plan describes the standalone library, its dependency-injection contracts, and its verification. No consuming service is implemented here.

## 1. Boundary and flow

The new outer library crate owns GitHub and GHES source access, in-memory ingest, complete source-free snapshots, incremental updates, PR-head overlays, related-file selection, shared context publication, and context read-back. The six existing crates remain IO-free and own extraction, indexing, resolution, scoring and packing. The outer crate calls their public facade instead of copying those algorithms.

A caller supplies verified tenant/installation/repository identity, GitHub credentials, and implementations of the library's narrow object-storage, snapshot-store and context-store traits. The crate exposes four independent facades:

1. **Cold build:** build or reuse a complete default-branch symbol snapshot.
2. **Incremental update:** move the default-branch pointer to a complete snapshot for a newer tree.
3. **PR context:** load a snapshot or scoped fallback, overlay a pinned PR head, rank related files and publish one selected-source context object.
4. **Context read:** verify identity and expiry, then return the exact shared context by ID.

The library does not start a server or worker, verify webhooks, run a job queue, call a model or publish review comments. Consumers choose when to call the facades. The integration tests in this repository compose them in the same order a consumer would.

```text
GitHub API -> cold snapshot -> immutable source-free facts
                         -> incremental update when requested
PR base/head + files -> private head overlay -> rank -> selected spans
                                                -> shared context object
context ID + tenant/installation -> read facade -> same selected spans
```

## 2. Symbolic library additions

- Export and load persistable declaration facts without docstrings, signatures or other source fragments. Loaded facts reproduce lookup and ranking from a fresh parse.
- Expose an extraction version and path-only language/eligibility check. A grammar or fact change invalidates incompatible snapshots.
- Rank across all supported changed head-side files: collect declarations/references on added lines, score each candidate once per distinct matched name, exclude every changed path, and sort by score then canonical path.
- Pack candidate-only spans under caller-supplied file and token limits. The PR diff is separate and still consumes the caller's overall review budget. Preserve the existing single-file public journey.

## 3. Cold snapshot and scoped fallback

The cold-build facade reads a pinned default-branch commit/tree, lists its eligible files, and streams the commit archive without a checkout or source file on disk. Eligible files are supported regular blobs no larger than the configured file bound, excluding vendored/generated paths and root `.gitattributes` overrides; LFS pointers, symlinks and submodules are excluded with reasons. Each archive entry is checked against its Git blob SHA. Archive substitutions or omissions use bounded verified blob fetches.

The index retains facts, path/blob identity, and coverage, then drops source bytes. A reusable snapshot is written at an immutable tenant/installation/repository/version/tree key only when every eligible file is indexed. The pointer is published only after the object is complete and validated. An incomplete or timed-out build leaves the old pointer unchanged, returns exact coverage and a retry reason, and can be retried from scratch. The PR facade uses a bounded scoped index around changed paths when a complete snapshot is unavailable; it never claims repository-wide coverage.

The caller bounds the cold build by dropping its future (`timeout`, `select!`); the library imposes no maximum. A caller may wait for readiness before beginning an independent review deadline.

## 4. Incremental snapshot update

The update facade reads the current default-branch tree and compares eligible `(path, blob_sha)` identities with the prior snapshot. It copies unchanged facts, verifies/parses added or changed blobs, removes deleted or now-ineligible paths, and treats renames as delete plus add. A failed replacement removes stale facts from the attempt and prevents pointer movement. If the change set exceeds the bounded incremental path, the facade does a new cold build. Only a complete new snapshot is published. Re-reading the branch head after publication reports when another update is needed.

The facade is safe under force-pushes because it compares trees rather than commit ancestry. An in-progress PR review keeps its immutable loaded snapshot. The caller bounds the update the same way; callers may invoke it after pushes or when the PR facade reports a stale default-branch pointer.

## 5. PR-context build and shared read

1. Load a compatible complete snapshot into a private index. If absent, use the bounded scoped fallback and record that scope.
2. Read the PR head tree and reconcile every eligible difference against the loaded index. A failed changed blob removes old facts and marks the overlay incomplete. Other PRs cannot see this overlay.
3. Read PR base/head SHAs, paginate the PR-files endpoint, and re-read the PR. If either SHA changed, discard the pages and retry within the review bound. Verify head blob identities against the pinned head tree. Missing or malformed patches are reported per file; they do not erase valid work.
4. Parse added head-side lines, collect distinct changed-line names, rank other files, and fetch only candidate blobs that can be packed. Reserve estimated diff and prompt tokens before candidate packing. Zero remaining context is an explicit budget outcome; downstream model callers must count their complete request separately.
5. Write one immutable selected-context object with source spans, pinned identities, matched names, ranges, coverage, omissions and budget. Publish its context ID and exact key only after validating the object.
6. On read, require the matching tenant and installation, check integrity and `expires_at`, and return the stored spans without another GitHub fetch or reranking. A missing/corrupt object fails explicitly.

The caller bounds the PR-context build independently of cold build and update. A context is readable for 24 hours from publication. Expired reads fail even if deletion is pending; the library exposes cleanup that deletes expired objects and reports/retries lag when invoked.

## 6. Injected dependencies and publication contract

The outer crate defines narrow traits for:

- **Object storage:** create-only immutable object writes, exact-key reads, integrity validation and deletion. The consumer implements it over its own storage handle (Wyrd over `wyrd-storage`); the library ships no backend or OpenDAL adapter. Snapshot and context objects may use separate instances.
- **Snapshot store:** load and conditionally publish snapshot metadata/pointers, and clean up expired unreferenced snapshots.
- **Context store:** publish context metadata, exact-key identity-bound lookup, and expired-context cleanup.

Each facade accepts only the stores it uses. The library accepts no raw PostgreSQL pool and ships no production SQL schema or migration. A consuming PostgreSQL adapter is responsible for tenant-bound RLS and separate privileged operator operations.

Tests use separate in-memory implementations of each trait to prove publication order, retries and tenant/installation isolation. Object bytes are written and validated before any pointer or context-ID metadata becomes visible. No transaction spans GitHub or storage IO. Snapshot objects and metadata contain no source text; only selected-context objects do.

## 7. Bounds and failure semantics

Configurable initial ceilings: 1 MiB per eligible file, 4 GiB expanded archive, 200 archive-fallback blobs, 2,000 changed blobs before a full rebuild, 300 PR overlay changes, 300 scoped files, the provider's 3,000 PR-file ceiling, 15 selected candidate files, a 64,000-token review allocation and a 4,096-token prompt reserve. The library honors GitHub primary/secondary rate limits, `retry-after`, and concurrency limits. A dropped call leaves the pointer on a complete snapshot; a retry converges. Every bound yields explicit coverage, omission or retry/terminal status. No source, credential or tenant-crossing cache key appears in logs or metrics.

## 8. Verification scenarios

Normal CI uses fake GitHub and injected in-memory dependencies. The real GitHub checks are explicitly invoked and stay outside normal CI and `mise run gate`.

| ID | Scenario | Required result |
|---|---|---|
| S1 | First PR on a cold 500-file repository | Complete source-free snapshot, correct context, separate build/review clocks |
| S2 | Second PR on the same tree | Snapshot reused; no archive download; bounded changed/candidate fetches |
| S3 | Three changed default-branch files | Only changed blobs fetched; immutable old snapshot; pointer moves |
| S4 | Rapid successive branch heads | Repeated update calls converge on current tree without stale pointer |
| S5 | Concurrent PRs | Each private overlay contains only its own changes |
| S6 | Two PRs edit the same path differently | Each result equals its own pinned-head oracle |
| S7 | Stale PR base | Overlay accounts for intervening default-branch changes or reports incomplete coverage |
| S8 | Extraction/format version change | New snapshot key; old facts are not reused |
| S9 | Missed update call | Review-time staleness is reported; caller may update separately without blocking review |
| S10 | Truncated tree or failed full build | Bounded scoped index and honest scope; no reusable partial snapshot |
| S11 | Archive substitution/omission and LFS pointer | Verified fallback, reasoned exclusion, no false complete pointer |
| S12 | Provider primary/secondary limits | Bounded retry or visible rate-limited outcome |
| S13 | Invalid invocation identity/input | No source access or publication; explicit failure |
| S14 | Two tenants/installations with identical source | Separate keys and metadata; cross-identity read/write rejection |
| S15 | Shared context and 24-hour expiry | Repeated identical reads before expiry; no source in snapshot/metadata; expired/corrupt reads fail; deletion lag reported |
| S16 | Force-push and default-branch change | Tree-based update works with current branch identity |
| S17 | Interruption after object write before metadata | Retry converges; no visible pointer to missing object |
| S18 | 400-file PR with absent/malformed patches | Pagination correct; affected files omitted; valid files still analyzed; changed base/head causes retry |
| S19 | Diff and prompt reserve exhaust budget | Empty candidate context with explicit reason; no false model-limit guarantee |
| S20 | Existing PR in the real `bohmian-ai/wyrd` repository | Configurable on-demand command runs this library's facades with real GitHub and injected test dependencies, analyzes a supported changed file, stores nonempty selected spans, reads them back by ID, and records identity/coverage/calls/time/memory; no checkout, PR mutation or target-repository code change |

A from-scratch parse of every eligible file at a fully covered head is the oracle for facts and ranking. Property sequences cover add, modify, delete, rename, mode and attributes changes. A controlled real-provider contract run checks archive redirects, tree/blob shapes, patch omissions and rate headers. Fixed-commit benchmarks at approximately 1k, 10k and 50k eligible files measure cold/warm time, peak memory, snapshot size and API calls; provisional limits are revised from evidence before rollout.

## 9. Acceptance and ownership

The change is complete when the public symbolic journey, four facade journey, S1–S20, injected trait contract tests, provider contract check and benchmark evidence pass, followed by independent task and integrated change review. S20 is mandatory recorded evidence but never part of normal CI. No task or acceptance gate requires a consuming repository to add code, schema, routes, jobs or tests.
