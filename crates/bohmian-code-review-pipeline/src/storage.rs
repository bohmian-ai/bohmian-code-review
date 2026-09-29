//! Injected immutable object storage.
//!
//! The library never accepts an unscoped key: every [`ObjectKey`] is built
//! here from a validated [`Identity`], so a consumer's storage implementation
//! only ever sees tenant-, installation-, and repository-scoped paths.
//! Objects are create-only; integrity is checked by the library through
//! [`ObjectDigest`], not by the backend.

use crate::identity::{Identity, InstallationId, InvalidIdentity, ObjectSha, TenantId};
use crate::persistence::ContextId;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::num::NonZeroU64;
use thiserror::Error;

/// Why a storage operation failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StorageError {
    /// A create-only write found an existing object.
    #[error("object already exists: {key}")]
    AlreadyExists {
        /// The conflicting key.
        key: ObjectKey,
    },
    /// An exact-key read found nothing.
    #[error("object not found: {key}")]
    NotFound {
        /// The missing key.
        key: ObjectKey,
    },
    /// The backend failed; `detail` must not contain credentials or source.
    #[error("storage backend failed: {detail}")]
    Backend {
        /// Backend-specific description.
        detail: String,
    },
}

/// Fully scoped storage key for one immutable object.
///
/// Keys are only produced by [`ObjectKey::snapshot`] and
/// [`ObjectKey::context`], both of which embed the caller identity, so two
/// tenants can never share an object even for identical source. A persisted
/// key is re-parsed against those two layouts on reload, so it can never be
/// unscoped or traverse out of its scope.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ObjectKey(String);

impl ObjectKey {
    /// Scoped prefix shared by every object of one identity.
    fn prefix(identity: &Identity) -> String {
        format!(
            "tenants/{}/installations/{}/repositories/{}",
            identity.tenant.as_str(),
            identity.installation.get(),
            identity.repository.id()
        )
    }

    /// Key for a snapshot of `commit` produced under one extraction
    /// version, format version, and file-size ceiling, so facts built under
    /// another ceiling are never reused.
    ///
    /// The extraction version is hex-encoded because it is free text.
    pub fn snapshot(
        identity: &Identity,
        extraction_version: &str,
        format_version: u32,
        max_file_bytes: u64,
        commit: &ObjectSha,
    ) -> ObjectKey {
        ObjectKey(format!(
            "{}/snapshots/{}/{format_version}/{max_file_bytes}/{commit}",
            ObjectKey::prefix(identity),
            hex(extraction_version.as_bytes())
        ))
    }

    /// Key for the review context published under `context_id`.
    pub fn context(identity: &Identity, context_id: &ContextId) -> ObjectKey {
        ObjectKey(format!(
            "{}/contexts/{context_id}",
            ObjectKey::prefix(identity)
        ))
    }

    /// Re-validate persisted key text against the two constructor layouts.
    ///
    /// Every variable part is parsed with the same validator its constructor
    /// input went through, then the key is re-formatted; only text that
    /// reproduces byte for byte is accepted, so unscoped keys, traversal,
    /// extra segments, and non-canonical numbers such as `01` are refused.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentity::ObjectKey`] when `raw` is not exactly a
    /// snapshot or context key.
    fn parse(raw: &str) -> Result<ObjectKey, InvalidIdentity> {
        ObjectKey::canonical(raw)
            .filter(|canonical| canonical == raw)
            .map(ObjectKey)
            .ok_or(InvalidIdentity::ObjectKey)
    }

    /// Re-format `raw` from its validated parts, or `None` when a part is
    /// invalid or the segments match neither layout.
    fn canonical(raw: &str) -> Option<String> {
        let segments: Vec<&str> = raw.split('/').collect();
        let [
            "tenants",
            tenant,
            "installations",
            installation,
            "repositories",
            repository,
            rest @ ..,
        ] = segments.as_slice()
        else {
            return None;
        };
        let prefix = format!(
            "tenants/{}/installations/{}/repositories/{}",
            TenantId::new(tenant).ok()?.as_str(),
            InstallationId::new(installation.parse().ok()?).ok()?.get(),
            repository.parse::<NonZeroU64>().ok()?
        );
        match rest {
            ["snapshots", version, format_version, max_file_bytes, commit] => {
                let version_is_hex = !version.is_empty()
                    && version.len() % 2 == 0
                    && version
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
                if !version_is_hex {
                    return None;
                }
                Some(format!(
                    "{prefix}/snapshots/{version}/{}/{}/{}",
                    format_version.parse::<u32>().ok()?,
                    max_file_bytes.parse::<u64>().ok()?,
                    ObjectSha::new(commit).ok()?
                ))
            }
            ["contexts", id] => Some(format!("{prefix}/contexts/{}", ContextId::new(id).ok()?)),
            _ => None,
        }
    }

