//! Outer review-context pipeline library: the only crate in this workspace
//! with Git provider or storage IO.
//!
//! This crate obtains verified GitHub/GHES repository, commit, tree, blob,
//! and pull-request identities through provider APIs ([`GitHubClient`]) and
//! defines the narrow async [`ObjectStorage`], [`SnapshotStore`], and
//! [`ContextStore`] traits the stage facades publish through. Every call is
//! an `async fn` on tokio that the caller cancels by dropping its future. [`ColdBuild`] is the first facade: it
//! streams a pinned default-branch archive into a complete, source-free
//! [`Snapshot`], and [`Snapshots`] reads snapshots back and removes those no
//! pointer names. [`Update`] advances that pointer incrementally from tree
//! differences and reports review-time [`Freshness`]. [`PrContext`]
//! overlays a pull request's pinned head on a private copy of that
//! snapshot, or on a bounded scoped index, ranks and packs candidates, and
//! publishes one shared [`ReviewContext`] that [`Contexts`] reads back by
//! id under the caller's identity until it expires. It never clones,
//! checks out, or persists source; it depends on the public
//! `bohmian-code-review` facade alone and imports no consumer type.

mod archive;
pub mod context;
pub mod eligibility;
pub mod github;
pub mod identity;
pub mod persistence;
pub mod pull_request;
pub mod snapshot;
pub mod storage;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod update;

pub use context::{
    Budget, CONTEXT_FORMAT_VERSION, CONTEXT_LIFETIME, ContextCleanup, ContextCoverage,
    ContextError, Contexts, NotAnalyzed, NotAnalyzedReason, OmittedReason, OmittedSpan,
    ReviewContext, Scope, SelectedSpan,
};
pub use eligibility::ExclusionReason;
pub use github::{
    Archive, BoxFuture, BranchHead, CallCounts, Commit, Disposition, FileStatus, GitHubClient,
    GitHubError, HttpBody, HttpRequest, HttpResponse, HttpTransport, Operation, PullRequest,
    PullRequestFile, PullRequestFilesPage, PullRequestState, RejectedPath, Repository,
    RequestLimits, ReqwestTransport, TransportError, TreeEntry, TreeEntryMode, TreeListing,
};
pub use identity::{
    GitHubHost, Identity, InstallationId, InstallationToken, InvalidIdentity, ObjectSha,
    RepositoryIdentity, TenantId,
};
pub use persistence::{
    ContextId, ContextRecord, ContextStore, PersistenceError, SnapshotPointer, SnapshotScope,
    SnapshotStore,
};
pub use pull_request::{PrContext, PrContextLimits, PrContextResult};
pub use snapshot::{
    ColdBuild, ColdBuildError, ColdBuildLimits, ColdBuildOutcome, ColdBuildResult, Coverage,
    FileOutcome, IncompleteReason, IndexedFile, IngestStats, SNAPSHOT_FORMAT_VERSION, SkipReason,
    SkippedFile, Snapshot, SnapshotFile, Snapshots,
};
pub use storage::{ObjectDigest, ObjectKey, ObjectStorage, StorageError};
pub use update::{Freshness, Update};
