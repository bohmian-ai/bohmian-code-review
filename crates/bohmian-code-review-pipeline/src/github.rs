//! Bounded, verified GitHub and GHES source access.
//!
//! [`GitHubClient`] performs the only provider IO in the workspace. Each
//! operation builds one authenticated request, applies a single retry policy
//! for rate limits and transient failures under the caller's [`Execution`],
//! parses a bounded response, and verifies the returned identity against
//! what was asked for before anything crosses into the symbolic library.
//! Transport is a one-method seam so the same policy and verification run
//! against a scripted fake in tests and `ureq` in production.

use crate::execution::{Execution, Interrupted};
use crate::identity::{GitHubHost, InstallationToken, ObjectSha, RepositoryIdentity};
use crate::storage::hex;
use bohmian_code_review::{InvalidRelativePath, RelativePath};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Media type for JSON API responses.
const ACCEPT_JSON: &str = "application/vnd.github+json";

/// Media type that returns raw blob bytes instead of base64 JSON.
const ACCEPT_RAW: &str = "application/vnd.github.raw+json";

/// Pinned REST API version.
const API_VERSION: &str = "2022-11-28";

/// Identifies this library to the provider.
const USER_AGENT: &str = "bohmian-code-review-pipeline";

/// Files requested per PR-files page; the provider's maximum.
const PR_FILES_PER_PAGE: u32 = 100;

/// Longest single sleep so cancellation is observed promptly while waiting.
const SLEEP_SLICE: Duration = Duration::from_millis(100);

/// One outbound GET request. Headers may include the bearer token, so this
/// type deliberately has no `Debug`.
pub struct HttpRequest {
    /// Absolute URL.
    pub url: String,
    /// Header name/value pairs.
    pub headers: Vec<(&'static str, String)>,
    /// Whole-request timeout including body transfer.
    pub timeout: Duration,
}

impl HttpRequest {
    /// Value of the first header named `name`, compared case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// One response with its body left unread so archives can stream.
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Lowercase header names with their values.
    pub headers: Vec<(String, String)>,
    /// Unread body.
    pub body: Box<dyn Read + Send>,
}

impl HttpResponse {
    /// Value of the first header named `name`, compared case-insensitively.
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// A request that produced no HTTP response.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransportError {
    /// The request exceeded its timeout.
    #[error("request timed out")]
    Timeout,
    /// Connection, TLS, or protocol failure; never includes headers.
    #[error("transport failed: {0}")]
    Failed(String),
}

/// Sends one GET request. Implemented by [`UreqTransport`] and by test fakes.
pub trait HttpTransport: Send + Sync {
    /// Send `request` without following redirects and without raising on status.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] only when no response arrived.
    fn get(&self, request: HttpRequest) -> Result<HttpResponse, TransportError>;
}

/// Production transport over `ureq` with redirects disabled.
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl UreqTransport {
    /// Build an agent that returns every status and never follows redirects.
    pub fn new() -> UreqTransport {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .user_agent(USER_AGENT)
            .build();
        UreqTransport {
            agent: config.into(),
        }
    }
}

impl Default for UreqTransport {
    /// Same as [`UreqTransport::new`].
    fn default() -> UreqTransport {
        UreqTransport::new()
    }
}

impl HttpTransport for UreqTransport {
    /// Sends the request with its own global timeout and streams the body.
    fn get(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut builder = self.agent.get(&request.url);
        for (name, value) in &request.headers {
            builder = builder.header(*name, value);
        }
        let response = builder
            .config()
            .timeout_global(Some(request.timeout))
            .build()
            .call()
            .map_err(|error| match error {
                ureq::Error::Timeout(_) => TransportError::Timeout,
                other => TransportError::Failed(other.to_string()),
            })?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect();
        Ok(HttpResponse {
            status,
            headers,
            body: Box::new(response.into_body().into_reader()),
        })
    }
}

/// Bounds applied to every provider call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestLimits {
    /// Attempts per operation including the first.
    pub max_attempts: u32,
    /// Longest in-call wait for `retry-after`, reset, or backoff; longer waits are reported as retryable errors.
    pub max_retry_wait: Duration,
    /// Per-request timeout, further capped by the remaining deadline.
    pub request_timeout: Duration,
    /// Largest JSON body accepted.
    pub max_json_bytes: usize,
    /// Largest blob accepted.
    pub max_blob_bytes: usize,
    /// Largest compressed archive stream accepted.
    pub max_archive_bytes: u64,
}

