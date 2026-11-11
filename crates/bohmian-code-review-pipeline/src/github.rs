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

use crate::history::{MergedPr, is_timestamp};
use crate::identity::{GitHubHost, InstallationToken, ObjectSha, RepositoryIdentity};
use crate::storage::hex;
use bohmian_code_review::RelativePath;
use serde::{Deserialize, Serialize};
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

/// Pull requests requested per closed-PR listing page; the provider's
/// maximum.
const PULL_REQUESTS_PER_PAGE: u32 = 100;

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
    /// `GET /repositories/{id}`, or `GET /repos/{owner}/{repo}` for a
    /// lookup by name.
    Repository,
    /// `GET /repositories/{id}/branches/{branch}`.
    BranchHead,
    /// `GET /repositories/{id}/git/commits/{sha}`.
    Commit,
    /// `GET /repositories/{id}/git/trees/{sha}`, recursive or not.
    Tree,
    /// `GET /repositories/{id}/git/blobs/{sha}` as raw bytes.
    Blob,
    /// `GET /repositories/{id}/tarball/{sha}` and its redirect hop.
    Archive,
    /// `GET /repositories/{id}/pulls/{number}`.
    PullRequest,
    /// `GET /repositories/{id}/pulls/{number}/files`.
    PullRequestFiles,
    /// `GET /repositories/{id}/compare/{base}...{head}`.
    Compare,
    /// `GET /repositories/{id}/pulls`, one page.
    PullRequests,
}

impl Operation {
    /// Every operation in accounting order.
    const ALL: [Operation; 10] = [
        Operation::Repository,
        Operation::BranchHead,
        Operation::Commit,
        Operation::Tree,
        Operation::Blob,
        Operation::Archive,
        Operation::PullRequest,
        Operation::PullRequestFiles,
        Operation::Compare,
        Operation::PullRequests,
    ];
}

/// HTTP requests sent per operation, counting retries and redirect hops.
#[derive(Debug, Default)]
pub struct CallCounts([AtomicU64; 10]);

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
    /// Committer time of that commit as `YYYY-MM-DDTHH:MM:SSZ`.
    pub committed_at: String,
}

/// A commit's root tree and committer time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// Commit id as requested.
    pub sha: ObjectSha,
    /// Root tree.
    pub tree: ObjectSha,
    /// Committer time as `YYYY-MM-DDTHH:MM:SSZ`.
    pub committed_at: String,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// Browser URL of the pull request.
    pub url: String,
    /// Title.
    pub title: String,
    /// Description; empty when the author left none.
    pub body: String,
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

/// The changed paths between two commits, as the compare API lists them.
///
/// A rename is its old path removed and its new path added; a copy adds
/// its new path; a modification or mode change is its path in both lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// The head descends from the base with new commits (status `ahead`),
    /// so the files are exactly the base-to-head changes.
    pub ahead: bool,
    /// The provider's file list reached its ceiling and may omit changes.
    pub truncated: bool,
    /// Changed files the provider listed.
    pub files: usize,
    /// Canonical paths present at the base that changed or went away.
    pub removed: Vec<RelativePath>,
    /// Canonical paths present at the head that are new or changed.
    pub added: Vec<RelativePath>,
    /// Non-canonical paths present at the head that are new or changed.
    pub invalid_added: usize,
}

impl Comparison {
    /// Record `raw` as removed from the base; a non-canonical path was
    /// never eligible, so it is dropped.
    fn remove(&mut self, raw: &str) {
        if let Ok(path) = RelativePath::new(raw) {
            self.removed.push(path);
        }
    }

    /// Record `raw` as present at the head, or count it as invalid.
    fn add(&mut self, raw: &str) {
        match RelativePath::new(raw) {
            Ok(path) => self.added.push(path),
            Err(_) => self.invalid_added += 1,
        }
    }
}

/// How a comparison's head relates to its base, as the compare API reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonStatus {
    /// The head descends from the base with new commits.
    Ahead,
    /// Base and head are the same commit.
    Identical,
    /// The base descends from the head.
    Behind,
    /// Neither descends from the other.
    Diverged,
}