    /// Whether this is a snapshot key of `identity` under any extraction
    /// version, format version, file ceiling, or commit.
    ///
    /// Keys only exist in the two validated layouts, so a prefix check is
    /// exact.
    pub(crate) fn is_snapshot_of(&self, identity: &Identity) -> bool {
        self.0
            .starts_with(&format!("{}/snapshots/", ObjectKey::prefix(identity)))
    }

    /// Slash-separated key text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ObjectKey {
    /// Deserializes through layout parsing so serde cannot mint a key the
    /// constructors would never produce.
    ///
    /// The wire shape stays a transparent string, so every constructor-built
    /// key round-trips byte-for-byte; only the invariant is added.
    ///
    /// # Errors
    ///
    /// Returns a serde error carrying [`InvalidIdentity::ObjectKey`] when the
    /// serialized string is not exactly a snapshot or context key.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<ObjectKey, D::Error> {
        let raw = String::deserialize(deserializer)?;
        ObjectKey::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ObjectKey {
    /// Writes the key text.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Lowercase hexadecimal encoding.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// SHA-256 of an object's bytes, recorded in metadata and checked on read.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectDigest(String);

impl ObjectDigest {
    /// Digest of `bytes`.
    pub fn of(bytes: &[u8]) -> ObjectDigest {
        ObjectDigest(hex(&Sha256::digest(bytes)))
    }

    /// Hexadecimal digest text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Create-only object storage a consumer implements or the library adapts.
///
/// Implementations must be safe to call from several threads and must not
/// interpret object contents.
///
/// Methods return `Send` futures and implementers may write `async fn`. The
/// trait is not dyn-compatible, so facades take it as a generic parameter.
pub trait ObjectStorage: Send + Sync {
    /// Write a new object, refusing to overwrite.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::AlreadyExists`] when `key` is taken and
    /// [`StorageError::Backend`] on any other failure. A failed write must
    /// not leave a partial object readable.
    fn create(
        &self,
        key: &ObjectKey,
        bytes: &[u8],
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Read the exact object at `key`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] when absent and
    /// [`StorageError::Backend`] on any other failure.
    fn read(&self, key: &ObjectKey) -> impl Future<Output = Result<Vec<u8>, StorageError>> + Send;

    /// Delete `key`; deleting an absent key succeeds so retries converge.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Backend`] when the object may still exist.
    fn delete(&self, key: &ObjectKey) -> impl Future<Output = Result<(), StorageError>> + Send;
}

#[cfg(test)]
/// Owner tests for identity-scoped object keys and digests.
pub(crate) mod tests {
    use super::*;
    use crate::identity::RepositoryIdentity;

    /// Key texts no constructor produces: unscoped, zero or non-canonical
    /// ids, malformed versions or trees, extra segments, and traversal.
    pub(crate) fn hostile_keys() -> Vec<String> {
        let tree = "b".repeat(40);
        let snapshots = "tenants/a/installations/1/repositories/7/snapshots";
        vec![
            "x".to_owned(),
            "a/b/c".to_owned(),
            "foo/bar".to_owned(),
            "tenants/a/installations/0/repositories/7/contexts/c".to_owned(),
            "tenants/a/installations/1/repositories/0/contexts/c".to_owned(),
            "tenants/a/installations/01/repositories/7/contexts/c".to_owned(),
            format!("{snapshots}/zz/1/1024/{tree}"),
            format!("{snapshots}/763/1/1024/{tree}"),
            format!("{snapshots}/7631/1/1024/nothex"),
            format!("{snapshots}/7631/1/01024/{tree}"),
            format!("{snapshots}/7631/1/{tree}"),
            format!("{snapshots}/7631/1/1024/{tree}/extra"),
            "tenants/a/installations/1/repositories/7/contexts/c/extra".to_owned(),
            "tenants/b/installations/9/repositories/3/contexts/../x".to_owned(),
            "tenants/b/installations/9/repositories/3/contexts/./x".to_owned(),
            "tenants/./installations/1/repositories/7/contexts/c".to_owned(),
            "tenants/../installations/1/repositories/7/contexts/c".to_owned(),
            "tenants/b//contexts/x".to_owned(),
            "/tenants/b".to_owned(),
            "tenants/b/".to_owned(),
            "tenants/b/contexts/x y".to_owned(),
            String::new(),
        ]
    }

    /// Builds an identity for one tenant.
    fn identity(tenant: &str) -> Identity {
        Identity {
            tenant: TenantId::new(tenant).expect("tenant"),
            installation: InstallationId::new(42).expect("installation"),
            repository: RepositoryIdentity::new(7, "o", "r").expect("repo"),
        }
    }

    /// Keys embed identity, versions, file ceiling, and tree so scopes
    /// never collide.
    #[test]
    fn keys_are_scoped_by_identity_and_version() {
        let tree = ObjectSha::new(&"b".repeat(40)).expect("sha");
        let a = ObjectKey::snapshot(&identity("a"), "v1", 1, 1024, &tree);
        let b = ObjectKey::snapshot(&identity("b"), "v1", 1, 1024, &tree);
        let a2 = ObjectKey::snapshot(&identity("a"), "v2", 1, 1024, &tree);
        let a_larger = ObjectKey::snapshot(&identity("a"), "v1", 1, 2048, &tree);
        assert_ne!(a, b);
        assert_ne!(a, a2);
        assert_ne!(a, a_larger);
        assert_eq!(
            a.as_str(),
            format!("tenants/a/installations/42/repositories/7/snapshots/7631/1/1024/{tree}")
        );
        let context = ObjectKey::context(&identity("a"), &ContextId::new("ctx1").expect("id"));
        assert_eq!(
            context.as_str(),
            "tenants/a/installations/42/repositories/7/contexts/ctx1"
        );
    }

    /// Constructor-built keys round-trip through serde; anything outside the
    /// two constructor layouts is refused on reload.
    #[test]
    fn keys_are_revalidated_on_deserialization() {
        let tree = ObjectSha::new(&"b".repeat(40)).expect("sha");
        let key = ObjectKey::snapshot(&identity("a"), "v1", 1, 1024, &tree);
        let json = serde_json::to_string(&key).expect("serialize");
        assert_eq!(json, format!("\"{key}\""));
        assert_eq!(
            serde_json::from_str::<ObjectKey>(&json).expect("round trip"),
            key
        );
        for hostile in hostile_keys() {
            assert!(
                serde_json::from_str::<ObjectKey>(&format!("\"{hostile}\"")).is_err(),
                "{hostile:?} must be refused"
            );
        }
    }

    /// Keys built from the real extraction version and a maximal tenant
    /// carry segments longer than a GitHub name yet still round-trip.
    #[test]
    fn long_constructor_segments_round_trip() {
        let tree = ObjectSha::new(&"b".repeat(40)).expect("sha");
        let identity = identity(&"t".repeat(128));
        let key = ObjectKey::snapshot(
            &identity,
            bohmian_code_review::EXTRACTION_VERSION,
            1,
            1024,
            &tree,
        );
        let json = serde_json::to_string(&key).expect("serialize");
        assert_eq!(
            serde_json::from_str::<ObjectKey>(&json).expect("round trip"),
            key
        );
        let context = ObjectKey::context(&identity, &ContextId::new("ctx1").expect("id"));
        let json = serde_json::to_string(&context).expect("serialize");
        assert_eq!(
            serde_json::from_str::<ObjectKey>(&json).expect("round trip"),
            context
        );
    }

    /// Digests are deterministic and distinguish different bytes.
    #[test]
    fn digest_is_sha256_hex() {
        assert_eq!(
            ObjectDigest::of(b"").as_str(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_ne!(ObjectDigest::of(b"a"), ObjectDigest::of(b"b"));
    }
}
