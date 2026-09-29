//! Cold default-branch snapshot build, publication, read-back, and cleanup.
//!
//! [`ColdBuild::run`] pins the default branch, classifies its tree with
//! `classify`, streams the commit tarball through bounded decompression,
//! verifies every eligible entry against its Git blob id, falls back to
//! bounded verified blob calls for anything the archive omitted or
//! substituted, and indexes through the public [`SymbolicContext`]. Source
//! bytes live only while one entry is being parsed. Before any source is
//! read, the build reads the default branch's [`CoChangeHistory`] before the
//! pinned commit's committer time, which the snapshot persists beside the
//! facts.
//!
//! A complete [`Snapshot`] is published in three idempotent steps: upsert
//! its record (refreshing `touched_at`), create its object if absent, and
//! compare-and-swap the pointer. An incomplete attempt writes nothing and
//! returns exact coverage. Dropping the build future at any `.await` leaves
//! the pointer on a complete snapshot or absent and every object recorded,
//! and a retry converges on the same key. [`Snapshots`] reads published
//! snapshots and collects records no pointer names.

use crate::archive::{TarStream, VerifiedEntry};
use crate::eligibility::{ExclusionReason, MAX_ELIGIBLE_FILE_BYTES, RootAttributes, classify};
use crate::github::{Archive, BranchHead, GitHubClient, GitHubError, TreeEntry, TreeEntryMode};
use crate::history::{CoChangeHistory, HistoryError};
use crate::identity::{Identity, ObjectSha};
use crate::persistence::{PersistenceError, SnapshotPointer, SnapshotScope, SnapshotStore};
use crate::storage::{ObjectDigest, ObjectKey, ObjectStorage, StorageError};
use bohmian_code_review::Error as SymbolicError;
use bohmian_code_review::{EXTRACTION_VERSION, IndexedFileFacts, RelativePath, SymbolicContext};
use flate2::write::GzDecoder;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::time::Duration;
use thiserror::Error;

/// Layout version of the serialized [`Snapshot`]; part of every snapshot key.
pub const SNAPSHOT_FORMAT_VERSION: u32 = 4;

/// Git LFS pointer files start with this line and are skipped by content.
const LFS_POINTER_PREFIX: &[u8] = b"version https://git-lfs.github.com/spec/v1";

/// Ceilings for one cold build beyond the provider request limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColdBuildLimits {
    /// Largest eligible file; larger blobs are excluded or skipped.
    pub max_file_bytes: u64,
    /// Largest decompressed archive stream before ingest stops reading it.
    pub max_expanded_archive_bytes: u64,
    /// Verified blob calls allowed for entries the archive did not supply.
    pub max_fallback_blobs: usize,
    /// Changed blobs an update fetches individually; beyond it the update
    /// streams the archive for them instead.
    pub max_changed_blobs: usize,
}

impl Default for ColdBuildLimits {
    /// 1 MiB files, 4 GiB expanded archive, 200 fallback blobs, 2,000
    /// changed blobs.
    fn default() -> ColdBuildLimits {
        ColdBuildLimits {
            max_file_bytes: MAX_ELIGIBLE_FILE_BYTES,
            max_expanded_archive_bytes: 4 * 1024 * 1024 * 1024,
            max_fallback_blobs: 200,
            max_changed_blobs: 2000,
        }
    }
}

/// Why an eligible file was not indexed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkipReason {
    /// Content is a Git LFS pointer, not source.
    LfsPointer,
    /// Fetched bytes, or a provider ceiling at least the file bound, proved
    /// the blob exceeds the file bound.
    Oversize,
    /// The verified content extracted to no declarations.
    NoFacts,
    /// The extractor rejected the content; `detail` carries no source.
    ParseFailed {
        /// Extractor error text.
        detail: String,
    },
    /// The fallback blob call failed; `detail` carries no source.
    FetchFailed {
        /// Provider error text.
        detail: String,
    },
    /// The archive omitted it and the fallback bound was reached.
    NotFetched,
}

impl SkipReason {
    /// Whether a complete snapshot may still carry this skip: the verified
    /// content itself is not indexable source, as opposed to a file the
    /// attempt failed to fetch or parse.
    fn is_benign(&self) -> bool {
        matches!(
            self,
            SkipReason::LfsPointer | SkipReason::Oversize | SkipReason::NoFacts
        )
    }
}

/// One eligible file that was not indexed, keeping its blob identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedFile {
    /// Repository-relative path.
    pub path: RelativePath,
    /// Blob id the tree listed.
    pub blob: ObjectSha,
    /// Why it was skipped.
    pub reason: SkipReason,
}

/// What one tree's facts cover, distinguishing excluded paths from
/// eligible files that were not indexed.
///
/// Only tree-determined facts live here so a published [`Snapshot`] is a
/// pure function of its tree; how the bytes were obtained is reported on
/// [`ColdBuildResult::ingest`] instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Coverage {
    /// Some tree's listing was truncated even without recursion, so the
    /// listing could not be completed; nothing was indexed.
    pub truncated_tree: bool,
    /// Eligible files in the listed tree.
    pub eligible: usize,
    /// Eligible files whose facts are in the index.
    pub indexed: usize,
    /// Eligible files not indexed, with reasons, in path order.
    pub skipped: Vec<SkippedFile>,
    /// Excluded entries counted by reason.
    pub excluded: BTreeMap<ExclusionReason, usize>,
}

impl Coverage {
    /// Coverage of a listed tree whose eligible files ended as `outcomes`,
    /// with `excluded` entries counted by reason.
    fn of(outcomes: &[FileOutcome], excluded: BTreeMap<ExclusionReason, usize>) -> Coverage {
        let mut skipped: Vec<SkippedFile> = outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                FileOutcome::Indexed(_) => None,
                FileOutcome::Skipped(file) => Some(file.clone()),
            })
            .collect();
        skipped.sort_by(|a, b| a.path.cmp(&b.path));
        Coverage {
            truncated_tree: false,
            eligible: outcomes.len(),
            indexed: outcomes.len() - skipped.len(),
            skipped,
            excluded,
        }
    }

    /// Every eligible file was indexed or skipped for a content reason.
    pub fn is_complete(&self) -> bool {
        !self.truncated_tree && self.unindexed() == 0
    }

    /// Eligible files that were neither indexed nor benignly skipped.
    fn unindexed(&self) -> usize {
        self.skipped
            .iter()
            .filter(|file| !file.reason.is_benign())
            .count()
    }
}

/// One eligible path with the blob id the pinned tree lists for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFile {
    /// Repository-relative path.
    pub path: RelativePath,
    /// Blob id from the pinned tree.
    pub blob: ObjectSha,
}

/// An eligible file indexed at `blob` with at least one persisted
/// declaration.
///
/// The fields are private and deserialization revalidates them, so no
/// value, built or read back, is an indexed file without a declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "IndexedFileRecord")]
pub struct IndexedFile {
    blob: ObjectSha,
    facts: IndexedFileFacts,
}

/// Unvalidated wire form of an [`IndexedFile`].
#[derive(Deserialize)]
struct IndexedFileRecord {
    blob: ObjectSha,
    facts: IndexedFileFacts,
}

impl TryFrom<IndexedFileRecord> for IndexedFile {
    type Error = &'static str;

    /// Accept only a record carrying a declaration.
    ///
    /// # Errors
    ///
    /// Returns a message when the record has no declarations.
    fn try_from(record: IndexedFileRecord) -> Result<IndexedFile, &'static str> {
        IndexedFile::new(record.blob, record.facts).ok_or("indexed file without declarations")
    }
}

impl IndexedFile {
    /// `facts` indexed at `blob`, or `None` when they carry no declaration.
    fn new(blob: ObjectSha, facts: IndexedFileFacts) -> Option<IndexedFile> {
        (!facts.declarations.is_empty()).then_some(IndexedFile { blob, facts })
    }

    /// Blob id the facts were extracted from.
    pub fn blob(&self) -> &ObjectSha {
        &self.blob
    }

    /// The file's persisted declaration facts; never empty.
    pub fn facts(&self) -> &IndexedFileFacts {
        &self.facts
    }
}

/// What one eligible file ended as: indexed with declarations, or skipped
/// with its blob and reason. Exactly one per eligible path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOutcome {
    /// Indexed with at least one declaration.
    Indexed(IndexedFile),
    /// Not indexed, keeping its blob identity.
    Skipped(SkippedFile),
}

impl FileOutcome {
    /// The eligible file's path.
    fn path(&self) -> &RelativePath {
        match self {
            FileOutcome::Indexed(file) => &file.facts.path,
            FileOutcome::Skipped(file) => &file.path,
        }
    }
}

/// The immutable, source-free object stored at a snapshot key.
///
/// It is keyed by commit because its co-change history depends on the
/// commit's time. Creation time and per-attempt ingest counters live
/// outside it, so the same identity, versions, commit, and history always
/// serialize to the same bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// [`SNAPSHOT_FORMAT_VERSION`] this object was written under.
    pub format_version: u32,
    /// Extractor that produced `facts`.
    pub extraction_version: String,
    /// Caller identity the key was scoped to.
    pub identity: Identity,
    /// Default-branch commit the snapshot was built at.
    pub commit: ObjectSha,
    /// Tree the snapshot indexes.
    pub tree: ObjectSha,
    /// Outcome of every eligible file, in canonical path order.
    pub files: Vec<FileOutcome>,
    /// Co-change history merged before `commit`'s committer time.
    pub history: CoChangeHistory,
}

impl Snapshot {
    /// Decode and validate object bytes against the identity, commit, and
    /// tree they were published for.
    ///
    /// # Errors
    ///
    /// Returns [`ColdBuildError::Corrupt`] when the bytes do not decode,
    /// name another identity, commit, tree, or version, or are not complete.
    fn decode(
        bytes: &[u8],
        identity: &Identity,
        commit: &ObjectSha,
        tree: &ObjectSha,
    ) -> Result<Snapshot, ColdBuildError> {
        let corrupt = |detail: String| ColdBuildError::Corrupt { detail };
        let snapshot: Snapshot =
            serde_json::from_slice(bytes).map_err(|error| corrupt(error.to_string()))?;
        if snapshot.format_version != SNAPSHOT_FORMAT_VERSION {
            return Err(corrupt(format!(
                "format version {}",
                snapshot.format_version
            )));
        }
        if snapshot.extraction_version != EXTRACTION_VERSION {
            return Err(corrupt(format!(
                "extraction version {}",
                snapshot.extraction_version
            )));
        }
        if snapshot.identity != *identity {
            return Err(corrupt("identity mismatch".to_owned()));
        }
        if snapshot.commit != *commit {
            return Err(corrupt(format!(
                "commit {} is not {commit}",
                snapshot.commit
            )));
        }
        if snapshot.tree != *tree {
            return Err(corrupt(format!("tree {} is not {tree}", snapshot.tree)));
        }
        if !snapshot.coverage().is_complete() {
            return Err(corrupt("incomplete coverage".to_owned()));
        }
        Ok(snapshot)
    }

    /// Coverage of the tree, derived from the file outcomes; always
    /// complete for a decoded snapshot. Exclusions are not persisted, so
    /// none are counted.
    pub fn coverage(&self) -> Coverage {
        Coverage::of(&self.files, BTreeMap::new())
    }

    /// Load every indexed file's facts into a fresh in-memory context
    /// without any source.
    ///
    /// # Errors
    ///
    /// Returns the facade's [`SymbolicError`] for a fact stamped by another
    /// extractor or violating a declaration invariant.
    pub fn into_context(self) -> Result<SymbolicContext, SymbolicError> {
        let mut context = SymbolicContext::new();
        for outcome in self.files {
            if let FileOutcome::Indexed(file) = outcome {
                context.load(file.facts)?;
            }
        }
        Ok(context)
    }
}

/// Read-back and collection of recorded snapshots over the caller's object
/// storage and snapshot store.
pub struct Snapshots<'a, O, S> {
    storage: &'a O,
    store: &'a S,
}

impl<'a, O: ObjectStorage, S: SnapshotStore> Snapshots<'a, O, S> {
    /// Snapshot access over the caller's storage and snapshot store.
    pub fn new(storage: &'a O, store: &'a S) -> Snapshots<'a, O, S> {
        Snapshots { storage, store }
    }

    /// Read and validate the snapshot a pointer names in `scope`.
    ///
    /// The pointer's key must equal the key this library builds for
    /// `scope` and the pointer's commit before
    /// storage is touched, so a pointer row leaked or copied from another
    /// tenant or installation can never read that identity's object.
    ///
    /// # Errors
    ///
    /// Returns [`ColdBuildError::ForeignKey`] when the pointer names any other
    /// key, [`ColdBuildError::Storage`] when the object is missing or the
    /// backend fails, and [`ColdBuildError::Corrupt`] when its bytes do not
    /// match the pointer's digest or do not decode as a complete snapshot of
    /// that identity, commit, and tree.
    pub async fn read(
        &self,
        scope: &SnapshotScope,
        pointer: &SnapshotPointer,
    ) -> Result<Snapshot, ColdBuildError> {
        let expected = snapshot_key(scope, &pointer.commit);
        if pointer.key != expected {
            return Err(ColdBuildError::ForeignKey {
                key: pointer.key.clone(),
            });
        }
        let bytes = self.storage.read(&expected).await?;
        if ObjectDigest::of(&bytes) != pointer.digest {
            return Err(ColdBuildError::Corrupt {
                detail: "digest mismatch".to_owned(),
            });
        }
        Snapshot::decode(&bytes, &scope.identity, &pointer.commit, &pointer.tree)
    }

    /// Delete every snapshot of `identity` that no pointer names and no
    /// build touched within `grace` of the store's clock, returning the
    /// deleted keys.
    ///
    /// The store claims each returned record as it selects it, so a build
    /// racing on the same key fails its record refresh with
    /// [`PersistenceError::Collecting`] before creating or pointing at the
    /// object, and a record a build refreshed first is never selected.
    /// Each object is deleted before its record, so a failure part-way
    /// leaves a claimed record the next call retries rather than an
    /// untracked object; an already missing object deletes cleanly. Calls
    /// for one identity must not overlap. The caller sets `grace` longer
    /// than its longest build so an in-flight build's record, touched when
    /// it began publishing, is never collected. Every key must be a snapshot key of `identity`, so a
    /// faulty store cannot steer deletion into another namespace.
    ///
    /// # Errors
    ///
    /// Returns [`ColdBuildError::ForeignKey`] for a key outside `identity`'s
    /// snapshot namespace, otherwise the first persistence or storage
    /// failure; keys deleted before it stay deleted.
    pub async fn collect(
        &self,
        identity: &Identity,
        grace: Duration,
    ) -> Result<Vec<ObjectKey>, ColdBuildError> {
        let mut deleted = Vec::new();
        for key in self.store.unreferenced(identity, grace).await? {
            if !key.is_snapshot_of(identity) {
                return Err(ColdBuildError::ForeignKey { key });
            }
            self.storage.delete(&key).await?;
            self.store.delete_record(identity, &key).await?;
            deleted.push(key);
        }
        Ok(deleted)
    }

    /// Remove `identity`'s snapshot namespace: drop its pointers, then
    /// collect every record with zero grace, returning the deleted keys.
    ///
    /// A failure part-way leaves the remaining records for a retry. The
    /// caller invokes it only when no build for `identity` is running.
    ///
    /// # Errors
    ///
    /// As [`Snapshots::collect`], plus the pointer removal failure.
    pub async fn remove_repository(
        &self,
        identity: &Identity,
    ) -> Result<Vec<ObjectKey>, ColdBuildError> {
        self.store.remove_pointers(identity).await?;
        self.collect(identity, Duration::ZERO).await
    }
}