/// The changed files between a base and a head, with patches, as one
/// three-dot compare read lists them: the changes from their merge base to
/// the head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileComparison {
    /// How the head relates to the requested base.
    pub status: ComparisonStatus,
    /// Merge base of the requested base and head.
    pub merge_base: ObjectSha,
    /// Files with valid paths, statuses, and ids.
    pub files: Vec<PullRequestFile>,
    /// Files dropped for an invalid path, status, or id.
    pub rejected: Vec<RejectedPath>,
    /// The provider's file list reached its per-comparison cap and may
    /// omit changes.
    pub truncated: bool,
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

/// Wire shape of `GET /repositories/{id}`.
#[derive(Deserialize)]
struct RepositoryWire {
    id: u64,
    full_name: String,
    default_branch: String,
    private: bool,
}

/// Wire shape of `GET /repositories/{id}/branches/{branch}`.
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

/// Inner commit body carrying the tree and committer.
#[derive(Deserialize)]
struct CommitBodyWire {
    tree: ShaWire,
    committer: CommitterWire,
}

/// Wire shape of `GET /repositories/{id}/git/commits/{sha}`.
#[derive(Deserialize)]
struct CommitWire {
    sha: ObjectSha,
    tree: ShaWire,
    committer: CommitterWire,
}

/// A commit's committer, reduced to its time.
#[derive(Deserialize)]
struct CommitterWire {
    date: String,
}

/// One entry of `GET /repositories/{id}/pulls`.
#[derive(Deserialize)]
struct ListedPullRequestWire {
    number: u64,
    updated_at: String,
    merged_at: Option<String>,
    base: ListedRefWire,
    head: ListedRefWire,
}

/// Base or head of a listed pull request; `repo` is absent or null for a
/// deleted fork head.
#[derive(Deserialize)]
struct ListedRefWire {
    #[serde(rename = "ref")]
    name: String,
    sha: ObjectSha,
    repo: Option<PullRepoWire>,
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

/// Wire shape of `GET /repositories/{id}/pulls/{number}`.
#[derive(Deserialize)]
struct PullRequestWire {
    number: u64,
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    merged: bool,
    /// Browser URL of the pull request; empty when the provider omits it.
    #[serde(default)]
    html_url: String,
    /// Pull request title; empty when the provider omits it.
    #[serde(default)]
    title: String,
    /// Description; absent or `null` means the PR has none, read as empty.
    body: Option<String>,
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

/// One page of closed pull requests into one base branch, most recently
/// updated first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedPullRequestsPage {
    /// The page's merged pull requests in listing order.
    pub merged: Vec<MergedPr>,
    /// `(number, updated_at)` of every entry in listing order, merged or
    /// not, so a re-read can prove the page did not change.
    pub listed: Vec<(u64, String)>,
    /// Update time of the page's last entry, merged or not. Every closed
    /// PR updated after it was listed on this page or an earlier one, and
    /// a PR merges no later than its last update.
    pub oldest_update: Option<String>,
    /// Whether the provider advertised a further page.
    pub has_next: bool,
}

/// One listed pull request with a validated number, times, and commits.
///
/// Invariant: `number` is nonzero, both times are `YYYY-MM-DDTHH:MM:SSZ`,
/// and `merged_at`, when present, is no later than `updated_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedPullRequest {
    /// PR number.
    pub number: u64,
    /// Last update time.
    pub updated_at: String,
    /// Merge time, absent when not merged.
    pub merged_at: Option<String>,
    /// Base branch name.
    pub base_branch: String,
    /// Base commit.
    pub base: ObjectSha,
    /// Head commit.
    pub head: ObjectSha,
}

/// One page of every pull request, oldest created first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestsPage {
    /// The page's pull requests in listing order.
    pub prs: Vec<ListedPullRequest>,
    /// Whether the provider advertised a further page.
    pub has_next: bool,
}

