# bohmian-code-review

Given a pull request, `bohmian-code-review` picks the other files a reviewer needs to see and returns their relevant spans, packed under a token budget. It reads the repository through the GitHub API, keeps a source-free symbol index per default-branch commit, and writes results to storage you provide. It never clones, checks out, or persists source outside the published context.

## Why it exists

A diff on its own doesn't show the callers of the function it changes or the definitions it calls. Cloning every repository for every PR to find them is slow and expensive, and it puts source on disk you then have to secure. This library parses the default branch once with tree-sitter into a symbol-to-file index, then for each PR:

1. extracts the names declared and referenced on the changed lines,
2. resolves them against the index,
3. ranks candidate files by symbol rarity fused with co-change history (which files changed together in recent merged PRs),
4. packs the highest-ranked declarations until `PipelineLimits.pr.context_tokens` (64,000 by default) or `max_candidate_files` (15) is reached.

The output is a `ReviewContext`: selected spans with path, line range, score, and matched names, plus what was omitted and why. `context_tokens` bounds the context alone. The library doesn't know your prompt or how you'll send the diff, so size it for the context you want and budget the rest of the model request yourself.

## Quick start

The crates aren't published. Depend on the pipeline crate by Git:

```toml
[dependencies]
bohmian-code-review-pipeline = { git = "https://github.com/bohmian-ai/bohmian-code-review" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Rust 1.94+ (edition 2024). A C compiler is required at build time for the vendored tree-sitter grammars.

`ReviewPipeline` is the only entry point. It takes one object storage and one metadata store, both of which you implement ([Storage](#implement-objectstorage) and [Postgres](#implement-the-metadata-store-postgres) below):

```rust
use bohmian_code_review_pipeline::{
    GitHubClient, GitHubHost, Identity, InstallationId, InstallationToken, PipelineLimits,
    RepositoryIdentity, RequestLimits, ReviewPipeline, TenantId,
};
use object_store::aws::AmazonS3Builder;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = Blobs(Arc::new(
        AmazonS3Builder::from_env()
            .with_bucket_name("review-context")
            .build()?,
    ));
    let store = Pg(sqlx::PgPool::connect(&std::env::var("DATABASE_URL")?).await?);
    let pipeline = ReviewPipeline::new(&storage, &store, PipelineLimits::default());

    let identity = Identity {
        tenant: TenantId::new("acme")?,
        installation: InstallationId::new(12345)?,
        repository: RepositoryIdentity::new(987654, "acme", "billing")?,
    };
    let token = InstallationToken::new(&std::env::var("GITHUB_TOKEN")?)?;
    let client = GitHubClient::new(GitHubHost::public(), token, RequestLimits::default())?;

    let published = pipeline.context(&client, &identity, 42).await?;
    println!("context id: {}", published.record.id);
    for span in &published.context.spans {
        println!(
            "{}:{}-{}  score={:.3}  {:?}",
            span.path, span.start, span.end, span.score, span.names
        );
    }

    let again = pipeline.read(&identity, &published.record.id).await?;
    assert_eq!(again, published.context);
    Ok(())
}
```

`RepositoryIdentity::new` takes GitHub's numeric repository id (`gh api repos/<owner>/<name> --jq .id`). That id is the repository's identity. Every GitHub request addresses the repository as `/repositories/{id}`, and only the id is checked against GitHub's answer. Owner and name are labels, so a rename or transfer needs no action: a stale name still works, and the existing snapshot, history, and contexts carry over. For GitHub Enterprise Server, use `GitHubHost::enterprise("https://ghe.example.com/api/v3")`.

## The four calls

| Call | When | GitHub access | What it does |
|---|---|---|---|
| `context(&client, &identity, pr)` | PR opened or synchronized | yes | Cold-builds a snapshot if none exists, then overlays the PR head, ranks, packs, and publishes a `ReviewContext`. Returns `PrContextResult { record, lifetime, context }`. |
| `pinned(&client, &identity, &pins, baseline)` | A caller froze the PR's base and head, and may name the head it last reviewed | yes | Compares exactly the pinned commits: `full` runs merge base → head, `active` runs `baseline` → head (or equals `full` without one). Every changed side carries verified `Source::Text`, or `Binary`, `Unavailable` or `Excluded`, with the provider's hunks. Context is selected and published for the active changes. Returns `PinnedReview { pull_request, full, active, context }`. The live PR supplies metadata only, so a PR that moved is still reviewed at the pins. |
| `refresh(&client, &identity)` | Push to the default branch | yes | Advances the snapshot to the branch head incrementally and extends co-change history. `context` never refreshes on its own. |
| `read(&identity, &id)` | Downstream consumer (e.g. a review agent) | no | Reads a published context back by `ContextId`. Fails after `CONTEXT_LIFETIME` (24 h). |

Every call is an `async fn` on tokio. To cancel or time out a call, drop its future (for example with `tokio::time::timeout`). Every durable step is idempotent, so a retry after a failure or a dropped future converges.

`GitHubClient` is a per-call argument because installation tokens are short-lived. The pipeline, storage, and store can live in app state.

## Implement `ObjectStorage`

The pipeline writes two kinds of immutable object: snapshots (source-free symbol facts) and contexts (the selected source spans). Every key comes from the library already scoped as `tenants/{tenant}/installations/{id}/repositories/{id}/...`, so your implementation never builds or validates paths. The library checks integrity with a SHA-256 digest, so the backend doesn't need to.

| Method | Contract |
|---|---|
| `create(key, bytes)` | Create-only. Return `AlreadyExists` if the key exists. A failed write must not leave a partial object readable. |
| `read(key)` | Exact key. Return `NotFound` if absent. |
| `delete(key)` | Deleting an absent key succeeds. |

Using the [`object_store`](https://docs.rs/object_store) crate (0.13) covers S3, GCS, Azure, and local disk in one adapter:

```rust
use bohmian_code_review_pipeline::{ObjectKey, ObjectStorage, StorageError};
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutMode, PutOptions};
use std::sync::Arc;

