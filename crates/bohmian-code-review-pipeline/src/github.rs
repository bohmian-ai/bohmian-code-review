//! Bounded, verified GitHub and GHES source access.
//!
//! [`GitHubClient`] performs the only provider IO in the workspace. Each
//! operation builds one authenticated request, applies a single retry policy
//! for rate limits and transient failures, parses a bounded response, and
//! verifies the returned identity against what was asked for before anything
//! crosses into the symbolic library. Every operation is an `async fn`; the
//! caller cancels one by dropping its future, which stops any retry wait
//! before another request is sent. Transport is a one-method seam so the same
//! policy and verification run against a scripted fake in tests and
//! `reqwest` in production.

use crate::identity::{GitHubHost, InstallationToken, ObjectSha, RepositoryIdentity};
use crate::storage::hex;
use bohmian_code_review::RelativePath;
use serde::Deserialize;
use serde_json::Error as JsonError;
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
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
pub(crate) const PR_FILES_PER_PAGE: u32 = 100;

/// Boxed `Send` future returned by the object-safe transport and body seams.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

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

/// An unread response body delivered in chunks so archives can stream.
pub trait HttpBody: Send {
    /// Next chunk, or `None` at end of body.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the body stream fails or times out.
    fn chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, TransportError>>;
}

impl HttpBody for reqwest::Response {
    /// Delegates to [`reqwest::Response::chunk`].
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Timeout`] when the request timeout elapses
    /// mid-body and [`TransportError::Failed`] for any other body failure.
    fn chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, TransportError>> {
        Box::pin(async move {
            reqwest::Response::chunk(self)
                .await
                .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
                .map_err(transport_error)
        })
    }
}

/// One response with its body left unread so archives can stream.
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Lowercase header names with their values.
    pub headers: Vec<(String, String)>,
    /// Unread body.
    pub body: Box<dyn HttpBody>,
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

/// Sends one GET request. Implemented by [`ReqwestTransport`] and by test
/// fakes; the boxed future keeps the seam object-safe for [`GitHubClient`].
pub trait HttpTransport: Send + Sync {
    /// Send `request` without following redirects and without raising on status.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] only when no response arrived.
    fn get(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, TransportError>>;
}

/// Production transport over `reqwest` with redirects disabled.
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// Build a client that never follows redirects.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Failed`] when the TLS backend cannot be
    /// initialised.
    pub fn new() -> Result<ReqwestTransport, TransportError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(USER_AGENT)
            .build()
            .map_err(transport_error)?;
        Ok(ReqwestTransport { client })
    }
}

/// Classify a `reqwest` failure without keeping its URL, which may carry an
/// archive redirect's signed token.
fn transport_error(error: reqwest::Error) -> TransportError {
    if error.is_timeout() {
        TransportError::Timeout
    } else {
        TransportError::Failed(error.without_url().to_string())
    }
}

