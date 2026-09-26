//! Outer review-context pipeline library: the only crate in this workspace
//! with Git provider or storage IO.
//!
//! This crate obtains verified GitHub/GHES repository, commit, tree, blob,
//! and pull-request identities through provider APIs ([`GitHubClient`]),
//! bounds every call by a caller-supplied [`Execution`], and defines the
//! narrow [`ObjectStorage`] and [`Persistence`] traits later stage facades
//! publish through. It never clones, checks out, or persists source; it
//! depends on the public `bohmian-code-review` facade alone and imports no
//! consumer type.

pub mod execution;
pub mod github;
pub mod identity;
pub mod persistence;
pub mod storage;

pub use execution::{CancelToken, Execution, Interrupted};
pub use github::{
    Archive, BranchHead, CallCounts, Commit, Disposition, FileStatus, GitHubClient, GitHubError,
    HttpRequest, HttpResponse, HttpTransport, Operation, PullRequest, PullRequestFile,
    PullRequestFilesPage, PullRequestState, RejectedPath, Repository, RequestLimits,
    TransportError, TreeEntry, TreeEntryMode, TreeListing, UreqTransport,
};
pub use identity::{
    GitHubHost, Identity, InstallationId, InstallationToken, InvalidIdentity, ObjectSha,
    RepositoryIdentity, TenantId,
};
pub use persistence::{
    ContextId, ContextRecord, Persistence, PersistenceError, SnapshotPointer, SnapshotScope,
};
pub use storage::{ObjectDigest, ObjectKey, ObjectStorage, StorageError};
