//! Outer review-context pipeline library: the only crate in this workspace
//! with Git provider or storage IO.
//!
//! This crate obtains verified GitHub/GHES repository, commit, tree, blob,
//! and pull-request identities through provider APIs ([`GitHubClient`]),
//! bounds every call by a caller-supplied [`Execution`], and defines the
//! narrow [`ObjectStorage`], [`SnapshotStore`], and [`ContextStore`] traits the stage facades
//! publish through. [`ColdBuild`] is the first facade: it streams a pinned
//! default-branch archive into a complete, source-free [`Snapshot`]. It never clones, checks out, or persists source; it
//! depends on the public `bohmian-code-review` facade alone and imports no
//! consumer type.

pub mod eligibility;
pub mod execution;
pub mod github;
pub mod identity;
pub mod persistence;
pub mod snapshot;
pub mod storage;
#[cfg(test)]
pub(crate) mod testing;

pub use eligibility::{
    EligibleTree, Exclusion, ExclusionReason, MAX_ELIGIBLE_FILE_BYTES, RootAttributes, classify,
};
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
    ContextId, ContextRecord, ContextStore, PersistenceError, SnapshotPointer, SnapshotScope,
    SnapshotStore,
};
pub use snapshot::{
    ColdBuild, ColdBuildError, ColdBuildLimits, ColdBuildOutcome, ColdBuildResult, Coverage,
    IncompleteReason, MAX_COLD_BUILD_DEADLINE, SNAPSHOT_FORMAT_VERSION, SkipReason, SkippedFile,
    Snapshot, SnapshotFile, read_snapshot,
};
pub use storage::{ObjectDigest, ObjectKey, ObjectStorage, StorageError};