/// Any `object_store` backend (S3, GCS, Azure, local) as pipeline storage.
pub struct Blobs(pub Arc<dyn ObjectStore>);

fn backend(error: object_store::Error) -> StorageError {
    StorageError::Backend {
        detail: error.to_string(),
    }
}

impl ObjectStorage for Blobs {
    async fn create(&self, key: &ObjectKey, bytes: &[u8]) -> Result<(), StorageError> {
        let options = PutOptions {
            mode: PutMode::Create,
            ..PutOptions::default()
        };
        match self
            .0
            .put_opts(&Path::from(key.as_str()), bytes.to_vec().into(), options)
            .await
        {
            Ok(_) => Ok(()),
            Err(object_store::Error::AlreadyExists { .. }) => {
                Err(StorageError::AlreadyExists { key: key.clone() })
            }
            Err(error) => Err(backend(error)),
        }
    }

    async fn read(&self, key: &ObjectKey) -> Result<Vec<u8>, StorageError> {
        match self.0.get(&Path::from(key.as_str())).await {
            Ok(object) => Ok(object.bytes().await.map_err(backend)?.to_vec()),
            Err(object_store::Error::NotFound { .. }) => {
                Err(StorageError::NotFound { key: key.clone() })
            }
            Err(error) => Err(backend(error)),
        }
    }

    async fn delete(&self, key: &ObjectKey) -> Result<(), StorageError> {
        match self.0.delete(&Path::from(key.as_str())).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(error) => Err(backend(error)),
        }
    }
}
```

`PutMode::Create` needs conditional-put support. On S3 that works by default in object_store 0.13. For S3-compatible stores that lack it, check `AmazonS3Builder::with_conditional_put`.

## Implement the metadata store (Postgres)

`ReviewPipeline` needs one type that implements both `SnapshotStore` and `ContextStore`. They're separate traits because snapshots and contexts have different lifecycles, but one database-backed struct usually serves both. The traits are async (`async fn` in the impl works) and not dyn-compatible, so the pipeline takes them as generic parameters.

Three rules apply to every method:

- **Scope by identity.** Every method receives the caller's `Identity` and must never read or write another identity's rows. Two identities are the same when tenant, installation, and numeric repository id match. Owner and name don't count.
- **Use the database clock.** `touched_at`, `published_at`, and `expires_at` come from the store's `now()`, never the host clock. That's why `ContextRecord` carries no timestamps.
- **No transaction across IO.** The library never asks you to hold one open across a GitHub or storage call. Each method is one statement below.

### Schema

```sql
CREATE TABLE bcr_snapshot_pointers (
    tenant             text   NOT NULL,
    installation       bigint NOT NULL,
    repository         bigint NOT NULL,
    extraction_version text   NOT NULL,
    format_version     bigint NOT NULL,
    max_file_bytes     bigint NOT NULL,
    pointer            jsonb  NOT NULL,
    tree               text   NOT NULL,
    PRIMARY KEY (tenant, installation, repository,
                 extraction_version, format_version, max_file_bytes)
);