/// The pointer scope of `identity` under the current versions and the
/// caller's `max_file_bytes`.
pub(crate) fn snapshot_scope(identity: &Identity, max_file_bytes: u64) -> SnapshotScope {
    SnapshotScope {
        identity: identity.clone(),
        extraction_version: EXTRACTION_VERSION.to_owned(),
        format_version: SNAPSHOT_FORMAT_VERSION,
        max_file_bytes,
    }
}

/// The only key a snapshot of `commit` in `scope` may live at.
fn snapshot_key(scope: &SnapshotScope, commit: &ObjectSha) -> ObjectKey {
    ObjectKey::snapshot(
        &scope.identity,
        &scope.extraction_version,
        scope.format_version,
        scope.max_file_bytes,
        commit,
    )
}

/// Why a complete attempt was not produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncompleteReason {
    /// Some tree's listing was truncated even without recursion, so the
    /// tree could not be listed completely.
    TruncatedTree,
    /// The compressed or decompressed archive passed its byte bound;
    /// coverage still reports what verified fallback indexed.
    ArchiveTooLarge,
    /// Some eligible files were not indexed; see [`Coverage::skipped`].
    UnindexedFiles {
        /// Eligible files neither indexed nor benignly skipped.
        count: usize,
    },
}

/// What the build did with the pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColdBuildOutcome {
    /// A new complete snapshot was written and the pointer moved to it.
    Published(SnapshotPointer),
    /// The current pointer already covered the branch tree and validated.
    Reused(SnapshotPointer),
    /// Nothing was written; the prior pointer is unchanged.
    Incomplete(IncompleteReason),
}

/// How one attempt obtained its bytes; reported, never persisted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IngestStats {
    /// Entries verified from the archive stream.
    pub archive_entries: usize,
    /// Blobs fetched individually after the archive, or as an update's
    /// changed blobs.
    pub fallback_blobs: usize,
    /// Eligible files an update carried from the prior snapshot because
    /// their path and blob were unchanged.
    pub reused_files: usize,
    /// Why the archive stream stopped early, if it did; never source text.
    pub archive_error: Option<String>,
    /// Most verified archive bytes waiting to be indexed at once.
    pub peak_verified_bytes: usize,
}

/// Result of one cold build invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColdBuildResult {
    /// Default-branch commit that was pinned.
    pub commit: ObjectSha,
    /// Its root tree.
    pub tree: ObjectSha,
    /// Exact coverage of this attempt, or of the reused snapshot.
    pub coverage: Coverage,
    /// How this attempt ingested; all zero when a snapshot was reused.
    pub ingest: IngestStats,
    /// Publication outcome.
    pub outcome: ColdBuildOutcome,
}

impl ColdBuildResult {
    /// Result for pinned `head`.
    fn new(
        head: &BranchHead,
        coverage: Coverage,
        ingest: IngestStats,
        outcome: ColdBuildOutcome,
    ) -> ColdBuildResult {
        ColdBuildResult {
            commit: head.commit.clone(),
            tree: head.tree.clone(),
            coverage,
            ingest,
            outcome,
        }
    }

    /// Incomplete result for `head` because some tree listing was truncated
    /// even without recursion; nothing was indexed.
    pub(crate) fn truncated(head: &BranchHead) -> ColdBuildResult {
        let coverage = Coverage {
            truncated_tree: true,
            ..Coverage::default()
        };
        ColdBuildResult::new(
            head,
            coverage,
            IngestStats::default(),
            ColdBuildOutcome::Incomplete(IncompleteReason::TruncatedTree),
        )
    }
}

/// Why a cold build could not produce a result at all.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ColdBuildError {
    /// A repository-level provider call failed.
    #[error(transparent)]
    GitHub(#[from] GitHubError),
    /// Object storage failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// Metadata persistence failed.
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    /// Co-change history could not be read completely.
    #[error(transparent)]
    History(#[from] HistoryError),
    /// A pointer names a key other than the caller's own snapshot key.
    #[error("snapshot pointer names a foreign key: {key}")]
    ForeignKey {
        /// The refused key.
        key: ObjectKey,
    },
    /// An object at the snapshot key is not a valid snapshot for it.
    #[error("snapshot object is corrupt: {detail}")]
    Corrupt {
        /// What failed validation; never source text.
        detail: String,
    },
    /// Another writer moved the pointer first; nothing is lost and a retry
    /// of the build converges.
    #[error("snapshot pointer changed during the build")]
    PointerConflict,
}

/// The cold-build facade over injected provider, storage, and snapshot store.
pub struct ColdBuild<'a, O, S> {
    pub(crate) client: &'a GitHubClient,
    pub(crate) storage: &'a O,
    pub(crate) store: &'a S,
    pub(crate) limits: ColdBuildLimits,
}

impl<'a, O: ObjectStorage, S: SnapshotStore> ColdBuild<'a, O, S> {
    /// Facade over the caller's capabilities and ceilings.
    pub fn new(
        client: &'a GitHubClient,
        storage: &'a O,
        store: &'a S,
        limits: ColdBuildLimits,
    ) -> ColdBuild<'a, O, S> {
        ColdBuild {
            client,
            storage,
            store,
            limits,
        }
    }

    /// Build or reuse the complete default-branch snapshot for `identity`.
    ///
    /// The caller bounds the call by dropping its future; the library
    /// imposes no maximum of its own. A current pointer at the branch tree
    /// is validated and reused; if its object has gone missing the snapshot
    /// is rebuilt at the same key. A complete build publishes through
    /// its three-step publication; a retry after a drop at any `.await`
    /// converges.
    ///
    /// # Errors
    ///
    /// Returns provider errors for the repository, branch, tree,
    /// attributes, and history calls; `History` for a history listing that
    /// is incomplete or rejected, or `History(ListingMoved)`, a retry state;
    /// storage and persistence errors, `ForeignKey` or
    /// `Corrupt` for an unusable current pointer or existing object, and
    /// `PointerConflict`, a retry state, when another writer moved the
    /// pointer first, or `Persistence(Collecting)`, a retry state, when a
    /// running collection claimed this commit's record. Per-file
    /// failures and an unusable archive never error; they appear in
    /// coverage and ingest stats.
    pub async fn run(&self, identity: &Identity) -> Result<ColdBuildResult, ColdBuildError> {
        let scope = snapshot_scope(identity, self.limits.max_file_bytes);
        let current = self.store.snapshot_pointer(&scope).await?;
        let head = self.pin(identity).await?;
        self.build(&scope, current.as_ref(), &head, None).await
    }

    /// Build or reuse the snapshot for the caller-chosen `commit` instead of
    /// the default branch's current head, so fixed-commit capacity runs
    /// measure the same tree however the branch moves.
    ///
    /// The repository identity and the commit's tree are verified with the
    /// provider, then the build proceeds exactly as [`ColdBuild::run`] and
    /// publishes the snapshot pointer at `commit`.
    ///
    /// # Errors
    ///
    /// As [`ColdBuild::run`], with the commit provider call in place of the
    /// branch call.
    pub async fn run_at(
        &self,
        identity: &Identity,
        commit: &ObjectSha,
    ) -> Result<ColdBuildResult, ColdBuildError> {
        let scope = snapshot_scope(identity, self.limits.max_file_bytes);
        let current = self.store.snapshot_pointer(&scope).await?;
        let repository = self.client.repository(&identity.repository).await?;
        let pinned = self.client.commit(&identity.repository, commit).await?;
        let head = BranchHead {
            name: repository.default_branch,
            commit: pinned.sha,
            tree: pinned.tree,
            committed_at: pinned.committed_at,
        };
        self.build(&scope, current.as_ref(), &head, None).await
    }

    /// Pin the default branch's current commit and tree.
    ///
    /// # Errors
    ///
    /// Returns the repository or branch provider error.
    pub(crate) async fn pin(&self, identity: &Identity) -> Result<BranchHead, GitHubError> {
        let repository = self.client.repository(&identity.repository).await?;
        self.client
            .branch_head(&identity.repository, &repository.default_branch)
            .await
    }

    /// Reuse `current` when it already covers `head`'s tree and validates,
    /// else read `head`'s co-change history, extending `prior` when given,
    /// ingest `head` from its archive, and publish over `current`.
    ///
    /// # Errors
    ///
    /// As [`ColdBuild::run`].
    pub(crate) async fn build(
        &self,
        scope: &SnapshotScope,
        current: Option<&SnapshotPointer>,
        head: &BranchHead,
        prior: Option<&CoChangeHistory>,
    ) -> Result<ColdBuildResult, ColdBuildError> {
        let identity = &scope.identity;
        if let Some(pointer) = current.filter(|pointer| pointer.tree == head.tree) {
            match Snapshots::new(self.storage, self.store)
                .read(scope, pointer)
                .await
            {
                Ok(snapshot) => {
                    return Ok(ColdBuildResult::new(
                        head,
                        snapshot.coverage(),
                        IngestStats::default(),
                        ColdBuildOutcome::Reused(pointer.clone()),
                    ));
                }
                Err(ColdBuildError::Storage(StorageError::NotFound { .. })) => {}
                Err(error) => return Err(error),
            }
        }
        let history = self.history(identity, head, prior).await?;
        let mut ingest = match self.start(identity, head).await? {
            Ok(ingest) => ingest,
            Err(truncated) => return Ok(truncated),
        };
        let archive_too_large = self.fetch(&mut ingest, identity, head).await;
        self.complete(scope, current, head, ingest, archive_too_large, history)
            .await
    }

    /// `head`'s co-change history on its branch before its committer time,
    /// extending `prior` when given.
    ///
    /// # Errors
    ///
    /// As [`CoChangeHistory::read`].
    pub(crate) async fn history(
        &self,
        identity: &Identity,
        head: &BranchHead,
        prior: Option<&CoChangeHistory>,
    ) -> Result<CoChangeHistory, ColdBuildError> {
        CoChangeHistory::read(
            self.client,
            &identity.repository,
            &head.name,
            &head.committed_at,
            prior,
        )
        .await
    }

    /// List and classify `head`'s tree, with every eligible file pending.
    ///
    /// Returns `Err` with the incomplete result when the tree could not be
    /// listed completely.
    ///
    /// # Errors
    ///
    /// Returns the tree or attributes provider error.
    async fn start(
        &self,
        identity: &Identity,
        head: &BranchHead,
    ) -> Result<Result<Ingest, ColdBuildResult>, ColdBuildError> {
        let listing = self.client.tree(&identity.repository, &head.tree).await?;
        if listing.truncated {
            return Ok(Err(ColdBuildResult::truncated(head)));
        }
        let attributes = root_attributes(
            self.client,
            identity,
            &listing.entries,
            self.limits.max_file_bytes,
        )
        .await?;
        Ok(Ok(Ingest::new(
            &listing.entries,
            listing.invalid_paths,
            &attributes,
            self.limits.max_file_bytes,
        )))
    }

    /// Stream `head`'s archive into `ingest`, then fetch what it omitted as
    /// bounded verified blobs. Returns whether the archive passed its byte
    /// bound.
    async fn fetch(&self, ingest: &mut Ingest, identity: &Identity, head: &BranchHead) -> bool {
        let archive_too_large = match self
            .client
            .archive(&identity.repository, &head.commit)
            .await
        {
            Ok(archive) => {
                ingest
                    .consume_archive(archive, self.limits.max_expanded_archive_bytes)
                    .await
            }
            Err(error) => {
                ingest.stats.archive_error = Some(error.to_string());
                false
            }
        };
        ingest
            .fallback(self.client, identity, self.limits.max_fallback_blobs)
            .await;
        archive_too_large
    }

    /// Report an incomplete `ingest` without writing, or publish its
    /// complete snapshot of `head` with `history` over `current`.
    ///
    /// # Errors
    ///
    /// Publication errors as in [`ColdBuild::publish`].
    pub(crate) async fn complete(
        &self,
        scope: &SnapshotScope,
        current: Option<&SnapshotPointer>,
        head: &BranchHead,
        ingest: Ingest,
        archive_too_large: bool,
        history: CoChangeHistory,
    ) -> Result<ColdBuildResult, ColdBuildError> {
        let ingested = ingest.finish();
        let coverage = ingested.coverage();
        let stats = ingested.stats;
        if archive_too_large {
            return Ok(ColdBuildResult::new(
                head,
                coverage,
                stats,
                ColdBuildOutcome::Incomplete(IncompleteReason::ArchiveTooLarge),
            ));
        }
        if !coverage.is_complete() {
            let count = coverage.unindexed();
            return Ok(ColdBuildResult::new(
                head,
                coverage,
                stats,
                ColdBuildOutcome::Incomplete(IncompleteReason::UnindexedFiles { count }),
            ));
        }

        let snapshot = Snapshot {
            format_version: SNAPSHOT_FORMAT_VERSION,
            extraction_version: EXTRACTION_VERSION.to_owned(),
            identity: scope.identity.clone(),
            commit: head.commit.clone(),
            tree: head.tree.clone(),
            files: ingested.outcomes,
            history,
        };
        let pointer = self.publish(scope, current, &snapshot).await?;
        Ok(ColdBuildResult::new(
            head,
            coverage,
            stats,
            ColdBuildOutcome::Published(pointer),
        ))
    }

    /// Publish `snapshot` in three idempotent steps: upsert its record,
    /// refreshing `touched_at`; create its object if absent, adopting an
    /// existing object only when its bytes equal the built bytes (which
    /// embed identity, versions, commit, tree, and history); then
    /// compare-and-swap the pointer from `current`.
    ///
    /// Dropped after any step, the pointer is untouched and every object
    /// has a record; a retry of the same commit reaches the same key and
    /// adopts what the dropped attempt wrote.
    ///
    /// # Errors
    ///
    /// Storage, persistence, `Corrupt`, and `PointerConflict` as in [`ColdBuild::run`].
    async fn publish(
        &self,
        scope: &SnapshotScope,
        current: Option<&SnapshotPointer>,
        snapshot: &Snapshot,
    ) -> Result<SnapshotPointer, ColdBuildError> {
        let identity = &scope.identity;
        let key = snapshot_key(scope, &snapshot.commit);
        let bytes = serde_json::to_vec(snapshot).map_err(|error| ColdBuildError::Corrupt {
            detail: error.to_string(),
        })?;
        self.store.upsert_record(identity, &key).await?;
        match self.storage.create(&key, &bytes).await {
            Ok(()) => {}
            Err(StorageError::AlreadyExists { .. }) => {
                if self.storage.read(&key).await? != bytes {
                    return Err(ColdBuildError::Corrupt {
                        detail: "existing object differs from the built snapshot".to_owned(),
                    });
                }
            }
            Err(error) => return Err(error.into()),
        }
        let pointer = SnapshotPointer {
            tree: snapshot.tree.clone(),
            commit: snapshot.commit.clone(),
            key,
            digest: ObjectDigest::of(&bytes),
        };
        let expected = current.map(|pointer| &pointer.tree);
        if !self
            .store
            .publish_snapshot_pointer(scope, expected, &pointer)
            .await?
        {
            return Err(ColdBuildError::PointerConflict);
        }
        Ok(pointer)
    }
}

