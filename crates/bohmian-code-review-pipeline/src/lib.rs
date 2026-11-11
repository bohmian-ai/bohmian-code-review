//! Outer review-context pipeline library: the only crate in this workspace
//! with Git provider or storage IO.
//!
//! Most consumers need one type: [`ReviewPipeline`], over their
//! [`ObjectStorage`] and one store implementing both [`SnapshotStore`] and
//! [`ContextStore`], with [`PipelineLimits`] and [`PipelineError`].
//! [`ReviewPipeline::context`] builds and publishes a pull request's
//! [`ReviewContext`], cold-building first only when no snapshot exists;
//! [`ReviewPipeline::refresh`] advances the snapshot and its co-change
//! history on a push; [`ReviewPipeline::read`] reads a context back by id.
//! [`ReviewPipeline::pinned`] reviews exactly the base and head commits a
//! caller froze, against an optional earlier reviewed head, and returns the
//! complete and active change sets with their source beside the context
//! selected for the active changes.
//! The [`GitHubClient`] is passed per call.
//!
//! The stage facades stay public for advanced callers. Every call is an
//! `async fn` on tokio that the caller cancels by dropping its future.
//! [`ColdBuild`] streams a pinned default-branch archive into a complete,
//! source-free [`Snapshot`] carrying bounded [`CoChangeHistory`] of
//! recently merged PRs, and [`Snapshots`] reads snapshots back and removes
//! those no pointer names. [`Update`] advances that pointer incrementally
//! from tree differences, extends the history with PRs merged since, and
//! reports review-time [`Freshness`]. [`PrContext`] overlays a pull
//! request's pinned head on a private copy of that snapshot, or on a
//! bounded scoped index, ranks candidates with the default ranker over the
//! persisted history merged before its base commit, packs them, and
//! publishes one shared [`ReviewContext`] that [`Contexts`] reads back by
//! id under the caller's identity until it expires. It never clones,
//! checks out, or persists source; it depends on the public
//! `bohmian-code-review` facade alone and imports no consumer type.

mod archive;
pub mod comparison;
pub mod context;
pub mod eligibility;
pub mod github;
pub mod history;
pub mod identity;
pub mod persistence;
pub mod pull_request;
pub mod review;
pub mod snapshot;
pub mod storage;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod update;

pub use comparison::{
    BaselineProblem, ChangeKind, ChangeSet, ChangedFile, ComparisonError, Hunk, PinnedReview,
    PullRequestPins, PullRequestSource, Source, UnavailableReason,
};
pub use context::{
    Budget, CONTEXT_FORMAT_VERSION, CONTEXT_LIFETIME, ContextCleanup, ContextCoverage,
    ContextError, Contexts, NotAnalyzed, NotAnalyzedReason, OmittedReason, OmittedSpan,
    ReviewContext, Scope, SelectedSpan,
};
pub use eligibility::ExclusionReason;
pub use github::{
    Archive, BoxFuture, BranchHead, CallCounts, ClosedPullRequestsPage, Commit, Comparison,
    ComparisonStatus, Disposition, FileComparison, FileStatus, GitHubClient, GitHubError, HttpBody,
    HttpRequest, HttpResponse, HttpTransport, ListedPullRequest, Operation, PullRequest,
    PullRequestFile, PullRequestFilesPage, PullRequestState, PullRequestsPage, RejectedPath,
    Repository, RequestLimits, ReqwestTransport, TransportError, TreeEntry, TreeEntryMode,
    TreeListing,
};
pub use history::{
    COLD_DEPTH, CoChangeHistory, History, HistoryError, HistoryPr, Listing, ListingReads,
    MASS_EDIT_FILES, MAX_DISCOVERY_PAGES, MAX_HISTORY_PRS, MAX_LISTING_PAGES, MergedPr,
    is_timestamp,
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
pub use review::{PipelineError, PipelineLimits, ReviewPipeline};
pub use snapshot::{
    ColdBuild, ColdBuildError, ColdBuildLimits, ColdBuildOutcome, ColdBuildResult, Coverage,
    FileOutcome, IncompleteReason, IndexedFile, IngestStats, SNAPSHOT_FORMAT_VERSION, SkipReason,
    SkippedFile, Snapshot, SnapshotFile, Snapshots,
};
pub use storage::{ObjectDigest, ObjectKey, ObjectStorage, StorageError};
pub use update::{Freshness, Update};
