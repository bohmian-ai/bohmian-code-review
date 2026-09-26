# Symbolic context service: design and test plan

Status: proposal for independent review, 2026-09-25. Not an approved spec. Approval would produce one library spec revision (section 4) and one new service change packet (sections 5–10).

## 1. Summary

We will build a code-review context service that reproduces Snap CodePal's "Building Code Context" flow ([eng.snap.com/codepal](https://eng.snap.com/codepal)) without cloning repositories:

1. **Parse source code.** Tree-sitter extracts each file's declarations (name, kind, line span).
2. **Analyze PR diff.** The PR's changed lines are read from diff text.
3. **Pass 1, symbol indexing.** A symbol → files map is built across every eligible repository file.
4. **Pass 2, intelligent selection.** Symbols on the changed lines are looked up. Other files are scored by how many distinct changed symbols they define. The top spans are picked within a token budget.
5. **Context for AI review.** The selected context is stored for the review agents.

The expensive part is step 3: every file must be downloaded and parsed. We make it cheap with three mechanisms:

- **Build once.** The first time a repository is seen, the whole default branch is downloaded as one in-memory tarball and parsed once.
- **Cache symbols, not source.** The result is saved as one immutable **symbol snapshot** per repository commit in object storage. The snapshot contains names, kinds and line numbers only, never source text.
- **Reuse per review.** Each review loads the snapshot and re-parses only files whose content differs from the PR head. It then fetches only the few candidate files it will actually send to the model.

**In scope:**
- The library changes the service needs (section 4).
- The service's ingest, snapshot lifecycle, review-context flow, storage, GitHub usage, security, failure handling, and tests.

**Out of scope:**
- The model review loop, posting comments, and feedback.
- Cross-repository search.
- Exact (compiler-grade) definition resolution.
- Full indexing of repositories whose Git tree exceeds GitHub's 100,000-entry listing limit. These get scoped indexing only (section 5.5).
- GitLab.

## 2. Current state: the `bohmian-code-review` library

The library is a Rust 2024 workspace of six crates. It is IO-free: it never clones, reads disk, calls the network, or persists anything. It is complete through TASK-005 on branch `review-helper-setup`, and `mise run gate` is green.

| Diagram step | Crate | Public API today |
|---|---|---|
| Parse source code | `bohmian-code-review-extract` | `Extractor::extract_from_source(path, &str) -> ExtractionResult { nodes, edges, references, diagnostics }`; about 30 native languages |
| Analyze PR diff | `bohmian-code-review-select` | `parse_unified_diff(&str) -> Vec<FileDiff { old_path, new_path, added }>`; `parse_hunks(&str) -> Vec<LineRange>` |
| Build symbol-to-files map | `bohmian-code-review-index` | `SymbolIndex::{upsert, remove, lookup}`; retains nodes and byte length, never source |
| Look up symbols | `bohmian-code-review-resolve` | `resolve(&index, names) -> Vec<NameCandidates>`; ambiguity kept |
| Score files | `bohmian-code-review-select` | `rank(...) -> Ranking`; score = count of distinct matched names, ties broken by path |
| Pick top files within budget | `bohmian-code-review-select` | `pack(...) -> Context { spans, omissions }`; caller-supplied token counter, file and token ceilings |
| Facade | `bohmian-code-review` | `SymbolicContext::{upsert, remove, lookup, rank}`, `build_context`, `parse_unified_diff`, `parse_hunks` |

The public journey test (`crates/bohmian-code-review/tests/symbolic_context_journey.rs`) runs the whole diagram in memory:
- It indexes four TypeScript files and parses a PR diff.
- `src/user.ts` scores 2 and `src/format.ts` scores 1; the unrelated file is excluded.
- It packs within an exact token budget and is deterministic on rerun.

**Gaps the service needs, closed by section 4:**
- Indexed facts cannot be saved and reloaded without re-parsing.
- There is no pre-download "can we parse this path?" check.
- There is no cache-key version for extraction output.
- Ranking takes one changed file, but a PR changes many.
- Packing always includes changed hunks, which duplicates the diff the model already receives and fails large PRs.

## 3. Architecture and ownership

```
GitHub (github.com or GHES)
   │  webhooks: push, pull_request, repository, installation*
   ▼
review-context service  (new repository; Rust, tokio, axum; Kubernetes Deployment)
   ├─ webhook receiver ──► jobs table (Postgres) ◄── workers (same binary, N replicas)
   ├─ GitHub client: App auth, per-installation rate budget, retries
   ├─ ingest: tree listing, eligibility filter, tarball stream, blob fetch
   ├─ snapshot store: object storage (S3-compatible) + pointer rows (Postgres)
   ├─ review builder: diff → overlay index → rank → pack → context object
   └─ uses bohmian-code-review (library, pinned git tag; no IO inside)
   ▼
review_contexts (Postgres row + object, 24 h TTL) ──► review agents (out of scope)
```