/// Fetch and parse the root `.gitattributes` among `entries`, in either
/// regular blob mode, reading at most `max_file_bytes`, or the defaults
/// when the tree has none.
///
/// # Errors
///
/// Returns the attributes blob provider error.
pub(crate) async fn root_attributes(
    client: &GitHubClient,
    identity: &Identity,
    entries: &[TreeEntry],
    max_file_bytes: u64,
) -> Result<RootAttributes, GitHubError> {
    match entries.iter().find(|entry| {
        entry.path.as_str() == ".gitattributes"
            && matches!(entry.mode, TreeEntryMode::File | TreeEntryMode::Executable)
    }) {
        Some(entry) => Ok(RootAttributes::parse(
            &client
                .blob(&identity.repository, &entry.sha, max_file_bytes)
                .await?,
        )),
        None => Ok(RootAttributes::default()),
    }
}

/// Whether `error` is a blob fetch refused by the file ceiling
/// `max_file_bytes` rather than by a smaller provider ceiling.
pub(crate) fn is_oversize(error: &GitHubError, max_file_bytes: u64) -> bool {
    matches!(error, GitHubError::ResponseTooLarge { limit, .. } if *limit >= max_file_bytes)
}

/// In-memory ingest state for one attempt: eligible entries still wanted,
/// and what was indexed and skipped.
pub(crate) struct Ingest {
    context: SymbolicContext,
    max_file_bytes: u64,
    files: Vec<SnapshotFile>,
    pending: BTreeMap<RelativePath, ObjectSha>,
    outcomes: Vec<FileOutcome>,
    indexed: usize,
    excluded: BTreeMap<ExclusionReason, usize>,
    stats: IngestStats,
}

/// A finished ingest: every eligible file's outcome and the context the
/// indexed ones populated.
pub(crate) struct Ingested {
    /// Outcome of every eligible file, in canonical path order.
    pub(crate) outcomes: Vec<FileOutcome>,
    /// Excluded tree entries counted by reason.
    pub(crate) excluded: BTreeMap<ExclusionReason, usize>,
    /// How the attempt obtained its bytes.
    pub(crate) stats: IngestStats,
    /// Every eligible path with its listed blob id, in canonical order.
    pub(crate) eligible: Vec<SnapshotFile>,
    /// Context holding exactly the indexed files' facts.
    pub(crate) context: SymbolicContext,
}

impl Ingested {
    /// Coverage derived from the outcomes.
    pub(crate) fn coverage(&self) -> Coverage {
        Coverage::of(&self.outcomes, self.excluded.clone())
    }
}

impl Ingest {
    /// Classify `entries` and start with every eligible file pending;
    /// `invalid_paths` listing entries with non-canonical paths are counted
    /// as [`ExclusionReason::Unsupported`].
    pub(crate) fn new(
        entries: &[TreeEntry],
        invalid_paths: usize,
        attributes: &RootAttributes,
        max_file_bytes: u64,
    ) -> Ingest {
        let context = SymbolicContext::new();
        let tree = classify(entries, attributes, &context, max_file_bytes);
        let mut files: Vec<SnapshotFile> = tree
            .eligible
            .into_iter()
            .map(|entry| SnapshotFile {
                path: entry.path,
                blob: entry.sha,
            })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let pending = files
            .iter()
            .map(|file| (file.path.clone(), file.blob.clone()))
            .collect();
        let mut excluded = BTreeMap::new();
        for exclusion in tree.excluded {
            *excluded.entry(exclusion.reason).or_insert(0) += 1;
        }
        if invalid_paths > 0 {
            *excluded.entry(ExclusionReason::Unsupported).or_insert(0) += invalid_paths;
        }
        Ingest {
            context,
            max_file_bytes,
            files,
            pending,
            outcomes: Vec::new(),
            indexed: 0,
            excluded,
            stats: IngestStats::default(),
        }
    }

    /// Complete an update started with [`Ingest::new`] over only the head
    /// entries of changed paths: carry `prior`'s outcome for every path not
    /// in `removed`.
    ///
    /// The outcomes equal ingesting the head tree afresh once the pending
    /// changed paths are fetched, provided the root attributes are
    /// unchanged between base and head. Exclusions count only the changed
    /// entries. A carried fact that fails to load stays pending at its
    /// prior blob and is fetched like a changed one.
    pub(crate) fn carry(&mut self, prior: Snapshot, removed: &[RelativePath]) {
        let removed: BTreeSet<&RelativePath> = removed.iter().collect();
        for outcome in prior.files {
            if removed.contains(outcome.path()) {
                continue;
            }
            match outcome {
                FileOutcome::Indexed(file) => {
                    let (path, blob) = (file.facts.path.clone(), file.blob);
                    self.files.push(SnapshotFile {
                        path: path.clone(),
                        blob: blob.clone(),
                    });
                    if self.context.load(file.facts).is_ok() {
                        self.stats.reused_files += 1;
                        self.settle(path, blob);
                    } else {
                        self.pending.insert(path, blob);
                    }
                }
                FileOutcome::Skipped(file) => {
                    self.files.push(SnapshotFile {
                        path: file.path.clone(),
                        blob: file.blob.clone(),
                    });
                    self.stats.reused_files += 1;
                    self.outcomes.push(FileOutcome::Skipped(file));
                }
            }
        }
        self.files.sort_by(|a, b| a.path.cmp(&b.path));
    }

    /// Stream the archive through gzip into [`TarStream`], indexing each
    /// verified entry as soon as the decompressed bytes completing it arrive.
    ///
    /// Only the chunk in flight, one decompressed buffer, one header or
    /// metadata record, and one wanted file are held; a provider,
    /// decompression, tar, or bound error stops the archive and is
    /// recorded, leaving the rest pending for [`Ingest::fallback`]. Entries
    /// indexed before it stay indexed. Returns whether the compressed or
    /// decompressed archive passed its byte bound.
    async fn consume_archive(&mut self, mut archive: Archive, max_expanded_bytes: u64) -> bool {
        let tar = TarStream::new(
            self.pending.clone(),
            self.max_file_bytes,
            max_expanded_bytes,
        );
        let mut decoder = GzDecoder::new(tar);
        let mut compressed_too_large = false;
        let error = loop {
            match archive.chunk().await {
                Ok(Some(chunk)) => {
                    if let Err(error) = self.decompress(&mut decoder, &chunk) {
                        break Some(error.to_string());
                    }
                }
                Ok(None) => {
                    let finished = decoder.try_finish();
                    self.ingest_verified(&mut decoder.get_mut().verified);
                    break finished.err().map(|error| error.to_string());
                }
                Err(error) => {
                    compressed_too_large = matches!(error, GitHubError::ResponseTooLarge { .. });
                    break Some(error.to_string());
                }
            }
        };
        self.stats.archive_error = error;
        compressed_too_large || decoder.get_ref().expansion_exceeded
    }

    /// Feed one compressed `chunk` through `decoder` one inner write at a
    /// time, indexing verified entries after each, so only the entries of
    /// one decompressed buffer (plus the file they complete) wait at once,
    /// however many files the chunk carries.
    ///
    /// # Errors
    ///
    /// Returns the decompression, tar, or expansion error, or
    /// [`io::ErrorKind::WriteZero`] when the decoder accepts no input.
    fn decompress(
        &mut self,
        decoder: &mut GzDecoder<TarStream>,
        mut chunk: &[u8],
    ) -> io::Result<()> {
        while !chunk.is_empty() {
            let written = decoder.write(chunk);
            self.ingest_verified(&mut decoder.get_mut().verified);
            match written? {
                0 => return Err(io::ErrorKind::WriteZero.into()),
                consumed => chunk = &chunk[consumed..],
            }
        }
        Ok(())
    }

    /// Index and drain entries the tar stream verified against their blob
    /// ids, recording the largest batch of bytes that waited.
    fn ingest_verified(&mut self, verified: &mut Vec<VerifiedEntry>) {
        let waiting = verified.iter().map(|(_, _, bytes)| bytes.len()).sum();
        self.stats.peak_verified_bytes = self.stats.peak_verified_bytes.max(waiting);
        for (path, blob, bytes) in verified.drain(..) {
            self.stats.archive_entries += 1;
            self.ingest(path, blob, &bytes);
        }
    }

    /// Eligible files indexed so far.
    pub(crate) fn indexed(&self) -> usize {
        self.indexed
    }

    /// Whether `path` is eligible and still wanted.
    pub(crate) fn is_pending(&self, path: &RelativePath) -> bool {
        self.pending.contains_key(path)
    }

    /// Blob id of eligible `path` in the classified tree, if it is eligible.
    pub(crate) fn eligible_blob(&self, path: &RelativePath) -> Option<&ObjectSha> {
        self.files
            .binary_search_by(|file| file.path.cmp(path))
            .ok()
            .map(|index| &self.files[index].blob)
    }

    /// Carry `prior`'s outcome for every pending path still at the blob
    /// `prior` recorded, so only changed or added paths stay pending.
    /// Outcomes of changed, deleted, renamed, or now-ineligible paths are
    /// never loaded. A fact that fails to load stays pending and is fetched
    /// like a changed blob.
    pub(crate) fn reuse(&mut self, prior: Snapshot) {
        for outcome in prior.files {
            let path = outcome.path().clone();
            let Some(blob) = self.pending.get(&path).cloned() else {
                continue;
            };
            match outcome {
                FileOutcome::Indexed(file) if file.blob == blob => {
                    if self.context.load(file.facts).is_ok() {
                        self.stats.reused_files += 1;
                        self.settle(path, blob);
                    }
                }
                FileOutcome::Skipped(file) if file.blob == blob => {
                    self.stats.reused_files += 1;
                    self.skip(file.path, file.blob, file.reason);
                }
                _ => {}
            }
        }
    }

    /// Fetch every still-pending blob individually, up to `max_blobs`,
    /// reading at most the file bound, and index it; everything still
    /// pending afterwards is skipped as not fetched. A blob refused at the
    /// file bound is a benign oversize skip; one refused at a smaller client
    /// blob bound proves nothing and is unfetched.
    pub(crate) async fn fallback(
        &mut self,
        client: &GitHubClient,
        identity: &Identity,
        max_blobs: usize,
    ) {
        while let Some((path, blob)) = self.pending.pop_first() {
            if self.stats.fallback_blobs >= max_blobs {
                self.skip(path, blob, SkipReason::NotFetched);
                continue;
            }
            self.stats.fallback_blobs += 1;
            match client
                .blob(&identity.repository, &blob, self.max_file_bytes)
                .await
            {
                Ok(bytes) => self.ingest(path, blob, &bytes),
                Err(error) if is_oversize(&error, self.max_file_bytes) => {
                    self.skip(path, blob, SkipReason::Oversize);
                }
                Err(error) => self.skip(
                    path,
                    blob,
                    SkipReason::FetchFailed {
                        detail: error.to_string(),
                    },
                ),
            }
        }
    }

    /// Index verified `bytes` for `path`, or skip it for an LFS pointer, no
    /// persisted declarations, or extractor failure. The bytes are dropped
    /// by the caller afterwards.
    pub(crate) fn ingest(&mut self, path: RelativePath, blob: ObjectSha, bytes: &[u8]) {
        if bytes.starts_with(LFS_POINTER_PREFIX) {
            self.skip(path, blob, SkipReason::LfsPointer);
            return;
        }
        match self.context.upsert(path.as_str(), bytes) {
            Ok(_) => self.settle(path, blob),
            Err(error) => self.skip(
                path,
                blob,
                SkipReason::ParseFailed {
                    detail: error.to_string(),
                },
            ),
        }
    }

    /// Record `path`, just upserted or loaded into the context, as an
    /// [`IndexedFile`] carrying its exported declarations; when the export
    /// holds none, remove it from the context and record a
    /// [`SkipReason::NoFacts`] skip instead.
    ///
    /// # Panics
    ///
    /// Never in practice: a path the context accepted is canonical.
    fn settle(&mut self, path: RelativePath, blob: ObjectSha) {
        match self
            .context
            .export_file(&path)
            .and_then(|facts| IndexedFile::new(blob.clone(), facts))
        {
            Some(file) => {
                self.pending.remove(&path);
                self.indexed += 1;
                self.outcomes.push(FileOutcome::Indexed(file));
            }
            None => {
                self.context
                    .remove(path.as_str())
                    .expect("a path the context accepted is canonical");
                self.skip(path, blob, SkipReason::NoFacts);
            }
        }
    }

    /// Record a skip and stop wanting the path.
    fn skip(&mut self, path: RelativePath, blob: ObjectSha, reason: SkipReason) {
        self.pending.remove(&path);
        self.outcomes
            .push(FileOutcome::Skipped(SkippedFile { path, blob, reason }));
    }

