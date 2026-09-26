//! Injected metadata persistence: a [`SnapshotStore`] for snapshot pointers
//! and a separate [`ContextStore`] for context records.
//!
//! The library publishes metadata only after the object it names exists and
//! validates, and every method carries the full [`Identity`] so an
//! implementation can bind each row to its tenant and installation. No raw
//! database handle crosses this boundary; a consumer supplies the
//! implementation and owns its own connection scoping.

use crate::identity::{Identity, InvalidIdentity, ObjectSha, validate_name};
use crate::storage::{ObjectDigest, ObjectKey};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
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
///
/// The text is used verbatim as the final segment of a context
/// [`ObjectKey`], so it is restricted to one key-safe segment and
/// re-validated on deserialization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ContextId(String);

impl ContextId {
    /// Validate a context identifier.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentity::Name`] when `raw` is empty, longer than 100
    /// bytes, exactly `.` or `..`, or outside `[A-Za-z0-9._-]`.
    pub fn new(raw: &str) -> Result<ContextId, InvalidIdentity> {
        validate_name(raw, "context id").map(ContextId)
    }

    /// Identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ContextId {
    /// Deserializes through [`ContextId::new`] so persisted ids are re-validated.
    ///
    /// # Errors
    ///
    /// Returns a serde error carrying the [`InvalidIdentity`] reason when the
    /// serialized string is not a single key-safe segment.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<ContextId, D::Error> {
        let raw = String::deserialize(deserializer)?;
        ContextId::new(&raw).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ContextId {
    /// Writes the identifier text.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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

/// Snapshot metadata store a consumer implements: the current
/// default-branch pointer per [`SnapshotScope`].
///
/// Every method receives the caller identity and must refuse to read or
/// write another identity's rows. Implementations must not hold a
/// transaction open across a call into GitHub or object storage; the library
/// never asks them to, and publishes a pointer only after its object exists.
pub trait SnapshotStore: Send + Sync {
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

    /// Record that a validated snapshot object exists at `key`, refreshing
    /// `recorded_at` when the row already exists.
    ///
    /// The library records an object after it validates and before any
    /// pointer names it, so an object orphaned by an interrupted build is
    /// still collectable.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn record_snapshot(
        &self,
        identity: &Identity,
        key: &ObjectKey,
        recorded_at: SystemTime,
    ) -> Result<(), PersistenceError>;

    /// Up to `limit` recorded snapshots, across identities, whose
    /// `recorded_at` is before `before` and that no pointer names.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn unreferenced_snapshots(
        &self,
        before: SystemTime,
        limit: usize,
    ) -> Result<Vec<(Identity, ObjectKey)>, PersistenceError>;

    /// Forget the record of `key` under `identity`; forgetting an absent
    /// record succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] when the row may still exist.
    fn delete_snapshot(&self, identity: &Identity, key: &ObjectKey)
    -> Result<(), PersistenceError>;

    /// Remove every snapshot pointer and record for `identity`, returning
    /// every recorded object key the caller must delete from snapshot
    /// storage.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn remove_repository(&self, identity: &Identity) -> Result<Vec<ObjectKey>, PersistenceError>;
}

/// Context metadata store a consumer implements: published PR context
/// records, kept separately from snapshots because their lifecycle differs.
///
/// Every method receives the caller identity (or returns it, for expiry
/// sweeps) and must refuse to read or write another identity's rows.
/// Implementations must not hold a transaction open across a call into
/// GitHub or object storage; the library publishes a record only after its
/// object exists.
pub trait ContextStore: Send + Sync {
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

    /// Remove every context record for `identity`, returning the object keys
    /// the caller must delete from context storage.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn remove_repository(&self, identity: &Identity) -> Result<Vec<ObjectKey>, PersistenceError>;
}

#[cfg(test)]
/// Owner tests for validated context ids and re-validated persisted records.
mod tests {
    use super::*;
    use crate::identity::{InstallationId, RepositoryIdentity, TenantId};
    use crate::storage::tests::hostile_keys;
    use std::time::UNIX_EPOCH;

    /// Context ids are single key-safe segments on construction and reload.
    #[test]
    fn context_ids_are_validated_on_construction_and_deserialization() {
        assert_eq!(ContextId::new("ctx-1").expect("id").as_str(), "ctx-1");
        assert_eq!(
            ContextId::new("../x"),
            Err(InvalidIdentity::Name("context id"))
        );
        assert_eq!(ContextId::new(""), Err(InvalidIdentity::Name("context id")));
        assert_eq!(
            ContextId::new("."),
            Err(InvalidIdentity::Name("context id"))
        );
        assert!(serde_json::from_str::<ContextId>("\"a/b\"").is_err());
        assert_eq!(
            serde_json::from_str::<ContextId>("\"a.b\"").expect("reload"),
            ContextId::new("a.b").expect("id")
        );
    }

    /// Persisted rows carrying a key outside the constructor layouts are
    /// refused on reload; a library-built row round-trips.
    #[test]
    fn records_revalidate_their_keys_on_deserialization() {
        let identity = Identity {
            tenant: TenantId::new(&"t".repeat(128)).expect("tenant"),
            installation: InstallationId::new(1).expect("installation"),
            repository: RepositoryIdentity::new(7, "o", "r").expect("repo"),
        };
        let sha = ObjectSha::new(&"a".repeat(40)).expect("sha");
        let id = ContextId::new("ctx1").expect("id");
        let record = ContextRecord {
            id: id.clone(),
            key: ObjectKey::context(&identity, &id),
            digest: ObjectDigest::of(b""),
            pull_request: 1,
            base: sha.clone(),
            head: sha.clone(),
            published_at: UNIX_EPOCH,
            expires_at: UNIX_EPOCH,
        };
        let json = serde_json::to_string(&record).expect("serialize");
        assert_eq!(
            serde_json::from_str::<ContextRecord>(&json).expect("round trip"),
            record
        );
        let pointer = SnapshotPointer {
            tree: sha.clone(),
            commit: sha,
            key: ObjectKey::snapshot(
                &identity,
                bohmian_code_review::EXTRACTION_VERSION,
                1,
                &record.base,
            ),
            digest: ObjectDigest::of(b""),
            published_at: UNIX_EPOCH,
        };
        let pointer_json = serde_json::to_string(&pointer).expect("serialize");
        assert_eq!(
            serde_json::from_str::<SnapshotPointer>(&pointer_json).expect("round trip"),
            pointer
        );
        for hostile in hostile_keys() {
            let record_json = json.replace(record.key.as_str(), &hostile);
            assert!(
                serde_json::from_str::<ContextRecord>(&record_json).is_err(),
                "{hostile:?}"
            );
            let hostile_pointer = pointer_json.replace(pointer.key.as_str(), &hostile);
            assert!(
                serde_json::from_str::<SnapshotPointer>(&hostile_pointer).is_err(),
                "{hostile:?}"
            );
        }
    }
}