CREATE TABLE bcr_snapshot_records (
    tenant       text        NOT NULL,
    installation bigint      NOT NULL,
    repository   bigint      NOT NULL,
    key          text        NOT NULL,
    touched_at   timestamptz NOT NULL,
    claimed      boolean     NOT NULL DEFAULT false,
    PRIMARY KEY (tenant, installation, repository, key)
);

CREATE TABLE bcr_contexts (
    tenant       text        NOT NULL,
    installation bigint      NOT NULL,
    repository   bigint      NOT NULL,
    id           text        NOT NULL,
    identity     jsonb       NOT NULL,
    record       jsonb       NOT NULL,
    published_at timestamptz NOT NULL,
    expires_at   timestamptz NOT NULL,
    PRIMARY KEY (tenant, installation, repository, id)
);
CREATE INDEX ON bcr_contexts (expires_at);
```

Rows are keyed on `(tenant, installation, repository id)`. Don't key on the serialized `Identity`: it includes the repository's owner and name, which change on a rename or transfer, and the library treats two identities with the same ids as the same repository. `SnapshotPointer` and `ContextRecord` are `serde` types, so they're stored whole as `jsonb`, and `bcr_contexts.identity` is kept only so `expired_contexts` can return it.

### What each method must do

| Method | Contract | Postgres shape |
|---|---|---|
| `snapshot_pointer(scope)` | Current pointer or `None` | `SELECT` by scope |
| `publish_snapshot_pointer(scope, expected, pointer)` | Compare-and-swap on the current pointer's tree. `true` if it moved, `false` if another build won. `expected: None` means "no pointer yet". | `INSERT ... ON CONFLICT DO NOTHING` or `UPDATE ... WHERE tree = $expected`, then check the affected row count |
| `upsert_record(identity, key)` | Insert or re-touch at store time. Return `Collecting` and change nothing if the record is claimed. | `INSERT ... ON CONFLICT DO UPDATE ... WHERE NOT claimed` |
| `unreferenced(identity, grace)` | Atomically claim and return records that no pointer names and that weren't touched within `grace`. Claimed records are returned again on the next call. | One `UPDATE ... RETURNING` |
| `delete_record` / `remove_pointers` | Idempotent deletes | `DELETE` |
| `upsert_context(identity, record)` | Stamp `published_at = now()`, `expires_at = now() + CONTEXT_LIFETIME`. A repeat upsert keeps the original stamps. | `INSERT ... ON CONFLICT DO UPDATE` (no-op) `RETURNING` |
| `context(identity, id)` | `Absent`, `Expired` (judged on store time), or `Live(record)` | `SELECT record, expires_at <= now()` |
| `expired_contexts(limit)` | Up to `limit` expired records across all identities | `SELECT ... WHERE expires_at <= now() LIMIT` |
| `delete_context` | Idempotent delete | `DELETE` |
| `remove_repository(identity)` | Delete all of an identity's contexts and return their object keys | `DELETE ... RETURNING` |

<details>
<summary>Full <code>sqlx</code> implementation (sqlx 0.9, features <code>postgres</code>, <code>runtime-tokio</code>, <code>chrono</code>, <code>json</code>)</summary>

```rust
use bohmian_code_review_pipeline::persistence::{ContextLifetime, ContextLookup};
use bohmian_code_review_pipeline::{
    CONTEXT_LIFETIME, ContextId, ContextRecord, ContextStore, Identity, ObjectKey, ObjectSha,
    PersistenceError, SnapshotPointer, SnapshotScope, SnapshotStore,
};
use sqlx::PgPool;
use sqlx::types::Json;
use sqlx::types::chrono::{DateTime, Utc};
use std::time::Duration;