    /// Every eligible file's outcome, with anything still pending skipped
    /// as not fetched, plus stats, the eligible listing, and the context.
    pub(crate) fn finish(mut self) -> Ingested {
        while let Some((path, blob)) = self.pending.pop_first() {
            self.skip(path, blob, SkipReason::NotFetched);
        }
        self.outcomes.sort_by(|a, b| a.path().cmp(b.path()));
        Ingested {
            outcomes: self.outcomes,
            excluded: self.excluded,
            stats: self.stats,
            eligible: self.files,
            context: self.context,
        }
    }
}

#[cfg(test)]
/// Owner tests for the cold build: complete verified snapshots, archive
/// fallback, every incomplete outcome, and crash-safe publication, driven
/// through a routed fake GitHub and in-memory storage and persistence. The
/// fixtures are shared with the update facade's tests.
pub(crate) mod tests {
    use super::*;
    use crate::github::{Operation, RequestLimits, git_blob_sha};
    use crate::identity::{
        GitHubHost, InstallationId, InstallationToken, RepositoryIdentity, TenantId,
    };
    use crate::testing::{MemorySnapshotStore, MemoryStorage, RoutedTransport};
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tar::EntryType;

    /// Pointer scope of `identity` under the default file ceiling.
    pub(crate) fn scope(identity: &Identity) -> SnapshotScope {
        snapshot_scope(identity, MAX_ELIGIBLE_FILE_BYTES)
    }

    /// Declares `lookupUser` and `saveUser`.
    pub(crate) const USER: &[u8] = b"export interface User {\n  id: string;\n}\n\nexport function lookupUser(id: string): User {\n  return { id };\n}\n\nexport function saveUser(user: User): void {\n  console.log(user.id);\n}\n";
    /// Declares `formatUser`.
    pub(crate) const FORMAT: &[u8] = b"import { User } from \"./user\";\n\nexport function formatUser(user: User): string {\n  return user.id;\n}\n";
    /// Declares `handleRequest`.
    pub(crate) const HANDLER: &[u8] = b"import { lookupUser } from \"./user\";\n\nexport function handleRequest(id: string): string {\n  return lookupUser(id).id;\n}\n";
    /// Declares `omittedFn`; never placed in the archive.
    pub(crate) const OMITTED: &[u8] = b"export function omittedFn(): number {\n  return 1;\n}\n";
    /// Declares `substitutedFn`; the archive carries other bytes for its path.
    pub(crate) const SUBSTITUTED: &[u8] =
        b"export function substitutedFn(): number {\n  return 2;\n}\n";
    /// A Git LFS pointer.
    pub(crate) const LFS: &[u8] =
        b"version https://git-lfs.github.com/spec/v1\noid sha256:0000\nsize 12\n";
    /// A parse-error file whose recovered tree holds no declarations.
    const BROKEN: &[u8] = b"export function (\n";
    /// A valid file whose only facts are a file node and an import, neither
    /// of which is a persisted declaration.
    const IMPORT_ONLY: &[u8] = b"import { X } from './x';\n";

    /// Forty copies of `ch` as an object id.
    pub(crate) fn sha(ch: char) -> ObjectSha {
        ObjectSha::new(&ch.to_string().repeat(40)).expect("sha")
    }

    /// Blob id of `bytes`.
    pub(crate) fn blob_sha(bytes: &[u8]) -> ObjectSha {
        ObjectSha::new(&git_blob_sha(bytes)).expect("blob sha")
    }

    /// The caller identity for the pinned test repository.
    pub(crate) fn identity() -> Identity {
        Identity {
            tenant: TenantId::new("acme").expect("tenant"),
            installation: InstallationId::new(42).expect("installation"),
            repository: RepositoryIdentity::new(1296269, "octocat", "Hello-World").expect("repo"),
        }
    }

    /// One tree entry of the fake repository.
    pub(crate) struct File {
        pub(crate) path: &'static str,
        pub(crate) mode: &'static str,
        pub(crate) content: &'static [u8],
        pub(crate) size: Option<u64>,
    }

    /// Provider JSON listing `files` as `tree`'s entries; a blob or
    /// subtree id is the Git blob id of the entry's content.
    pub(crate) fn listing(tree: &ObjectSha, truncated: bool, files: &[File]) -> String {
        let entries: Vec<String> = files.iter().map(|f| entry_json(f.path, f)).collect();
        tree_json(tree, truncated, &entries)
    }

    /// Provider JSON of a tree listing holding `entries`.
    fn tree_json(tree: &ObjectSha, truncated: bool, entries: &[String]) -> String {
        format!(
            r#"{{"sha":"{tree}","truncated":{truncated},"tree":[{}]}}"#,
            entries.join(",")
        )
    }

    /// Provider JSON of `f` as a tree entry named `path`.
    fn entry_json(path: &str, f: &File) -> String {
        let kind = match f.mode {
            "040000" => "tree",
            "160000" => "commit",
            _ => "blob",
        };
        let size = f.size.unwrap_or(f.content.len() as u64);
        let (mode, sha) = match f.mode {
            "bad-sha" => ("100644", "not-a-sha".to_owned()),
            mode => (mode, blob_sha(f.content).to_string()),
        };
        format!(
            r#"{{"path":"{path}","mode":"{mode}","type":"{kind}","sha":"{sha}","size":{size}}}"#
        )
    }

    /// Id of directory `dir` in `tree`: unique per tree and directory.
    fn dir_sha(tree: &ObjectSha, dir: &str) -> ObjectSha {
        blob_sha(format!("{tree}:{dir}").as_bytes())
    }

    /// Route a non-recursive listing of every directory of `tree`, which
    /// holds `files`; subdirectory ids come from [`dir_sha`].
    fn route_directories(transport: &RoutedTransport, tree: &ObjectSha, files: &[File]) {
        let mut dirs: BTreeMap<String, Vec<String>> = BTreeMap::new();
        dirs.insert(String::new(), Vec::new());
        for f in files.iter().filter(|f| f.mode != "040000") {
            let (mut parent, mut rest) = (String::new(), f.path);
            while let Some((name, tail)) = rest.split_once('/') {
                let child = if parent.is_empty() {
                    name.to_owned()
                } else {
                    format!("{parent}/{name}")
                };
                if !dirs.contains_key(&child) {
                    let sha = dir_sha(tree, &child);
                    dirs.entry(parent).or_default().push(format!(
                        r#"{{"path":"{name}","mode":"040000","type":"tree","sha":"{sha}"}}"#
                    ));
                    dirs.insert(child.clone(), Vec::new());
                }
                (parent, rest) = (child, tail);
            }
            dirs.entry(parent).or_default().push(entry_json(rest, f));
        }
        for (dir, entries) in dirs {
            let sha = if dir.is_empty() {
                tree.clone()
            } else {
                dir_sha(tree, &dir)
            };
            transport.json(
                &format!("/git/trees/{sha}"),
                &tree_json(&sha, false, &entries),
            );
        }
    }

