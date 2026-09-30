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
use std::time::{Duration, SystemTime};
use thiserror::Error;

/// Why a persistence operation failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PersistenceError {
    /// A running collection claimed the snapshot record of `key`; retry the
    /// build after that collection deletes it.
    #[error("snapshot record is being collected: {key}")]
    Collecting {
        /// The claimed key.
        key: ObjectKey,
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
    /// Largest eligible file the facts were built under.
    pub max_file_bytes: u64,
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
///
/// It carries no time: the [`ContextStore`] stamps publication and expiry
/// on its own clock.
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
}

/// When a [`ContextStore`] published a record and when it expires, both on
/// the store's clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextLifetime {
    /// When the store first recorded the context.
    pub published_at: SystemTime,
    /// `published_at` plus `CONTEXT_LIFETIME`; reads at or after it are
    /// refused even if deletion lags.
    pub expires_at: SystemTime,
}

/// What a [`ContextStore`] holds for one exact `(identity, id)`, judged on
/// its own clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextLookup {
    /// No record.
    Absent,
    /// A record whose `expires_at` is at or before the store's now.
    Expired,
    /// A readable record.
    Live(ContextRecord),
}

/// Snapshot metadata store a consumer implements: snapshot records and the
/// current default-branch pointer per [`SnapshotScope`].
///
/// A record tracks one snapshot object key and when a build last touched
/// it. The library upserts a record before creating its object and moves a
/// pointer only after the object validates, so every object has a record
/// and every pointer names a validated object. [`SnapshotStore::unreferenced`]
/// claims the records it returns in the same atomic step that selects them,
/// and [`SnapshotStore::upsert_record`] refuses a claimed record, so a build
/// can never refresh, and then publish, a key whose object a collection is
/// deleting. Every method receives the
/// caller identity and must refuse to read or write another identity's
/// rows. Implementations must not hold a transaction open across a call
/// into GitHub or object storage; the library never asks them to.
///
/// Methods return `Send` futures and implementers may write `async fn`. The
/// trait is not dyn-compatible, so facades take it as a generic parameter.
pub trait SnapshotStore: Send + Sync {
    /// Current pointer for `scope`, if one was published.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn snapshot_pointer(
        &self,
        scope: &SnapshotScope,
    ) -> impl Future<Output = Result<Option<SnapshotPointer>, PersistenceError>> + Send;

    /// Compare-and-swap: publish `pointer` only when the current pointer's
    /// tree equals `expected` (`None` meaning no pointer).
    ///
    /// Returns `true` when the pointer moved and `false` when another
    /// publication won, so concurrent builds converge without a lock held
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
    ) -> impl Future<Output = Result<bool, PersistenceError>> + Send;

    /// Insert the record of `key`, or refresh an existing unclaimed
    /// record, stamping `touched_at` with the store's current time.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Collecting`], changing nothing, when the
    /// record is claimed, and [`PersistenceError::Backend`] on backend failure.
    fn upsert_record(
        &self,
        identity: &Identity,
        key: &ObjectKey,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;

    /// Claim and return the keys of `identity`'s records that no pointer of
    /// `identity` names and whose `touched_at` is at or before the store's
    /// current time minus `grace`.
    ///
    /// Selection and claim are one atomic step against record upserts and
    /// pointer swaps (for example one `UPDATE ... RETURNING`). A claimed
    /// record stays claimed until deleted and is returned again by the next
    /// call, so a failed or dropped collection is retried; calls for one
    /// identity must not overlap.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn unreferenced(
        &self,
        identity: &Identity,
        grace: Duration,
    ) -> impl Future<Output = Result<Vec<ObjectKey>, PersistenceError>> + Send;

    /// Delete the record of `key` under `identity`; deleting an absent
    /// record succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] when the row may still exist.
    fn delete_record(
        &self,
        identity: &Identity,
        key: &ObjectKey,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;

    /// Remove every pointer of `identity`, leaving its records unreferenced.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn remove_pointers(
        &self,
        identity: &Identity,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;
}

/// Context metadata store a consumer implements: published PR context
/// records, kept separately from snapshots because their lifecycle differs.
///
/// The library upserts a record before creating the object it names, so
/// every context object has a record; a record whose object is absent is
/// a dropped or failed publication that readers treat as not found and
/// expiry cleanup removes. Every method receives the caller identity (or
/// returns it, for expiry sweeps) and must refuse to read or write another
/// identity's rows; lookup is by exact identity and id. Implementations
/// must not hold a transaction open across a call into GitHub or object
/// storage; the library never asks them to.
///
/// Methods return `Send` futures and implementers may write `async fn`. The
/// trait is not dyn-compatible, so facades take it as a generic parameter.
pub trait ContextStore: Send + Sync {
    /// Insert `record` for `identity`, stamping `published_at` with the
    /// store's current time and `expires_at` with that plus
    /// `CONTEXT_LIFETIME`, and return the stamps. An existing record with
    /// the same id keeps its stamps, so a repeated upsert converges.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn upsert_context(
        &self,
        identity: &Identity,
        record: &ContextRecord,
    ) -> impl Future<Output = Result<ContextLifetime, PersistenceError>> + Send;

    /// Exact lookup of `id` under `identity`, judging expiry on the store's
    /// clock; another identity's id is absent.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn context(
        &self,
        identity: &Identity,
        id: &ContextId,
    ) -> impl Future<Output = Result<ContextLookup, PersistenceError>> + Send;

    /// Up to `limit` records whose `expires_at` is at or before the store's
    /// current time, across identities.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn expired_contexts(
        &self,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<(Identity, ContextRecord)>, PersistenceError>> + Send;

    /// Remove `id` under `identity`; removing an absent id succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] when the row may still exist.
    fn delete_context(
        &self,
        identity: &Identity,
        id: &ContextId,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;

    /// Remove every context record for `identity`, returning the object keys
    /// the caller must delete from context storage.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::Backend`] on backend failure.
    fn remove_repository(
        &self,
        identity: &Identity,
    ) -> impl Future<Output = Result<Vec<ObjectKey>, PersistenceError>> + Send;
}

#[cfg(test)]
/// Owner tests for validated context ids and re-validated persisted records.
mod tests {
    use super::*;
    use crate::identity::{InstallationId, RepositoryIdentity, TenantId};
    use crate::storage::tests::hostile_keys;

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
                1024,
                &record.base,
            ),
            digest: ObjectDigest::of(b""),
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