/// One Postgres-backed store serving snapshots and contexts.
pub struct Pg(pub PgPool);

fn backend(error: sqlx::Error) -> PersistenceError {
    PersistenceError::Backend {
        detail: error.to_string(),
    }
}

fn bigint(n: u64) -> Result<i64, PersistenceError> {
    i64::try_from(n).map_err(|_| PersistenceError::Backend {
        detail: format!("{n} exceeds bigint"),
    })
}

/// Row key of an identity: tenant, installation id, and the stable
/// repository id. Owner and name are left out so a rename keeps its rows.
fn ids(identity: &Identity) -> Result<(&str, i64, i64), PersistenceError> {
    Ok((
        identity.tenant.as_str(),
        bigint(identity.installation.get())?,
        bigint(identity.repository.id())?,
    ))
}

/// `ObjectKey` has no public parser; serde re-validates the stored text.
fn key(text: String) -> Result<ObjectKey, PersistenceError> {
    serde_json::from_value(text.into()).map_err(|error| PersistenceError::Backend {
        detail: error.to_string(),
    })
}

impl SnapshotStore for Pg {
    async fn snapshot_pointer(
        &self,
        scope: &SnapshotScope,
    ) -> Result<Option<SnapshotPointer>, PersistenceError> {
        let (tenant, installation, repository) = ids(&scope.identity)?;
        let row: Option<(Json<SnapshotPointer>,)> = sqlx::query_as(
            "SELECT pointer FROM bcr_snapshot_pointers
             WHERE tenant = $1 AND installation = $2 AND repository = $3
               AND extraction_version = $4 AND format_version = $5 AND max_file_bytes = $6",
        )
        .bind(tenant)
        .bind(installation)
        .bind(repository)
        .bind(&scope.extraction_version)
        .bind(i64::from(scope.format_version))
        .bind(bigint(scope.max_file_bytes)?)
        .fetch_optional(&self.0)
        .await
        .map_err(backend)?;
        Ok(row.map(|(Json(pointer),)| pointer))
    }

    async fn publish_snapshot_pointer(
        &self,
        scope: &SnapshotScope,
        expected: Option<&ObjectSha>,
        pointer: &SnapshotPointer,
    ) -> Result<bool, PersistenceError> {
        let (tenant, installation, repository) = ids(&scope.identity)?;
        let sql = match expected {
            None => {
                "INSERT INTO bcr_snapshot_pointers (tenant, installation, repository,
                   extraction_version, format_version, max_file_bytes, pointer, tree)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT DO NOTHING"
            }
            Some(_) => {
                "UPDATE bcr_snapshot_pointers SET pointer = $7, tree = $8
                 WHERE tenant = $1 AND installation = $2 AND repository = $3
                   AND extraction_version = $4 AND format_version = $5
                   AND max_file_bytes = $6 AND tree = $9"
            }
        };
        let moved = sqlx::query(sql)
            .bind(tenant)
            .bind(installation)
            .bind(repository)
            .bind(&scope.extraction_version)
            .bind(i64::from(scope.format_version))
            .bind(bigint(scope.max_file_bytes)?)
            .bind(Json(pointer))
            .bind(pointer.tree.as_str())
            .bind(expected.map(ObjectSha::as_str))
            .execute(&self.0)
            .await
            .map_err(backend)?;
        Ok(moved.rows_affected() == 1)
    }

    async fn upsert_record(
        &self,
        identity: &Identity,
        key: &ObjectKey,
    ) -> Result<(), PersistenceError> {
        let (tenant, installation, repository) = ids(identity)?;
        let touched = sqlx::query(
            "INSERT INTO bcr_snapshot_records (tenant, installation, repository, key, touched_at)
             VALUES ($1, $2, $3, $4, now())
             ON CONFLICT (tenant, installation, repository, key) DO UPDATE SET touched_at = now()
             WHERE NOT bcr_snapshot_records.claimed",
        )
        .bind(tenant)
        .bind(installation)
        .bind(repository)
        .bind(key.as_str())
        .execute(&self.0)
        .await
        .map_err(backend)?;
        if touched.rows_affected() == 0 {
            return Err(PersistenceError::Collecting { key: key.clone() });
        }
        Ok(())
    }

    async fn unreferenced(
        &self,
        identity: &Identity,
        grace: Duration,
    ) -> Result<Vec<ObjectKey>, PersistenceError> {
        let (tenant, installation, repository) = ids(identity)?;
        let rows: Vec<(String,)> = sqlx::query_as(
            "UPDATE bcr_snapshot_records r SET claimed = true
             WHERE r.tenant = $1 AND r.installation = $2 AND r.repository = $3
               AND r.touched_at <= now() - make_interval(secs => $4)
               AND NOT EXISTS (
                 SELECT 1 FROM bcr_snapshot_pointers p
                 WHERE p.tenant = r.tenant AND p.installation = r.installation
                   AND p.repository = r.repository AND p.pointer->>'key' = r.key)
             RETURNING r.key",
        )
        .bind(tenant)
        .bind(installation)
        .bind(repository)
        .bind(grace.as_secs_f64())
        .fetch_all(&self.0)
        .await
        .map_err(backend)?;
        rows.into_iter().map(|(text,)| key(text)).collect()
    }

    async fn delete_record(
        &self,
        identity: &Identity,
        key: &ObjectKey,
    ) -> Result<(), PersistenceError> {
        let (tenant, installation, repository) = ids(identity)?;
        sqlx::query(
            "DELETE FROM bcr_snapshot_records
             WHERE tenant = $1 AND installation = $2 AND repository = $3 AND key = $4",
        )
        .bind(tenant)
        .bind(installation)
        .bind(repository)
        .bind(key.as_str())
        .execute(&self.0)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn remove_pointers(&self, identity: &Identity) -> Result<(), PersistenceError> {
        let (tenant, installation, repository) = ids(identity)?;
        sqlx::query(
            "DELETE FROM bcr_snapshot_pointers
             WHERE tenant = $1 AND installation = $2 AND repository = $3",
        )
        .bind(tenant)
        .bind(installation)
        .bind(repository)
        .execute(&self.0)
        .await
        .map_err(backend)?;
        Ok(())
    }
}