    /// Provider JSON comparing `base` (holding `old`) with `head` (holding
    /// `new`): added, removed, and modified paths, with an added path whose
    /// content and mode equal a removed one's reported as its rename.
    fn compare_json(
        base: &ObjectSha,
        head: &ObjectSha,
        old: &[(String, String)],
        new: &[(String, String)],
    ) -> String {
        let old_map: BTreeMap<&String, &String> = old.iter().map(|(p, id)| (p, id)).collect();
        let new_map: BTreeMap<&String, &String> = new.iter().map(|(p, id)| (p, id)).collect();
        let mut removed: Vec<(&String, &String)> = old_map
            .iter()
            .filter(|(p, _)| !new_map.contains_key(*p))
            .map(|(p, id)| (*p, *id))
            .collect();
        let mut files = Vec::new();
        for (path, id) in &new_map {
            match old_map.get(path) {
                Some(old_id) if old_id == id => {}
                Some(_) => files.push(format!(r#"{{"filename":"{path}","status":"modified"}}"#)),
                None => match removed.iter().position(|(_, old_id)| old_id == id) {
                    Some(index) => {
                        let (previous, _) = removed.remove(index);
                        files.push(format!(
                            r#"{{"filename":"{path}","status":"renamed","previous_filename":"{previous}"}}"#
                        ));
                    }
                    None => files.push(format!(r#"{{"filename":"{path}","status":"added"}}"#)),
                },
            }
        }
        for (path, _) in removed {
            files.push(format!(r#"{{"filename":"{path}","status":"removed"}}"#));
        }
        format!(
            r#"{{"status":"ahead","base_commit":{{"sha":"{base}"}},"commits":[{{"sha":"{head}"}}],"files":[{}]}}"#,
            files.join(",")
        )
    }

    /// A directory entry whose subtree id is the blob id of `id`.
    pub(crate) fn dir(path: &'static str, id: &'static [u8]) -> File {
        File {
            path,
            mode: "040000",
            content: id,
            size: None,
        }
    }

    /// A regular file whose listed size is its content length.
    pub(crate) fn file(path: &'static str, content: &'static [u8]) -> File {
        File {
            path,
            mode: "100644",
            content,
            size: None,
        }
    }

    /// A regular file the listing reports with a malformed blob id, which
    /// breaks the whole listing.
    pub(crate) fn bad_sha(path: &'static str) -> File {
        File {
            path,
            mode: "bad-sha",
            content: b"",
            size: None,
        }
    }

    /// `(path, mode:content id)` of every file of one pushed tree.
    type PushedEntries = Vec<(String, String)>;

    /// Closed-PR listing page `page` of the fake repository's `main`.
    pub(crate) fn closed_pulls(page: u32) -> String {
        format!(
            "/pulls?state=closed&base=main&sort=updated&direction=desc&per_page=100&page={page}"
        )
    }

    /// Committer time of the `n`th pushed commit: noon on day `n` of
    /// February 2026.
    pub(crate) fn pushed_at(n: usize) -> String {
        format!("2026-02-{n:02}T12:00:00Z")
    }

    /// A fake repository at commit `c`/tree `7` with routed responses.
    pub(crate) struct FakeRepo {
        pub(crate) transport: Arc<RoutedTransport>,
        pub(crate) tree: ObjectSha,
        pub(crate) commit: ObjectSha,
        pub(crate) limits: RequestLimits,
        /// Every pushed commit with its entries.
        history: Mutex<Vec<(ObjectSha, PushedEntries)>>,
    }

    impl FakeRepo {
        /// Route repository, branch, tree, every blob, the tarball
        /// containing `archive` entries, and no merged PRs under the GitHub
        /// prefix.
        pub(crate) fn new(files: &[File], archive: &[(&str, &[u8])]) -> FakeRepo {
            let transport = Arc::new(RoutedTransport::default());
            let (tree, commit) = (sha('7'), sha('c'));
            transport.json(
                "/repos/octocat/Hello-World",
                r#"{"id":1296269,"full_name":"octocat/Hello-World","default_branch":"main","private":true}"#,
            );
            let repo = FakeRepo {
                transport,
                tree,
                commit,
                limits: RequestLimits::default(),
                history: Mutex::default(),
            };
            repo.merged(&[]);
            repo.push(&repo.commit, &repo.tree, files, archive);
            repo
        }

        /// Route one closed-PR listing page of `prs` merged into `main`,
        /// each `(number, merged_at, changed paths)` updated when it
        /// merged, and each PR's one-page file listing.
        pub(crate) fn merged(&self, prs: &[(u64, &str, &[&str])]) {
            let mut listed: Vec<_> = prs.to_vec();
            listed.sort_by(|a, b| b.1.cmp(a.1));
            let entries: Vec<String> = listed
                .iter()
                .map(|(number, at, _)| {
                    format!(
                        r#"{{"number":{number},"updated_at":"{at}","merged_at":"{at}","base":{{"ref":"main","sha":"{}"}},"head":{{"ref":"f","sha":"{}"}}}}"#,
                        sha('a'),
                        sha('b')
                    )
                })
                .collect();
            self.transport
                .json(&closed_pulls(1), &format!("[{}]", entries.join(",")));
            for (number, _, paths) in prs {
                let files: Vec<String> = paths
                    .iter()
                    .map(|path| {
                        format!(
                            r#"{{"filename":"{path}","status":"modified","sha":"{}","patch":"@@ -1 +1 @@\n-a\n+b"}}"#,
                            sha('e')
                        )
                    })
                    .collect();
                self.transport.json(
                    &format!("/pulls/{number}/files?per_page=100&page=1"),
                    &format!("[{}]", files.join(",")),
                );
            }
        }

        /// Route a closed-PR listing that advertises another page at every
        /// page through the page bound, each page one closed unmerged PR
        /// updated in March, so history can never be proved.
        pub(crate) fn unproved_history(&self) {
            for page in 1..=crate::history::MAX_DISCOVERY_PAGES {
                let number = u64::from(page) + 100;
                self.transport.respond(
                    &closed_pulls(page),
                    vec![(
                        200,
                        vec![("link".to_owned(), r#"<x>; rel="next""#.to_owned())],
                        format!(
                            r#"[{{"number":{number},"updated_at":"2026-03-{:02}T00:00:00Z","merged_at":null,"base":{{"ref":"main","sha":"{}"}},"head":{{"ref":"f","sha":"{}"}}}}]"#,
                            30 - page,
                            sha('a'),
                            sha('b')
                        )
                        .into_bytes(),
                    )],
                );
            }
        }

        /// Route a two-page closed-PR listing, PRs merged early on 2026-02-03,
        /// whose page 1 gains PR 3, a page-2 entry, below its unchanged
        /// first entry before it is read again, so page 2 as read skipped
        /// PR 3.
        pub(crate) fn moving_history(&self) {
            let entry = |number: u64, hour: u32| {
                format!(
                    r#"{{"number":{number},"updated_at":"2026-02-03T{hour:02}:00:00Z","merged_at":"2026-02-03T{hour:02}:00:00Z","base":{{"ref":"main","sha":"{}"}},"head":{{"ref":"f","sha":"{}"}}}}"#,
                    sha('a'),
                    sha('b')
                )
            };
            let next = vec![("link".to_owned(), r#"<x>; rel="next""#.to_owned())];
            self.transport.respond(
                &closed_pulls(1),
                vec![
                    (
                        200,
                        next,
                        format!("[{},{}]", entry(5, 5), entry(4, 4)).into_bytes(),
                    ),
                    (
                        200,
                        Vec::new(),
                        format!("[{},{},{}]", entry(5, 5), entry(3, 3), entry(4, 4)).into_bytes(),
                    ),
                ],
            );
            self.transport
                .json(&closed_pulls(2), &format!("[{}]", entry(2, 2)));
            for number in 2..=5 {
                self.transport.json(
                    &format!("/pulls/{number}/files?per_page=100&page=1"),
                    &format!(
                        r#"[{{"filename":"src/p{number}.ts","status":"modified","sha":"{}","patch":"@@ -1 +1 @@\n-a\n+b"}}]"#,
                        sha('e')
                    ),
                );
            }
        }

        /// Move the branch to `commit`/`tree` listing `files`, routing its
        /// blobs, tree, directories, a tarball of `archive`, and a compare
        /// from every earlier pushed commit; earlier routes stay.
        pub(crate) fn push(
            &self,
            commit: &ObjectSha,
            tree: &ObjectSha,
            files: &[File],
            archive: &[(&str, &[u8])],
        ) {
            let transport = &self.transport;
            let at = pushed_at(self.history.lock().expect("history lock").len() + 1);
            transport.json(
                "/branches/main",
                &format!(
                    r#"{{"name":"main","commit":{{"sha":"{commit}","commit":{{"tree":{{"sha":"{tree}"}},"committer":{{"date":"{at}"}}}}}}}}"#
                ),
            );
            transport.json(
                &format!("/git/commits/{commit}"),
                &format!(r#"{{"sha":"{commit}","tree":{{"sha":"{tree}"}},"committer":{{"date":"{at}"}}}}"#),
            );
            self.route_tree(tree, files);
            route_directories(transport, tree, files);
            transport.route(&format!("/tarball/{commit}"), 200, tarball(archive));
            let entries: Vec<(String, String)> = files
                .iter()
                .map(|f| {
                    (
                        f.path.to_owned(),
                        format!("{}:{}", f.mode, blob_sha(f.content)),
                    )
                })
                .collect();
            let mut history = self.history.lock().expect("history lock");
            for (base, old) in history.iter().filter(|(base, _)| base != commit) {
                transport.json(
                    &format!("/compare/{base}...{commit}"),
                    &compare_json(base, commit, old, &entries),
                );
            }
            history.push((commit.clone(), entries));
        }

        /// Route `tree` listing `files` and every file's blob, without
        /// moving the branch.
        pub(crate) fn route_tree(&self, tree: &ObjectSha, files: &[File]) {
            for f in files {
                self.transport.route(
                    &format!("/git/blobs/{}", blob_sha(f.content)),
                    200,
                    f.content,
                );
            }
            self.transport.json(
                &format!("/git/trees/{tree}?recursive=1"),
                &listing(tree, false, files),
            );
        }

        /// Replace the tree's recursive and non-recursive routes with truncated
        /// listings, so the tree cannot be listed completely.
        pub(crate) fn truncate_tree(&self) {
            let tree = &self.tree;
            for query in ["?recursive=1", ""] {
                self.transport.json(
                    &format!("/git/trees/{tree}{query}"),
                    &listing(tree, true, &[]),
                );
            }
        }

        /// A client over this fake under `limits`.
        pub(crate) fn client(&self) -> GitHubClient {
            GitHubClient::with_transport(
                GitHubHost::public(),
                InstallationToken::new("ghs_token").expect("token"),
                self.limits.clone(),
                Box::new(Shared(Arc::clone(&self.transport))),
            )
        }
    }

    /// Transport handle shared with the test.
    struct Shared(Arc<RoutedTransport>);

    impl crate::github::HttpTransport for Shared {
        /// Delegates to the shared fake.
        ///
        /// # Errors
        ///
        /// Never.
        fn get(
            &self,
            request: crate::github::HttpRequest,
        ) -> crate::github::BoxFuture<
            '_,
            Result<crate::github::HttpResponse, crate::github::TransportError>,
        > {
            self.0.get(request)
        }
    }

    /// Gzipped tarball with `entries` under `octocat-Hello-World-ccccccc/`,
    /// plus a directory, a symlink, and a traversal entry that must be ignored.
    fn tarball(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        let mut dir = tar::Header::new_gnu();
        dir.set_entry_type(EntryType::Directory);
        dir.set_size(0);
        dir.set_mode(0o755);
        dir.set_cksum();
        builder
            .append_data(&mut dir, "octocat-Hello-World-ccccccc/", &b""[..])
            .expect("dir");
        for (path, bytes) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(
                    &mut header,
                    format!("octocat-Hello-World-ccccccc/{path}"),
                    *bytes,
                )
                .expect("entry");
        }
        let mut link = tar::Header::new_gnu();
        link.set_entry_type(EntryType::Symlink);
        link.set_size(0);
        link.set_mode(0o777);
        link.set_cksum();
        builder
            .append_link(
                &mut link,
                "octocat-Hello-World-ccccccc/link.ts",
                "src/user.ts",
            )
            .expect("link");
        // `append_data` refuses `..`, so the traversing name is written raw.
        let mut escape = tar::Header::new_gnu();
        let name = b"octocat-Hello-World-ccccccc/../escape.ts";
        escape.as_old_mut().name[..name.len()].copy_from_slice(name);
        escape.set_size(USER.len() as u64);
        escape.set_mode(0o644);
        escape.set_cksum();
        builder.append(&escape, USER).expect("escape");
        let tar = builder.into_inner().expect("tar");
        let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
        gz.write_all(&tar).expect("gzip");
        gz.finish().expect("gzip finish")
    }

    /// The standard fixture: six eligible files, one LFS pointer, every
    /// exclusion reason, an omitted and a substituted archive entry.
    pub(crate) fn standard_files() -> Vec<File> {
        vec![
            file("src/user.ts", USER),
            file("src/format.ts", FORMAT),
            file("src/handler.ts", HANDLER),
            file("src/omitted.ts", OMITTED),
            file("src/substituted.ts", SUBSTITUTED),
            file("src/lfs.ts", LFS),
            file(".gitattributes", b"generated/* linguist-generated\n"),
            file("generated/g.ts", FORMAT),
            file("node_modules/x.ts", FORMAT),
            file("README.md", b"# hi\n"),
            File {
                path: "link.ts",
                mode: "120000",
                content: b"src/user.ts",
                size: None,
            },
            File {
                path: "sub",
                mode: "160000",
                content: b"",
                size: None,
            },
            File {
                path: "big.ts",
                mode: "100644",
                content: FORMAT,
                size: Some(MAX_ELIGIBLE_FILE_BYTES + 1),
            },
        ]
    }

    /// Archive entries for the standard fixture: no `src/omitted.ts`, and
    /// other bytes under `src/substituted.ts`.
    pub(crate) fn standard_archive() -> Vec<(&'static str, &'static [u8])> {
        vec![
            ("src/user.ts", USER),
            ("src/format.ts", FORMAT),
            ("src/handler.ts", HANDLER),
            ("src/substituted.ts", b"export const stale = 1;\n"),
            ("src/lfs.ts", LFS),
            ("node_modules/x.ts", FORMAT),
            ("README.md", b"# hi\n"),
        ]
    }

    /// A fixed-commit build publishes at its requested commit and, after the
    /// default branch advances, reuses that commit's snapshot instead of
    /// following the new head, while the ordinary cold build follows the
    /// new head and publishes a fresh snapshot.
    #[tokio::test]
    async fn fixed_commit_build_ignores_head_advance_and_run_follows_it() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let (storage, store) = (MemoryStorage::default(), MemorySnapshotStore::default());
        let build = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default());
        let identity = identity();

        let first = build.run_at(&identity, &repo.commit).await.expect("pinned");
        assert!(matches!(first.outcome, ColdBuildOutcome::Published(_)));
        let (head, tree) = (sha('d'), sha('8'));
        repo.push(
            &head,
            &tree,
            &[file("src/user.ts", USER)],
            &[("src/user.ts", USER)],
        );

        let repeat = build.run_at(&identity, &repo.commit).await.expect("pinned");
        assert_eq!((&repeat.commit, &repeat.tree), (&repo.commit, &repo.tree));
        assert!(matches!(repeat.outcome, ColdBuildOutcome::Reused(_)));
        let followed = build.run(&identity).await.expect("cold build");
        assert_eq!((&followed.commit, &followed.tree), (&head, &tree));
        assert!(matches!(followed.outcome, ColdBuildOutcome::Published(_)));
    }

    /// Scenario 1: the cold build verifies archive entries, falls back to
    /// blobs for omitted and substituted entries, skips the LFS pointer by
    /// content, writes a source-free object, publishes the pointer only
    /// after read-back, reproduces facts on reload without source calls, and
    /// reuses the pointer on a second invocation without another archive.
    #[tokio::test]
    async fn complete_cold_snapshot_is_verified_source_free_and_reloadable() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let storage = MemoryStorage::default();
        let persistence = MemorySnapshotStore::default();
        let identity = identity();
        let build = ColdBuild::new(&client, &storage, &persistence, ColdBuildLimits::default());

        let result = build.run(&identity).await.expect("cold build");
        let ColdBuildOutcome::Published(pointer) = &result.outcome else {
            panic!("expected publication, got {:?}", result.outcome);
        };
        assert_eq!((&result.commit, &result.tree), (&repo.commit, &repo.tree));
        assert_eq!(pointer.tree, repo.tree);
        assert_eq!(pointer.commit, repo.commit);
        assert_eq!(
            pointer.key,
            ObjectKey::snapshot(
                &identity,
                EXTRACTION_VERSION,
                SNAPSHOT_FORMAT_VERSION,
                MAX_ELIGIBLE_FILE_BYTES,
                &repo.commit
            )
        );

        let coverage = &result.coverage;
        assert!(coverage.is_complete());
        assert_eq!((coverage.eligible, coverage.indexed), (6, 5));
        assert_eq!(
            coverage.skipped,
            [SkippedFile {
                path: RelativePath::new("src/lfs.ts").expect("path"),
                blob: blob_sha(LFS),
                reason: SkipReason::LfsPointer,
            }]
        );
        assert_eq!(
            coverage.excluded,
            BTreeMap::from([
                (ExclusionReason::Symlink, 1),
                (ExclusionReason::Submodule, 1),
                (ExclusionReason::Unsupported, 2),
                (ExclusionReason::VendoredSegment, 1),
                (ExclusionReason::GeneratedAttribute, 1),
                (ExclusionReason::Oversize, 1),
            ])
        );
        assert_eq!(
            (
                result.ingest.archive_entries,
                result.ingest.fallback_blobs,
                &result.ingest.archive_error
            ),
            (4, 2, &None)
        );

        let calls = client.calls();
        assert_eq!(calls.get(Operation::Archive), 1);
        assert_eq!(calls.get(Operation::Tree), 1);
        assert_eq!(calls.get(Operation::Blob), 3, "attributes + two fallbacks");
        let seen = repo.transport.seen();
        assert!(
            seen.iter()
                .any(|url| url.ends_with(&format!("/git/blobs/{}", blob_sha(OMITTED))))
        );
        assert!(
            seen.iter()
                .any(|url| url.ends_with(&format!("/git/blobs/{}", blob_sha(SUBSTITUTED))))
        );
        assert!(
            !seen
                .iter()
                .any(|url| url.ends_with(&format!("/git/blobs/{}", blob_sha(USER))))
        );

        assert_eq!(storage.keys(), std::slice::from_ref(&pointer.key));
        let bytes = storage.read(&pointer.key).await.expect("object");
        assert_eq!(ObjectDigest::of(&bytes), pointer.digest);
        let text = String::from_utf8(bytes.clone()).expect("json");
        assert!(text.contains("lookupUser") && text.contains("omittedFn"));
        for fragment in [
            "return { id }",
            "console.log(user.id)",
            "return user.id",
            "stale",
        ] {
            assert!(
                !text.contains(fragment),
                "source fragment {fragment:?} persisted"
            );
        }
        let snapshot =
            Snapshot::decode(&bytes, &identity, &repo.commit, &repo.tree).expect("decode");
        assert_eq!(
            snapshot.coverage(),
            Coverage {
                excluded: BTreeMap::new(),
                ..coverage.clone()
            },
            "exclusions are reported by the build, not persisted"
        );
        let paths: Vec<&str> = snapshot.files.iter().map(|f| f.path().as_str()).collect();
        assert_eq!(
            paths,
            [
                "src/format.ts",
                "src/handler.ts",
                "src/lfs.ts",
                "src/omitted.ts",
                "src/substituted.ts",
                "src/user.ts"
            ]
        );
        let pointer_scope = SnapshotScope {
            identity: identity.clone(),
            extraction_version: EXTRACTION_VERSION.to_owned(),
            format_version: SNAPSHOT_FORMAT_VERSION,
            max_file_bytes: MAX_ELIGIBLE_FILE_BYTES,
        };
        assert_eq!(
            persistence
                .snapshot_pointer(&pointer_scope)
                .await
                .expect("pointer")
                .as_ref(),
            Some(pointer)
        );

        let loaded = Snapshots::new(&storage, &persistence)
            .read(&pointer_scope, pointer)
            .await
            .expect("read")
            .into_context()
            .expect("load");
        assert_eq!(loaded.file_count(), 5);
        let hit = loaded.lookup("lookupUser");
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].path.as_str(), "src/user.ts");
        assert_eq!(
            loaded.lookup("substitutedFn")[0].path.as_str(),
            "src/substituted.ts"
        );
        assert_eq!(calls.get(Operation::Blob), 3, "reload fetched nothing");

        let again = build.run(&identity).await.expect("second build");
        assert_eq!(again.outcome, ColdBuildOutcome::Reused(pointer.clone()));
        assert_eq!(again.coverage, snapshot.coverage(), "reuse lists nothing");
        assert_eq!(again.ingest, IngestStats::default());
        assert_eq!(calls.get(Operation::Archive), 1);
        assert_eq!(calls.get(Operation::Tree), 1);
        assert_eq!(storage.keys().len(), 1);
    }

    /// An executable (`100755`) root `.gitattributes` is policy like a
    /// regular one: its `linguist-generated` rule excludes `generated/g.ts`.
    #[tokio::test]
    async fn executable_root_attributes_exclude_generated_files() {
        let mut files = standard_files();
        for f in files.iter_mut().filter(|f| f.path == ".gitattributes") {
            f.mode = "100755";
        }
        let repo = FakeRepo::new(&files, &standard_archive());
        let client = repo.client();
        let result = ColdBuild::new(
            &client,
            &MemoryStorage::default(),
            &MemorySnapshotStore::default(),
            ColdBuildLimits::default(),
        )
        .run(&identity())
        .await
        .expect("cold build");
        let coverage = &result.coverage;
        assert_eq!(coverage.eligible, 6);
        assert_eq!(
            coverage.excluded.get(&ExclusionReason::GeneratedAttribute),
            Some(&1)
        );
    }

    /// A verified file with no persisted declarations, whether malformed or
    /// a valid import-only file, is a recorded content skip carrying its
    /// blob identity and no fact record; the build stays complete and
    /// publishes.
    ///
    /// # Panics
    ///
    /// Panics if the build fails, does not publish, records different
    /// skips, or persists a fact record for a skipped file.
    #[tokio::test]
    async fn no_facts_file_is_a_recorded_skip_in_a_published_build() {
        let files = [
            ("src/user.ts", USER),
            ("src/broken.ts", BROKEN),
            ("src/only.ts", IMPORT_ONLY),
        ];
        let repo = FakeRepo::new(&files.map(|(path, bytes)| file(path, bytes)), &files);
        let client = repo.client();
        let storage = MemoryStorage::default();
        let persistence = MemorySnapshotStore::default();
        let result = ColdBuild::new(&client, &storage, &persistence, ColdBuildLimits::default())
            .run(&identity())
            .await
            .expect("cold build");
        assert!(
            matches!(result.outcome, ColdBuildOutcome::Published(_)),
            "expected publication, got {:?}",
            result.outcome
        );
        assert!(result.coverage.is_complete());
        assert_eq!((result.coverage.eligible, result.coverage.indexed), (3, 1));
        let no_facts = |path: &str, bytes: &[u8]| SkippedFile {
            path: RelativePath::new(path).expect("path"),
            blob: blob_sha(bytes),
            reason: SkipReason::NoFacts,
        };
        assert_eq!(
            result.coverage.skipped,
            [
                no_facts("src/broken.ts", BROKEN),
                no_facts("src/only.ts", IMPORT_ONLY)
            ]
        );
        let ColdBuildOutcome::Published(pointer) = result.outcome else {
            unreachable!("publication asserted above");
        };
        let bytes = storage.read(&pointer.key).await.expect("object");
        let snapshot =
            Snapshot::decode(&bytes, &identity(), &repo.commit, &repo.tree).expect("decode");
        let indexed: Vec<&str> = snapshot
            .files
            .iter()
            .filter_map(|outcome| match outcome {
                FileOutcome::Indexed(file) => Some(file.facts().path.as_str()),
                FileOutcome::Skipped(_) => None,
            })
            .collect();
        assert_eq!(indexed, ["src/user.ts"]);
    }

    /// The shared ingest that update reuse and PR head overlays route
    /// through classifies a freshly extracted import-only file as
    /// `NoFacts` and leaves no posting behind, and a reused `NoFacts`
    /// outcome carries over unchanged.
    ///
    /// # Panics
    ///
    /// Panics if either path counts the file indexed, records a different
    /// skip, or leaves it in the context.
    #[test]
    fn import_only_file_is_no_facts_on_ingest_and_reuse() {
        let path = RelativePath::new("src/only.ts").expect("path");
        let blob = blob_sha(IMPORT_ONLY);
        let entries = [TreeEntry {
            path: path.clone(),
            mode: TreeEntryMode::File,
            sha: blob.clone(),
            size: Some(IMPORT_ONLY.len() as u64),
        }];
        let fresh = || {
            Ingest::new(
                &entries,
                0,
                &RootAttributes::default(),
                MAX_ELIGIBLE_FILE_BYTES,
            )
        };
        let expected = [SkippedFile {
            path: path.clone(),
            blob: blob.clone(),
            reason: SkipReason::NoFacts,
        }];

        let mut ingest = fresh();
        ingest.ingest(path.clone(), blob.clone(), IMPORT_ONLY);
        let ingested = ingest.finish();
        let coverage = ingested.coverage();
        assert_eq!(
            (coverage.indexed, coverage.skipped.as_slice()),
            (0, &expected[..])
        );
        assert!(ingested.context.export().is_empty() && ingested.context.lookup("X").is_empty());

        let mut ingest = fresh();
        ingest.reuse(Snapshot {
            format_version: SNAPSHOT_FORMAT_VERSION,
            extraction_version: EXTRACTION_VERSION.to_owned(),
            identity: identity(),
            commit: sha('c'),
            tree: sha('7'),
            files: ingested.outcomes,
            history: CoChangeHistory::default(),
        });
        assert!(ingest.pending.is_empty());
        let ingested = ingest.finish();
        let coverage = ingested.coverage();
        assert_eq!(
            (coverage.indexed, coverage.skipped.as_slice()),
            (0, &expected[..])
        );
        assert!(ingested.context.export().is_empty());
    }

    /// An indexed outcome without a declaration cannot be read back: the
    /// serialized form of an indexed file must carry at least one.
    ///
    /// # Panics
    ///
    /// Panics if an empty indexed record deserializes or a declared one
    /// does not round-trip.
    #[test]
    fn indexed_outcome_without_declarations_does_not_decode() {
        let mut context = SymbolicContext::new();
        context.upsert("src/user.ts", USER).expect("upsert");
        let path = RelativePath::new("src/user.ts").expect("path");
        let facts = context.export_file(&path).expect("indexed");
        let file = IndexedFile::new(blob_sha(USER), facts.clone()).expect("declared");
        let bytes = serde_json::to_vec(&FileOutcome::Indexed(file.clone())).expect("encode");
        let decoded: FileOutcome = serde_json::from_slice(&bytes).expect("round-trip");
        assert_eq!(decoded, FileOutcome::Indexed(file));

        let empty = IndexedFileFacts {
            declarations: Vec::new(),
            ..facts
        };
        let bytes = serde_json::to_vec(&serde_json::json!({
            "Indexed": { "blob": blob_sha(USER), "facts": empty }
        }))
        .expect("encode");
        assert!(serde_json::from_slice::<FileOutcome>(&bytes).is_err());
        assert!(IndexedFile::new(blob_sha(USER), empty).is_none());
    }

    /// A complete snapshot published under the previous format, whose
    /// import-only file was recorded indexed with no declarations, is never
    /// reused at the same tree: cold build and update both publish a
    /// current-format snapshot recording that blob once as `NoFacts`, and
    /// the old bytes do not read as complete under the current scope.
    ///
    /// # Panics
    ///
    /// Panics if the old pointer is reused, the new snapshot records the
    /// file differently, or the old object decodes under the current scope.
    #[tokio::test]
    async fn prior_format_indexed_empty_snapshot_is_not_reused() {
        let files = [("src/only.ts", IMPORT_ONLY), ("src/user.ts", USER)];
        let repo = FakeRepo::new(&files.map(|(path, bytes)| file(path, bytes)), &files);
        let identity = identity();
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();

        let mut context = SymbolicContext::new();
        for (path, bytes) in files {
            context.upsert(path, bytes).expect("upsert");
        }
        let facts = context.export();
        assert!(facts[0].declarations.is_empty(), "import-only is empty");
        let old_bytes = serde_json::to_vec(&serde_json::json!({
            "format_version": 1,
            "extraction_version": EXTRACTION_VERSION,
            "identity": identity,
            "tree": repo.tree,
            "files": files.map(|(path, bytes)| serde_json::json!({
                "path": path, "blob": blob_sha(bytes)
            })),
            "facts": facts,
            "coverage": Coverage { eligible: 2, indexed: 2, ..Coverage::default() },
        }))
        .expect("encode");
        let old_scope = SnapshotScope {
            format_version: 1,
            ..scope(&identity)
        };
        let publish_old = |scope: SnapshotScope| {
            let (storage, store, bytes, tree, commit) =
                (&storage, &store, &old_bytes, &repo.tree, &repo.commit);
            async move {
                let key = snapshot_key(&scope, commit);
                store
                    .upsert_record(&scope.identity, &key)
                    .await
                    .expect("record");
                storage.create(&key, bytes).await.expect("object");
                let pointer = SnapshotPointer {
                    tree: tree.clone(),
                    commit: commit.clone(),
                    key,
                    digest: ObjectDigest::of(bytes),
                };
                assert!(
                    store
                        .publish_snapshot_pointer(&scope, None, &pointer)
                        .await
                        .expect("pointer")
                );
                pointer
            }
        };
        let old = publish_old(old_scope).await;

        let client = repo.client();
        let limits = ColdBuildLimits::default();
        let built = ColdBuild::new(&client, &storage, &store, limits.clone())
            .run(&identity)
            .await
            .expect("cold build");
        let ColdBuildOutcome::Published(pointer) = &built.outcome else {
            panic!("expected publication, got {:?}", built.outcome);
        };
        assert_ne!(pointer.key, old.key);
        assert!(built.coverage.is_complete());
        assert_eq!((built.coverage.eligible, built.coverage.indexed), (2, 1));
        assert_eq!(
            built.coverage.skipped,
            [SkippedFile {
                path: RelativePath::new("src/only.ts").expect("path"),
                blob: blob_sha(IMPORT_ONLY),
                reason: SkipReason::NoFacts,
            }]
        );
        let bytes = storage.read(&pointer.key).await.expect("object");
        let snapshot =
            Snapshot::decode(&bytes, &identity, &repo.commit, &repo.tree).expect("decode");
        assert_eq!(snapshot.coverage(), built.coverage);
        assert!(
            matches!(&snapshot.files[1], FileOutcome::Indexed(file) if file.facts().path.as_str() == "src/user.ts")
        );

        let updated = crate::update::Update::new(&client, &storage, &store, limits)
            .run(&identity)
            .await
            .expect("update");
        assert_eq!(updated.outcome, ColdBuildOutcome::Reused(pointer.clone()));
        assert_eq!(updated.coverage, built.coverage);

        let current = scope(&identity);
        store
            .remove_pointers(&identity)
            .await
            .expect("drop pointers");
        let _ = storage.delete(&pointer.key).await;
        let planted = publish_old(current.clone()).await;
        assert!(matches!(
            Snapshots::new(&storage, &store)
                .read(&current, &planted)
                .await,
            Err(ColdBuildError::Corrupt { .. })
        ));
    }

    /// Cold build history is complete or fails before publication: a
    /// genuinely exhausted empty listing publishes empty history, while a
    /// listing still advertising pages at the page bound, or one whose
    /// page 1 gained an entry below its first while page 2 was read,
    /// fails the build and publishes no pointer.
    #[tokio::test]
    async fn cold_build_history_is_complete_or_unpublished() {
        let identity = identity();
        let build = async |repo: &FakeRepo| {
            let (storage, store) = (MemoryStorage::default(), MemorySnapshotStore::default());
            let client = repo.client();
            let result = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default())
                .run(&identity)
                .await;
            let pointer = store
                .snapshot_pointer(&scope(&identity))
                .await
                .expect("pointer");
            (result, pointer, storage)
        };

        let empty = FakeRepo::new(&standard_files(), &standard_archive());
        let (result, pointer, storage) = build(&empty).await;
        let ColdBuildOutcome::Published(published) = result.expect("empty history").outcome else {
            panic!("empty history publishes");
        };
        assert_eq!(pointer, Some(published.clone()));
        let snapshot = Snapshots::new(&storage, &MemorySnapshotStore::default())
            .read(&scope(&identity), &published)
            .await
            .expect("read back");
        assert!(snapshot.history.prs.is_empty());

        let unproved = FakeRepo::new(&standard_files(), &standard_archive());
        unproved.unproved_history();
        let (result, pointer, storage) = build(&unproved).await;
        assert!(matches!(
            result,
            Err(ColdBuildError::History(HistoryError::UnprovedHistory {
                found: 0
            }))
        ));
        assert_eq!(pointer, None);
        assert!(storage.keys().is_empty());

        let moving = FakeRepo::new(&standard_files(), &standard_archive());
        moving.moving_history();
        let (result, pointer, storage) = build(&moving).await;
        assert!(matches!(
            result,
            Err(ColdBuildError::History(HistoryError::ListingMoved))
        ));
        assert_eq!(pointer, None);
        assert!(storage.keys().is_empty());
    }

    /// Scenario 2: an exhausted fallback
    /// bound, and an exhausted expansion or compressed archive bound each
    /// yield an incomplete result with exact skips; nothing is written and a
    /// prior pointer for another tree stays in place, even when fallback
    /// fetched every file after the archive bound. A parse-error file with
    /// no recovered declarations is a benign skip that still publishes.
    #[tokio::test]
    async fn incomplete_attempts_report_coverage_and_keep_the_prior_pointer() {
        let identity = identity();
        let scope = scope(&identity);
        let prior = SnapshotPointer {
            tree: sha('0'),
            commit: sha('1'),
            key: snapshot_key(&scope, &sha('0')),
            digest: ObjectDigest::of(b""),
        };
        let run = async |repo: &FakeRepo, limits: ColdBuildLimits| {
            let client = repo.client();
            let storage = MemoryStorage::default();
            let persistence = MemorySnapshotStore::default();
            assert!(
                persistence
                    .publish_snapshot_pointer(&scope, None, &prior)
                    .await
                    .expect("seed")
            );
            let result = ColdBuild::new(&client, &storage, &persistence, limits)
                .run(&identity)
                .await
                .expect("build result");
            let pointer = persistence.snapshot_pointer(&scope).await.expect("pointer");
            (
                result,
                storage.keys(),
                pointer,
                client.calls().get(Operation::Blob),
            )
        };

        let mut files = standard_files();
        files.push(file("src/broken.ts", BROKEN));
        let mut archive = standard_archive();
        archive.push(("src/broken.ts", BROKEN));
        let repo = FakeRepo::new(&files, &archive);
        let (result, keys, pointer, _) = run(&repo, ColdBuildLimits::default()).await;
        let ColdBuildOutcome::Published(published) = &result.outcome else {
            panic!(
                "a parse-error file must not block publication, got {:?}",
                result.outcome
            );
        };
        assert_eq!((result.coverage.eligible, result.coverage.indexed), (7, 5));
        let broken = result
            .coverage
            .skipped
            .iter()
            .find(|f| f.path.as_str() == "src/broken.ts")
            .expect("broken skipped");
        assert_eq!(broken.blob, blob_sha(BROKEN));
        assert_eq!(broken.reason, SkipReason::NoFacts);
        assert!(!keys.is_empty());
        assert_eq!(pointer.as_ref(), Some(published));

        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let (result, keys, pointer, blobs) = run(
            &repo,
            ColdBuildLimits {
                max_fallback_blobs: 1,
                ..ColdBuildLimits::default()
            },
        )
        .await;
        assert_eq!(
            result.outcome,
            ColdBuildOutcome::Incomplete(IncompleteReason::UnindexedFiles { count: 1 })
        );
        assert_eq!(blobs, 2, "attributes + one bounded fallback");
        let reasons: Vec<(&str, &SkipReason)> = result
            .coverage
            .skipped
            .iter()
            .map(|f| (f.path.as_str(), &f.reason))
            .collect();
        assert_eq!(
            reasons,
            [
                ("src/lfs.ts", &SkipReason::LfsPointer),
                ("src/substituted.ts", &SkipReason::NotFetched),
            ]
        );
        assert!(keys.is_empty());
        assert_eq!(pointer.as_ref(), Some(&prior));

        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let (result, keys, pointer, blobs) = run(
            &repo,
            ColdBuildLimits {
                max_expanded_archive_bytes: 64,
                ..ColdBuildLimits::default()
            },
        )
        .await;
        assert!(
            result
                .ingest
                .archive_error
                .as_deref()
                .is_some_and(|e| e.contains("archive expansion"))
        );
        assert_eq!(result.ingest.archive_entries, 0);
        assert_eq!(result.ingest.fallback_blobs, 6);
        assert_eq!(blobs, 7);
        assert!(result.coverage.is_complete());
        assert_eq!((result.coverage.eligible, result.coverage.indexed), (6, 5));
        assert_eq!(
            result.outcome,
            ColdBuildOutcome::Incomplete(IncompleteReason::ArchiveTooLarge)
        );
        assert!(keys.is_empty());
        assert_eq!(pointer.as_ref(), Some(&prior));

        let mut repo = FakeRepo::new(&standard_files(), &standard_archive());
        repo.limits.max_archive_bytes = 16;
        let (result, keys, pointer, blobs) = run(&repo, ColdBuildLimits::default()).await;
        assert!(
            result
                .ingest
                .archive_error
                .as_deref()
                .is_some_and(|e| e.contains("exceeds 16 bytes"))
        );
        assert_eq!(blobs, 7);
        assert!(result.coverage.is_complete());
        assert_eq!(
            result.outcome,
            ColdBuildOutcome::Incomplete(IncompleteReason::ArchiveTooLarge)
        );
        assert!(keys.is_empty());
        assert_eq!(pointer.as_ref(), Some(&prior));

        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let (result, keys, pointer, _) = run(
            &repo,
            ColdBuildLimits {
                max_expanded_archive_bytes: 64,
                max_fallback_blobs: 0,
                ..ColdBuildLimits::default()
            },
        )
        .await;
        assert_eq!(
            result.outcome,
            ColdBuildOutcome::Incomplete(IncompleteReason::ArchiveTooLarge)
        );
        assert_eq!(result.coverage.unindexed(), 6);
        assert!(
            result
                .coverage
                .skipped
                .iter()
                .all(|f| f.reason == SkipReason::NotFetched)
        );
        assert!(keys.is_empty());
        assert_eq!(pointer.as_ref(), Some(&prior));
    }

    /// A blob listed under the file bound whose verified bytes exceed it is
    /// a benign skip that keeps its path and blob identity, so the build
    /// still completes.
    #[tokio::test]
    async fn verified_oversize_blob_is_a_benign_skip() {
        let big: &'static [u8] =
            Box::leak(vec![b'x'; MAX_ELIGIBLE_FILE_BYTES as usize + 1].into_boxed_slice());
        let files = [
            file("src/user.ts", USER),
            File {
                path: "src/big.ts",
                mode: "100644",
                content: big,
                size: Some(1),
            },
        ];
        let repo = FakeRepo::new(&files, &[("src/user.ts", USER)]);
        let client = repo.client();
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let result = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default())
            .run(&identity())
            .await
            .expect("build");
        assert!(matches!(result.outcome, ColdBuildOutcome::Published(_)));
        assert_eq!(
            result.coverage.skipped,
            [SkippedFile {
                path: RelativePath::new("src/big.ts").expect("path"),
                blob: blob_sha(big),
                reason: SkipReason::Oversize,
            }]
        );
    }

    /// A provider blob ceiling below the file bound cannot prove an omitted
    /// eligible file oversize: the build is incomplete, names the file, and
    /// keeps the prior pointer. A ceiling at the file bound still proves a
    /// larger blob oversize, so that build publishes with a benign skip.
    #[tokio::test]
    async fn blob_ceiling_below_file_bound_is_not_oversize() {
        const SMALL: &[u8] = b"export const a = 1;\n";
        let omitted: &'static [u8] = Box::leak(vec![b'x'; 200].into_boxed_slice());
        let files = || {
            [
                file("src/a.ts", SMALL),
                File {
                    path: "src/omitted.ts",
                    mode: "100644",
                    content: omitted,
                    size: Some(1),
                },
            ]
        };
        let identity = identity();
        let prior = SnapshotPointer {
            tree: sha('0'),
            commit: sha('1'),
            key: snapshot_key(&scope(&identity), &sha('0')),
            digest: ObjectDigest::of(b""),
        };
        let build = async |max_file_bytes: u64| {
            let mut repo = FakeRepo::new(&files(), &[("src/a.ts", SMALL)]);
            repo.limits.max_blob_bytes = 100;
            let client = repo.client();
            let storage = MemoryStorage::default();
            let store = MemorySnapshotStore::default();
            assert!(
                store
                    .publish_snapshot_pointer(&scope(&identity), None, &prior)
                    .await
                    .expect("seed")
            );
            let limits = ColdBuildLimits {
                max_file_bytes,
                ..ColdBuildLimits::default()
            };
            let result = ColdBuild::new(&client, &storage, &store, limits)
                .run(&identity)
                .await
                .expect("build");
            let pointer = store
                .snapshot_pointer(&snapshot_scope(&identity, max_file_bytes))
                .await
                .expect("read");
            (result, pointer)
        };

        let (result, pointer) = build(ColdBuildLimits::default().max_file_bytes).await;
        assert_eq!(
            result.outcome,
            ColdBuildOutcome::Incomplete(IncompleteReason::UnindexedFiles { count: 1 })
        );
        let [skipped] = result.coverage.skipped.as_slice() else {
            panic!("one skip expected: {:?}", result.coverage.skipped);
        };
        assert_eq!(skipped.path.as_str(), "src/omitted.ts");
        assert_eq!(skipped.blob, blob_sha(omitted));
        assert!(
            matches!(&skipped.reason, SkipReason::FetchFailed { detail } if detail.contains("exceeds 100 bytes"))
        );
        assert_eq!(pointer, Some(prior.clone()));

        let (result, pointer) = build(100).await;
        assert!(matches!(result.outcome, ColdBuildOutcome::Published(_)));
        assert_eq!(result.coverage.skipped[0].reason, SkipReason::Oversize);
        assert_eq!(pointer.map(|p| p.tree), Some(result.tree));
    }

    /// One compressed body chunk carrying many wanted files is indexed while
    /// only a bounded batch of verified bytes waits, independent of how many
    /// files the chunk holds.
    #[tokio::test]
    async fn one_chunk_archive_keeps_verified_bytes_bounded() {
        let sources: Vec<(&'static str, &'static [u8])> = (0..128)
            .map(|n| {
                let path: &'static str = Box::leak(format!("src/f{n}.ts").into_boxed_str());
                let body = format!(
                    "export function f{n}(): number {{\n  return {n};\n}}\n// {}\n",
                    "p".repeat(2000)
                );
                (path, &*Box::leak(body.into_bytes().into_boxed_slice()))
            })
            .collect();
        let files: Vec<File> = sources
            .iter()
            .map(|&(path, bytes)| file(path, bytes))
            .collect();
        let repo = FakeRepo::new(&files, &sources);
        let client = repo.client();
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let result = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default())
            .run(&identity())
            .await
            .expect("build");
        assert!(matches!(result.outcome, ColdBuildOutcome::Published(_)));
        assert_eq!(
            (result.coverage.eligible, result.coverage.indexed),
            (128, 128)
        );
        assert_eq!(
            (result.ingest.archive_entries, result.ingest.fallback_blobs),
            (128, 0)
        );
        let total: usize = sources.iter().map(|(_, bytes)| bytes.len()).sum();
        let bound = 32 * 1024 + 2100;
        assert!(total > 4 * bound);
        assert!(
            (1..=bound).contains(&result.ingest.peak_verified_bytes),
            "peak {} of {total}",
            result.ingest.peak_verified_bytes
        );
    }

    /// Scenario 3: the build future is dropped by a paused-clock timeout
    /// after the record upsert, after the object create, and before the
    /// pointer swap, with and without a prior pointer. Each drop leaves the
    /// pointer where it was and every stored object recorded; the retry
    /// adopts what was written, lands on the same key, and advances.
    #[tokio::test(start_paused = true)]
    async fn dropped_publication_keeps_pointer_and_retry_converges() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let identity = identity();
        let key = snapshot_key(&scope(&identity), &repo.commit);
        let prior = SnapshotPointer {
            tree: sha('0'),
            commit: sha('1'),
            key: snapshot_key(&scope(&identity), &sha('0')),
            digest: ObjectDigest::of(b""),
        };
        type Stall = fn(&MemoryStorage, &MemorySnapshotStore);
        let steps: [(Stall, bool); 3] = [
            (|_, store| store.stall_after_upsert(), false),
            (|storage, _| storage.stall_after_create(), true),
            (|_, store| store.stall_before_swap(), true),
        ];
        for prior in [None, Some(prior)] {
            for (stall, object_written) in steps {
                let storage = MemoryStorage::default();
                let store = MemorySnapshotStore::default();
                if let Some(prior) = &prior {
                    assert!(
                        store
                            .publish_snapshot_pointer(&scope(&identity), None, prior)
                            .await
                            .expect("seed")
                    );
                }
                let build = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default());
                stall(&storage, &store);
                let dropped =
                    tokio::time::timeout(Duration::from_secs(60), build.run(&identity)).await;
                assert!(dropped.is_err(), "the stalled build must be dropped");
                let pointer = store
                    .snapshot_pointer(&scope(&identity))
                    .await
                    .expect("read");
                assert_eq!(pointer, prior);
                assert_eq!(store.records(), [(identity.clone(), key.clone())]);
                let expected_objects = if object_written {
                    vec![key.clone()]
                } else {
                    vec![]
                };
                assert_eq!(storage.keys(), expected_objects);

                let retry = build.run(&identity).await.expect("retry");
                let ColdBuildOutcome::Published(published) = retry.outcome else {
                    panic!("retry must publish, got {:?}", retry.outcome);
                };
                assert_eq!(published.key, key);
                assert_eq!(storage.keys(), std::slice::from_ref(&key));
                assert_eq!(store.records(), [(identity.clone(), key.clone())]);
                assert_eq!(
                    store
                        .snapshot_pointer(&scope(&identity))
                        .await
                        .expect("read"),
                    Some(published.clone())
                );
                assert!(
                    Snapshots::new(&storage, &store)
                        .read(&scope(&identity), &published)
                        .await
                        .is_ok()
                );
            }
        }
    }

