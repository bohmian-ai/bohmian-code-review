//! Validated caller and Git identities that cross the pipeline trust boundary.
//!
//! Every facade and every durable seam receives these values instead of raw
//! strings, so a tenant, installation, repository, commit, tree, or blob can
//! only be named in one validated spelling. Construction rejects anything the
//! library would otherwise have to sanitize later.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Longest accepted tenant identifier in bytes.
const MAX_TENANT_BYTES: usize = 128;

/// Longest accepted GitHub owner or repository name in bytes.
const MAX_NAME_BYTES: usize = 100;

/// Why an identity value was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidIdentity {
    /// A tenant identifier was empty, over-long, or used characters outside
    /// `[A-Za-z0-9._-]`.
    #[error("tenant id must be 1..={MAX_TENANT_BYTES} bytes of [A-Za-z0-9._-]")]
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

/// True when every byte is in `[A-Za-z0-9._-]`.
fn is_key_safe(raw: &str) -> bool {
    raw.bytes()
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
    /// or outside `[A-Za-z0-9._-]`.
    pub fn new(raw: &str) -> Result<TenantId, InvalidIdentity> {
        if raw.is_empty() || raw.len() > MAX_TENANT_BYTES || !is_key_safe(raw) {
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
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<TenantId, D::Error> {
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
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<InstallationId, D::Error> {
        InstallationId::new(u64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Validate a GitHub owner or repository name, or any other single storage-key segment.
///
/// # Errors
///
/// Returns [`InvalidIdentity::Name`] carrying `what` when `raw` is empty,
/// longer than 100 bytes, exactly `.` or `..`, or outside `[A-Za-z0-9._-]`.
pub(crate) fn validate_name(raw: &str, what: &'static str) -> Result<String, InvalidIdentity> {
    if raw.is_empty()
        || raw.len() > MAX_NAME_BYTES
        || raw == "."
        || raw == ".."
        || !is_key_safe(raw)
    {
        return Err(InvalidIdentity::Name(what));
    }
    Ok(raw.to_owned())
}

/// One GitHub repository named by its stable numeric id and its `owner/name`.
///
/// The numeric id survives renames, so provider responses are verified
/// against it before any tree, blob, or PR data is trusted. The names are
/// only used to build request paths.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<ObjectSha, D::Error> {
        let raw = String::deserialize(deserializer)?;
        ObjectSha::new(&raw).map_err(serde::de::Error::custom)
    }
}

/// Which GitHub deployment serves the API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubHost {
    /// `api.github.com`, whose archives redirect to `codeload.github.com`.
    Public,
    /// A GitHub Enterprise Server API base such as `https://ghe.example.com/api/v3`.
    Enterprise {
        /// Validated `https://host[/path]` base without trailing slash.
        api_base: String,
    },
}

impl GitHubHost {
    /// Validate a GHES API base URL.
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
        let host = rest.split('/').next().unwrap_or("");
        if host.is_empty()
            || host.contains('@')
            || rest.ends_with('/')
            || api_base
                .bytes()
                .any(|byte| matches!(byte, b'?' | b'#') || byte.is_ascii_whitespace())
        {
            return Err(InvalidIdentity::ApiBase);
        }
        Ok(GitHubHost::Enterprise {
            api_base: api_base.to_owned(),
        })
    }

    /// API base URL without trailing slash.
    pub fn api_base(&self) -> &str {
        match self {
            GitHubHost::Public => "https://api.github.com",
            GitHubHost::Enterprise { api_base } => api_base,
        }
    }

    /// The only host an archive redirect may point at.
    pub fn archive_host(&self) -> &str {
        match self {
            GitHubHost::Public => "codeload.github.com",
            GitHubHost::Enterprise { api_base } => {
                api_base["https://".len()..].split('/').next().unwrap_or("")
            }
        }
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
mod tests {
    use super::*;

    /// Key-safe identities construct; hostile or empty spellings fail.
    #[test]
    fn identities_validate_key_safe_spellings() {
        assert!(TenantId::new("tenant-1").is_ok());
        assert_eq!(TenantId::new(""), Err(InvalidIdentity::Tenant));
        assert_eq!(TenantId::new("a/b"), Err(InvalidIdentity::Tenant));
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
        assert_eq!(
            serde_json::from_str::<RepositoryIdentity>(&json).expect("round trip"),
            repo
        );
        assert!(
            serde_json::from_str::<RepositoryIdentity>(r#"{"id":0,"owner":"o","name":"r"}"#)
                .is_err()
        );
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

    /// GHES bases are validated and yield the same host for archive redirects.
    #[test]
    fn hosts_expose_api_base_and_archive_host() {
        assert_eq!(GitHubHost::Public.archive_host(), "codeload.github.com");
        let ghes = GitHubHost::enterprise("https://ghe.example.com/api/v3").expect("valid");
        assert_eq!(ghes.api_base(), "https://ghe.example.com/api/v3");
        assert_eq!(ghes.archive_host(), "ghe.example.com");
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