impl ContextStore for Pg {
    async fn upsert_context(
        &self,
        identity: &Identity,
        record: &ContextRecord,
    ) -> Result<ContextLifetime, PersistenceError> {
        let (tenant, installation, repository) = ids(identity)?;
        // The no-op DO UPDATE returns an existing row's original stamps.
        let (published_at, expires_at): (DateTime<Utc>, DateTime<Utc>) = sqlx::query_as(
            "INSERT INTO bcr_contexts
               (tenant, installation, repository, id, identity, record, published_at, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, now(), now() + make_interval(secs => $7))
             ON CONFLICT (tenant, installation, repository, id) DO UPDATE SET id = EXCLUDED.id
             RETURNING published_at, expires_at",
        )
        .bind(tenant)
        .bind(installation)
        .bind(repository)
        .bind(record.id.as_str())
        .bind(Json(identity))
        .bind(Json(record))
        .bind(CONTEXT_LIFETIME.as_secs_f64())
        .fetch_one(&self.0)
        .await
        .map_err(backend)?;
        Ok(ContextLifetime {
            published_at: published_at.into(),
            expires_at: expires_at.into(),
        })
    }

    async fn context(
        &self,
        identity: &Identity,
        id: &ContextId,
    ) -> Result<ContextLookup, PersistenceError> {
        let (tenant, installation, repository) = ids(identity)?;
        let row: Option<(Json<ContextRecord>, bool)> = sqlx::query_as(
            "SELECT record, expires_at <= now() FROM bcr_contexts
             WHERE tenant = $1 AND installation = $2 AND repository = $3 AND id = $4",
        )
        .bind(tenant)
        .bind(installation)
        .bind(repository)
        .bind(id.as_str())
        .fetch_optional(&self.0)
        .await
        .map_err(backend)?;
        Ok(match row {
            None => ContextLookup::Absent,
            Some((_, true)) => ContextLookup::Expired,
            Some((Json(record), false)) => ContextLookup::Live(record),
        })
    }