impl Default for RequestLimits {
    /// Three attempts, one-minute rate waits, 1 MiB blobs, 2 GiB archives.
    fn default() -> RequestLimits {
        RequestLimits {
            max_attempts: 3,
            max_retry_wait: Duration::from_secs(60),
            request_timeout: Duration::from_secs(60),
            max_json_bytes: 32 * 1024 * 1024,
            max_blob_bytes: 1024 * 1024,
            max_archive_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

/// Provider operations, for call accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operation {
    /// `GET /repos/{owner}/{repo}`.
    Repository,
    /// `GET /repos/{owner}/{repo}/branches/{branch}`.
    BranchHead,
    /// `GET /repos/{owner}/{repo}/git/commits/{sha}`.
    Commit,
    /// `GET /repos/{owner}/{repo}/git/trees/{sha}?recursive=1`.
    Tree,
    /// `GET /repos/{owner}/{repo}/git/blobs/{sha}` as raw bytes.
    Blob,
    /// `GET /repos/{owner}/{repo}/tarball/{sha}` and its redirect hop.
    Archive,
    /// `GET /repos/{owner}/{repo}/pulls/{number}`.
    PullRequest,
    /// `GET /repos/{owner}/{repo}/pulls/{number}/files`.
    PullRequestFiles,
}

impl Operation {
    /// Every operation in accounting order.
    const ALL: [Operation; 8] = [
        Operation::Repository,
        Operation::BranchHead,
        Operation::Commit,
        Operation::Tree,
        Operation::Blob,
        Operation::Archive,
        Operation::PullRequest,
        Operation::PullRequestFiles,
    ];
}

/// HTTP requests sent per operation, counting retries and redirect hops.
#[derive(Debug, Default)]
pub struct CallCounts([AtomicU64; 8]);

impl CallCounts {
    /// Record one request for `operation`.
    fn record(&self, operation: Operation) {
        self.0[operation as usize].fetch_add(1, Ordering::Relaxed);
    }

    /// Requests sent for `operation`.
    pub fn get(&self, operation: Operation) -> u64 {
        self.0[operation as usize].load(Ordering::Relaxed)
    }

    /// Requests sent per operation, omitting zero entries.
    pub fn snapshot(&self) -> BTreeMap<Operation, u64> {
        Operation::ALL
            .into_iter()
            .map(|operation| (operation, self.get(operation)))
            .filter(|(_, count)| *count > 0)
            .collect()
    }
}

/// Whether a failed operation may be retried by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Retrying the whole operation later may succeed; `after` is the
    /// provider's requested wait when it gave one.
    Retry {
        /// Suggested wait before retrying.
        after: Option<Duration>,
    },
    /// Retrying with the same inputs and credentials cannot succeed.
    Terminal,
}

/// Why a provider operation failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GitHubError {
    /// The caller cancelled or the deadline passed.
    #[error(transparent)]
    Interrupted(#[from] Interrupted),
    /// A rate limit could not be waited out inside the deadline or the retry cap.
    #[error("rate limited; retry after {retry_after:?}")]
    RateLimited {
        /// Wait the provider requested.
        retry_after: Duration,
    },
    /// Credentials were rejected.
    #[error("unauthorized")]
    Unauthorized,
    /// The credentials cannot access this resource.
    #[error("forbidden")]
    Forbidden,
    /// The resource does not exist or is invisible to these credentials.
    #[error("not found: {resource}")]
    NotFound {
        /// Request path without host.
        resource: String,
    },
    /// The provider kept failing with a server error.
    #[error("provider unavailable: status {status}")]
    Unavailable {
        /// Last status observed.
        status: u16,
    },
    /// No response arrived after every attempt.
    #[error(transparent)]
    Transport(TransportError),
    /// A status the operation does not accept.
    #[error("unexpected status {status} for {resource}")]
    UnexpectedStatus {
        /// Status code.
        status: u16,
        /// Request path without host.
        resource: String,
    },
    /// The body was not the expected shape.
    #[error("malformed response for {resource}: {detail}")]
    MalformedResponse {
        /// Request path without host.
        resource: String,
        /// Parser or validation detail; never body text.
        detail: String,
    },
    /// The body exceeded a configured byte limit.
    #[error("response for {resource} exceeds {limit} bytes")]
    ResponseTooLarge {
        /// Request path without host.
        resource: String,
        /// Limit that was exceeded.
        limit: u64,
    },
    /// The provider answered for a different repository, ref, or object.
    #[error("identity mismatch: expected {expected}, provider returned {actual}")]
    IdentityMismatch {
        /// What the request asked for.
        expected: String,
        /// What the response carried.
        actual: String,
    },
    /// An archive redirect pointed outside the trusted archive host.
    #[error("untrusted archive redirect to {location}")]
    UntrustedRedirect {
        /// Redirect target.
        location: String,
    },
}

impl GitHubError {
    /// Whether the caller may retry the whole operation.
    pub fn disposition(&self) -> Disposition {
        match self {
            GitHubError::RateLimited { retry_after } => Disposition::Retry {
                after: Some(*retry_after),
            },
            GitHubError::Interrupted(Interrupted::DeadlineExceeded)
            | GitHubError::Unavailable { .. }
            | GitHubError::Transport(_) => Disposition::Retry { after: None },
            _ => Disposition::Terminal,
        }
    }
}

/// A repository as the provider describes it, verified against the requested identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    /// Canonical `owner/name`.
    pub full_name: String,
    /// Default branch name.
    pub default_branch: String,
    /// Whether the repository is private.
    pub private: bool,
}

/// A branch's pinned commit and tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchHead {
    /// Branch name as requested.
    pub name: String,
    /// Commit at the branch tip.
    pub commit: ObjectSha,
    /// Root tree of that commit.
    pub tree: ObjectSha,
}

/// A commit's root tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// Commit id as requested.
    pub sha: ObjectSha,
    /// Root tree.
    pub tree: ObjectSha,
}

/// Git tree entry mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeEntryMode {
    /// `100644` regular file.
    File,
    /// `100755` executable file.
    Executable,
    /// `120000` symbolic link.
    Symlink,
    /// `040000` directory.
    Directory,
    /// `160000` submodule commit.
    Submodule,
}

impl TreeEntryMode {
    /// Parse the provider's octal mode text.
    fn parse(mode: &str) -> Option<TreeEntryMode> {
        match mode {
            "100644" => Some(TreeEntryMode::File),
            "100755" => Some(TreeEntryMode::Executable),
            "120000" => Some(TreeEntryMode::Symlink),
            "040000" => Some(TreeEntryMode::Directory),
            "160000" => Some(TreeEntryMode::Submodule),
            _ => None,
        }
    }
}

/// One validated entry of a recursive tree listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    /// Repository-relative path.
    pub path: RelativePath,
    /// Entry mode.
    pub mode: TreeEntryMode,
    /// Object id of the blob, tree, or submodule commit.
    pub sha: ObjectSha,
    /// Blob size when the provider reports it.
    pub size: Option<u64>,
}

/// A provider entry that could not become a trusted [`TreeEntry`] or PR file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedPath {
    /// Raw path text as reported; never used as an identity.
    pub path: String,
    /// Why it was rejected.
    pub reason: String,
}

/// A recursive tree listing verified against the requested tree id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeListing {
    /// Tree id as requested.
    pub sha: ObjectSha,
    /// Whether the provider truncated the listing.
    pub truncated: bool,
    /// Entries with valid paths and modes.
    pub entries: Vec<TreeEntry>,
    /// Entries dropped for an invalid path, mode, or id.
    pub rejected: Vec<RejectedPath>,
}

/// Provider pull-request state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestState {
    /// Open.
    Open,
    /// Closed, whether or not merged.
    Closed,
}

/// A pull request's pinned base and head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    /// Number as requested.
    pub number: u64,
    /// Open or closed.
    pub state: PullRequestState,
    /// Draft flag.
    pub draft: bool,
    /// Whether the PR was merged.
    pub merged: bool,
    /// Base branch name.
    pub base_ref: String,
    /// Base commit.
    pub base: ObjectSha,
    /// Head branch name.
    pub head_ref: String,
    /// Head commit.
    pub head: ObjectSha,
    /// Numeric id of the head repository, absent when the fork was deleted.
    pub head_repository_id: Option<u64>,
}

/// Provider change status for one PR file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// Added.
    Added,
    /// Removed.
    Removed,
    /// Modified.
    Modified,
    /// Renamed.
    Renamed,
    /// Copied.
    Copied,
    /// Mode or similar change.
    Changed,
    /// Unchanged.
    Unchanged,
}

impl FileStatus {
    /// Parse the provider's status text.
    fn parse(status: &str) -> Option<FileStatus> {
        match status {
            "added" => Some(FileStatus::Added),
            "removed" => Some(FileStatus::Removed),
            "modified" => Some(FileStatus::Modified),
            "renamed" => Some(FileStatus::Renamed),
            "copied" => Some(FileStatus::Copied),
            "changed" => Some(FileStatus::Changed),
            "unchanged" => Some(FileStatus::Unchanged),
            _ => None,
        }
    }
}

/// One file of a pull request as the provider lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestFile {
    /// Head-side path.
    pub path: RelativePath,
    /// Change status.
    pub status: FileStatus,
    /// Blob id the provider reports for the file.
    pub sha: ObjectSha,
    /// Unified patch text when the provider included one.
    pub patch: Option<String>,
    /// Previous path for renames and copies.
    pub previous_path: Option<RelativePath>,
}