impl HttpTransport for ReqwestTransport {
    /// Sends the request with its whole-request timeout; the body streams.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Timeout`] when `request.timeout` elapses
    /// before the response headers arrive and [`TransportError::Failed`] for
    /// any other connection, TLS, or protocol failure.
    fn get(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
        Box::pin(async move {
            let mut builder = self.client.get(&request.url).timeout(request.timeout);
            for (name, value) in &request.headers {
                builder = builder.header(*name, value);
            }
            let response = builder.send().await.map_err(transport_error)?;
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
                body: Box::new(response),
            })
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
    /// Per-request timeout including body transfer.
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
    /// `GET /repos/{owner}/{repo}/git/trees/{sha}`, recursive or not.
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
    /// A rate limit could not be waited out inside the retry cap.
    #[error("rate limited; retry after {retry_after:?}")]
    RateLimited {
        /// Wait the provider requested through `retry-after` or the reset
        /// time, or GitHub's documented one-minute minimum for a secondary
        /// limit that carried neither header.
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
    /// An archive redirect pointed outside the trusted archive hosts. The
    /// target is not kept because archive URLs carry a signed token.
    #[error("untrusted archive redirect")]
    UntrustedRedirect,
}

impl GitHubError {
    /// Whether the caller may retry the whole operation.
    pub fn disposition(&self) -> Disposition {
        match self {
            GitHubError::RateLimited { retry_after } => Disposition::Retry {
                after: Some(*retry_after),
            },
            GitHubError::Unavailable { .. } | GitHubError::Transport(_) => {
                Disposition::Retry { after: None }
            }
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

/// A provider PR file that could not become a trusted [`PullRequestFile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedPath {
    /// Raw path text as reported; never used as an identity.
    pub path: String,
    /// Why it was rejected.
    pub reason: String,
}

/// A complete tree listing verified against the requested tree id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeListing {
    /// Tree id as requested.
    pub sha: ObjectSha,
    /// Whether some tree's non-recursive listing was truncated, so the
    /// listing could not be split further and misses that tree's entries.
    pub truncated: bool,
    /// Entries with valid paths and modes.
    pub entries: Vec<TreeEntry>,
    /// Entries left out because their path is not a canonical
    /// [`RelativePath`]; no such path can be eligible.
    pub invalid_paths: usize,
}

impl TreeListing {
    /// Validate `entry` with `prefix` prepended to its path and keep it, or
    /// count it as an invalid path.
    ///
    /// # Errors
    ///
    /// [`GitHubError::MalformedResponse`] for `resource` when the entry has
    /// an unknown mode or a malformed id.
    fn push(
        &mut self,
        resource: &str,
        prefix: &str,
        mut entry: TreeEntryWire,
    ) -> Result<(), GitHubError> {
        entry.path.insert_str(0, prefix);
        match validate_tree_entry(&entry) {
            Ok(Some(valid)) => self.entries.push(valid),
            Ok(None) => self.invalid_paths += 1,
            Err(detail) => {
                return Err(GitHubError::MalformedResponse {
                    resource: resource.to_owned(),
                    detail,
                });
            }
        }
        Ok(())
    }
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
    body: Box<dyn HttpBody>,
    read: u64,
    limit: u64,
    resource: String,
}

impl Archive {
    /// Next chunk of compressed tarball bytes, or `None` at the end.
    ///
    /// # Errors
    ///
    /// Returns [`GitHubError::ResponseTooLarge`] once the stream passes
    /// `max_archive_bytes` (it fails instead of truncating) and
    /// [`GitHubError::Transport`] when the body stream fails.
    pub async fn chunk(&mut self) -> Result<Option<Vec<u8>>, GitHubError> {
        let Some(chunk) = self.body.chunk().await.map_err(GitHubError::Transport)? else {
            return Ok(None);
        };
        self.read = self.read.saturating_add(chunk.len() as u64);
        if self.read > self.limit {
            return Err(GitHubError::ResponseTooLarge {
                resource: self.resource.clone(),
                limit: self.limit,
            });
        }
        Ok(Some(chunk))
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

/// Wire shape of a tree listing, recursive or not.
#[derive(Deserialize)]
struct TreeWire {
    sha: ObjectSha,
    truncated: bool,
    tree: Vec<TreeEntryWire>,
}

/// One raw tree entry; validated into [`TreeEntry`] or counted as an invalid path.
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
pub(crate) fn git_blob_sha(bytes: &[u8]) -> String {
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

/// GitHub's documented minimum wait after a secondary rate limit that
/// carries no `retry-after` or reset header.
const SECONDARY_RATE_LIMIT_WAIT: Duration = Duration::from_secs(60);

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

/// Longest 403 body prefix read to recognise a header-less secondary limit.
const RATE_LIMIT_BODY_BYTES: usize = 4096;

/// Whether a 403 body names a rate limit, as GitHub's secondary-limit
/// responses do when they carry no rate headers.
///
/// Only the [`rate_limit_prefix`] is inspected; the text never leaves this
/// function.
async fn names_rate_limit(response: &mut HttpResponse) -> bool {
    let prefix = rate_limit_prefix(response.body.as_mut()).await;
    String::from_utf8_lossy(&prefix)
        .to_ascii_lowercase()
        .contains("rate limit")
}

/// The first [`RATE_LIMIT_BODY_BYTES`] of `body`, copied into a buffer
/// that never grows past that bound: each chunk is sliced to the room left
/// before it is copied, so one oversized chunk costs no extra copy here.
///
/// A read error ends the prefix early and counts as no signal.
async fn rate_limit_prefix(body: &mut dyn HttpBody) -> Vec<u8> {
    let mut prefix = Vec::with_capacity(RATE_LIMIT_BODY_BYTES);
    while prefix.len() < RATE_LIMIT_BODY_BYTES {
        match body.chunk().await {
            Ok(Some(chunk)) => {
                let room = RATE_LIMIT_BODY_BYTES - prefix.len();
                prefix.extend_from_slice(&chunk[..chunk.len().min(room)]);
            }
            _ => break,
        }
    }
    prefix
}

/// Whether a `Location` is `https://<trusted archive host>/...`.
fn is_trusted_redirect(location: &str, host: &GitHubHost) -> bool {
    location
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|authority| host.trusts_archive_host(authority))
}

/// Authenticated, bounded client for one GitHub host and installation.
///
/// The client is stateless apart from call accounting, so one instance may
/// serve many concurrent invocations.
pub struct GitHubClient {
    host: GitHubHost,
    token: InstallationToken,
    transport: Box<dyn HttpTransport>,
    limits: RequestLimits,
    calls: CallCounts,
}

impl GitHubClient {
    /// Client over the production `reqwest` transport.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Failed`] when the TLS backend cannot be
    /// initialised.
    pub fn new(
        host: GitHubHost,
        token: InstallationToken,
        limits: RequestLimits,
    ) -> Result<GitHubClient, TransportError> {
        let transport = ReqwestTransport::new()?;
        Ok(GitHubClient::with_transport(
            host,
            token,
            limits,
            Box::new(transport),
        ))
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
    pub async fn repository(
        &self,
        repository: &RepositoryIdentity,
    ) -> Result<Repository, GitHubError> {
        let resource = format!("/repos/{}", repository.full_name());
        let wire: RepositoryWire = self
            .get_json(Operation::Repository, &resource, ACCEPT_JSON)
            .await?;
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
    pub async fn branch_head(
        &self,
        repository: &RepositoryIdentity,
        branch: &str,
    ) -> Result<BranchHead, GitHubError> {
        let resource = format!(
            "/repos/{}/branches/{}",
            repository.full_name(),
            encode_segment(branch)
        );
        let wire: BranchWire = self
            .get_json(Operation::BranchHead, &resource, ACCEPT_JSON)
            .await?;
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
    pub async fn commit(
        &self,
        repository: &RepositoryIdentity,
        sha: &ObjectSha,
    ) -> Result<Commit, GitHubError> {
        let resource = format!("/repos/{}/git/commits/{sha}", repository.full_name());
        let wire: CommitWire = self
            .get_json(Operation::Commit, &resource, ACCEPT_JSON)
            .await?;
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

    /// List `tree` completely, validating every path, mode, and id.
    ///
    /// When the provider truncates a recursive listing, the tree is listed
    /// without recursion and each subtree is listed recursively in its
    /// place, splitting again wherever a listing is still truncated. The
    /// joined entries carry full repository-relative paths in the order an
    /// untruncated recursive listing returns them. Only a non-recursive
    /// listing of one tree that is itself truncated leaves
    /// [`TreeListing::truncated`] set; that tree's entries are left out.
    ///
    /// An entry whose path is not a canonical [`RelativePath`] is counted in
    /// [`TreeListing::invalid_paths`] and left out.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the provider returns another
    /// tree id, [`GitHubError::MalformedResponse`] when any entry has an
    /// unknown mode or a malformed id, plus any policy or transport error.
    pub async fn tree(
        &self,
        repository: &RepositoryIdentity,
        tree: &ObjectSha,
    ) -> Result<TreeListing, GitHubError> {
        let mut listing = TreeListing {
            sha: tree.clone(),
            truncated: false,
            entries: Vec::new(),
            invalid_paths: 0,
        };
        self.list_into(repository, tree, "", &mut listing).await?;
        Ok(listing)
    }

    /// Append `tree`'s entries to `listing` with `prefix` (empty or ending
    /// in `/`) prepended to each path, splitting a truncated recursive
    /// listing into its subtrees.
    ///
    /// # Errors
    ///
    /// As [`GitHubClient::tree`].
    async fn list_into(
        &self,
        repository: &RepositoryIdentity,
        tree: &ObjectSha,
        prefix: &str,
        listing: &mut TreeListing,
    ) -> Result<(), GitHubError> {
        let (resource, wire) = self.tree_wire(repository, tree, true).await?;
        if !wire.truncated {
            for entry in wire.tree {
                listing.push(&resource, prefix, entry)?;
            }
            return Ok(());
        }
        let (resource, wire) = self.tree_wire(repository, tree, false).await?;
        if wire.truncated {
            listing.truncated = true;
            return Ok(());
        }
        for entry in wire.tree {
            let subtree = match entry.mode.as_str() {
                "040000" => Some((
                    format!("{prefix}{}/", entry.path),
                    ObjectSha::new(&entry.sha).map_err(|error| GitHubError::MalformedResponse {
                        resource: resource.clone(),
                        detail: format!("tree entry id: {error}"),
                    })?,
                )),
                _ => None,
            };
            listing.push(&resource, prefix, entry)?;
            if let Some((subprefix, sha)) = subtree {
                Box::pin(self.list_into(repository, &sha, &subprefix, listing)).await?;
            }
        }
        Ok(())
    }

    /// Fetch one listing of `tree`, recursive or not, and check its id.
    /// Returns the request path with the wire listing.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the provider returns another
    /// tree id, plus any policy or transport error.
    async fn tree_wire(
        &self,
        repository: &RepositoryIdentity,
        tree: &ObjectSha,
        recursive: bool,
    ) -> Result<(String, TreeWire), GitHubError> {
        let query = if recursive { "?recursive=1" } else { "" };
        let resource = format!("/repos/{}/git/trees/{tree}{query}", repository.full_name());
        let wire: TreeWire = self
            .get_json(Operation::Tree, &resource, ACCEPT_JSON)
            .await?;
        if wire.sha != *tree {
            return Err(GitHubError::IdentityMismatch {
                expected: tree.to_string(),
                actual: wire.sha.to_string(),
            });
        }
        Ok((resource, wire))
    }

    /// Fetch blob `sha` as raw bytes, reading at most the smaller of
    /// `max_bytes` and `max_blob_bytes`, and verify its Git id. A blob past
    /// that bound is never returned; the error's `limit` says which bound
    /// applied.
    ///
    /// # Errors
    ///
    /// [`GitHubError::ResponseTooLarge`] past the bound,
    /// [`GitHubError::IdentityMismatch`] when the bytes do not hash to `sha`,
    /// plus any policy or transport error.
    pub async fn blob(
        &self,
        repository: &RepositoryIdentity,
        sha: &ObjectSha,
        max_bytes: u64,
    ) -> Result<Vec<u8>, GitHubError> {
        let resource = format!("/repos/{}/git/blobs/{sha}", repository.full_name());
        let mut response = self
            .request(Operation::Blob, &resource, ACCEPT_RAW, true)
            .await?;
        expect_status(&response, 200, &resource)?;
        let limit = usize::try_from(max_bytes)
            .unwrap_or(usize::MAX)
            .min(self.limits.max_blob_bytes);
        let bytes = read_bounded(response.body.as_mut(), limit, &resource).await?;
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
    /// without credentials, and only to one of the host's archive hosts.
    ///
    /// # Errors
    ///
    /// [`GitHubError::UntrustedRedirect`] for any other redirect target,
    /// [`GitHubError::UnexpectedStatus`] when neither hop yields the body,
    /// plus any policy or transport error. Reading the returned stream past
    /// `max_archive_bytes` fails with [`GitHubError::ResponseTooLarge`].
    pub async fn archive(
        &self,
        repository: &RepositoryIdentity,
        commit: &ObjectSha,
    ) -> Result<Archive, GitHubError> {
        let resource = format!("/repos/{}/tarball/{commit}", repository.full_name());
        let first = self
            .request(Operation::Archive, &resource, ACCEPT_JSON, true)
            .await?;
        let response = match first.status {
            200 => first,
            301..=303 | 307 | 308 => {
                let location = first.header("location").unwrap_or("").to_owned();
                if !is_trusted_redirect(&location, &self.host) {
                    return Err(GitHubError::UntrustedRedirect);
                }
                let hop = self
                    .send_with_policy(Operation::Archive, &location, ACCEPT_JSON, false)
                    .await?;
                expect_status(&hop, 200, &resource)?;
                hop
            }
            _ => return Err(status_error(&first, &resource)),
        };
        Ok(Archive {
            body: response.body,
            read: 0,
            limit: self.limits.max_archive_bytes,
            resource,
        })
    }

    /// Fetch pull request `number` and verify its number and base repository.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the number or base repository
    /// id differs, [`GitHubError::MalformedResponse`] for an unknown state,
    /// plus any policy or transport error.
    pub async fn pull_request(
        &self,
        repository: &RepositoryIdentity,
        number: u64,
    ) -> Result<PullRequest, GitHubError> {
        let resource = format!("/repos/{}/pulls/{number}", repository.full_name());
        let wire: PullRequestWire = self
            .get_json(Operation::PullRequest, &resource, ACCEPT_JSON)
            .await?;
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
    pub async fn pull_request_files(
        &self,
        repository: &RepositoryIdentity,
        number: u64,
        page: u32,
    ) -> Result<PullRequestFilesPage, GitHubError> {
        let resource = format!(
            "/repos/{}/pulls/{number}/files?per_page={PR_FILES_PER_PAGE}&page={page}",
            repository.full_name()
        );
        let mut response = self
            .request(Operation::PullRequestFiles, &resource, ACCEPT_JSON, true)
            .await?;
        expect_status(&response, 200, &resource)?;
        let has_next = response
            .header("link")
            .is_some_and(|link| link.contains("rel=\"next\""));
        let bytes = read_bounded(
            response.body.as_mut(),
            self.limits.max_json_bytes,
            &resource,
        )
        .await?;
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
    ///
    /// # Errors
    ///
    /// Propagates every [`GitHubError`] of [`GitHubClient::request`], then
    /// the status errors of [`expect_status`] for a non-200 reply,
    /// [`GitHubError::ResponseTooLarge`] or [`GitHubError::Transport`] from
    /// [`read_bounded`], and [`GitHubError::MalformedResponse`] from
    /// [`parse_json`].
    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        operation: Operation,
        resource: &str,
        accept: &str,
    ) -> Result<T, GitHubError> {
        let mut response = self.request(operation, resource, accept, true).await?;
        expect_status(&response, 200, resource)?;
        let bytes =
            read_bounded(response.body.as_mut(), self.limits.max_json_bytes, resource).await?;
        parse_json(&bytes, resource)
    }

    /// Send `resource` against the API base under the retry policy.
    ///
    /// # Errors
    ///
    /// Returns exactly the errors of [`GitHubClient::send_with_policy`].
    async fn request(
        &self,
        operation: Operation,
        resource: &str,
        accept: &str,
        authenticated: bool,
    ) -> Result<HttpResponse, GitHubError> {
        let url = format!("{}{resource}", self.host.api_base());
        self.send_with_policy(operation, &url, accept, authenticated)
            .await
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

    /// The single request policy: send, and retry rate limits, server
    /// errors, and transport failures within `max_attempts`.
    ///
    /// Retry waits use tokio time, so dropping the returned future during a
    /// wait sends no further request.
    ///
    /// # Errors
    ///
    /// Returns [`GitHubError::Forbidden`] for a 403 with neither rate-limit
    /// headers nor a body naming a rate limit; [`GitHubError::RateLimited`]
    /// for a 403/429 whose wait (from headers, or
    /// [`SECONDARY_RATE_LIMIT_WAIT`] without them) exceeds `max_retry_wait`
    /// or the attempt budget; [`GitHubError::Unavailable`] for a 5xx and
    /// [`GitHubError::Transport`] for a transport failure once `max_attempts`
    /// is exhausted. Any other status is returned as a successful response
    /// for the caller to classify.
    async fn send_with_policy(
        &self,
        operation: Operation,
        url: &str,
        accept: &str,
        authenticated: bool,
    ) -> Result<HttpResponse, GitHubError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.calls.record(operation);
            let outcome = self
                .transport
                .get(self.build(url, accept, authenticated, self.limits.request_timeout))
                .await;
            let backoff = Duration::from_secs(attempt.into()).min(self.limits.max_retry_wait);
            let (wait, failure) = match outcome {
                Ok(mut response) if matches!(response.status, 403 | 429) => {
                    let wait = match rate_limit_wait(&response) {
                        Some(wait) => wait,
                        None if response.status == 429 || names_rate_limit(&mut response).await => {
                            SECONDARY_RATE_LIMIT_WAIT
                        }
                        None => return Err(GitHubError::Forbidden),
                    };
                    (wait, GitHubError::RateLimited { retry_after: wait })
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
            if attempt >= self.limits.max_attempts || wait > self.limits.max_retry_wait {
                return Err(failure);
            }
            tokio::time::sleep(wait).await;
        }
    }
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
///
/// # Errors
///
/// Returns [`GitHubError::Unauthorized`] for 401, [`GitHubError::Forbidden`]
/// for 403, [`GitHubError::NotFound`] for 404, and
/// [`GitHubError::UnexpectedStatus`] for any other status that is not
/// `expected`.
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
///
/// # Errors
///
/// Returns [`GitHubError::Transport`] when the body stream fails and
/// [`GitHubError::ResponseTooLarge`] when more than `limit` bytes are
/// available.
async fn read_bounded(
    body: &mut dyn HttpBody,
    limit: usize,
    resource: &str,
) -> Result<Vec<u8>, GitHubError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = body.chunk().await.map_err(GitHubError::Transport)? {
        if chunk.len() > limit - bytes.len() {
            return Err(GitHubError::ResponseTooLarge {
                resource: resource.to_owned(),
                limit: limit as u64,
            });
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Parse JSON without echoing the body into the error.
///
/// # Errors
///
/// Returns [`GitHubError::MalformedResponse`] whose `detail` names only the
/// serde error category and position when `bytes` is not valid JSON of the
/// requested shape.
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
fn json_error_category(error: &JsonError) -> &'static str {
    match error.classify() {
        serde_json::error::Category::Io => "io error",
        serde_json::error::Category::Syntax => "invalid json",
        serde_json::error::Category::Data => "unexpected json shape",
        serde_json::error::Category::Eof => "truncated json",
    }
}

/// Validate a raw tree entry; `Ok(None)` when its path is not a canonical
/// [`RelativePath`], which makes it ineligible rather than untrusted.
///
/// # Errors
///
/// Returns the detail as text when the mode is unknown or the sha is not a
/// valid [`ObjectSha`]: the listing itself is broken.
fn validate_tree_entry(entry: &TreeEntryWire) -> Result<Option<TreeEntry>, String> {
    let mode = TreeEntryMode::parse(&entry.mode)
        .ok_or_else(|| format!("unknown tree mode {}", entry.mode))?;
    let sha = ObjectSha::new(&entry.sha).map_err(|error| format!("tree entry id: {error}"))?;
    let Ok(path) = RelativePath::new(&entry.path) else {
        return Ok(None);
    };
    Ok(Some(TreeEntry {
        path,
        mode,
        sha,
        size: entry.size,
    }))
}

/// Validate a raw PR file.
///
/// # Errors
///
/// Returns the rejection reason as text when the filename or previous
/// filename is not a canonical [`RelativePath`], the status is unknown, or
/// the sha is not a valid [`ObjectSha`]; the caller records it in
/// [`PullRequestFilesPage::rejected`].
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
/// Owner tests for the request policy, identity verification, and archive trust rules, driven through a scripted transport.
mod tests {
    use super::*;
    use crate::testing::FixedBody;
    use std::collections::VecDeque;
    use std::sync::Mutex;

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
        ///
        /// # Errors
        ///
        /// Returns the scripted [`TransportError`] when the next entry is one.
        fn get(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
            self.seen.lock().expect("seen lock").push(SeenRequest {
                url: request.url.clone(),
                authorized: request.header("authorization") == Some("Bearer ghs_token"),
                accept: request.header("accept").unwrap_or("").to_owned(),
            });
            let scripted = self
                .responses
                .lock()
                .expect("responses lock")
                .pop_front()
                .expect("scripted response for every request");
            Box::pin(async move {
                let (status, headers, body) = scripted?;
                Ok(HttpResponse {
                    status,
                    headers,
                    body: Box::new(FixedBody::new(body)),
                })
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
        ///
        /// # Errors
        ///
        /// Returns whatever [`FakeTransport::get`] returns.
        fn get(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
            self.0.get(request)
        }
    }

    /// Public-host client with default limits.
    fn client() -> (GitHubClient, &'static FakeTransport) {
        client_on(GitHubHost::public(), RequestLimits::default())
    }

    /// Every remaining chunk of `archive`, concatenated.
    ///
    /// # Errors
    ///
    /// Returns the first [`Archive::chunk`] error.
    async fn read_archive(archive: &mut Archive) -> Result<Vec<u8>, GitHubError> {
        let mut bytes = Vec::new();
        while let Some(chunk) = archive.chunk().await? {
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
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
    #[tokio::test]
    async fn verified_provider_identities_cross_and_mismatches_are_rejected() {
        let (client, fake) = client();
        let repo = repo();
        let commit = sha('c');
        let tree = sha('7');
        let blob_bytes = b"hello\n";
        let blob = ObjectSha::new(&git_blob_sha(blob_bytes)).expect("blob sha");

        fake.push_json(
            r#"{"id":1296269,"full_name":"octocat/Hello-World","default_branch":"main","private":true}"#,
        );
        let repository = client.repository(&repo).await.expect("repository");
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
        let head = client.branch_head(&repo, "main").await.expect("branch");
        assert_eq!((&head.commit, &head.tree), (&commit, &tree));

        fake.push_json(&format!(
            r#"{{"sha":"{commit}","tree":{{"sha":"{tree}"}}}}"#
        ));
        assert_eq!(
            client.commit(&repo, &commit).await.expect("commit").tree,
            tree
        );

        fake.push_json(&format!(
            r#"{{"sha":"{tree}","truncated":false,"tree":[
                {{"path":"src/lib.rs","mode":"100644","type":"blob","sha":"{blob}","size":6}},
                {{"path":"../escape","mode":"100644","type":"blob","sha":"{blob}"}},
                {{"path":"src","mode":"040000","type":"tree","sha":"{tree}"}}
            ]}}"#
        ));
        let listing = client.tree(&repo, &tree).await.expect("tree");
        assert!(!listing.truncated);
        assert_eq!(listing.entries.len(), 2);
        assert_eq!(listing.entries[0].path.as_str(), "src/lib.rs");
        assert_eq!(listing.entries[0].mode, TreeEntryMode::File);
        assert_eq!(listing.entries[0].size, Some(6));
        assert_eq!(listing.entries[1].mode, TreeEntryMode::Directory);
        assert_eq!(listing.invalid_paths, 1);

        // An unknown mode or malformed id breaks the whole listing.
        for (mode, sha) in [("777777", blob.to_string()), ("100644", "zz".to_owned())] {
            fake.push_json(&format!(
                r#"{{"sha":"{tree}","truncated":false,"tree":[
                    {{"path":"src/lib.rs","mode":"{mode}","type":"blob","sha":"{sha}"}}
                ]}}"#
            ));
            assert!(matches!(
                client.tree(&repo, &tree).await,
                Err(GitHubError::MalformedResponse { .. })
            ));
        }

        fake.push(200, &[], blob_bytes.to_vec());
        assert_eq!(
            client.blob(&repo, &blob, u64::MAX).await.expect("blob"),
            blob_bytes
        );
        fake.push(200, &[], blob_bytes.to_vec());
        assert!(matches!(
            client.blob(&repo, &blob, 1).await,
            Err(GitHubError::ResponseTooLarge { limit: 1, .. })
        ));
        assert_eq!(fake.seen().last().map(|s| s.2.as_str()), Some(ACCEPT_RAW));

        fake.push_json(&format!(
            r#"{{"number":42,"state":"open","draft":false,"merged":false,
                "base":{{"ref":"main","sha":"{commit}","repo":{{"id":1296269}}}},
                "head":{{"ref":"feature","sha":"{tree}","repo":{{"id":99}}}}}}"#
        ));
        let pr = client.pull_request(&repo, 42).await.expect("pr");
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
            .pull_request_files(&repo, 42, 1)
            .await
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
            client.repository(&repo).await,
            Err(GitHubError::IdentityMismatch { .. })
        ));
        fake.push_json(&format!(r#"{{"sha":"{tree}","tree":{{"sha":"{tree}"}}}}"#));
        assert!(matches!(
            client.commit(&repo, &commit).await,
            Err(GitHubError::IdentityMismatch { .. })
        ));
        fake.push_json(&format!(
            r#"{{"sha":"{commit}","truncated":true,"tree":[]}}"#
        ));
        assert!(matches!(
            client.tree(&repo, &tree).await,
            Err(GitHubError::IdentityMismatch { .. })
        ));
        fake.push(200, &[], b"tampered\n".to_vec());
        assert!(matches!(
            client.blob(&repo, &blob, u64::MAX).await,
            Err(GitHubError::IdentityMismatch { .. })
        ));
        fake.push_json(&format!(
            r#"{{"number":42,"state":"open","base":{{"ref":"main","sha":"{commit}","repo":{{"id":5}}}},"head":{{"ref":"f","sha":"{tree}","repo":null}}}}"#
        ));
        assert!(matches!(
            client.pull_request(&repo, 42).await,
            Err(GitHubError::IdentityMismatch { .. })
        ));
        fake.push_json(r#"{"name":"other","commit":{"sha":"cccccccccccccccccccccccccccccccccccccccc","commit":{"tree":{"sha":"7777777777777777777777777777777777777777"}}}}"#);
        assert!(matches!(
            client.branch_head(&repo, "main").await,
            Err(GitHubError::IdentityMismatch { .. })
        ));

        let calls = client.calls().snapshot();
        assert_eq!(calls[&Operation::Repository], 2);
        assert_eq!(calls[&Operation::Blob], 3);
        assert_eq!(calls.values().sum::<u64>(), 16);
    }

    /// Scenario 1: an archive redirect is followed once, without credentials,
    /// only to the trusted archive host; the stream is byte-bounded.
    #[tokio::test]
    async fn archive_redirect_is_trusted_only_on_archive_host_and_never_forwards_credentials() {
        let (client, fake) = client_on(
            GitHubHost::public(),
            RequestLimits {
                max_archive_bytes: 4,
                ..RequestLimits::default()
            },
        );
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
        let mut archive = client.archive(&repo, &commit).await.expect("archive");
        let bytes = read_archive(&mut archive).await.expect("within limit");
        assert_eq!(bytes, b"abcd");
        let seen = fake.seen();
        assert_eq!(
            seen[0].0,
            "https://api.github.com/repos/octocat/Hello-World/tarball/cccccccccccccccccccccccccccccccccccccccc"
        );
        assert!(seen[0].1, "API hop carries credentials");
        assert!(seen[1].0.starts_with("https://codeload.github.com/"));
        assert!(!seen[1].1, "redirect hop must not forward credentials");

        let evil = "https://evil.example.com/archive.tar.gz?token=secret";
        fake.push(302, &[("location", evil)], Vec::new());
        let untrusted = client
            .archive(&repo, &commit)
            .await
            .err()
            .expect("untrusted redirect");
        assert_eq!(untrusted, GitHubError::UntrustedRedirect);
        let message = untrusted.to_string();
        assert!(
            !message.contains("secret") && !message.contains(evil),
            "{message}"
        );
        assert_eq!(fake.seen().len(), 3, "untrusted redirect is never followed");

        fake.push(
            302,
            &[("location", "https://codeload.github.com/x")],
            Vec::new(),
        );
        fake.push(200, &[], b"abcde".to_vec());
        let mut archive = client.archive(&repo, &commit).await.expect("archive");
        assert!(matches!(
            read_archive(&mut archive).await,
            Err(GitHubError::ResponseTooLarge { limit: 4, .. })
        ));

        let ghes = GitHubHost::enterprise("https://ghe.example.com/api/v3").expect("ghes");
        let (client, fake) = client_on(ghes, RequestLimits::default());
        fake.push(
            302,
            &[("location", "https://ghe.example.com/_codeload/x")],
            Vec::new(),
        );
        fake.push(200, &[], b"gz".to_vec());
        assert!(client.archive(&repo, &commit).await.is_ok());
        assert_eq!(
            fake.seen()[0].0,
            "https://ghe.example.com/api/v3/repos/octocat/Hello-World/tarball/cccccccccccccccccccccccccccccccccccccccc"
        );
        fake.push(
            302,
            &[("location", "https://codeload.ghe.example.com/x")],
            Vec::new(),
        );
        fake.push(200, &[], b"gz".to_vec());
        assert!(client.archive(&repo, &commit).await.is_ok());
        let seen = fake.seen();
        assert_eq!(seen[3].0, "https://codeload.ghe.example.com/x");
        assert!(!seen[3].1, "codeload hop must not forward credentials");
        fake.push(
            302,
            &[("location", "https://codeload.github.com/x")],
            Vec::new(),
        );
        assert!(matches!(
            client.archive(&repo, &commit).await,
            Err(GitHubError::UntrustedRedirect)
        ));
        assert_eq!(client.calls().get(Operation::Archive), 5);
    }

    /// A caller that drops a call while it waits out a rate limit sends no
    /// further request and sees no library error: drop is cancellation.
    #[tokio::test(start_paused = true)]
    async fn dropped_call_sends_no_further_requests() {
        let (client, fake) = client();
        let repo = repo();
        fake.push(429, &[("retry-after", "30")], Vec::new());
        fake.push_json(r#"{"id":1296269,"full_name":"octocat/Hello-World","default_branch":"main","private":false}"#);

        let dropped = tokio::time::timeout(Duration::from_secs(1), client.repository(&repo)).await;
        assert!(dropped.is_err(), "call must still be waiting when dropped");
        tokio::time::advance(Duration::from_secs(120)).await;

        assert_eq!(fake.seen().len(), 1);
        assert_eq!(client.calls().get(Operation::Repository), 1);
    }

    /// A header-less 403 is classified from a hard-bounded body prefix: one
    /// oversized chunk is copied only up to the bound, a marker past it is
    /// ignored, a marker inside it still signals a limit, and neither case
    /// sends a retry request.
    #[tokio::test]
    async fn headerless_403_inspects_only_a_bounded_prefix() {
        let mut oversized = FixedBody::new(vec![b'x'; 4 * RATE_LIMIT_BODY_BYTES]);
        let prefix = rate_limit_prefix(&mut oversized).await;
        assert_eq!(prefix.len(), RATE_LIMIT_BODY_BYTES);
        assert!(prefix.capacity() <= RATE_LIMIT_BODY_BYTES);

        let (client, fake) = client_on(
            GitHubHost::public(),
            RequestLimits {
                max_retry_wait: Duration::from_secs(5),
                ..RequestLimits::default()
            },
        );
        let mut beyond = vec![b'x'; RATE_LIMIT_BODY_BYTES];
        beyond.extend_from_slice(b"rate limit");
        fake.push(403, &[], beyond);
        assert_eq!(
            client.repository(&repo()).await.err(),
            Some(GitHubError::Forbidden)
        );

        let mut within = vec![b'x'; RATE_LIMIT_BODY_BYTES - "rate limit".len()];
        within.extend_from_slice(b"rate limit");
        within.extend(vec![b'x'; 2 * RATE_LIMIT_BODY_BYTES]);
        fake.push(403, &[], within);
        assert_eq!(
            client.repository(&repo()).await.err(),
            Some(GitHubError::RateLimited {
                retry_after: SECONDARY_RATE_LIMIT_WAIT
            })
        );
        assert_eq!(fake.seen().len(), 2);
    }

    /// Scenario 2: rate limits with a short wait are retried once under one
    /// policy; long waits, exhausted attempts, header-less secondary limits,
    /// and plain 403s are typed.
    #[tokio::test]
    async fn rate_limits_retry_within_bounds_or_report_typed_outcomes() {
        let (client, fake) = client_on(
            GitHubHost::public(),
            RequestLimits {
                max_attempts: 2,
                max_retry_wait: Duration::from_secs(5),
                ..RequestLimits::default()
            },
        );
        let repo = repo();
        let ok = r#"{"id":1296269,"full_name":"octocat/Hello-World","default_branch":"main","private":false}"#;

        fake.push(403, &[("retry-after", "0")], Vec::new());
        fake.push_json(ok);
        assert!(client.repository(&repo).await.is_ok());
        assert_eq!(client.calls().get(Operation::Repository), 2);

        fake.push(
            429,
            &[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "0")],
            Vec::new(),
        );
        fake.push_json(ok);
        assert!(client.repository(&repo).await.is_ok());

        let far = (now_epoch_seconds() + 3600).to_string();
        fake.push(
            403,
            &[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", &far)],
            Vec::new(),
        );
        let error = client.repository(&repo).await.expect_err("rate limited");
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
            client.repository(&repo).await.err(),
            Some(GitHubError::RateLimited {
                retry_after: Duration::ZERO
            })
        );

        fake.push(
            403,
            &[("x-ratelimit-remaining", "4990")],
            b"You have exceeded a secondary rate limit".to_vec(),
        );
        let secondary = client.repository(&repo).await.expect_err("secondary limit");
        assert_eq!(
            secondary,
            GitHubError::RateLimited {
                retry_after: SECONDARY_RATE_LIMIT_WAIT
            }
        );
        assert_eq!(
            secondary.disposition(),
            Disposition::Retry {
                after: Some(SECONDARY_RATE_LIMIT_WAIT)
            }
        );

        fake.push(403, &[], Vec::new());
        let forbidden = client.repository(&repo).await.expect_err("forbidden");
        assert_eq!(forbidden, GitHubError::Forbidden);
        assert_eq!(forbidden.disposition(), Disposition::Terminal);

        fake.push(401, &[], Vec::new());
        assert_eq!(
            client.repository(&repo).await.err(),
            Some(GitHubError::Unauthorized)
        );
        fake.push(404, &[], Vec::new());
        assert!(matches!(
            client.repository(&repo).await,
            Err(GitHubError::NotFound { .. })
        ));
        fake.push(429, &[], Vec::new());
        assert_eq!(
            client.repository(&repo).await.err(),
            Some(GitHubError::RateLimited {
                retry_after: SECONDARY_RATE_LIMIT_WAIT
            }),
            "429 without rate headers waits GitHub's documented minute, not slept"
        );
    }

    /// Scenario 2: malformed or oversized payloads are terminal; server and
    /// transport failures retry then surface as retryable.
    #[tokio::test]
    async fn malformed_payloads_and_failures_are_explicit() {
        let (client, fake) = client_on(
            GitHubHost::public(),
            RequestLimits {
                max_attempts: 2,
                max_json_bytes: 64,
                max_retry_wait: Duration::ZERO,
                ..RequestLimits::default()
            },
        );
        let repo = repo();

        fake.push_json(r#"{"id":"not a number"}"#);
        let error = client.repository(&repo).await.expect_err("malformed");
        assert!(
            matches!(&error, GitHubError::MalformedResponse { detail, .. } if detail.starts_with("unexpected json shape"))
        );
        assert_eq!(error.disposition(), Disposition::Terminal);

        fake.push_json(r#"{"id":1296269,"full_name":"octocat/Hello-World","default_branch":"main","private":false,"pad":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}"#);
        assert!(matches!(
            client.repository(&repo).await,
            Err(GitHubError::ResponseTooLarge { limit: 64, .. })
        ));

        fake.push(503, &[], Vec::new());
        fake.push(502, &[], Vec::new());
        let error = client.repository(&repo).await.expect_err("unavailable");
        assert_eq!(error, GitHubError::Unavailable { status: 502 });
        assert_eq!(error.disposition(), Disposition::Retry { after: None });

        fake.push_failure(TransportError::Timeout);
        fake.push_failure(TransportError::Failed("reset".into()));
        assert_eq!(
            client.repository(&repo).await.err(),
            Some(GitHubError::Transport(TransportError::Failed(
                "reset".into()
            )))
        );
    }

    /// Branch names are percent-encoded except `/`; blob ids hash like Git.
    #[tokio::test]
    async fn helpers_encode_paths_and_hash_blobs_like_git() {
        assert_eq!(encode_segment("feature/a b#1"), "feature/a%20b%231");
        assert_eq!(
            git_blob_sha(b""),
            "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
        );
        assert_eq!(
            git_blob_sha(b"hello\n"),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
        let public = GitHubHost::public();
        assert!(is_trusted_redirect(
            "https://CodeLoad.github.com/x",
            &public
        ));
        assert!(!is_trusted_redirect(
            "http://codeload.github.com/x",
            &public
        ));
        assert!(!is_trusted_redirect(
            "https://codeload.github.com.evil/x",
            &public
        ));
    }

    /// A limit of `usize::MAX` reads the whole body instead of overflowing.
    #[tokio::test]
    async fn read_bounded_accepts_the_maximum_limit() {
        let mut body = FixedBody::new(b"hello".to_vec());
        let bytes = read_bounded(&mut body, std::hint::black_box(usize::MAX), "blob").await;
        assert_eq!(bytes.as_deref(), Ok(&b"hello"[..]));
    }
}
