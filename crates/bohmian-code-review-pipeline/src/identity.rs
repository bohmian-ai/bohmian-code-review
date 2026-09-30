//! Validated caller and Git identities that cross the pipeline trust boundary.
//!
//! Every facade and every durable seam receives these values instead of raw
//! strings, so a tenant, installation, repository, commit, tree, or blob can
//! only be named in one validated spelling. Construction rejects anything the
//! library would otherwise have to sanitize later.

use serde::{Deserialize, Deserializer, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use thiserror::Error;

/// Longest accepted tenant identifier in bytes.
const MAX_TENANT_BYTES: usize = 128;

/// Longest accepted GitHub owner or repository name in bytes.
const MAX_NAME_BYTES: usize = 100;

/// Why an identity value was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidIdentity {
    /// A tenant identifier was empty, over-long, dot-only, or used characters
    /// outside `[A-Za-z0-9._-]`.
    #[error("tenant id must be 1..={MAX_TENANT_BYTES} bytes of [A-Za-z0-9._-] and not `.`/`..`")]
    Tenant,
    /// An installation id was zero.
    #[error("installation id must be nonzero")]
    Installation,
    /// A repository id was zero.
    #[error("repository id must be nonzero")]
    RepositoryId,
    /// An owner or repository name was empty, over-long, dot-only, or used
    /// characters outside `[A-Za-z0-9._-]`.
    #[error("{0} must be 1..={MAX_NAME_BYTES} bytes of [A-Za-z0-9._-] and not `.`/`..`")]
    Name(&'static str),
    /// A persisted storage key did not match the snapshot or context layout.
    #[error("object key must match the snapshot or context key layout")]
    ObjectKey,
    /// A Git object id was not 40 lowercase hexadecimal characters.
    #[error("git object id must be 40 lowercase hex characters")]
    ObjectSha,
    /// A GHES API base was not an `https://host[/path]` URL without
    /// userinfo, query, fragment, or trailing slash.
    #[error(
        "GHES api base must be https://host[/path] without userinfo, query, fragment, or trailing slash"
    )]
    ApiBase,
    /// A token was empty or contained whitespace or control characters.
    #[error("installation token must be nonempty printable text without whitespace")]
    Token,
}

/// True when `raw` is usable verbatim as one storage-key segment.
///
/// The one segment rule shared by tenant ids, owner and repository names, and
/// context ids: 1..=`max_bytes` bytes of `[A-Za-z0-9._-]`, and never `.` or
/// `..`, so a segment can neither be empty nor traverse out of its scope.
fn is_key_segment(raw: &str, max_bytes: usize) -> bool {
    !raw.is_empty()
        && raw.len() <= max_bytes
        && raw != "."
        && raw != ".."
        && raw
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// Caller-owned tenant identifier, distinct from any GitHub id.
///
/// The value is used verbatim inside storage keys, so it is restricted to a
/// key-safe alphabet.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct TenantId(String);

impl TenantId {
    /// Validate a tenant identifier.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentity::Tenant`] when empty, longer than 128 bytes,
    /// exactly `.` or `..`, or outside `[A-Za-z0-9._-]`.
    pub fn new(raw: &str) -> Result<TenantId, InvalidIdentity> {
        if !is_key_segment(raw, MAX_TENANT_BYTES) {
            return Err(InvalidIdentity::Tenant);
        }
        Ok(TenantId(raw.to_owned()))
    }

    /// Validated identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for TenantId {
    /// Deserializes through [`TenantId::new`] so persisted values are re-validated.
    ///
    /// # Errors
    ///
    /// Returns a serde error carrying the [`InvalidIdentity::Tenant`] reason
    /// when the serialized string is not a key-safe tenant id.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<TenantId, D::Error> {
        let raw = String::deserialize(deserializer)?;
        TenantId::new(&raw).map_err(serde::de::Error::custom)
    }
}

/// GitHub App installation id supplied by the caller after its own verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct InstallationId(u64);

impl InstallationId {
    /// Validate an installation id.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentity::Installation`] when zero.
    pub fn new(raw: u64) -> Result<InstallationId, InvalidIdentity> {
        if raw == 0 {
            return Err(InvalidIdentity::Installation);
        }
        Ok(InstallationId(raw))
    }

