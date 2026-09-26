//! Injected immutable object storage.
//!
//! The library never accepts an unscoped key: every [`ObjectKey`] is built
//! here from a validated [`Identity`], so a consumer's storage implementation
//! only ever sees tenant-, installation-, and repository-scoped paths.
//! Objects are create-only; integrity is checked by the library through
//! [`ObjectDigest`], not by the backend.

use crate::identity::{Identity, InvalidIdentity, ObjectSha, validate_name};
use crate::persistence::ContextId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
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
/// tenants can never share an object even for identical source. Every
/// `/`-separated segment is key-safe, so a persisted key is re-validated on
/// reload and can never traverse out of its scope.
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

    /// Key for a snapshot of `tree` produced under one extraction and format version.
    ///
    /// The extraction version is hex-encoded because it is free text.
    pub fn snapshot(
        identity: &Identity,
        extraction_version: &str,
        format_version: u32,
        tree: &ObjectSha,
    ) -> ObjectKey {
        ObjectKey(format!(
            "{}/snapshots/{}/{format_version}/{tree}",
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

    /// Re-validate persisted key text: every `/`-separated segment must be a
    /// key-safe name, which refuses empty, `.`, `..`, and foreign characters.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentity::Name`] for the first segment that fails
    /// [`validate_name`].
    fn parse(raw: &str) -> Result<ObjectKey, InvalidIdentity> {
        for segment in raw.split('/') {
            validate_name(segment, "object key segment")?;
        }
        Ok(ObjectKey(raw.to_owned()))
    }

    /// Slash-separated key text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ObjectKey {
    /// Deserializes through [`ObjectKey::parse`] so serde cannot mint a key
    /// the constructors would never produce.
    ///
    /// The wire shape stays a transparent string, so every constructor-built
    /// key round-trips byte-for-byte; only the invariant is added.
    ///
    /// # Errors
    ///
    /// Returns a serde error carrying the [`InvalidIdentity`] reason when any
    /// segment of the serialized string is not key-safe.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<ObjectKey, D::Error> {
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
pub trait ObjectStorage: Send + Sync {
    /// Write a new object, refusing to overwrite.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::AlreadyExists`] when `key` is taken and
    /// [`StorageError::Backend`] on any other failure. A failed write must
    /// not leave a partial object readable.
    fn create(&self, key: &ObjectKey, bytes: &[u8]) -> Result<(), StorageError>;

    /// Read the exact object at `key`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] when absent and
    /// [`StorageError::Backend`] on any other failure.
    fn read(&self, key: &ObjectKey) -> Result<Vec<u8>, StorageError>;

    /// Delete `key`; deleting an absent key succeeds so retries converge.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Backend`] when the object may still exist.
    fn delete(&self, key: &ObjectKey) -> Result<(), StorageError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{InstallationId, RepositoryIdentity, TenantId};

    /// Builds an identity for one tenant.
    fn identity(tenant: &str) -> Identity {
        Identity {
            tenant: TenantId::new(tenant).expect("tenant"),
            installation: InstallationId::new(42).expect("installation"),
            repository: RepositoryIdentity::new(7, "o", "r").expect("repo"),
        }
    }

    /// Keys embed identity, version, and tree so scopes never collide.
    #[test]
    fn keys_are_scoped_by_identity_and_version() {
        let tree = ObjectSha::new(&"b".repeat(40)).expect("sha");
        let a = ObjectKey::snapshot(&identity("a"), "v1", 1, &tree);
        let b = ObjectKey::snapshot(&identity("b"), "v1", 1, &tree);
        let a2 = ObjectKey::snapshot(&identity("a"), "v2", 1, &tree);
        assert_ne!(a, b);
        assert_ne!(a, a2);
        assert_eq!(
            a.as_str(),
            format!("tenants/a/installations/42/repositories/7/snapshots/7631/1/{tree}")
        );
        let context = ObjectKey::context(&identity("a"), &ContextId::new("ctx1").expect("id"));
        assert_eq!(
            context.as_str(),
            "tenants/a/installations/42/repositories/7/contexts/ctx1"
        );
    }

    /// Constructor-built keys round-trip through serde; a traversing, empty,
    /// dot, or non-key-safe segment is refused on reload.
    #[test]
    fn keys_are_revalidated_on_deserialization() {
        let tree = ObjectSha::new(&"b".repeat(40)).expect("sha");
        let key = ObjectKey::snapshot(&identity("a"), "v1", 1, &tree);
        let json = serde_json::to_string(&key).expect("serialize");
        assert_eq!(json, format!("\"{key}\""));
        assert_eq!(
            serde_json::from_str::<ObjectKey>(&json).expect("round trip"),
            key
        );
        for hostile in [
            "tenants/b/installations/9/repositories/3/contexts/../x",
            "tenants/b/installations/9/repositories/3/contexts/./x",
            "tenants/b//contexts/x",
            "/tenants/b",
            "tenants/b/",
            "tenants/b/contexts/x y",
            "",
        ] {
            assert!(
                serde_json::from_str::<ObjectKey>(&format!("\"{hostile}\"")).is_err(),
                "{hostile:?} must be refused"
            );
        }
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