    /// An existing object under the key is adopted only when its bytes equal
    /// the built snapshot; other bytes are refused and the pointer stays
    /// absent, while the record keeps the object collectable.
    #[tokio::test]
    async fn existing_object_with_other_bytes_is_refused() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let identity = identity();
        let key = snapshot_key(&scope(&identity), &repo.commit);
        storage
            .create(&key, b"{\"not\":\"a snapshot\"}")
            .await
            .expect("seed");
        let result = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default())
            .run(&identity)
            .await;
        assert!(matches!(result, Err(ColdBuildError::Corrupt { .. })));
        assert_eq!(
            store
                .snapshot_pointer(&scope(&identity))
                .await
                .expect("read"),
            None
        );
        assert_eq!(store.records(), [(identity.clone(), key)]);
    }

    /// A swap that loses to another writer returns the typed retry state and
    /// leaves the winner's pointer in place.
    #[tokio::test]
    async fn lost_pointer_swap_returns_retry_state() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let identity = identity();
        let build = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default());
        let ColdBuildOutcome::Published(pointer) =
            build.run(&identity).await.expect("build").outcome
        else {
            panic!("expected publication");
        };
        let snapshot = Snapshots::new(&storage, &store)
            .read(&scope(&identity), &pointer)
            .await
            .expect("read");
        let winner = SnapshotPointer {
            tree: sha('0'),
            key: snapshot_key(&scope(&identity), &sha('0')),
            ..pointer.clone()
        };
        assert!(
            store
                .publish_snapshot_pointer(&scope(&identity), Some(&pointer.tree), &winner)
                .await
                .expect("winner")
        );
        assert_eq!(
            build
                .publish(&scope(&identity), Some(&pointer), &snapshot)
                .await,
            Err(ColdBuildError::PointerConflict)
        );
        assert_eq!(
            store
                .snapshot_pointer(&scope(&identity))
                .await
                .expect("read"),
            Some(winner)
        );
    }

    /// Scenario 4 race: paused-store interleavings of `collect` and a build
    /// of the same deterministic key. When the collector claims first, the
    /// racing build fails with the retry state before writing anything,
    /// the collector deletes object then record, and the retried build
    /// publishes a present object. When the build refreshes first, the
    /// collector selects nothing and the build publishes. No state names a
    /// missing object or leaves an object unrecorded.
    #[tokio::test]
    async fn collection_claim_and_publication_never_race() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let owner = identity();
        let key = snapshot_key(&scope(&owner), &repo.commit);
        let recorded = || vec![(owner.clone(), key.clone())];

        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let build = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default());
        let snapshots = Snapshots::new(&storage, &store);
        store.upsert_record(&owner, &key).await.expect("seed");
        storage.create(&key, b"stale").await.expect("seed");
        store.clock.advance(Duration::from_secs(3600));
        storage.hold_deletes.store(true, Ordering::SeqCst);
        let (collected, raced) =
            tokio::join!(snapshots.collect(&owner, Duration::from_secs(60)), async {
                let raced = build.run(&owner).await;
                assert_eq!(
                    store.snapshot_pointer(&scope(&owner)).await.expect("read"),
                    None
                );
                assert_eq!(store.records(), recorded());
                assert_eq!(storage.keys(), std::slice::from_ref(&key));
                storage.hold_deletes.store(false, Ordering::SeqCst);
                raced
            });
        assert_eq!(
            raced,
            Err(ColdBuildError::Persistence(PersistenceError::Collecting {
                key: key.clone()
            }))
        );
        assert_eq!(collected.expect("collect"), std::slice::from_ref(&key));
        assert!(storage.keys().is_empty() && store.records().is_empty());
        let ColdBuildOutcome::Published(pointer) = build.run(&owner).await.expect("retry").outcome
        else {
            panic!("retry must publish");
        };
        assert_eq!(pointer.key, key);
        assert_eq!(store.records(), recorded());
        assert!(snapshots.read(&scope(&owner), &pointer).await.is_ok());

        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let build = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default());
        let snapshots = Snapshots::new(&storage, &store);
        store.upsert_record(&owner, &key).await.expect("seed");
        store.clock.advance(Duration::from_secs(3600));
        store.hold_swaps.store(true, Ordering::SeqCst);
        let (built, collected) = tokio::join!(build.run(&owner), async {
            let collected = snapshots.collect(&owner, Duration::from_secs(60)).await;
            assert_eq!(store.records(), recorded());
            assert_eq!(storage.keys(), std::slice::from_ref(&key));
            assert_eq!(
                store.snapshot_pointer(&scope(&owner)).await.expect("read"),
                None
            );
            store.hold_swaps.store(false, Ordering::SeqCst);
            collected
        });
        assert_eq!(collected.expect("collect"), []);
        let ColdBuildOutcome::Published(pointer) = built.expect("build").outcome else {
            panic!("build must publish");
        };
        assert_eq!(
            snapshots
                .collect(&owner, Duration::ZERO)
                .await
                .expect("collect"),
            []
        );
        assert_eq!(store.records(), recorded());
        assert!(snapshots.read(&scope(&owner), &pointer).await.is_ok());
    }

    /// A snapshot is a pure function of identity, versions, and tree: an
    /// archive-fed build and an all-fallback build of the same tree write
    /// identical bytes even though their ingest stats differ, and an archive
    /// entry larger than the file bound is refetched as a verified blob
    /// instead of becoming a benign oversize skip.
    #[tokio::test]
    async fn same_tree_serializes_identically_across_ingest_paths() {
        let big: &'static [u8] =
            Box::leak(vec![b'x'; MAX_ELIGIBLE_FILE_BYTES as usize + 1].into_boxed_slice());
        let mut archive = standard_archive();
        archive.retain(|(path, _)| *path != "src/format.ts");
        archive.push(("src/format.ts", big));
        let identity = identity();
        let publish = async |repo: FakeRepo| {
            let client = repo.client();
            let storage = MemoryStorage::default();
            let store = MemorySnapshotStore::default();
            let result = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default())
                .run(&identity)
                .await
                .expect("build");
            let ColdBuildOutcome::Published(pointer) = &result.outcome else {
                panic!("expected publication, got {:?}", result.outcome);
            };
            (
                result.ingest,
                storage.read(&pointer.key).await.expect("object"),
            )
        };

        let (streamed, streamed_bytes) = publish(FakeRepo::new(&standard_files(), &archive)).await;
        assert_eq!((streamed.archive_entries, streamed.fallback_blobs), (3, 3));
        let (fallback, fallback_bytes) = publish(FakeRepo::new(&standard_files(), &[])).await;
        assert_eq!((fallback.archive_entries, fallback.fallback_blobs), (0, 6));
        assert_eq!(streamed_bytes, fallback_bytes);
        let text = String::from_utf8(streamed_bytes).expect("json");
        for field in [
            "archive_entries",
            "fallback_blobs",
            "archive_error",
            "published_at",
        ] {
            assert!(!text.contains(field), "{field} persisted in the snapshot");
        }
    }

    /// A truncated root listing and a truncated nested listing are split
    /// into subtree listings whose joined entries publish the same snapshot
    /// bytes and coverage as the untruncated recursive listing.
    #[tokio::test]
    async fn truncated_listing_splits_into_the_complete_snapshot() {
        let files = [
            file("README.md", b"# hi\n"),
            dir("src", b"tree:src"),
            file("src/user.ts", USER),
            file("src/format.ts", FORMAT),
            dir("src/lib", b"tree:src/lib"),
            file("src/lib/handler.ts", HANDLER),
        ];
        let archive = [
            ("src/user.ts", USER),
            ("src/format.ts", FORMAT),
            ("src/lib/handler.ts", HANDLER),
        ];
        let identity = identity();
        let publish = async |repo: &FakeRepo| {
            let client = repo.client();
            let storage = MemoryStorage::default();
            let store = MemorySnapshotStore::default();
            let result = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default())
                .run(&identity)
                .await
                .expect("build");
            let ColdBuildOutcome::Published(pointer) = &result.outcome else {
                panic!("expected publication, got {:?}", result.outcome);
            };
            let bytes = storage.read(&pointer.key).await.expect("object");
            (result.coverage, bytes, client.calls().get(Operation::Tree))
        };

        let (oracle_coverage, oracle_bytes, _) = publish(&FakeRepo::new(&files, &archive)).await;
        assert_eq!(oracle_coverage.indexed, 3);

        let repo = FakeRepo::new(&files, &archive);
        let (src, lib) = (blob_sha(b"tree:src"), blob_sha(b"tree:src/lib"));
        let route = |tree: &ObjectSha, query: &str, truncated: bool, files: &[File]| {
            repo.transport.json(
                &format!("/git/trees/{tree}{query}"),
                &listing(tree, truncated, files),
            );
        };
        route(&repo.tree, "?recursive=1", true, &files[..2]);
        route(
            &repo.tree,
            "",
            false,
            &[file("README.md", b"# hi\n"), dir("src", b"tree:src")],
        );
        route(&src, "?recursive=1", true, &[]);
        route(
            &src,
            "",
            false,
            &[
                file("user.ts", USER),
                file("format.ts", FORMAT),
                dir("lib", b"tree:src/lib"),
            ],
        );
        route(&lib, "?recursive=1", false, &[file("handler.ts", HANDLER)]);
        let (coverage, bytes, tree_calls) = publish(&repo).await;
        assert!(coverage.is_complete());
        assert_eq!(coverage, oracle_coverage);
        assert_eq!(bytes, oracle_bytes);
        assert_eq!(tree_calls, 5);
    }

    /// A tree whose non-recursive listing is itself truncated cannot be
    /// split: the attempt is incomplete with a truncated-tree coverage
    /// reason, nothing is written, and the prior pointer stays.
    #[tokio::test]
    async fn unsplittable_truncated_listing_is_incomplete_and_unpublished() {
        let identity = identity();
        let scope = scope(&identity);
        let prior = SnapshotPointer {
            tree: sha('0'),
            commit: sha('1'),
            key: snapshot_key(&scope, &sha('0')),
            digest: ObjectDigest::of(b""),
        };
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        repo.truncate_tree();
        let client = repo.client();
        let storage = MemoryStorage::default();
        let persistence = MemorySnapshotStore::default();
        assert!(
            persistence
                .publish_snapshot_pointer(&scope, None, &prior)
                .await
                .expect("seed")
        );
        let result = ColdBuild::new(&client, &storage, &persistence, ColdBuildLimits::default())
            .run(&identity)
            .await
            .expect("build result");
        assert_eq!(
            result.outcome,
            ColdBuildOutcome::Incomplete(IncompleteReason::TruncatedTree)
        );
        assert!(result.coverage.truncated_tree && !result.coverage.is_complete());
        assert!(storage.keys().is_empty());
        assert_eq!(
            persistence.snapshot_pointer(&scope).await.expect("pointer"),
            Some(prior)
        );
    }

    /// S14 snapshot-store isolation: an identity with no rows sees no
    /// pointer even when another tenant or installation of the same
    /// repository has one; a pointer naming another identity's key is
    /// refused before storage is read; each identity's build publishes its
    /// own key without advancing the other's pointer.
    #[tokio::test]
    async fn identities_cannot_read_or_advance_each_others_snapshots() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let build = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default());
        let owner = identity();
        let other_tenant = Identity {
            tenant: TenantId::new("other").expect("tenant"),
            ..owner.clone()
        };
        let other_installation = Identity {
            installation: InstallationId::new(43).expect("installation"),
            ..owner.clone()
        };

        let result = build.run(&owner).await.expect("owner build");
        let ColdBuildOutcome::Published(pointer) = result.outcome else {
            panic!("owner must publish");
        };
        for intruder in [&other_tenant, &other_installation] {
            assert_eq!(
                store
                    .snapshot_pointer(&scope(intruder))
                    .await
                    .expect("read"),
                None
            );
            assert_eq!(
                Snapshots::new(&storage, &store)
                    .read(&scope(intruder), &pointer)
                    .await,
                Err(ColdBuildError::ForeignKey {
                    key: pointer.key.clone()
                })
            );

            let own = build.run(intruder).await.expect("intruder build");
            let ColdBuildOutcome::Published(own) = own.outcome else {
                panic!("intruder builds its own snapshot");
            };
            assert_ne!(own.key, pointer.key);
            assert_eq!(
                store.snapshot_pointer(&scope(&owner)).await.expect("read"),
                Some(pointer.clone())
            );
        }
        assert_eq!(storage.keys().len(), 3);
        assert!(
            Snapshots::new(&storage, &store)
                .read(&scope(&owner), &pointer)
                .await
                .is_ok()
        );
    }

    /// Scenario 4: `collect` deletes the object, then the record, of each
    /// stale unreferenced record of its identity only. A missing object
    /// counts as deleted; a failed object delete keeps its record for the
    /// next call; the pointed-to snapshot and a record re-touched by an
    /// in-flight build survive; another identity's records are untouched;
    /// namespace removal returns only that repository's keys; a pointer
    /// whose object went missing is rebuilt at the same key.
    #[tokio::test(start_paused = true)]
    async fn collect_removes_only_stale_unreferenced_records() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let build = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default());
        let snapshots = Snapshots::new(&storage, &store);
        let owner = identity();
        let other = Identity {
            installation: InstallationId::new(43).expect("installation"),
            ..owner.clone()
        };
        let head = snapshot_key(&scope(&owner), &repo.commit);
        let stale = snapshot_key(&scope(&owner), &sha('1'));
        let gone = snapshot_key(&scope(&owner), &sha('2'));
        let foreign = snapshot_key(&scope(&other), &sha('1'));
        for (identity, key) in [
            (&owner, &head),
            (&owner, &stale),
            (&owner, &gone),
            (&other, &foreign),
        ] {
            store
                .upsert_record(identity, key)
                .await
                .expect("seed record");
        }
        storage.create(&stale, b"stale").await.expect("seed");
        storage.create(&foreign, b"foreign").await.expect("seed");

        let grace = Duration::from_secs(60);
        store.clock.advance(Duration::from_secs(3600));
        store.stall_before_swap();
        assert!(
            tokio::time::timeout(Duration::from_secs(60), build.run(&owner))
                .await
                .is_err()
        );
        storage.fail_next_delete();
        assert!(matches!(
            snapshots.collect(&owner, grace).await,
            Err(ColdBuildError::Storage(StorageError::Backend { .. }))
        ));
        assert!(store.records().contains(&(owner.clone(), stale.clone())));
        assert_eq!(
            snapshots.collect(&owner, grace).await.expect("collect"),
            [stale.clone(), gone.clone()]
        );
        assert_eq!(storage.keys(), [head.clone(), foreign.clone()]);
        assert_eq!(
            store.records(),
            [
                (owner.clone(), head.clone()),
                (other.clone(), foreign.clone())
            ]
        );

        let ColdBuildOutcome::Published(pointer) = build.run(&owner).await.expect("build").outcome
        else {
            panic!("expected publication");
        };
        assert_eq!(
            snapshots
                .collect(&owner, Duration::ZERO)
                .await
                .expect("collect"),
            []
        );
        assert!(snapshots.read(&scope(&owner), &pointer).await.is_ok());

        storage.delete(&head).await.expect("lose object");
        let ColdBuildOutcome::Published(rebuilt) =
            build.run(&owner).await.expect("rebuild").outcome
        else {
            panic!("a missing pointed-to object is rebuilt");
        };
        assert_eq!(rebuilt.key, head);
        assert!(snapshots.read(&scope(&owner), &rebuilt).await.is_ok());

        assert_eq!(
            snapshots.remove_repository(&owner).await.expect("remove"),
            [head]
        );
        assert_eq!(
            store.snapshot_pointer(&scope(&owner)).await.expect("read"),
            None
        );
        assert_eq!(storage.keys(), std::slice::from_ref(&foreign));
        assert_eq!(store.records(), [(other, foreign)]);
    }

    /// A faulty store that returns a key outside the identity's namespace
    /// cannot steer deletion there.
    #[tokio::test]
    async fn collect_refuses_foreign_keys() {
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let owner = identity();
        let victim = Identity {
            tenant: TenantId::new("victim").expect("tenant"),
            ..owner.clone()
        };
        let key = snapshot_key(&scope(&victim), &sha('1'));
        storage.create(&key, b"victim").await.expect("seed");
        store
            .upsert_record(&owner, &key)
            .await
            .expect("forge record");
        assert_eq!(
            Snapshots::new(&storage, &store)
                .collect(&owner, Duration::ZERO)
                .await,
            Err(ColdBuildError::ForeignKey { key: key.clone() })
        );
        assert_eq!(storage.keys(), [key]);
    }

    /// The cold-build facade future is `Send`, so a consumer can run it on a
    /// multi-threaded runtime; this fails to compile otherwise.
    #[test]
    fn cold_build_future_is_send() {
        fn assert_send<T: Send>(_: T) {}
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let identity = identity();
        let build = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default());
        assert_send(build.run(&identity));
        assert_send(Snapshots::new(&storage, &store).collect(&identity, Duration::ZERO));
    }

    /// A listing entry at a non-canonical path is excluded as unsupported
    /// and the snapshot still publishes complete.
    #[tokio::test]
    async fn non_canonical_tree_path_is_an_unsupported_exclusion() {
        let files = [file("src/user.ts", USER), file("src/a\\\\b.ts", FORMAT)];
        let repo = FakeRepo::new(&files, &[("src/user.ts", USER)]);
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let result = ColdBuild::new(&repo.client(), &storage, &store, ColdBuildLimits::default())
            .run(&identity())
            .await
            .expect("build");
        assert!(matches!(result.outcome, ColdBuildOutcome::Published(_)));
        assert_eq!(
            result.coverage.excluded,
            BTreeMap::from([(ExclusionReason::Unsupported, 1)])
        );
        assert!(result.coverage.is_complete());
        assert_eq!(storage.keys().len(), 1);
    }

    /// A listing entry with an unknown mode or malformed id fails the build
    /// with a provider error and publishes nothing.
    #[tokio::test]
    async fn broken_tree_entry_fails_the_build() {
        let odd_mode = File {
            path: "docs/notes.md",
            mode: "100600",
            content: HANDLER,
            size: None,
        };
        for broken in [odd_mode, bad_sha("docs/notes.md")] {
            let repo = FakeRepo::new(&[file("src/user.ts", USER), broken], &[]);
            let storage = MemoryStorage::default();
            let store = MemorySnapshotStore::default();
            let error =
                ColdBuild::new(&repo.client(), &storage, &store, ColdBuildLimits::default())
                    .run(&identity())
                    .await
                    .expect_err("broken listing");
            assert!(matches!(
                error,
                ColdBuildError::GitHub(GitHubError::MalformedResponse { .. })
            ));
            assert!(storage.keys().is_empty());
            assert!(store.records().is_empty());
        }
    }
}