    /// Numeric id.
    pub fn get(self) -> u64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for InstallationId {
    /// Deserializes through [`InstallationId::new`] so persisted values are re-validated.
    ///
    /// # Errors
    ///
    /// Returns a serde error carrying the [`InvalidIdentity::Installation`]
    /// reason when the serialized id is zero.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<InstallationId, D::Error> {
        InstallationId::new(u64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Validate a GitHub owner or repository name, or a context id.
///
/// # Errors
///
/// Returns [`InvalidIdentity::Name`] carrying `what` when `raw` is empty,
/// longer than 100 bytes, exactly `.` or `..`, or outside `[A-Za-z0-9._-]`.
pub(crate) fn validate_name(raw: &str, what: &'static str) -> Result<String, InvalidIdentity> {
    if !is_key_segment(raw, MAX_NAME_BYTES) {
        return Err(InvalidIdentity::Name(what));
    }
    Ok(raw.to_owned())
}

/// One GitHub repository named by its stable numeric id and its `owner/name`.
///
/// The numeric id survives renames and transfers, so it alone is the
/// repository's identity: requests address the repository by id, and
/// equality, hashing, and ordering compare only the id, so snapshots,
/// history, and contexts stored under an earlier `owner/name` stay
/// reachable after a rename. The names are labels for logs and messages;
/// a stale name is harmless.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawRepository")]
pub struct RepositoryIdentity {
    id: u64,
    owner: String,
    name: String,
}

/// Wire shape used to re-validate a deserialized [`RepositoryIdentity`].
#[derive(Deserialize)]
struct RawRepository {
    id: u64,
    owner: String,
    name: String,
}

impl TryFrom<RawRepository> for RepositoryIdentity {
    type Error = InvalidIdentity;

    /// Re-validates persisted repository identity fields.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentity::RepositoryId`] for a zero id and
    /// [`InvalidIdentity::Name`] for an owner or name outside the key-safe
    /// alphabet, exactly as [`RepositoryIdentity::new`] does.
    fn try_from(raw: RawRepository) -> Result<RepositoryIdentity, InvalidIdentity> {
        RepositoryIdentity::new(raw.id, &raw.owner, &raw.name)
    }
}

impl RepositoryIdentity {
    /// Validate a repository identity.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentity::RepositoryId`] for a zero id and
    /// [`InvalidIdentity::Name`] for an owner or name outside GitHub's
    /// `[A-Za-z0-9._-]` alphabet.
    pub fn new(id: u64, owner: &str, name: &str) -> Result<RepositoryIdentity, InvalidIdentity> {
        if id == 0 {
            return Err(InvalidIdentity::RepositoryId);
        }
        Ok(RepositoryIdentity {
            id,
            owner: validate_name(owner, "owner")?,
            name: validate_name(name, "repository name")?,
        })
    }

    /// Stable numeric repository id.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Owner login.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Repository name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `owner/name` as GitHub reports it in `full_name`.
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

impl PartialEq for RepositoryIdentity {
    /// Same repository when the stable ids match, whatever the names.
    fn eq(&self, other: &RepositoryIdentity) -> bool {
        self.id == other.id
    }
}

impl Eq for RepositoryIdentity {}

impl Hash for RepositoryIdentity {
    /// Hashes the id only, consistent with [`PartialEq`].
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialOrd for RepositoryIdentity {
    /// Total order by id; see [`Ord`].
    fn partial_cmp(&self, other: &RepositoryIdentity) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RepositoryIdentity {
    /// Orders by id only, consistent with [`PartialEq`].
    fn cmp(&self, other: &RepositoryIdentity) -> Ordering {
        self.id.cmp(&other.id)
    }
}

/// Complete caller identity required at every durable seam.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Identity {
    /// Caller tenant.
    pub tenant: TenantId,
    /// Verified GitHub App installation.
    pub installation: InstallationId,
    /// Verified repository.
    pub repository: RepositoryIdentity,
}

/// A 40-character lowercase SHA-1 Git object id for a commit, tree, or blob.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ObjectSha(String);

impl ObjectSha {
    /// Validate a Git object id.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentity::ObjectSha`] unless the text is exactly 40
    /// lowercase hexadecimal characters.
    pub fn new(raw: &str) -> Result<ObjectSha, InvalidIdentity> {
        if raw.len() != 40
            || !raw
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(InvalidIdentity::ObjectSha);
        }
        Ok(ObjectSha(raw.to_owned()))
    }

    /// Hexadecimal text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObjectSha {
    /// Writes the hexadecimal id.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ObjectSha {
    /// Deserializes through [`ObjectSha::new`] so provider and persisted ids are validated.
    ///
    /// # Errors
    ///
    /// Returns a serde error carrying the [`InvalidIdentity::ObjectSha`]
    /// reason when the serialized string is not 40 lowercase hex characters.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<ObjectSha, D::Error> {
        let raw = String::deserialize(deserializer)?;
        ObjectSha::new(&raw).map_err(serde::de::Error::custom)
    }
}

/// Which GitHub deployment serves the API, and which hosts may serve its
/// archives.
///
/// The fields are private and computed once by [`GitHubHost::public`] or
/// [`GitHubHost::enterprise`], so every host a client sees is validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubHost {
    /// `https://host[/path]` API base without trailing slash.
    api_base: String,
    /// Hosts an archive redirect may point at.
    archive_hosts: Vec<String>,
}

impl GitHubHost {
    /// `api.github.com`, whose archives redirect to `codeload.github.com`.
    pub fn public() -> GitHubHost {
        GitHubHost {
            api_base: "https://api.github.com".to_owned(),
            archive_hosts: vec!["codeload.github.com".to_owned()],
        }
    }

    /// Validate a GHES API base URL such as `https://ghe.example.com/api/v3`.
    ///
    /// Archives are trusted on the API host itself (`/_codeload/`) and on
    /// `codeload.<host>`, which subdomain isolation uses.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentity::ApiBase`] unless the value is
    /// `https://host[/path]` with a nonempty host and no userinfo, query,
    /// fragment, whitespace, or trailing slash.
    pub fn enterprise(api_base: &str) -> Result<GitHubHost, InvalidIdentity> {
        let rest = api_base
            .strip_prefix("https://")
            .ok_or(InvalidIdentity::ApiBase)?;
        let host = rest.split_once('/').map_or(rest, |(host, _)| host);
        if host.is_empty()
            || host.contains('@')
            || rest.ends_with('/')
            || api_base
                .bytes()
                .any(|byte| matches!(byte, b'?' | b'#') || byte.is_ascii_whitespace())
        {
            return Err(InvalidIdentity::ApiBase);
        }
        Ok(GitHubHost {
            api_base: api_base.to_owned(),
            archive_hosts: vec![host.to_owned(), format!("codeload.{host}")],
        })
    }

    /// API base URL without trailing slash.
    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    /// Whether an archive redirect may point at `host`, compared ASCII
    /// case-insensitively.
    pub fn trusts_archive_host(&self, host: &str) -> bool {
        self.archive_hosts
            .iter()
            .any(|trusted| trusted.eq_ignore_ascii_case(host))
    }
}

/// A GitHub App installation access token minted by the consumer.
///
/// `Debug` and `Display` never reveal the secret; only
/// [`InstallationToken::expose`] does, and the client uses it solely to
/// build an `Authorization` header for the API host.
#[derive(Clone, PartialEq, Eq)]
pub struct InstallationToken(String);

impl InstallationToken {
    /// Validate a token.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentity::Token`] when empty or containing
    /// whitespace or control characters.
    pub fn new(raw: &str) -> Result<InstallationToken, InvalidIdentity> {
        if raw.is_empty()
            || raw
                .chars()
                .any(|ch| ch.is_whitespace() || ch.is_control() || !ch.is_ascii())
        {
            return Err(InvalidIdentity::Token);
        }
        Ok(InstallationToken(raw.to_owned()))
    }

    /// The secret text, for the `Authorization` header only.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for InstallationToken {
    /// Redacts the secret.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("InstallationToken(<redacted>)")
    }
}

#[cfg(test)]
/// Owner tests for identity validation, host configuration, and token redaction.
mod tests {
    use super::*;