/// `wire` from `resource`, a listing of `repository`, as a
/// [`ListedPullRequest`]. Only the base repository is checked; the head
/// may be a fork.
///
/// # Errors
///
/// [`GitHubError::IdentityMismatch`] when the base repository id is absent
/// or is not `repository`'s, and [`GitHubError::MalformedResponse`] for PR
/// number 0, a time that is not `YYYY-MM-DDTHH:MM:SSZ`, or a merge after
/// the last update.
fn listed_pull_request(
    wire: ListedPullRequestWire,
    repository: &RepositoryIdentity,
    resource: &str,
) -> Result<ListedPullRequest, GitHubError> {
    let base_repo = wire.base.repo.as_ref().map(|repo| repo.id);
    if base_repo != Some(repository.id()) {
        return Err(GitHubError::IdentityMismatch {
            expected: format!("PR #{} in repository {}", wire.number, repository.id()),
            actual: format!("PR #{} in repository {:?}", wire.number, base_repo),
        });
    }
    let malformed = |detail: String| GitHubError::MalformedResponse {
        resource: resource.to_owned(),
        detail: format!("PR #{}: {detail}", wire.number),
    };
    if wire.number == 0 {
        return Err(malformed("number 0".to_owned()));
    }
    let updated_at = timestamp(wire.updated_at, resource)?;
    let merged_at = wire
        .merged_at
        .map(|merged_at| timestamp(merged_at, resource))
        .transpose()?;
    if let Some(merged_at) = merged_at
        .as_ref()
        .filter(|merged_at| **merged_at > updated_at)
    {
        return Err(malformed(format!(
            "merged at {merged_at} after its update at {updated_at}"
        )));
    }
    Ok(ListedPullRequest {
        number: wire.number,
        updated_at,
        merged_at,
        base_branch: wire.base.name,
        base: wire.base.sha,
        head: wire.head.sha,
    })
}

/// Wire shape of `GET /repositories/{id}/compare/{base}...{head}`.
#[derive(Deserialize)]
struct CompareWire {
    status: String,
    base_commit: ShaWire,
    commits: Vec<ShaWire>,
    files: Vec<CompareFileWire>,
}

/// Wire shape of a three-dot comparison read for its files and patches.
#[derive(Deserialize)]
struct FileCompareWire {
    /// Provider relation of head to base: `ahead`, `behind`, `identical`,
    /// or `diverged`.
    status: String,
    /// The comparison's base commit, checked against the requested base.
    base_commit: ShaWire,
    /// Merge base of the base and head commits.
    merge_base_commit: ShaWire,
    /// One page of commits from base to head, oldest first; a full page
    /// may stop short of the head.
    commits: Vec<ShaWire>,
    /// Changed files with patches; the provider caps the list, so a full
    /// list marks the comparison truncated.
    files: Vec<PullRequestFileWire>,
}

/// One changed file of a comparison.
#[derive(Deserialize)]
struct CompareFileWire {
    filename: String,
    status: String,
    previous_filename: Option<String>,
}

/// `text` when it is a `YYYY-MM-DDTHH:MM:SSZ` time from `resource`.
///
/// # Errors
///
/// [`GitHubError::MalformedResponse`] for any other text.
fn timestamp(text: String, resource: &str) -> Result<String, GitHubError> {
    if is_timestamp(&text) {
        return Ok(text);
    }
    Err(GitHubError::MalformedResponse {
        resource: resource.to_owned(),
        detail: format!("time `{text}` is not YYYY-MM-DDTHH:MM:SSZ"),
    })
}

/// Most files the compare API lists for one comparison.
pub(crate) const COMPARE_MAX_FILES: usize = 300;