/// One page of PR files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestFilesPage {
    /// Files with valid paths, statuses, and ids.
    pub files: Vec<PullRequestFile>,
    /// Files dropped for an invalid path, status, or id.
    pub rejected: Vec<RejectedPath>,
    /// Whether the provider advertised a next page.
    pub has_next: bool,
}

/// A streaming, byte-bounded compressed archive body.
pub struct Archive {
    /// Compressed tarball bytes; reading past the archive limit fails.
    pub reader: Box<dyn Read + Send>,
}

/// Reader that fails instead of truncating once `limit` bytes were read.
struct BoundedReader<R> {
    inner: R,
    remaining: u64,
    resource: String,
}

impl<R: Read> Read for BoundedReader<R> {
    /// Reads at most the remaining budget; a further byte is an error.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            let mut probe = [0u8; 1];
            return match self.inner.read(&mut probe)? {
                0 => Ok(0),
                _ => Err(io::Error::other(format!(
                    "{} exceeds archive byte limit",
                    self.resource
                ))),
            };
        }
        let want = usize::try_from(self.remaining.min(buf.len() as u64)).unwrap_or(buf.len());
        let read = self.inner.read(&mut buf[..want])?;
        self.remaining -= read as u64;
        Ok(read)
    }
}

/// Wire shape of `GET /repos/{owner}/{repo}`.
#[derive(Deserialize)]
struct RepositoryWire {
    id: u64,
    full_name: String,
    default_branch: String,
    private: bool,
}

/// Wire shape of `GET /repos/{owner}/{repo}/branches/{branch}`.
#[derive(Deserialize)]
struct BranchWire {
    name: String,
    commit: BranchCommitWire,
}

/// Commit object inside a branch response.
#[derive(Deserialize)]
struct BranchCommitWire {
    sha: ObjectSha,
    commit: CommitBodyWire,
}

/// Inner commit body carrying the tree.
#[derive(Deserialize)]
struct CommitBodyWire {
    tree: ShaWire,
}

/// Wire shape of `GET /repos/{owner}/{repo}/git/commits/{sha}`.
#[derive(Deserialize)]
struct CommitWire {
    sha: ObjectSha,
    tree: ShaWire,
}

/// Any object reference carrying only a `sha`.
#[derive(Deserialize)]
struct ShaWire {
    sha: ObjectSha,
}

/// Wire shape of a recursive tree listing.
#[derive(Deserialize)]
struct TreeWire {
    sha: ObjectSha,
    truncated: bool,
    tree: Vec<TreeEntryWire>,
}

/// One raw tree entry; validated into [`TreeEntry`] or [`RejectedPath`].
#[derive(Deserialize)]
struct TreeEntryWire {
    path: String,
    mode: String,
    sha: String,
    size: Option<u64>,
}

/// Wire shape of `GET /repos/{owner}/{repo}/pulls/{number}`.
#[derive(Deserialize)]
struct PullRequestWire {
    number: u64,
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    merged: bool,
    base: PullRefWire,
    head: PullRefWire,
}

/// Base or head reference of a pull request.
#[derive(Deserialize)]
struct PullRefWire {
    #[serde(rename = "ref")]
    name: String,
    sha: ObjectSha,
    repo: Option<PullRepoWire>,
}

/// Repository carried by a PR reference.
#[derive(Deserialize)]
struct PullRepoWire {
    id: u64,
}

/// One raw PR file; validated into [`PullRequestFile`] or [`RejectedPath`].
#[derive(Deserialize)]
struct PullRequestFileWire {
    filename: String,
    status: String,
    sha: String,
    patch: Option<String>,
    previous_filename: Option<String>,
}

/// Percent-encode a path segment, keeping `/` so branch names route unchanged.
fn encode_segment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Git blob id of `bytes`: SHA-1 over `blob <len>\0<bytes>`.
fn git_blob_sha(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", bytes.len()).as_bytes());
    hasher.update(bytes);
    hex(&hasher.finalize())
}

/// Seconds since the Unix epoch.
fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// Wait the provider requested for a 403/429, if it signalled a rate limit.
fn rate_limit_wait(response: &HttpResponse) -> Option<Duration> {
    if let Some(seconds) = response
        .header("retry-after")
        .and_then(|value| value.trim().parse::<u64>().ok())
    {
        return Some(Duration::from_secs(seconds));
    }
    let exhausted = response
        .header("x-ratelimit-remaining")
        .and_then(|value| value.trim().parse::<u64>().ok())
        == Some(0);
    if !exhausted {
        return None;
    }
    let reset = response
        .header("x-ratelimit-reset")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0);
    Some(Duration::from_secs(
        reset.saturating_sub(now_epoch_seconds()),
    ))
}

/// Whether a `Location` is `https://<archive_host>/...`.
fn is_trusted_redirect(location: &str, archive_host: &str) -> bool {
    location
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|host| host.eq_ignore_ascii_case(archive_host))
}

/// Authenticated, bounded client for one GitHub host and installation.
///
/// The client is stateless apart from call accounting, so one instance may
/// serve many invocations, each bounded by its own [`Execution`].
pub struct GitHubClient {
    host: GitHubHost,
    token: InstallationToken,
    transport: Box<dyn HttpTransport>,
    limits: RequestLimits,
    calls: CallCounts,
}

impl GitHubClient {
    /// Client over the production `ureq` transport.
    pub fn new(host: GitHubHost, token: InstallationToken, limits: RequestLimits) -> GitHubClient {
        GitHubClient::with_transport(host, token, limits, Box::new(UreqTransport::new()))
    }

    /// Client over an injected transport, for tests and consumers with their own HTTP stack.
    pub fn with_transport(
        host: GitHubHost,
        token: InstallationToken,
        limits: RequestLimits,
        transport: Box<dyn HttpTransport>,
    ) -> GitHubClient {
        GitHubClient {
            host,
            token,
            transport,
            limits,
            calls: CallCounts::default(),
        }
    }

    /// Requests sent so far, per operation.
    pub fn calls(&self) -> &CallCounts {
        &self.calls
    }

    /// Fetch and verify the repository named by `repository`.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the provider's numeric id or
    /// `full_name` differs, plus any policy or transport error.
    pub fn repository(
        &self,
        execution: &Execution,
        repository: &RepositoryIdentity,
    ) -> Result<Repository, GitHubError> {
        let resource = format!("/repos/{}", repository.full_name());
        let wire: RepositoryWire =
            self.get_json(execution, Operation::Repository, &resource, ACCEPT_JSON)?;
        if wire.id != repository.id()
            || !wire.full_name.eq_ignore_ascii_case(&repository.full_name())
        {
            return Err(GitHubError::IdentityMismatch {
                expected: format!("{} (id {})", repository.full_name(), repository.id()),
                actual: format!("{} (id {})", wire.full_name, wire.id),
            });
        }
        Ok(Repository {
            full_name: wire.full_name,
            default_branch: wire.default_branch,
            private: wire.private,
        })
    }