    async fn expired_contexts(
        &self,
        limit: usize,
    ) -> Result<Vec<(Identity, ContextRecord)>, PersistenceError> {
        let rows: Vec<(Json<Identity>, Json<ContextRecord>)> = sqlx::query_as(
            "SELECT identity, record FROM bcr_contexts WHERE expires_at <= now() LIMIT $1",
        )
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.0)
        .await
        .map_err(backend)?;
        Ok(rows
            .into_iter()
            .map(|(Json(identity), Json(record))| (identity, record))
            .collect())
    }

    async fn delete_context(
        &self,
        identity: &Identity,
        id: &ContextId,
    ) -> Result<(), PersistenceError> {
        let (tenant, installation, repository) = ids(identity)?;
        sqlx::query(
            "DELETE FROM bcr_contexts
             WHERE tenant = $1 AND installation = $2 AND repository = $3 AND id = $4",
        )
        .bind(tenant)
        .bind(installation)
        .bind(repository)
        .bind(id.as_str())
        .execute(&self.0)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn remove_repository(
        &self,
        identity: &Identity,
    ) -> Result<Vec<ObjectKey>, PersistenceError> {
        let (tenant, installation, repository) = ids(identity)?;
        let rows: Vec<(String,)> = sqlx::query_as(
            "DELETE FROM bcr_contexts
             WHERE tenant = $1 AND installation = $2 AND repository = $3
             RETURNING record->>'key'",
        )
        .bind(tenant)
        .bind(installation)
        .bind(repository)
        .fetch_all(&self.0)
        .await
        .map_err(backend)?;
        rows.into_iter().map(|(text,)| key(text)).collect()
    }
}
```

</details>

This implementation and the storage adapter above are compiled and tested against Postgres 14 and `object_store::memory::InMemory`. They aren't part of this repository's gate, because the library has no database or storage dependency by design.

## Operations

Nothing in the library runs in the background. Schedule these yourself, for example from a cron job:

| Task | Call | Why |
|---|---|---|
| Expire contexts | `Contexts::new(&storage, &store).collect_expired(limit)` | Deletes context objects and records past `CONTEXT_LIFETIME`. Anything it couldn't delete comes back in `lagging` for the next sweep. |
| Collect snapshots | `Snapshots::new(&storage, &store).collect(&identity, grace)` | Deletes snapshot objects that no pointer names anymore. Use a `grace` longer than your slowest build. Don't overlap calls for one identity. |
| Offboard a repository | `Snapshots::remove_repository` plus `ContextStore::remove_repository` | Drops pointers and records, and returns keys to delete. Run it only when no build for that identity is in flight. |

## Errors and retries

`PipelineError` wraps the failing stage's error as `Snapshot(ColdBuildError)`, `Context(ContextError)` or `Comparison(ComparisonError)`, so you can match on it. `ComparisonError::BaselineUnusable { problem }` means the supplied baseline was rewritten out of the head's history or can't be found; the pipeline never substitutes another baseline, so call again without one for a full review. Some variants mean "a concurrent change won, retry the call":

| Error | Meaning |
|---|---|
| `ColdBuildError::PointerConflict` | Another build moved the snapshot pointer first |
| `PersistenceError::Collecting` | A snapshot collection claimed the record this build wanted |
| `HistoryError::ListingMoved` | The closed-PR listing changed mid-read |
| `ContextError::PullRequestMoved` | The PR's base or head moved during the call |
| `GitHubError::*` | Call `.disposition()`. `Retry { after }` means back off (rate limit or outage). `Terminal` means retrying with the same token won't help. |

`StorageError::Backend` and `PersistenceError::Backend` carry your adapter's `detail` string. Keep credentials and source out of it.

## Guarantees and limits

- **Coverage is reported, not assumed.** `context.coverage.is_full()` is `true` only when ranking used a complete repository snapshot. If a cold build is incomplete (bounds hit, archive too large), the PR still gets context from a scoped index, and `coverage` says so.
- **Ambiguous matches stay ambiguous.** Resolution is by exact, case-sensitive name, with no type inference. A span's `score` only orders spans within one context. Don't compare it across contexts.
- **Eligible files:** supported language by path; not a symlink, submodule, vendored or build directory, `.min.js`/`.min.css`, or `linguist-generated`/`linguist-vendored`; at most `PipelineLimits::max_file_bytes` (1 MiB by default). Supported languages are listed in [`language.rs`](crates/bohmian-code-review-core/src/language.rs).
- **Deletion-only changes don't drive ranking.** A changed file whose patch adds no head-side lines is listed in `context.not_analyzed` with `NotAnalyzedReason::DeletionOnly`. Removed files are listed there too.
- **Pinned comparisons fail rather than guess.** A comparison listing 300 or more files (the compare API's ceiling), a truncated tree, a rejected path, or a text change without a patch fails `pinned` with a `ComparisonError`. Context-budget omissions are not failures; they stay in `context.omissions`.
- **Source never leaves in a snapshot.** Snapshots hold declarations and reference names only. The only object carrying source is the `ReviewContext`, and it expires after 24 hours.

## How ranking works

The default ranker is `Fused(RaritySymbol, CoChange)`, using reciprocal-rank fusion with `k = 60`.

| Ranker | Scores a candidate file by |
|---|---|
| `RaritySymbol` | Names shared with the changed lines, weighted by `1/document-frequency`. Names that appear in more than 100 files are ignored. |
| `CoChange` | How often it changed together with each changed file across the snapshot's merged-PR history (up to 1,000 PRs; PRs touching more than 30 files are dropped). Only PRs merged before the PR's base count. |

With no history, the fused ranking is identical to `RaritySymbol`, so a new repository loses nothing. The full scoring contract is in the [design doc](architecture/bohmian-code-review-design.md#public-behavior).

## Benchmark

The default ranker's hit@10, meaning how often a file the PR author actually changed ranks in the top 10:

| Repository | Language | hit@10 | vs. symbols only (`RaritySymbol`) |
|---|---|---|---|
| pydantic/pydantic-ai | Python | 60.3% | +14.1 pts (95% CI +4.5 to +24.2) |
| tokio-rs/tokio | Rust | 38.6% | +17.5 pts (95% CI +9.5 to +26.7) |
| colinhacks/zod | TypeScript | 61.8% | +32.3 pts (95% CI +22.8 to +42.0) |

**Setup.** For each PR, run 5 trials. Each trial hides one file the PR modified, picked at random, and ranks the repository from the PR's other changes. A trial is a hit if the hidden file ranks in the top 10. A PR with fewer than 5 modified files gets one trial per file. A PR's score is its mean over its trials, and a repository's score is the mean over its PRs.

- **PRs:** the 50 most recent merged PRs per repository that change 2 to 15 source files, merged before 2026-08-23 (pydantic-ai), 2026-05-21 (tokio), and 2026-09-01 (zod). That gives 157, 143, and 165 trials.
- **No leakage:** each trial ranks a snapshot of the PR's base commit and uses co-change history only from PRs merged before it.
- **Comparison:** the last column is the default ranker minus `RaritySymbol` alone. The interval is a 95% paired bootstrap over PRs.

**Limits.** 50 PRs per repository only detects large effects, and one repository per language can't separate language from codebase. "The author changed it" stands in for "the reviewer needed it", and that label favours the co-change half of the ranker. The benchmark measures ranking only, not packing.

Reproduce with `GITHUB_TOKEN=<read-only token> mise run live:context-relevance`, which is read-only and slow. Full reports are in [`changes/completed/2026/cochange-eval-reports/`](changes/completed/2026/cochange-eval-reports/).

## Development

Tasks run through [mise](https://mise.jdx.dev):

| Task | Runs |
|---|---|
| `mise run gate` | Everything CI runs: fmt, clippy, type-check, every test lane, skill-mirror sync |
| `mise run test:pipeline` | Pipeline unit and doc tests |
| `mise run test:pipeline:journey` | `ReviewPipeline` end to end over a fake GitHub and in-memory stores |
| `mise run live:pr` | A real PR against GitHub (`GITHUB_TOKEN`, `BCR_REPO_ID`; optional `BCR_REPO`, `BCR_PR`). Read-only. Never part of the gate. |

For in-memory `ObjectStorage` and store implementations in your own tests, enable the `testing` feature and use `bohmian_code_review_pipeline::testing::{MemoryStorage, MemoryStore}`.

Contributor rules are in [`AGENTS.md`](AGENTS.md). Architecture authority is [`architecture/bohmian-code-review-design.md`](architecture/bohmian-code-review-design.md).

## License

[MIT](LICENSE). Ported code and vendored grammars are listed in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
