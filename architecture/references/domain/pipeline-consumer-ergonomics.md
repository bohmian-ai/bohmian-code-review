# Pipeline consumer ergonomics

Target consumer shape for `bohmian-code-review-pipeline`, written from the point of view of Wyrd importing it with its own Postgres and OpenDAL. Review pipeline API changes against this: if a change makes the consumer code below longer or adds a concept the consumer must learn, it needs a reason.

`ReviewPipeline`, `PipelineLimits`, and `PipelineError` are built. Everything else names existing types.

## Seams

| Seam | bcr type | Consumer provides | Lifetime |
|---|---|---|---|
| GitHub access | `GitHubClient` (concrete; only `HttpTransport` is `dyn`) | Host + installation token | Per event (tokens expire in one hour) |
| Blob storage | `ObjectStorage` | OpenDAL `Operator` | Long-lived, app state |
| Snapshot metadata | `SnapshotStore` | Postgres | Long-lived, app state |
| Context metadata | `ContextStore` | Postgres, same type as `SnapshotStore` | Long-lived, app state |
| Caller | `Identity { tenant, installation, repository }` | Verified tenant, install, repo | Every call |

The store traits return `impl Future`, so they are generic parameters rather than `dyn` objects. Consumers implement them on concrete types. Durable time comes from the store's clock (`now()` in SQL), never the host clock.

## Consumer adapters

```rust
// wyrd/src/review_context/opendal_storage.rs
use bohmian_code_review_pipeline::{ObjectKey, ObjectStorage, StorageError};
use opendal::{ErrorKind, Operator};

/// Snapshot and context objects in Wyrd's bucket, keyed by bcr's ObjectKey.
pub struct OpendalStorage {
    op: Operator,
}

impl ObjectStorage for OpendalStorage {
    async fn create(&self, key: &ObjectKey, bytes: &[u8]) -> Result<(), StorageError> {
        // bcr objects are immutable: create-if-absent, never overwrite.
        match self.op.write_with(&key.to_string(), bytes.to_vec()).if_not_exists(true).await {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == ErrorKind::ConditionNotMatch => {
                Err(StorageError::AlreadyExists { key: key.clone() })
            }
            Err(e) => Err(StorageError::Backend { detail: e.to_string() }),
        }
    }

    async fn read(&self, key: &ObjectKey) -> Result<Vec<u8>, StorageError> {
        match self.op.read(&key.to_string()).await {
            Ok(buf) => Ok(buf.to_vec()),
            Err(e) if e.kind() == ErrorKind::NotFound => Err(StorageError::NotFound { key: key.clone() }),
            Err(e) => Err(StorageError::Backend { detail: e.to_string() }),
        }
    }

    async fn delete(&self, key: &ObjectKey) -> Result<(), StorageError> {
        self.op.delete(&key.to_string()).await
            .map_err(|e| StorageError::Backend { detail: e.to_string() })
    }
}
```

```rust
// wyrd/src/review_context/pg_store.rs
use bohmian_code_review_pipeline::{
    ContextId, ContextLifetime, ContextLookup, ContextRecord, ContextStore, Identity, ObjectSha,
    PersistenceError, SnapshotPointer, SnapshotScope, SnapshotStore,
};
use sqlx::PgPool;

/// One Postgres-backed type implements both stores.
pub struct PgReviewStore {
    pool: PgPool,
}

impl SnapshotStore for PgReviewStore {
    async fn snapshot_pointer(&self, scope: &SnapshotScope)
        -> Result<Option<SnapshotPointer>, PersistenceError> {
        // SELECT tree, commit, key, digest FROM bcr_snapshot_pointer
        //  WHERE tenant=$1 AND repo_id=$2 AND extraction_version=$3 AND format_version=$4 ...
        todo!()
    }

    async fn publish_snapshot_pointer(&self, scope: &SnapshotScope,
        expected: Option<&ObjectSha>, pointer: &SnapshotPointer)
        -> Result<bool, PersistenceError> {
        // Compare-and-swap: UPDATE ... WHERE commit IS NOT DISTINCT FROM $expected
        // Returns false when another writer moved it first (bcr retries).
        todo!()
    }

    // upsert_record / unreferenced / delete_record / remove_pointers:
    // object bookkeeping for garbage collection; timestamps come from now() in SQL.
}

impl ContextStore for PgReviewStore {
    async fn upsert_context(&self, identity: &Identity, record: &ContextRecord)
        -> Result<ContextLifetime, PersistenceError> {
        // INSERT ... RETURNING published_at, expires_at  -- DB clock, never host clock
        todo!()
    }

    async fn context(&self, identity: &Identity, id: &ContextId)
        -> Result<ContextLookup, PersistenceError> {
        // Absent / Expired / Live(record), scoped by identity (tenant isolation)
        todo!()
    }

    // expired_contexts / delete_context / remove_repository: cleanup
}
```