    /// Pin `branch` to its current commit and tree.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the provider names another
    /// branch, plus any policy or transport error.
    pub fn branch_head(
        &self,
        execution: &Execution,
        repository: &RepositoryIdentity,
        branch: &str,
    ) -> Result<BranchHead, GitHubError> {
        let resource = format!(
            "/repos/{}/branches/{}",
            repository.full_name(),
            encode_segment(branch)
        );
        let wire: BranchWire =
            self.get_json(execution, Operation::BranchHead, &resource, ACCEPT_JSON)?;
        if wire.name != branch {
            return Err(GitHubError::IdentityMismatch {
                expected: branch.to_owned(),
                actual: wire.name,
            });
        }
        Ok(BranchHead {
            name: wire.name,
            commit: wire.commit.sha,
            tree: wire.commit.commit.tree.sha,
        })
    }

    /// Resolve `sha` to its root tree.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the provider returns another
    /// commit id, plus any policy or transport error.
    pub fn commit(
        &self,
        execution: &Execution,
        repository: &RepositoryIdentity,
        sha: &ObjectSha,
    ) -> Result<Commit, GitHubError> {
        let resource = format!("/repos/{}/git/commits/{sha}", repository.full_name());
        let wire: CommitWire =
            self.get_json(execution, Operation::Commit, &resource, ACCEPT_JSON)?;
        if wire.sha != *sha {
            return Err(GitHubError::IdentityMismatch {
                expected: sha.to_string(),
                actual: wire.sha.to_string(),
            });
        }
        Ok(Commit {
            sha: wire.sha,
            tree: wire.tree.sha,
        })
    }

    /// List `tree` recursively, validating every path and mode.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the provider returns another
    /// tree id, plus any policy or transport error. Individual bad entries
    /// are reported in [`TreeListing::rejected`] rather than failing the call.
    pub fn tree(
        &self,
        execution: &Execution,
        repository: &RepositoryIdentity,
        tree: &ObjectSha,
    ) -> Result<TreeListing, GitHubError> {
        let resource = format!(
            "/repos/{}/git/trees/{tree}?recursive=1",
            repository.full_name()
        );
        let wire: TreeWire = self.get_json(execution, Operation::Tree, &resource, ACCEPT_JSON)?;
        if wire.sha != *tree {
            return Err(GitHubError::IdentityMismatch {
                expected: tree.to_string(),
                actual: wire.sha.to_string(),
            });
        }
        let mut entries = Vec::with_capacity(wire.tree.len());
        let mut rejected = Vec::new();
        for entry in wire.tree {
            match validate_tree_entry(&entry) {
                Ok(valid) => entries.push(valid),
                Err(reason) => rejected.push(RejectedPath {
                    path: entry.path,
                    reason,
                }),
            }
        }
        Ok(TreeListing {
            sha: wire.sha,
            truncated: wire.truncated,
            entries,
            rejected,
        })
    }

    /// Fetch blob `sha` as raw bytes and verify its Git id.
    ///
    /// # Errors
    ///
    /// [`GitHubError::ResponseTooLarge`] past `max_blob_bytes`,
    /// [`GitHubError::IdentityMismatch`] when the bytes do not hash to `sha`,
    /// plus any policy or transport error.
    pub fn blob(
        &self,
        execution: &Execution,
        repository: &RepositoryIdentity,
        sha: &ObjectSha,
    ) -> Result<Vec<u8>, GitHubError> {
        let resource = format!("/repos/{}/git/blobs/{sha}", repository.full_name());
        let mut response = self.request(execution, Operation::Blob, &resource, ACCEPT_RAW, true)?;
        expect_status(&response, 200, &resource)?;
        let bytes = read_bounded(&mut response.body, self.limits.max_blob_bytes, &resource)?;
        let actual = git_blob_sha(&bytes);
        if actual != sha.as_str() {
            return Err(GitHubError::IdentityMismatch {
                expected: sha.to_string(),
                actual,
            });
        }
        Ok(bytes)
    }

    /// Open the commit-pinned tarball stream for `commit`.
    ///
    /// The API request carries credentials; a redirect is followed once,
    /// without credentials, and only to the host's archive host.
    ///
    /// # Errors
    ///
    /// [`GitHubError::UntrustedRedirect`] for any other redirect target,
    /// [`GitHubError::UnexpectedStatus`] when neither hop yields the body,
    /// plus any policy or transport error. Reading the returned stream past
    /// `max_archive_bytes` fails with an IO error.
    pub fn archive(
        &self,
        execution: &Execution,
        repository: &RepositoryIdentity,
        commit: &ObjectSha,
    ) -> Result<Archive, GitHubError> {
        let resource = format!("/repos/{}/tarball/{commit}", repository.full_name());
        let first = self.request(execution, Operation::Archive, &resource, ACCEPT_JSON, true)?;
        let response = match first.status {
            200 => first,
            301..=303 | 307 | 308 => {
                let location = first.header("location").unwrap_or("").to_owned();
                if !is_trusted_redirect(&location, self.host.archive_host()) {
                    return Err(GitHubError::UntrustedRedirect { location });
                }
                let hop = self.send_with_policy(
                    execution,
                    Operation::Archive,
                    &location,
                    ACCEPT_JSON,
                    false,
                )?;
                expect_status(&hop, 200, &resource)?;
                hop
            }
            _ => return Err(status_error(&first, &resource)),
        };
        Ok(Archive {
            reader: Box::new(BoundedReader {
                inner: response.body,
                remaining: self.limits.max_archive_bytes,
                resource,
            }),
        })
    }

    /// Fetch pull request `number` and verify its number and base repository.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the number or base repository
    /// id differs, [`GitHubError::MalformedResponse`] for an unknown state,
    /// plus any policy or transport error.
    pub fn pull_request(
        &self,
        execution: &Execution,
        repository: &RepositoryIdentity,
        number: u64,
    ) -> Result<PullRequest, GitHubError> {
        let resource = format!("/repos/{}/pulls/{number}", repository.full_name());
        let wire: PullRequestWire =
            self.get_json(execution, Operation::PullRequest, &resource, ACCEPT_JSON)?;
        let base_repo = wire.base.repo.as_ref().map(|repo| repo.id);
        if wire.number != number || base_repo != Some(repository.id()) {
            return Err(GitHubError::IdentityMismatch {
                expected: format!("PR #{number} in repository {}", repository.id()),
                actual: format!("PR #{} in repository {:?}", wire.number, base_repo),
            });
        }
        let state = match wire.state.as_str() {
            "open" => PullRequestState::Open,
            "closed" => PullRequestState::Closed,
            other => {
                return Err(GitHubError::MalformedResponse {
                    resource,
                    detail: format!("unknown pull request state `{other}`"),
                });
            }
        };
        Ok(PullRequest {
            number: wire.number,
            state,
            draft: wire.draft,
            merged: wire.merged,
            base_ref: wire.base.name,
            base: wire.base.sha,
            head_ref: wire.head.name,
            head: wire.head.sha,
            head_repository_id: wire.head.repo.map(|repo| repo.id),
        })
    }

