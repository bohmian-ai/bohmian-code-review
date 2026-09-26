//! Injected metadata persistence: snapshot pointers and context records.
//!
//! The library publishes metadata only after the object it names exists and
//! validates, and every method carries the full [`Identity`] so an
//! implementation can bind each row to its tenant and installation. No raw
//! database handle crosses this boundary; a consumer supplies the
//! implementation and owns its own connection scoping.

use crate::identity::{Identity, ObjectSha};
use crate::storage::{ObjectDigest, ObjectKey};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use thiserror::Error;

/// Why a persistence operation failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PersistenceError {
    /// A context id was already published for this identity.
    #[error("context already published: {id}")]
    AlreadyPublished {
        /// The duplicate id.
        id: ContextId,
    },
    /// The backend failed; `detail` must not contain credentials or source.
    #[error("persistence backend failed: {detail}")]
    Backend {
        /// Backend-specific description.
        detail: String,
    },
}

/// The versioned scope one default-branch pointer belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotScope {
    /// Caller identity.
    pub identity: Identity,
    /// Extraction version the facts were produced under.
    pub extraction_version: String,
    /// Snapshot object format version.
    pub format_version: u32,
}

/// The current default-branch snapshot for one [`SnapshotScope`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotPointer {
    /// Tree the snapshot indexes.
    pub tree: ObjectSha,
    /// Branch commit the tree was read from.
    pub commit: ObjectSha,
    /// Exact object key of the validated snapshot.
    pub key: ObjectKey,
    /// Digest of the snapshot bytes.
    pub digest: ObjectDigest,
    /// When the pointer was published.
    pub published_at: SystemTime,
}

/// Opaque identifier a caller hands to downstream workflows.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContextId(pub String);

impl ContextId {
    /// Identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ContextId {
    /// Writes the identifier text.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Source-free metadata binding a [`ContextId`] to one context object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextRecord {
    /// Published identifier.
    pub id: ContextId,
    /// Exact object key holding the selected spans.
    pub key: ObjectKey,
    /// Digest of the context bytes.
    pub digest: ObjectDigest,
    /// Pull request number.
    pub pull_request: u64,
    /// Base commit the context was built against.
    pub base: ObjectSha,
    /// Head commit the context was built against.
    pub head: ObjectSha,
    /// When the record became visible.
    pub published_at: SystemTime,
    /// Reads at or after this instant are refused even if deletion lags.
    pub expires_at: SystemTime,
}

/// Metadata persistence a consumer implements.
///
/// Every method receives the caller identity and must refuse to read or
/// write another identity's rows. Implementations must not hold a
/// transaction open across a call into GitHub or object storage; the library
/// never asks them to.
pub trait Persistence: Send + Sync {
    /// Current pointer for `scope`, if one was published.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn snapshot_pointer(
        &self,
        scope: &SnapshotScope,
    ) -> Result<Option<SnapshotPointer>, PersistenceError>;

    /// Publish `pointer` only when the current pointer's tree equals `expected`.
    ///
    /// Returns `true` when the pointer moved and `false` when another
    /// publication won, so concurrent updates converge without a lock held
    /// across IO.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn publish_snapshot_pointer(
        &self,
        scope: &SnapshotScope,
        expected: Option<&ObjectSha>,
        pointer: &SnapshotPointer,
    ) -> Result<bool, PersistenceError>;

    /// Make `record` readable for `identity`.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::AlreadyPublished`] when the id exists for
    /// this identity and [`PersistenceError::Backend`] otherwise.
    fn publish_context(
        &self,
        identity: &Identity,
        record: &ContextRecord,
    ) -> Result<(), PersistenceError>;

    /// Exact lookup of `id` under `identity`; another identity's id is absent.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn context(
        &self,
        identity: &Identity,
        id: &ContextId,
    ) -> Result<Option<ContextRecord>, PersistenceError>;

    /// Up to `limit` records whose `expires_at` is at or before `now`, across identities.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn expired_contexts(
        &self,
        now: SystemTime,
        limit: usize,
    ) -> Result<Vec<(Identity, ContextRecord)>, PersistenceError>;

    /// Remove `id` under `identity`; removing an absent id succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] when the row may still exist.
    fn delete_context(&self, identity: &Identity, id: &ContextId) -> Result<(), PersistenceError>;

    /// Remove every pointer and context record for `identity`, returning the
    /// object keys the caller must delete from storage.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn remove_repository(&self, identity: &Identity) -> Result<Vec<ObjectKey>, PersistenceError>;
}