The SQL is illustrative; the consumer owns its schema.

## Today: stage facades

Three entry points, three constructors, two limit types, and a five-argument `PrContext::new` that separates snapshot and context storage no consumer separates.

```rust
// wyrd/src/review_context/mod.rs
use bohmian_code_review_pipeline::{
    ColdBuildLimits, ContextId, Contexts, GitHubClient, GitHubHost, Identity, InstallationToken,
    PrContext, PrContextLimits, RequestLimits, ReviewContext, Update,
};

pub struct ReviewContextService {
    storage: OpendalStorage, // long-lived, in AppState
    store: PgReviewStore,    // long-lived, in AppState
}

impl ReviewContextService {
    /// Push to default branch: cold build or incremental snapshot + co-change history.
    pub async fn on_push(&self, identity: &Identity, token: InstallationToken) -> anyhow::Result<()> {
        let client = GitHubClient::new(GitHubHost::github_com(), token, RequestLimits::default())?;
        Update::new(&client, &self.storage, &self.store, ColdBuildLimits::default())
            .run(identity)
            .await?;
        Ok(())
    }

    /// PR opened/synchronized: diff symbols → symbol map → F ranking → packed context.
    pub async fn on_pull_request(&self, identity: &Identity, token: InstallationToken, pr: u64)
        -> anyhow::Result<ReviewContext> {
        let client = GitHubClient::new(GitHubHost::github_com(), token, RequestLimits::default())?;
        let result = PrContext::new(
            &client,
            &self.storage, &self.store,   // snapshots
            &self.storage, &self.store,   // contexts
            PrContextLimits::default(),
        )
        .run(identity, pr)
        .await?;
        Ok(result.context)                // also published; result.record.id for read-back
    }

    /// Review agents fetch the published context by id.
    pub async fn read(&self, identity: &Identity, id: &ContextId) -> anyhow::Result<ReviewContext> {
        Ok(Contexts::new(&self.storage, &self.store).read(identity, id).await?)
    }
}
```

## Target: `ReviewPipeline`

```rust
// in bcr: bohmian-code-review-pipeline
pub struct ReviewPipeline<'a, O, S> {
    storage: &'a O,
    store: &'a S,              // S: SnapshotStore + ContextStore
    limits: PipelineLimits,    // ColdBuildLimits + PrContextLimits, Default
}

impl<'a, O: ObjectStorage, S: SnapshotStore + ContextStore> ReviewPipeline<'a, O, S> {
    pub fn new(storage: &'a O, store: &'a S, limits: PipelineLimits) -> Self;

    /// Cold-builds if no snapshot exists, then ranks with F and publishes.
    pub async fn context(&self, client: &GitHubClient, identity: &Identity, pr: u64)
        -> Result<PrContextResult, PipelineError>;

    /// Incremental snapshot + co-change history refresh.
    pub async fn refresh(&self, client: &GitHubClient, identity: &Identity)
        -> Result<ColdBuildResult, PipelineError>;

    /// Read-back by id; no GitHub access needed.
    pub async fn read(&self, identity: &Identity, id: &ContextId)
        -> Result<ReviewContext, PipelineError>;
}
```

```rust
// Wyrd, with the wrapper
let pipeline = ReviewPipeline::new(&state.storage, &state.store, PipelineLimits::default());

let client = GitHubClient::new(host, token, RequestLimits::default())?; // per event
let ctx = pipeline.context(&client, &identity, pr).await?.context;
```

## Review checklist

- A consumer needs one type (`ReviewPipeline`), one limits type, and one error type for the common path.
- `context` never blocks on a full refresh when a snapshot exists; it cold-builds only when none exists. Refresh belongs to push events.
- `client` is a per-call argument, never a field: installation tokens are short-lived while the pipeline and stores live in app state.
- One storage and one store serve snapshots and contexts.
- Stage facades (`ColdBuild`, `Update`, `Snapshots`, `PrContext`, `Contexts`) stay public for advanced callers; docs and README lead with `ReviewPipeline`.
- No background loops, no builder, no algorithm-selection config: the default ranker is fixed.