    /// Fetch one page (1-based) of pull request `number`'s files.
    ///
    /// # Errors
    ///
    /// Any policy or transport error. Individual bad files are reported in
    /// [`PullRequestFilesPage::rejected`] rather than failing the page.
    pub fn pull_request_files(
        &self,
        execution: &Execution,
        repository: &RepositoryIdentity,
        number: u64,
        page: u32,
    ) -> Result<PullRequestFilesPage, GitHubError> {
        let resource = format!(
            "/repos/{}/pulls/{number}/files?per_page={PR_FILES_PER_PAGE}&page={page}",
            repository.full_name()
        );
        let mut response = self.request(
            execution,
            Operation::PullRequestFiles,
            &resource,
            ACCEPT_JSON,
            true,
        )?;
        expect_status(&response, 200, &resource)?;
        let has_next = response
            .header("link")
            .is_some_and(|link| link.contains("rel=\"next\""));
        let bytes = read_bounded(&mut response.body, self.limits.max_json_bytes, &resource)?;
        let wire: Vec<PullRequestFileWire> = parse_json(&bytes, &resource)?;
        let mut files = Vec::with_capacity(wire.len());
        let mut rejected = Vec::new();
        for file in wire {
            match validate_pull_request_file(&file) {
                Ok(valid) => files.push(valid),
                Err(reason) => rejected.push(RejectedPath {
                    path: file.filename,
                    reason,
                }),
            }
        }
        Ok(PullRequestFilesPage {
            files,
            rejected,
            has_next,
        })
    }

    /// GET `resource` from the API host, require 200, and parse bounded JSON.
    fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        execution: &Execution,
        operation: Operation,
        resource: &str,
        accept: &str,
    ) -> Result<T, GitHubError> {
        let mut response = self.request(execution, operation, resource, accept, true)?;
        expect_status(&response, 200, resource)?;
        let bytes = read_bounded(&mut response.body, self.limits.max_json_bytes, resource)?;
        parse_json(&bytes, resource)
    }

    /// Send `resource` against the API base under the retry policy.
    fn request(
        &self,
        execution: &Execution,
        operation: Operation,
        resource: &str,
        accept: &str,
        authenticated: bool,
    ) -> Result<HttpResponse, GitHubError> {
        let url = format!("{}{resource}", self.host.api_base());
        self.send_with_policy(execution, operation, &url, accept, authenticated)
    }

    /// Build one request for `url`.
    fn build(
        &self,
        url: &str,
        accept: &str,
        authenticated: bool,
        timeout: Duration,
    ) -> HttpRequest {
        let mut headers = vec![
            ("Accept", accept.to_owned()),
            ("X-GitHub-Api-Version", API_VERSION.to_owned()),
            ("User-Agent", USER_AGENT.to_owned()),
        ];
        if authenticated {
            headers.push(("Authorization", format!("Bearer {}", self.token.expose())));
        }
        HttpRequest {
            url: url.to_owned(),
            headers,
            timeout,
        }
    }

    /// The single request policy: check the execution bound, send, and
    /// retry rate limits, server errors, and transport failures within
    /// `max_attempts` and the remaining deadline.
    fn send_with_policy(
        &self,
        execution: &Execution,
        operation: Operation,
        url: &str,
        accept: &str,
        authenticated: bool,
    ) -> Result<HttpResponse, GitHubError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let remaining = execution.check()?;
            let timeout = remaining.min(self.limits.request_timeout);
            self.calls.record(operation);
            let outcome = self
                .transport
                .get(self.build(url, accept, authenticated, timeout));
            let backoff = Duration::from_secs(attempt.into()).min(self.limits.max_retry_wait);
            let (wait, failure) = match outcome {
                Ok(response) if matches!(response.status, 403 | 429) => {
                    match rate_limit_wait(&response) {
                        Some(wait) => (wait, GitHubError::RateLimited { retry_after: wait }),
                        None if response.status == 403 => return Err(GitHubError::Forbidden),
                        None => {
                            return Err(GitHubError::RateLimited {
                                retry_after: self.limits.max_retry_wait,
                            });
                        }
                    }
                }
                Ok(response) if response.status >= 500 => (
                    backoff,
                    GitHubError::Unavailable {
                        status: response.status,
                    },
                ),
                Ok(response) => return Ok(response),
                Err(error) => (backoff, GitHubError::Transport(error)),
            };
            let too_long = wait > self.limits.max_retry_wait
                || execution.remaining().is_none_or(|left| wait >= left);
            if attempt >= self.limits.max_attempts || too_long {
                return Err(failure);
            }
            sleep_bounded(execution, wait)?;
        }
    }
}

/// Sleep for `wait` in slices, stopping early on cancellation.
fn sleep_bounded(execution: &Execution, wait: Duration) -> Result<(), Interrupted> {
    let mut left = wait;
    while !left.is_zero() {
        execution.check()?;
        let slice = left.min(SLEEP_SLICE);
        thread::sleep(slice);
        left -= slice;
    }
    Ok(())
}

/// Map a non-success status to its typed error.
fn status_error(response: &HttpResponse, resource: &str) -> GitHubError {
    match response.status {
        401 => GitHubError::Unauthorized,
        403 => GitHubError::Forbidden,
        404 => GitHubError::NotFound {
            resource: resource.to_owned(),
        },
        status => GitHubError::UnexpectedStatus {
            status,
            resource: resource.to_owned(),
        },
    }
}

/// Require `expected` status.
fn expect_status(
    response: &HttpResponse,
    expected: u16,
    resource: &str,
) -> Result<(), GitHubError> {
    if response.status == expected {
        Ok(())
    } else {
        Err(status_error(response, resource))
    }
}

/// Read a body of at most `limit` bytes.
fn read_bounded(body: &mut dyn Read, limit: usize, resource: &str) -> Result<Vec<u8>, GitHubError> {
    let mut bytes = Vec::new();
    body.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| GitHubError::Transport(TransportError::Failed(error.to_string())))?;
    if bytes.len() > limit {
        return Err(GitHubError::ResponseTooLarge {
            resource: resource.to_owned(),
            limit: limit as u64,
        });
    }
    Ok(bytes)
}

/// Parse JSON without echoing the body into the error.
fn parse_json<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    resource: &str,
) -> Result<T, GitHubError> {
    serde_json::from_slice(bytes).map_err(|error| GitHubError::MalformedResponse {
        resource: resource.to_owned(),
        detail: format!(
            "{} at line {} column {}",
            json_error_category(&error),
            error.line(),
            error.column()
        ),
    })
}

/// Serde error category as a fixed label, never payload text.
fn json_error_category(error: &serde_json::Error) -> &'static str {
    match error.classify() {
        serde_json::error::Category::Io => "io error",
        serde_json::error::Category::Syntax => "invalid json",
        serde_json::error::Category::Data => "unexpected json shape",
        serde_json::error::Category::Eof => "truncated json",
    }
}