/// Most commits the compare API lists on its first page.
const COMPARE_COMMITS_PER_PAGE: usize = 250;

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

    /// Fetch and verify the repository whose stable id `repository` names.
    ///
    /// Requests address the repository by id, so a rename or transfer
    /// since the caller learned its `owner/name` changes nothing;
    /// [`Repository::full_name`] reports the current name.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the provider's numeric id
    /// differs, plus any policy or transport error.
    pub async fn repository(
        &self,
        repository: &RepositoryIdentity,
    ) -> Result<Repository, GitHubError> {
        let resource = format!("/repositories/{}", repository.id());
        let wire: RepositoryWire = self
            .get_json(Operation::Repository, &resource, ACCEPT_JSON)
            .await?;
        if wire.id != repository.id() {
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
    /// branch, [`GitHubError::MalformedResponse`] for a committer time that
    /// is not `YYYY-MM-DDTHH:MM:SSZ`, plus any policy or transport error.
    pub async fn branch_head(
        &self,
        repository: &RepositoryIdentity,
        branch: &str,
    ) -> Result<BranchHead, GitHubError> {
        let resource = format!(
            "/repositories/{}/branches/{}",
            repository.id(),
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
        let committed_at = timestamp(wire.commit.commit.committer.date, &resource)?;
        Ok(BranchHead {
            name: wire.name,
            commit: wire.commit.sha,
            tree: wire.commit.commit.tree.sha,
            committed_at,
        })
    }

    /// Resolve `sha` to its root tree and committer time.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the provider returns another
    /// commit id, [`GitHubError::MalformedResponse`] for a committer time
    /// that is not `YYYY-MM-DDTHH:MM:SSZ`, plus any policy or transport
    /// error.
    pub async fn commit(
        &self,
        repository: &RepositoryIdentity,
        sha: &ObjectSha,
    ) -> Result<Commit, GitHubError> {
        let resource = format!("/repositories/{}/git/commits/{sha}", repository.id());
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
            committed_at: timestamp(wire.committer.date, &resource)?,
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

    /// List `tree` alone without recursion, with `prefix` (empty or ending
    /// in `/`) prepended to each path; subtrees appear as directory entries.
    /// [`TreeListing::truncated`] is set when the provider truncated even
    /// this listing.
    ///
    /// # Errors
    ///
    /// As [`GitHubClient::tree`].
    pub async fn directory(
        &self,
        repository: &RepositoryIdentity,
        tree: &ObjectSha,
        prefix: &str,
    ) -> Result<TreeListing, GitHubError> {
        let (resource, wire) = self.tree_wire(repository, tree, false).await?;
        let mut listing = TreeListing {
            sha: tree.clone(),
            truncated: wire.truncated,
            entries: Vec::new(),
            invalid_paths: 0,
        };
        for entry in wire.tree {
            listing.push(&resource, prefix, entry)?;
        }
        Ok(listing)
    }

    /// Compare `base` with `head` and sort the changed files into paths
    /// removed from the base and paths present at the head.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the provider compares another
    /// base or, for an `ahead` comparison, ends at another head;
    /// [`GitHubError::MalformedResponse`] for an unknown file status or a
    /// rename without its previous path; plus any policy or transport error.
    pub async fn compare(
        &self,
        repository: &RepositoryIdentity,
        base: &ObjectSha,
        head: &ObjectSha,
    ) -> Result<Comparison, GitHubError> {
        let resource = format!("/repositories/{}/compare/{base}...{head}", repository.id());
        let wire: CompareWire = self
            .get_json(Operation::Compare, &resource, ACCEPT_JSON)
            .await?;
        let ahead = wire.status == "ahead";
        let last = wire.commits.last().map(|commit| &commit.sha);
        if wire.base_commit.sha != *base || (ahead && last != Some(head)) {
            return Err(GitHubError::IdentityMismatch {
                expected: format!("{base}...{head}"),
                actual: format!(
                    "{}...{}",
                    wire.base_commit.sha,
                    last.map_or_else(String::new, ToString::to_string)
                ),
            });
        }
        let mut comparison = Comparison {
            ahead,
            truncated: wire.files.len() >= COMPARE_MAX_FILES,
            files: wire.files.len(),
            removed: Vec::new(),
            added: Vec::new(),
            invalid_added: 0,
        };
        let malformed = |detail: String| GitHubError::MalformedResponse {
            resource: resource.clone(),
            detail,
        };
        for file in wire.files {
            let status = FileStatus::parse(&file.status)
                .ok_or_else(|| malformed(format!("unknown status {}", file.status)))?;
            match status {
                FileStatus::Added | FileStatus::Copied => comparison.add(&file.filename),
                FileStatus::Removed => comparison.remove(&file.filename),
                FileStatus::Modified | FileStatus::Changed => {
                    comparison.remove(&file.filename);
                    comparison.add(&file.filename);
                }
                FileStatus::Renamed => {
                    let previous = file.previous_filename.as_deref().ok_or_else(|| {
                        malformed(format!("rename of {} without previous path", file.filename))
                    })?;
                    comparison.remove(previous);
                    comparison.add(&file.filename);
                }
                FileStatus::Unchanged => {}
            }
        }
        Ok(comparison)
    }

    /// Compare `base` with `head` three-dot and keep every changed file with
    /// its patch and head-side blob id.
    ///
    /// The files are the merge base's changes to the head. The provider's
    /// base must be `base`; when the head is ahead and the commit list is
    /// below the provider's page of 250, its last commit must be `head`.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the provider compares another
    /// base or ends at another head, [`GitHubError::MalformedResponse`] for
    /// an unknown comparison status, plus any policy or transport error.
    /// Individual bad files are reported in [`FileComparison::rejected`].
    pub async fn compare_files(
        &self,
        repository: &RepositoryIdentity,
        base: &ObjectSha,
        head: &ObjectSha,
    ) -> Result<FileComparison, GitHubError> {
        let resource = format!("/repositories/{}/compare/{base}...{head}", repository.id());
        let wire: FileCompareWire = self
            .get_json(Operation::Compare, &resource, ACCEPT_JSON)
            .await?;
        let status = match wire.status.as_str() {
            "ahead" => ComparisonStatus::Ahead,
            "identical" => ComparisonStatus::Identical,
            "behind" => ComparisonStatus::Behind,
            "diverged" => ComparisonStatus::Diverged,
            other => {
                return Err(GitHubError::MalformedResponse {
                    resource,
                    detail: format!("unknown comparison status `{other}`"),
                });
            }
        };
        let last = wire.commits.last().map(|commit| &commit.sha);
        let paged = wire.commits.len() >= COMPARE_COMMITS_PER_PAGE;
        let ends_elsewhere = status == ComparisonStatus::Ahead && !paged && last != Some(head);
        if wire.base_commit.sha != *base || ends_elsewhere {
            return Err(GitHubError::IdentityMismatch {
                expected: format!("{base}...{head}"),
                actual: format!(
                    "{}...{}",
                    wire.base_commit.sha,
                    last.map_or_else(String::new, ToString::to_string)
                ),
            });
        }
        let mut comparison = FileComparison {
            status,
            merge_base: wire.merge_base_commit.sha,
            truncated: wire.files.len() >= COMPARE_MAX_FILES,
            files: Vec::with_capacity(wire.files.len()),
            rejected: Vec::new(),
        };
        for file in wire.files {
            match validate_pull_request_file(&file) {
                Ok(valid) => comparison.files.push(valid),
                Err(reason) => comparison.rejected.push(RejectedPath {
                    path: file.filename,
                    reason,
                }),
            }
        }
        Ok(comparison)
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
        let resource = format!("/repositories/{}/git/trees/{tree}{query}", repository.id());
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
        let resource = format!("/repositories/{}/git/blobs/{sha}", repository.id());
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
        let resource = format!("/repositories/{}/tarball/{commit}", repository.id());
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
        let resource = format!("/repositories/{}/pulls/{number}", repository.id());
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
            url: wire.html_url,
            title: wire.title,
            body: wire.body.unwrap_or_default(),
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
            "/repositories/{}/pulls/{number}/files?per_page={PR_FILES_PER_PAGE}&page={page}",
            repository.id()
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

    /// Fetch one page (1-based) of the pull requests closed into `base`,
    /// most recently updated first, keeping the merged ones.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] for an entry into another base
    /// branch or whose base repository id is absent or not `repository`'s,
    /// [`GitHubError::MalformedResponse`] for PR number 0, a time
    /// that is not `YYYY-MM-DDTHH:MM:SSZ`, or a merge after the last
    /// update, plus any policy or transport error. One bad entry fails the
    /// page, so a listing is never silently partial.
    pub async fn closed_pull_requests(
        &self,
        repository: &RepositoryIdentity,
        base: &str,
        page: u32,
    ) -> Result<ClosedPullRequestsPage, GitHubError> {
        let resource = format!(
            "/repositories/{}/pulls?state=closed&base={}&sort=updated&direction=desc&per_page={PULL_REQUESTS_PER_PAGE}&page={page}",
            repository.id(),
            encode_segment(base)
        );
        let mut response = self
            .request(Operation::PullRequests, &resource, ACCEPT_JSON, true)
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
        let wire: Vec<ListedPullRequestWire> = parse_json(&bytes, &resource)?;
        let mut listed = ClosedPullRequestsPage {
            merged: Vec::new(),
            listed: Vec::new(),
            oldest_update: None,
            has_next,
        };
        for wire in wire {
            let pr = listed_pull_request(wire, repository, &resource)?;
            if pr.base_branch != base {
                return Err(GitHubError::IdentityMismatch {
                    expected: format!("PR into `{base}`"),
                    actual: format!("PR #{} into `{}`", pr.number, pr.base_branch),
                });
            }
            listed.listed.push((pr.number, pr.updated_at.clone()));
            if let Some(merged_at) = pr.merged_at {
                listed.merged.push(MergedPr {
                    number: pr.number,
                    merged_at,
                    base: pr.base,
                    head: pr.head,
                });
            }
            listed.oldest_update = Some(pr.updated_at);
        }
        Ok(listed)
    }

    /// Look up `owner/name` and return its verified identity with its
    /// description, for a caller that knows only the name.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] when the provider names another
    /// repository, [`GitHubError::MalformedResponse`] for an unusable id or
    /// name, plus any policy or transport error.
    pub async fn find_repository(
        &self,
        owner: &str,
        name: &str,
    ) -> Result<(RepositoryIdentity, Repository), GitHubError> {
        let resource = format!("/repos/{}/{}", encode_segment(owner), encode_segment(name));
        let wire: RepositoryWire = self
            .get_json(Operation::Repository, &resource, ACCEPT_JSON)
            .await?;
        let identity = RepositoryIdentity::new(wire.id, owner, name).map_err(|error| {
            GitHubError::MalformedResponse {
                resource: resource.clone(),
                detail: error.to_string(),
            }
        })?;
        if !wire.full_name.eq_ignore_ascii_case(&identity.full_name()) {
            return Err(GitHubError::IdentityMismatch {
                expected: identity.full_name(),
                actual: wire.full_name,
            });
        }
        Ok((
            identity,
            Repository {
                full_name: wire.full_name,
                default_branch: wire.default_branch,
                private: wire.private,
            },
        ))
    }

    /// Fetch one page (1-based) of every pull request in any state, oldest
    /// created first, so pages already read can only be appended to.
    ///
    /// # Errors
    ///
    /// [`GitHubError::IdentityMismatch`] for an entry whose base repository
    /// id is absent or not `repository`'s, [`GitHubError::MalformedResponse`]
    /// for PR number 0, a time that is not `YYYY-MM-DDTHH:MM:SSZ`, or a
    /// merge after the last update, plus any policy or transport error. One
    /// bad entry fails the page.
    pub async fn pull_requests(
        &self,
        repository: &RepositoryIdentity,
        page: u32,
    ) -> Result<PullRequestsPage, GitHubError> {
        let resource = format!(
            "/repositories/{}/pulls?state=all&sort=created&direction=asc&per_page={PULL_REQUESTS_PER_PAGE}&page={page}",
            repository.id()
        );
        let mut response = self
            .request(Operation::PullRequests, &resource, ACCEPT_JSON, true)
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
        let wire: Vec<ListedPullRequestWire> = parse_json(&bytes, &resource)?;
        let prs = wire
            .into_iter()
            .map(|wire| listed_pull_request(wire, repository, &resource))
            .collect::<Result<_, _>>()?;
        Ok(PullRequestsPage { prs, has_next })
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

    /// Compare sorts files into base and head paths (a rename is both, a
    /// copy only its new path), counts non-canonical paths, flags a list at
    /// the provider ceiling, and rejects another base, another head, or an
    /// unknown status.
    #[tokio::test]
    async fn compare_sorts_changed_paths_and_verifies_its_ends() {
        let (client, fake) = client();
        let repo = repo();
        let (base, head) = (sha('b'), sha('d'));
        let body = |base: &ObjectSha, head: &ObjectSha, status: &str| {
            format!(
                r#"{{"status":"ahead","base_commit":{{"sha":"{base}"}},"commits":[{{"sha":"{head}"}}],"files":[
                    {{"filename":"a.rs","status":"added"}},
                    {{"filename":"b.rs","status":"removed"}},
                    {{"filename":"c.rs","status":"{status}"}},
                    {{"filename":"new.rs","status":"renamed","previous_filename":"old.rs"}},
                    {{"filename":"copy.rs","status":"copied","previous_filename":"a.rs"}},
                    {{"filename":"bad\\name.rs","status":"added"}}
                ]}}"#
            )
        };
        fake.push_json(&body(&base, &head, "modified"));
        let comparison = client.compare(&repo, &base, &head).await.expect("compare");
        let paths = |paths: &[RelativePath]| -> Vec<String> {
            paths.iter().map(|path| path.as_str().to_owned()).collect()
        };
        assert!(comparison.ahead && !comparison.truncated);
        assert_eq!(comparison.files, 6);
        assert_eq!(paths(&comparison.removed), ["b.rs", "c.rs", "old.rs"]);
        assert_eq!(
            paths(&comparison.added),
            ["a.rs", "c.rs", "new.rs", "copy.rs"]
        );
        assert_eq!(comparison.invalid_added, 1);
        assert!(
            fake.seen()[0]
                .0
                .ends_with(&format!("/repositories/1296269/compare/{base}...{head}"))
        );

        let files: Vec<String> = (0..COMPARE_MAX_FILES)
            .map(|i| format!(r#"{{"filename":"f{i}.rs","status":"added"}}"#))
            .collect();
        fake.push_json(&format!(
            r#"{{"status":"ahead","base_commit":{{"sha":"{base}"}},"commits":[{{"sha":"{head}"}}],"files":[{}]}}"#,
            files.join(",")
        ));
        assert!(
            client
                .compare(&repo, &base, &head)
                .await
                .expect("ceiling")
                .truncated
        );

        fake.push_json(&body(&sha('e'), &head, "modified"));
        fake.push_json(&body(&base, &sha('e'), "modified"));
        for _ in 0..2 {
            assert!(matches!(
                client.compare(&repo, &base, &head).await,
                Err(GitHubError::IdentityMismatch { .. })
            ));
        }
        fake.push_json(&body(&base, &head, "exploded"));
        assert!(matches!(
            client.compare(&repo, &base, &head).await,
            Err(GitHubError::MalformedResponse { .. })
        ));
    }

    /// Repository reads address the stable id, so a stale `owner/name`
    /// still verifies; a provider answering with another id is refused.
    #[tokio::test]
    async fn repository_is_read_and_verified_by_id() {
        let (client, fake) = client();
        let stale = RepositoryIdentity::new(1296269, "old-owner", "old-name").expect("repo");
        fake.push_json(
            r#"{"id":1296269,"full_name":"octocat/Hello-World","default_branch":"main","private":false}"#,
        );
        let repository = client.repository(&stale).await.expect("repository");
        assert_eq!(repository.full_name, "octocat/Hello-World");
        assert!(fake.seen()[0].0.ends_with("/repositories/1296269"));
        fake.push_json(
            r#"{"id":7,"full_name":"octocat/Hello-World","default_branch":"main","private":false}"#,
        );
        assert!(matches!(
            client.repository(&stale).await,
            Err(GitHubError::IdentityMismatch { .. })
        ));
    }

    /// Name-only discovery reads for a caller that knows no repository id:
    /// a repository looked up by name comes back with its verified
    /// identity, and one naming another repository is rejected; the
    /// all-PR listing keeps every state and base, reports its next page,
    /// and fails the page on an unusable entry or a provider error. Both
    /// listings accept a forked head of the requested base repository and
    /// reject an entry whose base repository id is missing or another's.
    #[tokio::test]
    async fn repository_lookup_and_pull_request_listing_are_verified() {
        let (client, fake) = client();
        fake.push_json(
            r#"{"id":1296269,"full_name":"Octocat/Hello-World","default_branch":"main","private":false}"#,
        );
        let (identity, repository) = client
            .find_repository("octocat", "Hello-World")
            .await
            .expect("repository");
        assert_eq!(identity, repo());
        assert_eq!(repository.default_branch, "main");
        assert!(fake.seen()[0].0.ends_with("/repos/octocat/Hello-World"));
        fake.push_json(
            r#"{"id":7,"full_name":"other/repo","default_branch":"main","private":false}"#,
        );
        assert!(matches!(
            client.find_repository("octocat", "Hello-World").await,
            Err(GitHubError::IdentityMismatch { .. })
        ));
        fake.push(404, &[], "");
        assert!(client.find_repository("octocat", "gone").await.is_err());

        let listed = |number: u64, merged: &str, base: &str, repo: &str| {
            format!(
                r#"{{"number":{number},"updated_at":"2026-01-02T00:00:00Z","merged_at":{merged},"base":{{"ref":"{base}","sha":"{}"{repo}}},"head":{{"ref":"f","sha":"{}","repo":{{"id":99}}}}}}"#,
                sha('a'),
                sha('b')
            )
        };
        let entry = |number: u64, merged: &str, base: &str| {
            listed(number, merged, base, r#","repo":{"id":1296269}"#)
        };
        fake.push(
            200,
            &[("link", r#"<x>; rel="next""#)],
            format!(
                "[{},{}]",
                entry(1, r#""2026-01-01T00:00:00Z""#, "main"),
                entry(2, "null", "release")
            ),
        );
        let page = client.pull_requests(&repo(), 1).await.expect("page");
        assert!(page.has_next);
        assert_eq!(
            page.prs,
            [
                ListedPullRequest {
                    number: 1,
                    updated_at: "2026-01-02T00:00:00Z".to_owned(),
                    merged_at: Some("2026-01-01T00:00:00Z".to_owned()),
                    base_branch: "main".to_owned(),
                    base: sha('a'),
                    head: sha('b'),
                },
                ListedPullRequest {
                    number: 2,
                    updated_at: "2026-01-02T00:00:00Z".to_owned(),
                    merged_at: None,
                    base_branch: "release".to_owned(),
                    base: sha('a'),
                    head: sha('b'),
                },
            ]
        );
        assert!(fake.seen().last().expect("seen").0.ends_with(
            "/repositories/1296269/pulls?state=all&sort=created&direction=asc&per_page=100&page=1"
        ));
        for bad in [
            entry(0, "null", "main"),
            entry(3, r#""2026-01-03T00:00:00Z""#, "main"),
            entry(3, r#""yesterday""#, "main"),
        ] {
            fake.push_json(&format!("[{bad}]"));
            assert!(matches!(
                client.pull_requests(&repo(), 1).await,
                Err(GitHubError::MalformedResponse { .. })
            ));
        }
        fake.push(404, &[], "");
        assert!(client.pull_requests(&repo(), 2).await.is_err());

        let merged = r#""2026-01-01T00:00:00Z""#;
        fake.push_json(&format!("[{}]", entry(1, merged, "main")));
        let closed = client
            .closed_pull_requests(&repo(), "main", 1)
            .await
            .expect("closed page");
        assert_eq!(closed.merged.len(), 1);
        for repo_field in ["", r#","repo":null"#, r#","repo":{"id":7}"#] {
            let foreign = format!("[{}]", listed(1, merged, "main", repo_field));
            fake.push_json(&foreign);
            assert!(matches!(
                client.pull_requests(&repo(), 1).await,
                Err(GitHubError::IdentityMismatch { .. })
            ));
            fake.push_json(&foreign);
            assert!(matches!(
                client.closed_pull_requests(&repo(), "main", 1).await,
                Err(GitHubError::IdentityMismatch { .. })
            ));
        }
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
            r#"{{"name":"main","commit":{{"sha":"{commit}","commit":{{"tree":{{"sha":"{tree}"}},"committer":{{"date":"2026-01-02T03:04:05Z"}}}}}}}}"#
        ));
        let head = client.branch_head(&repo, "main").await.expect("branch");
        assert_eq!((&head.commit, &head.tree), (&commit, &tree));
        assert_eq!(head.committed_at, "2026-01-02T03:04:05Z");

        fake.push_json(&format!(
            r#"{{"sha":"{commit}","tree":{{"sha":"{tree}"}},"committer":{{"date":"2026-01-02T03:04:05Z"}}}}"#
        ));
        let pinned = client.commit(&repo, &commit).await.expect("commit");
        assert_eq!(
            (&pinned.tree, pinned.committed_at.as_str()),
            (&tree, "2026-01-02T03:04:05Z")
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
            url.starts_with("https://api.github.com/repositories/1296269") && *authorized
        }));

        // Mismatched identities are rejected before any data is returned.
        fake.push_json(
            r#"{"id":1,"full_name":"octocat/Hello-World","default_branch":"main","private":true}"#,
        );
        assert!(matches!(
            client.repository(&repo).await,
            Err(GitHubError::IdentityMismatch { .. })
        ));
        fake.push_json(&format!(
            r#"{{"sha":"{tree}","tree":{{"sha":"{tree}"}},"committer":{{"date":"2026-01-02T03:04:05Z"}}}}"#
        ));
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
        fake.push_json(r#"{"name":"other","commit":{"sha":"cccccccccccccccccccccccccccccccccccccccc","commit":{"tree":{"sha":"7777777777777777777777777777777777777777"},"committer":{"date":"2026-01-02T03:04:05Z"}}}}"#);
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
            "https://api.github.com/repositories/1296269/tarball/cccccccccccccccccccccccccccccccccccccccc"
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
            "https://ghe.example.com/api/v3/repositories/1296269/tarball/cccccccccccccccccccccccccccccccccccccccc"
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