**Decisions:**
- **Separate repository.** The service lives in `bohmian-code-review-service`, depending on the library by a pinned git tag. The library's no-IO rule (AGENTS.md) stays enforceable because network, database and object-store code never enter the library workspace.
- **Rust with the tokio/axum stack.** Parsing runs in `tokio::task::spawn_blocking` through the library's existing bounded `ParsePool`, so parsing never blocks the async runtime.
- **Postgres is the only database.** It holds pointers, jobs, deliveries and context metadata. Symbol snapshots and contexts live in object storage, accessed through the `object_store` crate (S3, GCS, or MinIO in tests).
- **No Redis, no message broker.** Jobs use a Postgres table with `SELECT … FOR UPDATE SKIP LOCKED`.
- **Stateless workers.** Any replica can run any job, and all state is in Postgres or object storage. There is no worker-memory cache in v1; add an LRU of loaded indexes only if profiling shows snapshot GET and decode dominating review latency.

## 4. Library changes (spec revision on `bohmian-code-review`)

All are additive, IO-free, and delivered through the `cr-*` lifecycle as one spec revision with one task each.

**L1 — Load facts without parsing.**
- `SymbolicContext::upsert_facts(path: &str, byte_len: usize, facts: Vec<SymbolFact>) -> Result<(), Error>` indexes pre-extracted facts.
- `SymbolicContext::facts(&self) -> impl Iterator<Item = (&RelativePath, usize, Vec<SymbolFact>)>` exports them.
- `SymbolFact` is defined in `core` with `{ id, kind, name, qualified_name, language, start_line, end_line, start_column, end_column }`. It derives `Serialize`/`Deserialize` (serde is already a core dependency).
- `SymbolFact` deliberately omits `docstring`, `signature`, `decorators`, `type_parameters`, `return_type` and `extra`, because those are verbatim source fragments. Only declarations (`is_declaration`) are exported.
- Invariant: ranking over an index loaded by `upsert_facts` equals ranking over the index built by `upsert` from the same sources.

**L2 — Extraction version.**
- `pub const EXTRACTION_VERSION: &str` in `extract`, part of every snapshot key.
- A golden test hashes the `SymbolFact`s extracted from the checked-in fixture corpus. If extraction output changes, the test fails with the instruction "bump EXTRACTION_VERSION and update this hash". Grammar pin bumps are therefore caught mechanically.

**L3 — Eligibility by path.**
- `Extractor::language_for_path(path: &str) -> Option<Language>` returns the routed language using the path alone, via the existing `LanguageRouter::detect(path, None)`, or `None` when unsupported.
- The service calls it on tree entries before downloading anything. Content-based disambiguation (such as `.h` C versus C++) still happens at parse time.

**L4 — PR-level ranking.**
- `SymbolicContext::rank_changes(changes: &[ChangedFile<'_>]) -> Result<Ranking, Error>`, where `ChangedFile { path: &str, head: &[u8], lines: Vec<LineRange> }`.
- Query names are the union over every changed file of:
  - declarations overlapping its changed lines, and
  - references on its changed lines.
- Every changed path is excluded from candidates.
- Score is still the count of **distinct** matched names per file. Ties are broken by ascending path.
- `Ranking.changed` becomes a list of `(path, ChangedLines)` in path order.
- The existing single-file `rank` becomes `rank_changes` with one element, and its behavior and tests are unchanged.

**L5 — Candidates-only packing.**
- `ContextLimits` gains `include_changed_hunks: bool`. `true` keeps today's behavior.
- The service passes `false` because the review agent receives the full PR diff separately. Changed hunks then consume no budget and a large PR can no longer fail packing.

**Library verification:**
- Owner tests for each of L1–L5.
- The public journey is extended:
  - (a) index → export facts → postcard round trip → new `SymbolicContext` via `upsert_facts` → identical `Ranking` and `Context`;
  - (b) a two-file PR through `rank_changes`.
- `mise run gate` must pass.

## 5. Service: ingest and main snapshot lifecycle

### 5.1 Eligibility (what gets parsed)

