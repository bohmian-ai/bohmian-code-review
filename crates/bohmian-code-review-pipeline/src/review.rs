//! The consumer-facing pipeline: one type over one object storage and one
//! metadata store serving both snapshots and contexts.
//!
//! [`ReviewPipeline`] composes the stage facades and adds only one decision:
//! [`ReviewPipeline::context`] cold-builds when no snapshot pointer exists,
//! and otherwise reviews on the current snapshot without refreshing it.
//! Refreshing belongs to push events through [`ReviewPipeline::refresh`].
//! The GitHub client is a per-call argument because installation tokens
//! are short-lived while the pipeline and its stores live in app state.

use crate::context::{ContextError, Contexts, ReviewContext};
use crate::github::GitHubClient;
use crate::identity::Identity;
use crate::persistence::{ContextId, ContextStore, SnapshotStore};
use crate::pull_request::{PrContext, PrContextLimits, PrContextResult};
use crate::snapshot::{ColdBuildError, ColdBuildLimits, ColdBuildResult, snapshot_scope};
use crate::storage::ObjectStorage;
use crate::update::Update;
use thiserror::Error;

/// Ceilings for every stage the pipeline runs.
///
/// `cold.max_file_bytes` and `pr.max_file_bytes` select the snapshot scope
/// each stage uses and must agree, as the defaults do; otherwise reviews
/// never find the snapshot a refresh publishes and fall back to scoped
/// context.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PipelineLimits {
    /// Cold-build and update ceilings.
    pub cold: ColdBuildLimits,
    /// PR-context ceilings.
    pub pr: PrContextLimits,
}

/// Why a pipeline call failed; the stage error is kept whole so its retry
/// states (`PointerConflict`, `Persistence(Collecting)`, `History(ListingMoved)`,
/// `PullRequestMoved`) stay matchable.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PipelineError {
    /// The snapshot build or update failed.
    #[error(transparent)]
    Snapshot(#[from] ColdBuildError),
    /// The PR context build, publication, or read failed.
    #[error(transparent)]
    Context(#[from] ContextError),
}

/// The review-context pipeline over the caller's object storage and one
/// store implementing both [`SnapshotStore`] and [`ContextStore`].
pub struct ReviewPipeline<'a, O, S> {
    storage: &'a O,
    store: &'a S,
    limits: PipelineLimits,
}

impl<'a, O: ObjectStorage, S: SnapshotStore + ContextStore> ReviewPipeline<'a, O, S> {
    /// Pipeline over long-lived storage and store with fixed ceilings.
    pub fn new(storage: &'a O, store: &'a S, limits: PipelineLimits) -> ReviewPipeline<'a, O, S> {
        ReviewPipeline {
            storage,
            store,
            limits,
        }
    }

    /// Build and publish the review context of pull request `pr`.
    ///
    /// When `identity` has no snapshot pointer under the current versions,
    /// runs [`ReviewPipeline::refresh`] first, which cold-builds. Otherwise
    /// only the pointer is read and the existing snapshot is used as is,
    /// however far the default branch has moved; a missing snapshot object
    /// yields scoped context rather than a rebuild. A cold build that ends
    /// incomplete publishes no pointer, so the review uses scoped context.
    /// Dropping the future cancels as each stage documents.
    ///
    /// # Errors
    ///
    /// `Snapshot` for the pointer read or the errors of [`Update::run`];
    /// `Context` for the errors of [`PrContext::run`].
    pub async fn context(
        &self,
        client: &GitHubClient,
        identity: &Identity,
        pr: u64,
    ) -> Result<PrContextResult, PipelineError> {
        let scope = snapshot_scope(identity, self.limits.cold.max_file_bytes);
        if self
            .store
            .snapshot_pointer(&scope)
            .await
            .map_err(ColdBuildError::from)?
            .is_none()
        {
            self.refresh(client, identity).await?;
        }
        let result = PrContext::new(
            client,
            self.storage,
            self.store,
            self.storage,
            self.store,
            self.limits.pr.clone(),
        )
        .run(identity, pr)
        .await?;
        Ok(result)
    }

    /// Advance `identity`'s snapshot to the default branch head and extend
    /// its co-change history, cold-building when no pointer exists.
    ///
    /// # Errors
    ///
    /// The errors of [`Update::run`] as `Snapshot`.
    pub async fn refresh(
        &self,
        client: &GitHubClient,
        identity: &Identity,
    ) -> Result<ColdBuildResult, PipelineError> {
        let result = Update::new(client, self.storage, self.store, self.limits.cold.clone())
            .run(identity)
            .await?;
        Ok(result)
    }

    /// Read back the context `id` published for `identity`, without GitHub
    /// access.
    ///
    /// # Errors
    ///
    /// The errors of [`Contexts::read`] as `Context`.
    pub async fn read(
        &self,
        identity: &Identity,
        id: &ContextId,
    ) -> Result<ReviewContext, PipelineError> {
        let context = Contexts::new(self.storage, self.store)
            .read(identity, id)
            .await?;
        Ok(context)
    }
}