/// Validate a raw tree entry.
fn validate_tree_entry(entry: &TreeEntryWire) -> Result<TreeEntry, String> {
    let path =
        RelativePath::new(&entry.path).map_err(|error: InvalidRelativePath| error.to_string())?;
    let mode =
        TreeEntryMode::parse(&entry.mode).ok_or_else(|| format!("unknown mode {}", entry.mode))?;
    let sha = ObjectSha::new(&entry.sha).map_err(|error| error.to_string())?;
    Ok(TreeEntry {
        path,
        mode,
        sha,
        size: entry.size,
    })
}

/// Validate a raw PR file.
fn validate_pull_request_file(file: &PullRequestFileWire) -> Result<PullRequestFile, String> {
    let path = RelativePath::new(&file.filename).map_err(|error| error.to_string())?;
    let status =
        FileStatus::parse(&file.status).ok_or_else(|| format!("unknown status {}", file.status))?;
    let sha = ObjectSha::new(&file.sha).map_err(|error| error.to_string())?;
    let previous_path = file
        .previous_filename
        .as_deref()
        .map(RelativePath::new)
        .transpose()
        .map_err(|error| format!("previous path: {error}"))?;
    Ok(PullRequestFile {
        path,
        status,
        sha,
        patch: file.patch.clone(),
        previous_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::CancelToken;
    use std::collections::VecDeque;
    use std::io::Cursor;
    use std::sync::Mutex;
    use std::time::Instant;

    /// One request the fake observed, with its headers but never its token value.
    struct SeenRequest {
        url: String,
        authorized: bool,
        accept: String,
    }

    /// One scripted response: status, headers, body.
    type Scripted = Result<(u16, Vec<(String, String)>, Vec<u8>), TransportError>;

    /// Scripted transport: answers requests in order and records what was sent.
    #[derive(Default)]
    struct FakeTransport {
        responses: Mutex<VecDeque<Scripted>>,
        seen: Mutex<Vec<SeenRequest>>,
    }

    impl FakeTransport {
        /// Queue a response.
        fn push(&self, status: u16, headers: &[(&str, &str)], body: impl Into<Vec<u8>>) {
            self.responses
                .lock()
                .expect("responses lock")
                .push_back(Ok((
                    status,
                    headers
                        .iter()
                        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                        .collect(),
                    body.into(),
                )));
        }

        /// Queue a JSON 200.
        fn push_json(&self, json: &str) {
            self.push(200, &[], json);
        }

        /// Queue a transport failure.
        fn push_failure(&self, error: TransportError) {
            self.responses
                .lock()
                .expect("responses lock")
                .push_back(Err(error));
        }

        /// Requests seen so far.
        fn seen(&self) -> Vec<(String, bool, String)> {
            self.seen
                .lock()
                .expect("seen lock")
                .iter()
                .map(|seen| (seen.url.clone(), seen.authorized, seen.accept.clone()))
                .collect()
        }
    }

    impl HttpTransport for FakeTransport {
        /// Pops the next scripted response; an exhausted script is a test bug.
        fn get(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            self.seen.lock().expect("seen lock").push(SeenRequest {
                url: request.url.clone(),
                authorized: request.header("authorization") == Some("Bearer ghs_token"),
                accept: request.header("accept").unwrap_or("").to_owned(),
            });
            let (status, headers, body) = self
                .responses
                .lock()
                .expect("responses lock")
                .pop_front()
                .expect("scripted response for every request")?;
            Ok(HttpResponse {
                status,
                headers,
                body: Box::new(Cursor::new(body)),
            })
        }
    }

    /// Client over a fresh fake for `host`.
    fn client_on(
        host: GitHubHost,
        limits: RequestLimits,
    ) -> (GitHubClient, &'static FakeTransport) {
        let fake: &'static FakeTransport = Box::leak(Box::default());
        let token = InstallationToken::new("ghs_token").expect("token");
        let client = GitHubClient::with_transport(host, token, limits, Box::new(FakeRef(fake)));
        (client, fake)
    }

    /// Transport wrapper so a test keeps its handle to the leaked fake.
    struct FakeRef(&'static FakeTransport);

    impl HttpTransport for FakeRef {
        /// Delegates to the shared fake.
        fn get(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            self.0.get(request)
        }
    }

    /// Public-host client with default limits.
    fn client() -> (GitHubClient, &'static FakeTransport) {
        client_on(GitHubHost::Public, RequestLimits::default())
    }

    /// A one-minute execution bound.
    fn execution() -> Execution {
        Execution::with_timeout(Duration::from_secs(60))
    }

    /// The pinned test repository.
    fn repo() -> RepositoryIdentity {
        RepositoryIdentity::new(1296269, "octocat", "Hello-World").expect("repo")
    }

    /// Forty copies of `ch` as an object id.
    fn sha(ch: char) -> ObjectSha {
        ObjectSha::new(&ch.to_string().repeat(40)).expect("sha")
    }

    /// Scenario 1: pinned repository, branch, commit, tree, blob, and PR data
    /// come back verified; mismatched repository, commit, tree, or blob
    /// identity is rejected before any bytes are returned.
    #[test]
    fn verified_provider_identities_cross_and_mismatches_are_rejected() {
        let (client, fake) = client();
        let execution = execution();
        let repo = repo();
        let commit = sha('c');
        let tree = sha('7');
        let blob_bytes = b"hello\n";
        let blob = ObjectSha::new(&git_blob_sha(blob_bytes)).expect("blob sha");

        fake.push_json(
            r#"{"id":1296269,"full_name":"octocat/Hello-World","default_branch":"main","private":true}"#,
        );
        let repository = client.repository(&execution, &repo).expect("repository");
        assert_eq!(
            repository,
            Repository {
                full_name: "octocat/Hello-World".into(),
                default_branch: "main".into(),
                private: true
            }
        );

        fake.push_json(&format!(
            r#"{{"name":"main","commit":{{"sha":"{commit}","commit":{{"tree":{{"sha":"{tree}"}}}}}}}}"#
        ));
        let head = client
            .branch_head(&execution, &repo, "main")
            .expect("branch");
        assert_eq!((&head.commit, &head.tree), (&commit, &tree));

        fake.push_json(&format!(
            r#"{{"sha":"{commit}","tree":{{"sha":"{tree}"}}}}"#
        ));
        assert_eq!(
            client
                .commit(&execution, &repo, &commit)
                .expect("commit")
                .tree,
            tree
        );

        fake.push_json(&format!(
            r#"{{"sha":"{tree}","truncated":false,"tree":[
                {{"path":"src/lib.rs","mode":"100644","type":"blob","sha":"{blob}","size":6}},
                {{"path":"../escape","mode":"100644","type":"blob","sha":"{blob}"}},
                {{"path":"weird","mode":"777777","type":"blob","sha":"{blob}"}},
                {{"path":"src","mode":"040000","type":"tree","sha":"{tree}"}}
            ]}}"#
        ));
        let listing = client.tree(&execution, &repo, &tree).expect("tree");
        assert!(!listing.truncated);
        assert_eq!(listing.entries.len(), 2);
        assert_eq!(listing.entries[0].path.as_str(), "src/lib.rs");
        assert_eq!(listing.entries[0].mode, TreeEntryMode::File);
        assert_eq!(listing.entries[0].size, Some(6));
        assert_eq!(listing.entries[1].mode, TreeEntryMode::Directory);
        assert_eq!(
            listing
                .rejected
                .iter()
                .map(|r| r.path.as_str())
                .collect::<Vec<_>>(),
            ["../escape", "weird"]
        );

        fake.push(200, &[], blob_bytes.to_vec());
        assert_eq!(
            client.blob(&execution, &repo, &blob).expect("blob"),
            blob_bytes
        );
        assert_eq!(fake.seen().last().map(|s| s.2.as_str()), Some(ACCEPT_RAW));

        fake.push_json(&format!(
            r#"{{"number":42,"state":"open","draft":false,"merged":false,
                "base":{{"ref":"main","sha":"{commit}","repo":{{"id":1296269}}}},
                "head":{{"ref":"feature","sha":"{tree}","repo":{{"id":99}}}}}}"#
        ));
        let pr = client.pull_request(&execution, &repo, 42).expect("pr");
        assert_eq!(
            (pr.state, pr.base.clone(), pr.head_repository_id),
            (PullRequestState::Open, commit.clone(), Some(99))
        );

        fake.push(
            200,
            &[("link", "<https://api.github.com/x?page=2>; rel=\"next\"")],
            format!(
                r#"[{{"filename":"src/lib.rs","status":"modified","sha":"{blob}","patch":"@@ -1 +1 @@"}},
                    {{"filename":"/abs","status":"added","sha":"{blob}"}},
                    {{"filename":"src/x.rs","status":"teleported","sha":"{blob}"}}]"#
            ),
        );
        let page = client
            .pull_request_files(&execution, &repo, 42, 1)
            .expect("files");
        assert!(page.has_next);
        assert_eq!(page.files.len(), 1);
        assert_eq!(page.files[0].patch.as_deref(), Some("@@ -1 +1 @@"));
        assert_eq!(page.rejected.len(), 2);

        // Every request so far went to the API host with credentials.
        assert!(fake.seen().iter().all(|(url, authorized, _)| {
            url.starts_with("https://api.github.com/repos/octocat/Hello-World") && *authorized
        }));

        // Mismatched identities are rejected before any data is returned.
        fake.push_json(
            r#"{"id":1,"full_name":"octocat/Hello-World","default_branch":"main","private":true}"#,
        );
        assert!(matches!(
            client.repository(&execution, &repo),
            Err(GitHubError::IdentityMismatch { .. })
        ));
        fake.push_json(&format!(r#"{{"sha":"{tree}","tree":{{"sha":"{tree}"}}}}"#));
        assert!(matches!(
            client.commit(&execution, &repo, &commit),
            Err(GitHubError::IdentityMismatch { .. })
        ));
        fake.push_json(&format!(
            r#"{{"sha":"{commit}","truncated":true,"tree":[]}}"#
        ));
        assert!(matches!(
            client.tree(&execution, &repo, &tree),
            Err(GitHubError::IdentityMismatch { .. })
        ));
        fake.push(200, &[], b"tampered\n".to_vec());
        assert!(matches!(
            client.blob(&execution, &repo, &blob),
            Err(GitHubError::IdentityMismatch { .. })
        ));
        fake.push_json(&format!(
            r#"{{"number":42,"state":"open","base":{{"ref":"main","sha":"{commit}","repo":{{"id":5}}}},"head":{{"ref":"f","sha":"{tree}","repo":null}}}}"#
        ));
        assert!(matches!(
            client.pull_request(&execution, &repo, 42),
            Err(GitHubError::IdentityMismatch { .. })
        ));
        fake.push_json(r#"{"name":"other","commit":{"sha":"cccccccccccccccccccccccccccccccccccccccc","commit":{"tree":{"sha":"7777777777777777777777777777777777777777"}}}}"#);
        assert!(matches!(
            client.branch_head(&execution, &repo, "main"),
            Err(GitHubError::IdentityMismatch { .. })
        ));

        let calls = client.calls().snapshot();
        assert_eq!(calls[&Operation::Repository], 2);
        assert_eq!(calls[&Operation::Blob], 2);
        assert_eq!(calls.values().sum::<u64>(), 13);
    }

    /// Scenario 1: an archive redirect is followed once, without credentials,
    /// only to the trusted archive host; the stream is byte-bounded.
    #[test]
    fn archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials() {
        let (client, fake) = client_on(
            GitHubHost::Public,
            RequestLimits {
                max_archive_bytes: 4,
                ..RequestLimits::default()
            },
        );
        let execution = execution();
        let repo = repo();
        let commit = sha('c');

        fake.push(
            302,
            &[(
                "location",
                "https://codeload.github.com/octocat/Hello-World/legacy.tar.gz/c?token=t",
            )],
            Vec::new(),
        );
        fake.push(200, &[], b"abcd".to_vec());
        let mut archive = client.archive(&execution, &repo, &commit).expect("archive");
        let mut bytes = Vec::new();
        archive
            .reader
            .read_to_end(&mut bytes)
            .expect("within limit");
        assert_eq!(bytes, b"abcd");
        let seen = fake.seen();
        assert_eq!(
            seen[0].0,
            "https://api.github.com/repos/octocat/Hello-World/tarball/cccccccccccccccccccccccccccccccccccccccc"
        );
        assert!(seen[0].1, "API hop carries credentials");
        assert!(seen[1].0.starts_with("https://codeload.github.com/"));
        assert!(!seen[1].1, "redirect hop must not forward credentials");

        fake.push(
            302,
            &[("location", "https://evil.example.com/archive.tar.gz")],
            Vec::new(),
        );
        assert_eq!(
            client.archive(&execution, &repo, &commit).err(),
            Some(GitHubError::UntrustedRedirect {
                location: "https://evil.example.com/archive.tar.gz".into()
            })
        );
        assert_eq!(fake.seen().len(), 3, "untrusted redirect is never followed");

        fake.push(
            302,
            &[("location", "https://codeload.github.com/x")],
            Vec::new(),
        );
        fake.push(200, &[], b"abcde".to_vec());
        let mut archive = client.archive(&execution, &repo, &commit).expect("archive");
        let error = archive
            .reader
            .read_to_end(&mut Vec::new())
            .expect_err("over limit");
        assert!(error.to_string().contains("exceeds archive byte limit"));

        let ghes = GitHubHost::enterprise("https://ghe.example.com/api/v3").expect("ghes");
        let (client, fake) = client_on(ghes, RequestLimits::default());
        fake.push(
            302,
            &[("location", "https://ghe.example.com/_codeload/x")],
            Vec::new(),
        );
        fake.push(200, &[], b"gz".to_vec());
        assert!(client.archive(&execution, &repo, &commit).is_ok());
        assert_eq!(
            fake.seen()[0].0,
            "https://ghe.example.com/api/v3/repos/octocat/Hello-World/tarball/cccccccccccccccccccccccccccccccccccccccc"
        );
        fake.push(
            302,
            &[("location", "https://codeload.github.com/x")],
            Vec::new(),
        );
        assert!(matches!(
            client.archive(&execution, &repo, &commit),
            Err(GitHubError::UntrustedRedirect { .. })
        ));
        assert_eq!(client.calls().get(Operation::Archive), 3);
    }

    /// Scenario 2: rate limits with a short wait are retried once under one
    /// policy; long waits, exhausted attempts, and plain 403s are typed.
    #[test]
    fn rate_limits_retry_within_bounds_or_report_typed_outcomes() {
        let (client, fake) = client_on(
            GitHubHost::Public,
            RequestLimits {
                max_attempts: 2,
                max_retry_wait: Duration::from_secs(5),
                ..RequestLimits::default()
            },
        );
        let execution = execution();
        let repo = repo();
        let ok = r#"{"id":1296269,"full_name":"octocat/Hello-World","default_branch":"main","private":false}"#;

        fake.push(403, &[("retry-after", "0")], Vec::new());
        fake.push_json(ok);
        assert!(client.repository(&execution, &repo).is_ok());
        assert_eq!(client.calls().get(Operation::Repository), 2);

        fake.push(
            429,
            &[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "0")],
            Vec::new(),
        );
        fake.push_json(ok);
        assert!(client.repository(&execution, &repo).is_ok());

        let far = (now_epoch_seconds() + 3600).to_string();
        fake.push(
            403,
            &[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", &far)],
            Vec::new(),
        );
        let error = client
            .repository(&execution, &repo)
            .expect_err("rate limited");
        match error {
            GitHubError::RateLimited { retry_after } => {
                assert!(retry_after >= Duration::from_secs(3590));
                assert_eq!(
                    error.disposition(),
                    Disposition::Retry {
                        after: Some(retry_after)
                    }
                );
            }
            other => panic!("unexpected {other:?}"),
        }

        fake.push(403, &[("retry-after", "0")], Vec::new());
        fake.push(403, &[("retry-after", "0")], Vec::new());
        assert_eq!(
            client.repository(&execution, &repo).err(),
            Some(GitHubError::RateLimited {
                retry_after: Duration::ZERO
            })
        );

        fake.push(403, &[], Vec::new());
        let forbidden = client.repository(&execution, &repo).expect_err("forbidden");
        assert_eq!(forbidden, GitHubError::Forbidden);
        assert_eq!(forbidden.disposition(), Disposition::Terminal);

        fake.push(401, &[], Vec::new());
        assert_eq!(
            client.repository(&execution, &repo).err(),
            Some(GitHubError::Unauthorized)
        );
        fake.push(404, &[], Vec::new());
        assert!(matches!(
            client.repository(&execution, &repo),
            Err(GitHubError::NotFound { .. })
        ));
        fake.push(429, &[], Vec::new());
        assert_eq!(
            client.repository(&execution, &repo).err(),
            Some(GitHubError::RateLimited {
                retry_after: Duration::from_secs(5)
            }),
            "429 without rate headers is reported with the maximum wait, not slept"
        );
    }

    /// Scenario 2: malformed or oversized payloads are terminal; server and
    /// transport failures retry then surface as retryable; cancellation and
    /// deadlines stop before any request is sent.
    #[test]
    fn malformed_payloads_and_interruptions_are_explicit() {
        let (client, fake) = client_on(
            GitHubHost::Public,
            RequestLimits {
                max_attempts: 2,
                max_json_bytes: 64,
                max_retry_wait: Duration::ZERO,
                ..RequestLimits::default()
            },
        );
        let execution = execution();
        let repo = repo();

        fake.push_json(r#"{"id":"not a number"}"#);
        let error = client.repository(&execution, &repo).expect_err("malformed");
        assert!(
            matches!(&error, GitHubError::MalformedResponse { detail, .. } if detail.starts_with("unexpected json shape"))
        );
        assert_eq!(error.disposition(), Disposition::Terminal);

        fake.push_json(r#"{"id":1296269,"full_name":"octocat/Hello-World","default_branch":"main","private":false,"pad":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}"#);
        assert!(matches!(
            client.repository(&execution, &repo),
            Err(GitHubError::ResponseTooLarge { limit: 64, .. })
        ));

        fake.push(503, &[], Vec::new());
        fake.push(502, &[], Vec::new());
        let error = client
            .repository(&Execution::with_timeout(Duration::from_secs(10)), &repo)
            .expect_err("unavailable");
        assert_eq!(error, GitHubError::Unavailable { status: 502 });
        assert_eq!(error.disposition(), Disposition::Retry { after: None });

        fake.push_failure(TransportError::Timeout);
        fake.push_failure(TransportError::Failed("reset".into()));
        assert_eq!(
            client
                .repository(&Execution::with_timeout(Duration::from_secs(10)), &repo)
                .err(),
            Some(GitHubError::Transport(TransportError::Failed(
                "reset".into()
            )))
        );

        let before = fake.seen().len();
        let cancelled = Execution::with_timeout(Duration::from_secs(60));
        cancelled.cancel_token().cancel();
        assert_eq!(
            client.repository(&cancelled, &repo).err(),
            Some(GitHubError::Interrupted(Interrupted::Cancelled))
        );
        let expired = Execution::new(Instant::now(), CancelToken::new());
        assert_eq!(
            client.tree(&expired, &repo, &sha('7')).err(),
            Some(GitHubError::Interrupted(Interrupted::DeadlineExceeded))
        );
        assert_eq!(
            fake.seen().len(),
            before,
            "no request is sent once interrupted"
        );

        // A retry wait that would cross the deadline is reported instead of slept.
        fake.push(403, &[("retry-after", "2")], Vec::new());
        let short = Execution::with_timeout(Duration::from_secs(1));
        assert_eq!(
            client.repository(&short, &repo).err(),
            Some(GitHubError::RateLimited {
                retry_after: Duration::from_secs(2)
            })
        );
    }

    /// Branch names are percent-encoded except `/`; blob ids hash like Git.
    #[test]
    fn helpers_encode_paths_and_hash_blobs_like_git() {
        assert_eq!(encode_segment("feature/a b#1"), "feature/a%20b%231");
        assert_eq!(
            git_blob_sha(b""),
            "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
        );
        assert_eq!(
            git_blob_sha(b"hello\n"),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
        assert!(is_trusted_redirect(
            "https://CodeLoad.github.com/x",
            "codeload.github.com"
        ));
        assert!(!is_trusted_redirect(
            "http://codeload.github.com/x",
            "codeload.github.com"
        ));
        assert!(!is_trusted_redirect(
            "https://codeload.github.com.evil/x",
            "codeload.github.com"
        ));
    }
}