    /// Key-safe identities construct; hostile or empty spellings fail.
    #[test]
    fn identities_validate_key_safe_spellings() {
        assert!(TenantId::new("tenant-1").is_ok());
        assert_eq!(TenantId::new(""), Err(InvalidIdentity::Tenant));
        assert_eq!(TenantId::new("a/b"), Err(InvalidIdentity::Tenant));
        for dots in [".", ".."] {
            assert_eq!(TenantId::new(dots), Err(InvalidIdentity::Tenant));
            assert!(serde_json::from_str::<TenantId>(&format!("\"{dots}\"")).is_err());
        }
        assert_eq!(InstallationId::new(0), Err(InvalidIdentity::Installation));
        assert_eq!(
            RepositoryIdentity::new(0, "o", "r"),
            Err(InvalidIdentity::RepositoryId)
        );
        assert_eq!(
            RepositoryIdentity::new(1, "o", ".."),
            Err(InvalidIdentity::Name("repository name"))
        );
        assert_eq!(
            RepositoryIdentity::new(1, "o/x", "r"),
            Err(InvalidIdentity::Name("owner"))
        );
        let repo = RepositoryIdentity::new(7, "bohmian-ai", "wyrd").expect("valid repo");
        assert_eq!(repo.full_name(), "bohmian-ai/wyrd");
        let json = serde_json::to_string(&repo).expect("serialize");
        let reloaded = serde_json::from_str::<RepositoryIdentity>(&json).expect("round trip");
        assert_eq!(reloaded, repo);
        assert_eq!(reloaded.full_name(), repo.full_name());
        assert!(
            serde_json::from_str::<RepositoryIdentity>(r#"{"id":0,"owner":"o","name":"r"}"#)
                .is_err()
        );
    }

    /// A renamed or transferred repository keeps its identity, hash, and
    /// order, so scope lookups keyed by `Identity` survive the rename; a
    /// different id under the same name is a different repository.
    #[test]
    fn repository_identity_is_the_stable_id() {
        use std::collections::hash_map::DefaultHasher;
        let hash = |repo: &RepositoryIdentity| {
            let mut state = DefaultHasher::new();
            repo.hash(&mut state);
            state.finish()
        };
        let before = RepositoryIdentity::new(7, "acme", "billing").expect("repo");
        let after = RepositoryIdentity::new(7, "new-owner", "payments").expect("repo");
        let other = RepositoryIdentity::new(8, "acme", "billing").expect("repo");
        assert_eq!(before, after);
        assert_eq!(hash(&before), hash(&after));
        assert_eq!(before.cmp(&after), Ordering::Equal);
        assert_ne!(before, other);
        assert!(before < other);
        assert_eq!(after.full_name(), "new-owner/payments");
    }

    /// Object ids accept only lowercase 40-hex, including through serde.
    #[test]
    fn object_sha_accepts_only_lowercase_hex() {
        let hex = "a".repeat(40);
        assert!(ObjectSha::new(&hex).is_ok());
        assert_eq!(
            ObjectSha::new(&"A".repeat(40)),
            Err(InvalidIdentity::ObjectSha)
        );
        assert_eq!(
            ObjectSha::new(&"a".repeat(39)),
            Err(InvalidIdentity::ObjectSha)
        );
        assert!(serde_json::from_str::<ObjectSha>("\"zz\"").is_err());
    }

    /// Public and GHES hosts expose their API base and trust only their own
    /// archive hosts; GHES bases are validated.
    #[test]
    fn hosts_expose_api_base_and_archive_host() {
        let public = GitHubHost::public();
        assert_eq!(public.api_base(), "https://api.github.com");
        assert!(public.trusts_archive_host("codeload.github.com"));
        assert!(!public.trusts_archive_host("api.github.com"));
        let ghes = GitHubHost::enterprise("https://ghe.example.com/api/v3").expect("valid");
        assert_eq!(ghes.api_base(), "https://ghe.example.com/api/v3");
        assert!(ghes.trusts_archive_host("ghe.example.com"));
        assert!(ghes.trusts_archive_host("CODELOAD.ghe.example.com"));
        for untrusted in [
            "codeload.ghe.example.com.evil",
            "evil.example.com",
            "codeload.github.com",
        ] {
            assert!(!ghes.trusts_archive_host(untrusted), "{untrusted}");
        }
        for bad in [
            "http://ghe.example.com",
            "https://user@ghe.example.com",
            "https://ghe.example.com/api/v3/",
            "https://ghe.example.com/api?x=1",
            "https://",
        ] {
            assert_eq!(
                GitHubHost::enterprise(bad),
                Err(InvalidIdentity::ApiBase),
                "{bad}"
            );
        }
    }

    /// Tokens validate and never print their secret.
    #[test]
    fn token_debug_is_redacted() {
        let token = InstallationToken::new("ghs_secret").expect("valid token");
        assert_eq!(format!("{token:?}"), "InstallationToken(<redacted>)");
        assert_eq!(InstallationToken::new("a b"), Err(InvalidIdentity::Token));
        assert_eq!(InstallationToken::new(""), Err(InvalidIdentity::Token));
    }
}