A tree entry is eligible when all of the following hold:
1. Its mode is `100644` or `100755`. Symlinks (`120000`) and submodules (`160000`) are skipped.
2. `language_for_path(path)` returns a language (L3).
3. Its size is at most `MAX_FILE_BYTES` (default 1 MiB), taken from the tree listing's `size` field.
4. It is not vendored or generated:
   - Its path has no segment equal to `node_modules`, `vendor`, `third_party`, `dist`, `build`, `.git`.
   - It does not end in `.min.js` or `.min.css`.
   - It is not marked `linguist-generated` or `linguist-vendored` in the **root** `.gitattributes` ([linguist overrides](https://github.com/github-linguist/linguist/blob/main/docs/overrides.md)). Nested `.gitattributes` files are ignored in v1.
5. At parse time, content starting with `version https://git-lfs.github.com/spec/v1` is skipped as an LFS pointer.

Skips are counted by reason and stored in the snapshot header.

### 5.2 First-time snapshot build (cold start)

Triggered when a review or push arrives for a repository with no pointer row for the current `EXTRACTION_VERSION`.

1. Take the per-repo build lock (section 7, `snapshot_heads` row, `SELECT … FOR UPDATE`). A second job for the same repo waits, then finds the snapshot built.
2. `GET /repos/{o}/{r}/branches/{default}` gives the head commit SHA and root tree SHA. (1 call)
3. `GET /repos/{o}/{r}/git/trees/{tree_sha}?recursive=1` gives every path, mode, blob SHA and size. (1 call)
   - If `truncated: true` (over 100,000 entries or 7 MB, per [Trees API](https://docs.github.com/en/rest/git/trees)), abort the build. Record `mode = scoped` on the repository; section 5.5 then applies.
4. Fetch the root `.gitattributes` blob if present (0–1 call), then apply section 5.1 to get the eligible set `E` of `(path, blob_sha, size)`.
5. `GET /repos/{o}/{r}/tarball/{commit_sha}` follows the 302 redirect to `codeload`. (1 API call; the redirected download does not count against the REST budget.)
   - The gzip stream is decoded with `flate2` and read with the `tar` crate in a blocking task fed by a bounded channel from the HTTP body. Nothing is written to disk.
   - For each entry, strip the top-level `{owner}-{repo}-{sha7}/` directory. If the path is not in `E`, discard the bytes.
   - Otherwise compute the Git blob SHA-1 `sha1("blob " + len + "\0" + bytes)` and compare it with `E`'s SHA.
     - **Match:** `SymbolicContext::upsert(path, bytes)` parses and indexes it, then the bytes are dropped.
     - **Mismatch or missing** (tarballs honor `export-subst` and `export-ignore`): queue the path for blob fetch.
   - Limits: total uncompressed bytes at most `MAX_TARBALL_BYTES` (default 4 GiB). Exceeding it aborts, and the build falls back to section 5.5.
6. Fetch queued paths with `GET /repos/{o}/{r}/git/blobs/{sha}` (raw media type), at most `MAX_FALLBACK_BLOBS` = 200. Beyond that, the remaining files are recorded as skipped with reason `archive_mismatch`, and the snapshot is marked partial.
7. Parse failures per file (unsupported, deferred, malformed, not UTF-8) are counted by reason. They never fail the build.
8. Serialize the snapshot (section 7.1) and `PUT` it at its key with create-only semantics. An existing object is identical by construction, so the PUT is skipped.
9. Insert the `snapshots` row, upsert the `snapshot_heads` pointer, commit, and release the lock.

**Cost:** 3–4 API calls plus fallback blobs, one download of about the repo's compressed size, and one parse of every eligible file. Parse time, snapshot size and index memory are not yet measured; section 11.6 measures them before rollout.

### 5.3 Incremental update when `main` moves

Triggers:
1. A `push` webhook whose `ref` is the default branch. Primary.
2. A review-time staleness check. At the start of each review, a conditional `GET branches/{default}` with the stored ETag is made; a 304 is free ([best practices](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api)). If the head differs from the pointer, an update job is enqueued. The review does **not** wait.
3. An `EXTRACTION_VERSION` change: no pointer exists for the new version, so section 5.2 runs.
4. A `repository` `edited` webhook that changes the default branch: update `repositories.default_branch` and enqueue an update.

Not triggers: timers, PR events, and pushes to other branches. A merge is a push to the default branch.

Update job, holding the per-repo lock:
1. `GET branches/{default}` returns the **current** head, not the webhook's `after` SHA, so a burst of pushes collapses into one update. If it equals the pointer, finish.
2. `GET trees/{tree}?recursive=1`, plus `.gitattributes` if its SHA changed. Truncation switches the repo to scoped mode.
3. Load the current snapshot and compare eligible sets by `(path, blob_sha)`:
   - **Unchanged:** facts are copied.
   - **Added or changed:** blob fetch, then `upsert`.
   - **Deleted:** dropped.
   - **Renamed:** treated as delete plus add, and re-parsed, because node IDs hash the file path.
4. If more than `MAX_INCREMENTAL_BLOBS` (default 2,000) need fetching, run section 5.2 from a tarball instead.
5. Write the new snapshot object, insert the row, advance the pointer, commit.
6. After commit, re-read the branch head. If it moved during the job, enqueue another update.

**Properties:**
- Snapshots are immutable and keyed by tree SHA, so a review that already loaded the old snapshot is unaffected.
- The pointer changes only after the object exists, so no reader ever sees a half-written snapshot.
- Force-pushes and history rewrites need no special handling, because updates compare trees, not history.

### 5.4 Garbage collection

A daily job deletes, for each repo and extraction version, every snapshot object and row that is neither the pointer target nor younger than 24 h. Removing an installation or repository (webhooks `installation` `deleted`, `installation_repositories` `removed`) deletes all of its rows and objects immediately.

### 5.5 Scoped mode (repository too large for a full index)

Used when the tree listing is truncated, the tarball exceeds limits, or a full build fails.

- Per review, the service indexes the eligible files in the directories containing the PR's changed files and in their parent directories, **non-recursively**, capped at `MAX_SCOPED_FILES` = 300.
- Directory listings use non-recursive tree calls walked from the root tree: one call per path segment, cached per review.
- Nothing is persisted in scoped mode.
- The context's coverage says `mode: scoped` with file counts, so the model and humans know ranking saw a subset.

## 6. Service: PR review flow

Triggered by `pull_request` actions `opened`, `reopened`, `synchronize` and `ready_for_review`. Drafts are skipped until `ready_for_review`.

The job dedupe key is `review:{repo_id}:{pr_number}:{head_sha}`. A newer `synchronize` for the same PR cancels queued jobs for older head SHAs.

1. **Snapshot.** Read the pointer. If none exists, run section 5.2 inline (the only blocking build). If the build falls back to scoped mode, go to section 5.5. Start the staleness check from section 5.3 (trigger 2).
2. **Load.** `GET` the snapshot object, verify its header (`format_version`, `extraction_version`, `repo_id`), then `upsert_facts` every file into a fresh `SymbolicContext`. This index is private to the job.
3. **Head tree.** `GET trees/{pr_head_tree}?recursive=1` (1 call), then compare eligible sets against the snapshot as in section 5.3 step 3:
   - Changed or added files are fetched as blobs and upserted.
   - Deleted files are removed.
   - The index now equals a from-scratch parse of the PR head. This includes files `main` changed after the PR's base (the "stale base" case).
   - If more than `MAX_REVIEW_OVERLAY` = 300 files differ, only the PR's own changed files are applied, and coverage is marked `mode: approximate`.
   - Blob bytes of the PR's own changed files are kept in memory until step 6.
4. **Diff.** `GET /repos/{o}/{r}/pulls/{n}/files`, paginated at 100 per page and up to 3,000 files ([PR files](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests-files)). For each file, `parse_hunks(patch)` gives its added lines.
   - A file without a `patch` field (too large or binary) is listed as `changed, not analyzed`.
   - A `DiffError` on one file marks that file `not analyzed` and never fails the review.
5. **Rank.** `rank_changes` over every changed file that has head bytes, a supported language and non-empty added lines (L4). The result is candidate files, ordered, with the matched names as reasons.
6. **Budget.**
   - `total = MAX_REVIEW_TOKENS` (per-installation config, default 64,000).
   - `diff_tokens = estimate(diff text)`.
   - `context_tokens = total − diff_tokens`.
   - If `context_tokens ≤ 0`, the context is empty with reason `diff_exceeds_budget`.
   - The token estimator is `ceil(utf8_bytes / 3)`. This deliberately over-counts relative to Claude tokenization of code, so the real prompt stays under budget. Section 11.5 validates it against the token-counting API on the benchmark corpus.
7. **Pack.**
   - Fetch candidate blobs in rank order, only for files that can still be selected, up to `max_files` (default 15).
   - Call `build_context(ranking, …, ContextLimits { max_files, max_tokens: context_tokens, include_changed_hunks: false }, estimate)` (L5).
   - `MissingBlob` cannot occur because every selectable file was fetched. A 404 on a blob (a force-push race) aborts the job for retry.
8. **Publish.**
   - Write the context object (section 7.1) with a 24 h TTL.
   - Insert the `review_contexts` row and mark the job done.
   - Drop the in-memory index and all source bytes.

**Concurrency:**
- Different PRs, including PRs touching the same files, run on separate private indexes built from the same read-only snapshot.
- No PR's content is ever written to a shared snapshot, so PRs cannot see each other's changes, and closing a PR needs no cleanup.

## 7. Storage

### 7.1 Object storage layout

| Object | Key | Contents | Lifetime |
|---|---|---|---|
| Symbol snapshot | `snapshots/{installation_id}/{repo_id}/{extraction_version}/{tree_sha}.snap.zst` | Header + per-file facts, **no source** | Until GC (section 5.4) |
| Review context | `contexts/{installation_id}/{repo_id}/{pr_number}/{head_sha}.json.zst` | Selected source spans, reasons, omissions, coverage | Bucket lifecycle deletes after 24 h |

**Snapshot encoding:**
- zstd (level 3) over postcard-serialized `Snapshot`.
- `Snapshot` fields:
  - `format_version: u16`
  - `extraction_version`
  - `installation_id`, `repo_id`
  - `commit_sha`, `tree_sha`
  - `created_at`
  - `coverage { eligible, indexed, skipped_by_reason }`
  - `files: Vec<SnapshotFile { path, blob_sha, language, byte_len, facts: Vec<SymbolFact> }>`, sorted by path.
- The encoding is deterministic, so identical input produces identical bytes.

**Context encoding:**
- zstd JSON with fields:
  - `repo`, `pr`, `head_sha`
  - `coverage { mode: full | approximate | scoped, eligible, indexed }`
  - `budget { total, diff_tokens, context_tokens, used }`
  - `spans [ { path, start_line, end_line, score, names, tokens, text } ]`
  - `omissions [ { path, lines, names, reason } ]`
  - `not_analyzed [ { path, reason } ]`

Both buckets are private, encrypted at rest (SSE), and access is restricted to the service's workload identity.

### 7.2 Postgres schema

```sql
create table installations (
  installation_id bigint primary key,
  account_login   text not null,
  max_review_tokens int not null default 64000,
  max_files       int not null default 15,
  created_at      timestamptz not null default now()
);

create table repositories (
  repo_id         bigint primary key,
  installation_id bigint not null references installations on delete cascade,
  full_name       text not null,
  default_branch  text not null,
  index_mode      text not null default 'full' check (index_mode in ('full','scoped')),
  branch_etag     text
);

create table snapshots (
  repo_id            bigint not null references repositories on delete cascade,
  extraction_version text   not null,
  tree_sha           text   not null,
  commit_sha         text   not null,
  object_key         text   not null,
  files_eligible     int    not null,
  files_indexed      int    not null,
  object_bytes       bigint not null,
  created_at         timestamptz not null default now(),
  primary key (repo_id, extraction_version, tree_sha)
);

create table snapshot_heads (            -- pointer + per-repo build lock row
  repo_id            bigint not null references repositories on delete cascade,
  extraction_version text   not null,
  tree_sha           text,               -- null until first build completes
  commit_sha         text,
  updated_at         timestamptz not null default now(),
  primary key (repo_id, extraction_version)
);

create table jobs (
  job_id       bigserial primary key,
  kind         text not null check (kind in ('snapshot_update','review','gc')),
  repo_id      bigint,
  dedupe_key   text not null unique,     -- deleted on completion so the key can recur
  payload      jsonb not null,
  priority     int  not null,            -- review = 0 (first), snapshot_update = 10, gc = 20
  state        text not null default 'queued' check (state in ('queued','running','done','failed')),
  attempts     int  not null default 0,
  run_after    timestamptz not null default now(),
  locked_until timestamptz
);

create table webhook_deliveries (         -- replay protection, pruned after 7 days
  delivery_id text primary key,
  received_at timestamptz not null default now()
);

create table review_contexts (
  repo_id    bigint not null references repositories on delete cascade,
  pr_number  int    not null,
  head_sha   text   not null,
  object_key text   not null,
  coverage   text   not null,
  created_at timestamptz not null default now(),
  primary key (repo_id, pr_number, head_sha)
);
```

**Job rules:**
- Workers claim work with `update jobs set state='running', locked_until=now()+interval '15 min' … where job_id = (select job_id from jobs where state='queued' and run_after<=now() order by priority, job_id for update skip locked limit 1)`.
- Expired `locked_until` rows return to `queued`.
- A job is retried at most 5 times, with exponential backoff and jitter, and then marked `failed`.

**Why no source text in Postgres:** snapshot payloads (tens of MB for large repos) would bloat backups and vacuum. Postgres holds only pointers and metadata.

## 8. GitHub API usage and rate limits

**Auth:** a GitHub App.
- JWT signed with the App private key, then an installation access token, cached until 5 minutes before expiry.
- One code path serves github.com and GHES, with the base URL from configuration.

**Budgets** ([REST rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)):

| Limit | github.com value | Service behavior |
|---|---|---|
| Installation hourly | 5,000 base, +50 per repo and per user over 20, max 12,500; 15,000 on Enterprise Cloud | Per-installation budget tracker from `x-ratelimit-remaining` / `x-ratelimit-reset`. Below 20% remaining, `snapshot_update` jobs are deferred to reset; reviews continue. At 0, all jobs for that installation wait until reset. |
| Concurrency | 100 concurrent | Global semaphore of 50; per-installation semaphore of 8 |
| REST points | 900 points per minute | Per-installation limiter of 10 requests per second |
| Secondary 403/429 | `retry-after` header | Honor `retry-after`; otherwise back off 60 s, doubling to 15 min |
| GHES | API rate limiting is disabled by default ([GHES](https://docs.github.com/en/enterprise-server@latest/admin/configuring-settings/configuring-user-applications-for-your-enterprise/configuring-rate-limits)) | Missing rate headers mean no hourly budget; concurrency caps still apply |

**Calls per operation.** `k` = changed blobs, `f` = PR files, `d` = files `main` changed since the PR base, `c` = candidate blobs fetched (≤ `max_files`):

| Operation | Calls |
|---|---|
| Cold build | branch 1 + tree 1 + `.gitattributes` 0–1 + tarball 1 + fallback blobs 0–200 |
| Main update | branch 1 + tree 1 + `.gitattributes` 0–1 + `k` blobs |
| Review, warm | staleness check 1 (free on 304) + head tree 1 + PR files ⌈f/100⌉ + (`f`+`d`) blobs + `c` blobs |
| Example: 5-file PR, fresh base, 15 candidates | 1 + 1 + 1 + 5 + 15 = 23 calls, so about 540 reviews per hour per 12,500-call installation |

## 9. Security, tenancy and source handling

- **Webhooks:**
  - `X-Hub-Signature-256` HMAC is verified with a constant-time compare; invalid requests get 401.
  - A `X-GitHub-Delivery` already in `webhook_deliveries` gets 200 with no action.
  - Payloads over 25 MB are rejected.
- **Secrets:** the App private key and webhook secret come from the cluster secret store as environment variables. Tokens are never logged.
- **Source at rest:**
  - Snapshots contain no source text: no docstrings or signatures (L1).
  - Source exists at rest only in review-context objects, which are encrypted, private, and deleted after 24 h. This is the one deliberate exception to "no source on disk", because review agents in other workflows must read the same context (as in CodePal's shared store). Removing it would mean handing contexts to agents in-process.
- **Logs and metrics** carry IDs, SHAs, counts and durations only. A test enforces this (section 11.4, S15).
- **Tenancy:**
  - All keys and rows are scoped by `installation_id` and `repo_id`.
  - Blob-SHA facts are never shared across installations. A cross-tenant cache hit would reveal that another customer has identical file content.
  - Reuse across repositories within one installation is also not done in v1, because keys are per repo.
- **Integrity:** tarball bytes are verified against the tree's blob SHAs before parsing. Paths pass `RelativePath` validation, which rejects `..`, absolute paths and NUL.
- **Resource bombs:** caps apply to file size (1 MiB), total tarball bytes (4 GiB), fallback blobs (200), overlay files (300) and PR files (3,000). Each job has a wall-clock deadline: build 15 min, update 10 min, review 5 min.

## 10. Failure modes

| Failure | Behavior |
|---|---|
| Webhook missed or GitHub delivery fails | The review-time staleness check (section 5.3, trigger 2) enqueues the update. |
| Tree truncated | Scoped mode (section 5.5); coverage labeled `scoped`. |
| Tarball 5xx, timeout or over size | Retried twice, then scoped mode for this review; the build is retried on the next push. |
| Tarball content differs from tree (`export-subst`, `export-ignore`) | Blob fallback up to 200 files, then `archive_mismatch` skips; coverage counts the skipped files. |
| Blob 404 (force-push race) | Job retried; the retry reads the new head. |
| Snapshot object missing or corrupt, or header mismatch | Pointer row deleted and cold build enqueued; this review uses scoped mode. |
| Worker crash mid-build | Lock released by transaction abort. Object PUT is create-only and content-addressed, so a rerun is idempotent. The pointer never points at a missing object. |
| Extraction of one file fails | Counted by reason; never fails a build or review. |
| PR diff for one file malformed or missing | File listed `not analyzed`; the review continues. |
| Diff alone exceeds token budget | Empty context with reason `diff_exceeds_budget`. |
| Rate budget exhausted | Updates deferred; reviews wait until reset. Waits over 15 min mark the review failed with reason `rate_limited`. |

## 11. Test plan

Every layer runs in CI without GitHub credentials, except the opt-in contract suite in 11.5.

### 11.1 Library (bohmian-code-review, `mise run gate`)

- **L1:**
  - `upsert_facts` then `facts` round-trips.
  - Facts carry no docstring or signature fields.
  - Ranking equivalence between the parsed index and the facts-loaded index across the fixture corpus languages.
- **L2:** the golden hash test fails when extraction output changes; proven by a deliberately altered fixture during implementation.
- **L3:** a path table covering every native language extension, unsupported extensions, and ambiguous `.h`.
- **L4:**
  - A two-file PR: the union of names, both changed paths excluded, a distinct-name score that doesn't double-count a name used in both files, deterministic order.
  - The single-file `rank` results are unchanged.
- **L5:** with `include_changed_hunks: false`, no changed spans and the full budget goes to candidates. The existing `true` tests are unchanged.
- **Journey additions:** a postcard snapshot round trip gives an identical ranking and context, and a multi-file PR through `rank_changes`.

### 11.2 Service unit tests (`cargo test`, no containers)

- **Blob SHA:** known vectors, including `sha1("blob 6\0hello\n") = ce013625030ba8dba906f756967f9e9ca394464a`, which matches `git hash-object`.
- **Eligibility:** modes, size cap, vendored segments, `.min.js`, root `.gitattributes` parsing (`linguist-generated`, `linguist-vendored`, `-linguist-generated` negation), LFS pointer detection.
- **Tarball reader over in-memory `.tar.gz` fixtures:**
  - prefix stripping and SHA match;
  - an `export-subst` mismatch routed to the fallback queue;
  - a missing file;
  - over-cap total bytes;
  - a non-UTF-8 file;
  - an assertion that nothing is written to disk (the reader takes `impl Read` only; no filesystem APIs in the ingest module, enforced by a `rg` gate).
- **Tree diff:** added, changed, deleted, renamed, and mode changes to or from symlink, against expected overlay sets.
- **Snapshot codec:** deterministic bytes, round-trip equality, rejection of an unknown `format_version`, rejection of a header for another repo or version.
- **Rate limiter:** header parsing, the 20% deferral threshold, `retry-after`, backoff sequence, missing-header (GHES) mode.
- **Token estimator:** `ceil(bytes/3)` on ASCII and multibyte input.
- **Budget:** `diff_exceeds_budget`, and `context_tokens` arithmetic.

### 11.3 Fake GitHub

An in-process axum server that implements exactly the endpoints the service calls:
- installation token;
- `branches/{b}` with ETag and 304;
- `git/trees` (recursive, with configurable truncation);
- `git/blobs` (raw);
- `tarball` with a 302 to a second fake host;
- `pulls/{n}/files` with pagination and patch omission;
- rate-limit headers.

It serves a **fixture repository model**: an in-memory list of commits, each a map of path to bytes and mode. The fake computes real Git blob SHAs, generates tree listings, and builds tarballs on request, applying `export-ignore` and `export-subst` if configured. It records every call by endpoint. Fault injection covers:
- a 403 secondary limit with `retry-after`;
- a 5xx on the tarball;
- a blob 404;
- a truncated tree;
- `remaining = 0`;
- delayed responses.

**Oracle check.** One test builds a real git repository with the `git` CLI in a temp dir from the same fixture model. It asserts that the fake's blob SHAs and tree SHAs equal `git`'s, so the fake cannot drift from real Git semantics.

### 11.4 Service integration scenarios (Testcontainers: Postgres + MinIO + fake GitHub)

The core oracle, used by several scenarios: **incremental equals from scratch**. For any head commit, the index built from snapshot plus overlay must yield the same `facts()` and the same ranking as a fresh `SymbolicContext` that parsed every eligible file of that head directly.

| ID | Scenario | Assertions |
|---|---|---|
| S1 | Cold start: first PR on a 500-file fixture repo | Snapshot built from tarball; exact calls: branch 1, tree 1, tarball 1, 0 blobs except candidates; pointer set; context coverage `full`; ranking equals the oracle |
| S2 | Warm review: second PR | 0 tarball calls; 1 snapshot GET; call count equals the formula in section 8 exactly |
| S3 | Main push changing 3 files | Update fetches exactly 3 blobs; a new object appears; the old object's bytes are unchanged; pointer moves; a review that started before the update completes against the old snapshot with the correct result |
| S4 | 10 pushes within 5 s | At most 2 update runs; final pointer equals the last commit |
| S5 | Concurrent PRs A (edits `a.ts`, adds symbol `newA`) and B (edits `c.ts`) | B's ranking never references `newA`; each context reflects only its own PR; snapshot object bytes are identical before and after |
| S6 | Two PRs edit the same file differently | Each ranking uses its own version (checked against the oracle per head) |
| S7 | Stale base: PR branched before main changed 20 files | Overlay includes those 20; index equals the oracle for the PR head; above 300 differences, coverage is `approximate` and the PR files are still ranked |
| S8 | `EXTRACTION_VERSION` bumped | New key; cold build; GC removes the old version's objects after 24 h (clock injected) |
| S9 | Push webhook dropped | Next review's staleness check enqueues the update; the review does not wait; the pointer later advances |
| S10 | Tree truncated | Scoped mode; at most 300 files indexed; coverage `scoped`; nothing persisted |
| S11 | `export-subst` file, `export-ignore` file, LFS pointer | The first two are fetched via blob fallback with SHAs verified; the LFS pointer is skipped and counted |
| S12 | Rate limits | `remaining` at 15% defers updates while reviews still run; `remaining = 0` makes the review wait for reset; a secondary 403 honors `retry-after` (all timed with a paused tokio clock) |
| S13 | Webhook security | Bad signature gets 401 and no job; a duplicate delivery ID gets one job; an oversize payload is rejected |
| S14 | Tenancy: two installations, identical file content | Two separate snapshot objects under separate prefixes; neither job reads the other's keys (MinIO access log) |
| S15 | No source at rest or in logs | After S1–S14, scan every `snapshots/` object, every Postgres row, and captured logs and metrics for any fixture source line of 20 or more characters: zero hits. `contexts/` objects carry the 24 h lifecycle rule. |
| S16 | Force-push to main; default-branch rename | Update succeeds from tree comparison; rename webhook updates the branch and triggers an update |
| S17 | Worker killed after object PUT, before pointer commit | Rerun succeeds; no pointer to a missing object; exactly one object for that tree |
| S18 | Large PR: 400 files, one without `patch`, one malformed patch | Pagination correct; those two are `not analyzed`; others ranked; review succeeds |
| S19 | Diff larger than budget | Empty context, reason `diff_exceeds_budget` |

**Property test.** Random sequences of 1–50 commits over the fixture model (add, modify, delete, rename, mode change, `.gitattributes` edits), with pushes and PRs interleaved. After each step, snapshot plus overlay must equal the from-scratch oracle. Runs 256 cases per CI run with a fixed seed. Failing seeds are recorded as regression cases.

### 11.5 Contract tests against real GitHub (opt-in, nightly)

These run in a sandbox org with a test App and a fixture repository pushed by the test. They confirm the assumptions the fake encodes:
- tarball 302 and prefix format;
- the tree `truncated` field and entry shape;
- raw blob media type;
- PR files `patch` omission for a large file;
- rate-limit headers present;
- a conditional 304 leaves `x-ratelimit-used` unchanged.

The estimator check also runs here: `ceil(bytes/3) ≥` the Anthropic token-counting API's count for every span in the benchmark corpus.

### 11.6 Benchmarks (manual before rollout, then nightly)

Pinned public repositories at fixed commits in three sizes:
- about 1k eligible files (this library),
- about 10k (a large TypeScript monorepo),
- about 50k.

Recorded per repository:
- tarball bytes;
- cold-build wall time and peak RSS;
- snapshot bytes compressed and uncompressed;
- per-file fact bytes;
- warm review p50 and p95 latency;
- API calls per review.

**Provisional targets, to confirm or revise after the first run:**
- Warm review p95 ≤ 10 s for ≤ 20k eligible files.
- Cold build ≤ 5 min for 50k files.
- Snapshot ≤ 50 MB compressed for 50k files.
- Worker memory limits are set from the measured peak RSS plus 50%.

## 12. Decisions, risks and acceptance

### 12.1 Decision log

| # | Decision | Rejected alternative and reason |
|---|---|---|
| D1 | Cache extracted symbols, never source | Cache blobs: violates the no-stored-source rule and costs far more storage |
| D2 | One immutable snapshot per (repo, extraction version, tree) in object storage, with a pointer row in Postgres | Snapshots in Postgres: bloat. Redis: volatile; a lost snapshot costs a full rebuild in rate budget |
| D3 | Cold build from one in-memory tarball, verified against blob SHAs | Per-blob fetch: thousands of calls. Partial clone: writes an object store to disk and fetches missing objects one at a time ([partial clone](https://git-scm.com/docs/partial-clone)). `git archive --remote`: unsupported on GitHub |
| D4 | Update on default-branch push, plus a staleness check at review time | Timer polling: wasted calls. Updating on PR events: leaks unmerged code into the shared snapshot |
| D5 | PR overlays are private, in memory, per job | Per-PR snapshots: storage and cleanup cost, no reuse benefit |
| D6 | Overlay compares the snapshot tree with the PR head tree, not compare/PR-files alone | Compare API caps at 300 files and misses stale-base drift |
| D7 | PR files API plus `parse_hunks` for the diff | `.diff` media type: fails outright on very large PRs; per-file handling degrades per file instead |
| D8 | PR-level ranking in the library (L4) | Per-file ranking merged in the service: duplicates scoring logic outside its owner |
| D9 | Model receives the diff separately; packing is candidates only (L5) | Changed hunks in context: duplicates the diff and fails large PRs on budget |
| D10 | Token estimator `ceil(bytes/3)` | Token-counting API per span: a network call per span. Model tokenizer offline: not published |
| D11 | Postgres job table with `SKIP LOCKED` | A broker: new infrastructure for modest volume |
| D12 | Snapshot and cache keys scoped per installation and repo | Global blob-SHA dedup: cross-tenant content-existence leak |
| D13 | Scoped mode for trees over 100k entries | Walking subtrees: one call per directory, unbounded |
| D14 | Context objects kept 24 h, encrypted | No persistence: review agents in other workflows couldn't share one context (CodePal's pattern) |
| D15 | Separate service repository | Service inside the library workspace: breaks the library's enforceable no-IO boundary |

### 12.2 Risks

| Risk | Mitigation |
|---|---|
| Snapshot size or index memory higher than assumed for large repos | Section 11.6 benchmarks gate rollout; per-worker memory sized from measurement; scoped mode as the ceiling |
| Tarballs for very large repos are slow or huge | 4 GiB cap and fallback to scoped mode; cold build runs once per repo and extraction version |
| Name-based ranking picks wrong same-named files (ambiguity) | Ambiguity is preserved and scored; spans carry reasons; exact resolution is out of scope |
| Root-only `.gitattributes` misses nested generated markers | Size cap and vendored path list; nested support later if skips show generated noise |
| Estimator under-counts some content | Nightly contract check against the token-counting API (section 11.5) |
| Cold build delays the first review of each repo | Only once per repo; the review falls back to scoped mode if the build exceeds its 15 min deadline |

### 12.3 Acceptance criteria for the whole effort

1. The library revision (L1–L5) passes `mise run gate`, with the extended public journey proving snapshot round-trip equivalence and multi-file ranking.
2. Service scenarios S1–S19 and the property test pass in CI with Testcontainers and the fake GitHub, whose blob and tree SHAs match real `git`.
3. The call counts for S1, S2 and S3 equal the section 8 formulas exactly.
4. S15 proves no source text in snapshots, Postgres, logs or metrics, and a 24 h lifecycle on contexts.
5. The contract suite passes against real GitHub at least once before rollout.
6. Benchmarks are recorded for all three repository sizes, the provisional targets are confirmed or revised in writing, and worker memory limits are set from the measurements.
7. A shadow run on internal repositories for one week produces contexts for every eligible PR, with zero `failed` review jobs other than `rate_limited`, before contexts feed any review agent.
