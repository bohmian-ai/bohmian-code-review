//! Cold default-branch snapshot build, publication, read-back, and cleanup.
//!
//! [`ColdBuild::run`] pins the default branch, classifies its tree with
//! [`classify`], streams the commit tarball through bounded decompression,
//! verifies every eligible entry against its Git blob id, falls back to
//! bounded verified blob calls for anything the archive omitted or
//! substituted, and indexes through the public [`SymbolicContext`]. Source
//! bytes live only while one entry is being parsed.
//!
//! A complete [`Snapshot`] is published in three idempotent steps: upsert
//! its record (refreshing `touched_at`), create its object if absent, and
//! compare-and-swap the pointer. An incomplete attempt writes nothing and
//! returns exact coverage. Dropping the build future at any `.await` leaves
//! the pointer on a complete snapshot or absent and every object recorded,
//! and a retry converges on the same key. [`Snapshots`] reads published
//! snapshots and collects records no pointer names.

use crate::archive::{TarStream, VerifiedEntry};
use crate::eligibility::{
    Exclusion, ExclusionReason, MAX_ELIGIBLE_FILE_BYTES, RootAttributes, classify,
};
use crate::github::{Archive, GitHubClient, GitHubError, TreeEntry, TreeEntryMode};
use crate::identity::{Identity, ObjectSha};
use crate::persistence::{PersistenceError, SnapshotPointer, SnapshotScope, SnapshotStore};
use crate::storage::{ObjectDigest, ObjectKey, ObjectStorage, StorageError};
use bohmian_code_review::Error as SymbolicError;
use bohmian_code_review::{EXTRACTION_VERSION, IndexedFileFacts, RelativePath, SymbolicContext};
use flate2::write::GzDecoder;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::time::SystemTime;
use thiserror::Error;

/// Layout version of the serialized [`Snapshot`]; part of every snapshot key.
pub const SNAPSHOT_FORMAT_VERSION: u32 = 1;

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
}

impl Default for ColdBuildLimits {
    /// 1 MiB files, 4 GiB expanded archive, 200 fallback blobs.
    fn default() -> ColdBuildLimits {
        ColdBuildLimits {
            max_file_bytes: MAX_ELIGIBLE_FILE_BYTES,
            max_expanded_archive_bytes: 4 * 1024 * 1024 * 1024,
            max_fallback_blobs: 200,
        }
    }
}

/// Why an eligible file was not indexed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkipReason {
    /// Content is a Git LFS pointer, not source.
    LfsPointer,
    /// Fetched bytes exceeded the file bound.
    Oversize,
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
        matches!(self, SkipReason::LfsPointer | SkipReason::Oversize)
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
    /// The provider truncated the recursive listing; nothing was indexed.
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
    /// Every eligible file was indexed or skipped for a content reason.
    pub fn is_complete(&self) -> bool {
        !self.truncated_tree && self.skipped.iter().all(|file| file.reason.is_benign())
    }

    /// Eligible files that were neither indexed nor benignly skipped.
    fn unindexed(&self) -> usize {
        self.skipped
            .iter()
            .filter(|file| !file.reason.is_benign())
            .count()
    }
}

/// One eligible path with the blob id it was indexed or skipped at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFile {
    /// Repository-relative path.
    pub path: RelativePath,
    /// Blob id from the pinned tree.
    pub blob: ObjectSha,
}

/// The immutable, source-free object stored at a snapshot key.
///
/// Creation time, branch commit, and per-attempt ingest counters live
/// outside it, so the same identity, versions, and tree always serialize to
/// the same bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// [`SNAPSHOT_FORMAT_VERSION`] this object was written under.
    pub format_version: u32,
    /// Extractor that produced `facts`.
    pub extraction_version: String,
    /// Caller identity the key was scoped to.
    pub identity: Identity,
    /// Tree the snapshot indexes.
    pub tree: ObjectSha,
    /// Every eligible path with its blob id, in canonical order.
    pub files: Vec<SnapshotFile>,
    /// Source-free declaration facts of every indexed file.
    pub facts: Vec<IndexedFileFacts>,
    /// Coverage of the tree; always complete.
    pub coverage: Coverage,
}

impl Snapshot {
    /// Decode and validate object bytes against the identity and tree they
    /// were published for.
    ///
    /// # Errors
    ///
    /// Returns [`ColdBuildError::Corrupt`] when the bytes do not decode,
    /// name another identity, tree, or version, or are not complete.
    fn decode(
        bytes: &[u8],
        identity: &Identity,
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
        if snapshot.tree != *tree {
            return Err(corrupt(format!("tree {} is not {tree}", snapshot.tree)));
        }
        if !snapshot.coverage.is_complete() {
            return Err(corrupt("incomplete coverage".to_owned()));
        }
        Ok(snapshot)
    }

    /// Load every fact into a fresh in-memory context without any source.
    ///
    /// # Errors
    ///
    /// Returns the facade's [`SymbolicError`] for a fact stamped by another
    /// extractor or violating a declaration invariant.
    pub fn into_context(self) -> Result<SymbolicContext, SymbolicError> {
        let mut context = SymbolicContext::new();
        for facts in self.facts {
            context.load(facts)?;
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

    /// Read and validate the snapshot a pointer names for `identity`.
    ///
    /// The pointer's key must equal the key this library builds for
    /// `identity`, the current versions, and the pointer's tree before
    /// storage is touched, so a pointer row leaked or copied from another
    /// tenant or installation can never read that identity's object.
    ///
    /// # Errors
    ///
    /// Returns [`ColdBuildError::ForeignKey`] when the pointer names any other
    /// key, [`ColdBuildError::Storage`] when the object is missing or the
    /// backend fails, and [`ColdBuildError::Corrupt`] when its bytes do not
    /// match the pointer's digest or do not decode as a complete snapshot of
    /// that identity and tree.
    pub async fn read(
        &self,
        identity: &Identity,
        pointer: &SnapshotPointer,
    ) -> Result<Snapshot, ColdBuildError> {
        let expected = snapshot_key(identity, &pointer.tree);
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
        Snapshot::decode(&bytes, identity, &pointer.tree)
    }

    /// Delete every snapshot of `identity` that no pointer names and no
    /// build touched since `older_than`, returning the deleted keys.
    ///
    /// Each object is deleted before its record, so a failure part-way
    /// leaves a record the next call retries rather than an untracked
    /// object; an already missing object deletes cleanly. The caller sets
    /// `older_than` further back than its longest build so an in-flight
    /// build's record, touched when it began publishing, is never
    /// collected. Every key must be a snapshot key of `identity`, so a
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
        older_than: SystemTime,
    ) -> Result<Vec<ObjectKey>, ColdBuildError> {
        let mut deleted = Vec::new();
        for key in self.store.unreferenced(identity, older_than).await? {
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
    /// collect every record touched before now, returning the deleted keys.
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
        self.collect(identity, SystemTime::now()).await
    }
}

/// The only key a snapshot of `tree` for `identity` may live at under the
/// current extraction and format versions.
fn snapshot_key(identity: &Identity, tree: &ObjectSha) -> ObjectKey {
    ObjectKey::snapshot(identity, EXTRACTION_VERSION, SNAPSHOT_FORMAT_VERSION, tree)
}

/// Why a complete attempt was not produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncompleteReason {
    /// The provider truncated the recursive tree listing.
    TruncatedTree,
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
    /// Blobs fetched individually after the archive.
    pub fallback_blobs: usize,
    /// Why the archive stream stopped early, if it did; never source text.
    pub archive_error: Option<String>,
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
    client: &'a GitHubClient,
    storage: &'a O,
    store: &'a S,
    limits: ColdBuildLimits,
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
    /// [`ColdBuild::publish`]; a retry after a drop at any `.await`
    /// converges.
    ///
    /// # Errors
    ///
    /// Returns provider errors for the repository, branch, tree, and
    /// attributes calls, storage and persistence errors, `ForeignKey` or
    /// `Corrupt` for an unusable current pointer or existing object, and
    /// `PointerConflict`, a retry state, when another writer moved the
    /// pointer first. Per-file
    /// failures and an unusable archive never error; they appear in
    /// coverage and ingest stats.
    pub async fn run(&self, identity: &Identity) -> Result<ColdBuildResult, ColdBuildError> {
        let scope = SnapshotScope {
            identity: identity.clone(),
            extraction_version: EXTRACTION_VERSION.to_owned(),
            format_version: SNAPSHOT_FORMAT_VERSION,
        };
        let current = self.store.snapshot_pointer(&scope).await?;
        let repository = self.client.repository(&identity.repository).await?;
        let head = self
            .client
            .branch_head(&identity.repository, &repository.default_branch)
            .await?;
        let result = |coverage, ingest, outcome| ColdBuildResult {
            commit: head.commit.clone(),
            tree: head.tree.clone(),
            coverage,
            ingest,
            outcome,
        };
        if let Some(pointer) = current.as_ref().filter(|pointer| pointer.tree == head.tree) {
            match Snapshots::new(self.storage, self.store)
                .read(identity, pointer)
                .await
            {
                Ok(snapshot) => {
                    return Ok(result(
                        snapshot.coverage,
                        IngestStats::default(),
                        ColdBuildOutcome::Reused(pointer.clone()),
                    ));
                }
                Err(ColdBuildError::Storage(StorageError::NotFound { .. })) => {}
                Err(error) => return Err(error),
            }
        }

        let listing = self.client.tree(&identity.repository, &head.tree).await?;
        if listing.truncated {
            let coverage = Coverage {
                truncated_tree: true,
                ..Coverage::default()
            };
            return Ok(result(
                coverage,
                IngestStats::default(),
                ColdBuildOutcome::Incomplete(IncompleteReason::TruncatedTree),
            ));
        }
        let attributes = match listing.entries.iter().find(|entry| {
            entry.path.as_str() == ".gitattributes" && entry.mode == TreeEntryMode::File
        }) {
            Some(entry) => {
                RootAttributes::parse(&self.client.blob(&identity.repository, &entry.sha).await?)
            }
            None => RootAttributes::default(),
        };
        let mut ingest = Ingest::new(
            &listing.entries,
            listing.rejected.len(),
            &attributes,
            self.limits.max_file_bytes,
        );
        match self
            .client
            .archive(&identity.repository, &head.commit)
            .await
        {
            Ok(archive) => {
                ingest
                    .consume_archive(archive, self.limits.max_expanded_archive_bytes)
                    .await
            }
            Err(error) => ingest.stats.archive_error = Some(error.to_string()),
        }
        ingest
            .fallback(self.client, identity, self.limits.max_fallback_blobs)
            .await;
        let (coverage, stats, files, context) = ingest.finish();
        if !coverage.is_complete() {
            let count = coverage.unindexed();
            return Ok(result(
                coverage,
                stats,
                ColdBuildOutcome::Incomplete(IncompleteReason::UnindexedFiles { count }),
            ));
        }

        let snapshot = Snapshot {
            format_version: SNAPSHOT_FORMAT_VERSION,
            extraction_version: EXTRACTION_VERSION.to_owned(),
            identity: identity.clone(),
            tree: head.tree.clone(),
            files,
            facts: context.export(),
            coverage: coverage.clone(),
        };
        let pointer = self
            .publish(&scope, current.as_ref(), &head.commit, &snapshot)
            .await?;
        Ok(result(
            coverage,
            stats,
            ColdBuildOutcome::Published(pointer),
        ))
    }

    /// Publish `snapshot` in three idempotent steps: upsert its record,
    /// refreshing `touched_at`; create its object if absent, adopting an
    /// existing object only when its bytes equal the built bytes (which
    /// embed identity, versions, and tree); then compare-and-swap the
    /// pointer from `current`.
    ///
    /// Dropped after any step, the pointer is untouched and every object
    /// has a record; a retry of the same tree reaches the same key and
    /// adopts what the dropped attempt wrote.
    ///
    /// # Errors
    ///
    /// Storage, persistence, `Corrupt`, and `PointerConflict` as in [`ColdBuild::run`].
    async fn publish(
        &self,
        scope: &SnapshotScope,
        current: Option<&SnapshotPointer>,
        commit: &ObjectSha,
        snapshot: &Snapshot,
    ) -> Result<SnapshotPointer, ColdBuildError> {
        let identity = &scope.identity;
        let key = snapshot_key(identity, &snapshot.tree);
        let bytes = serde_json::to_vec(snapshot).map_err(|error| ColdBuildError::Corrupt {
            detail: error.to_string(),
        })?;
        self.store
            .upsert_record(identity, &key, SystemTime::now())
            .await?;
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
            commit: commit.clone(),
            key,
            digest: ObjectDigest::of(&bytes),
            published_at: SystemTime::now(),
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

/// In-memory ingest state for one attempt: eligible entries still wanted,
/// and what was indexed and skipped.
struct Ingest {
    context: SymbolicContext,
    max_file_bytes: u64,
    files: Vec<SnapshotFile>,
    pending: BTreeMap<RelativePath, ObjectSha>,
    skipped: Vec<SkippedFile>,
    excluded: Vec<Exclusion>,
    rejected: usize,
    indexed: usize,
    stats: IngestStats,
}

impl Ingest {
    /// Classify `entries` and start with every eligible file pending;
    /// `rejected` listing entries failed validation and are counted as
    /// [`ExclusionReason::InvalidEntry`].
    fn new(
        entries: &[TreeEntry],
        rejected: usize,
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
        Ingest {
            context,
            max_file_bytes,
            files,
            pending,
            skipped: Vec::new(),
            excluded: tree.excluded,
            rejected,
            indexed: 0,
            stats: IngestStats::default(),
        }
    }

    /// Stream the archive through gzip into [`TarStream`], indexing each
    /// verified entry as soon as the chunk that completes it arrives.
    ///
    /// Only the chunk in flight, one header or metadata record, and one
    /// wanted file are held; a provider, decompression, tar, or expansion
    /// error stops the archive and is recorded, leaving the rest pending
    /// for [`Ingest::fallback`]. Entries indexed before it stay indexed.
    async fn consume_archive(&mut self, mut archive: Archive, max_expanded_bytes: u64) {
        let tar = TarStream::new(
            self.pending.clone(),
            self.max_file_bytes,
            max_expanded_bytes,
        );
        let mut decoder = GzDecoder::new(tar);
        let error = loop {
            let written = match archive.chunk().await {
                Ok(Some(chunk)) => decoder.write_all(&chunk),
                Ok(None) => break decoder.try_finish().err().map(|error| error.to_string()),
                Err(error) => break Some(error.to_string()),
            };
            self.ingest_verified(&mut decoder.get_mut().verified);
            if let Err(error) = written {
                break Some(error.to_string());
            }
        };
        self.ingest_verified(&mut decoder.get_mut().verified);
        self.stats.archive_error = error;
    }

    /// Index and drain entries the tar stream verified against their blob ids.
    fn ingest_verified(&mut self, verified: &mut Vec<VerifiedEntry>) {
        for (path, blob, bytes) in verified.drain(..) {
            self.stats.archive_entries += 1;
            self.ingest(path, blob, &bytes);
        }
    }

    /// Fetch every still-pending blob individually, up to `max_blobs`, and
    /// index it; everything still pending afterwards is skipped as not
    /// fetched. A blob larger than the file bound, or than the client's
    /// blob bound, is a benign oversize skip.
    async fn fallback(&mut self, client: &GitHubClient, identity: &Identity, max_blobs: usize) {
        while let Some((path, blob)) = self.pending.pop_first() {
            if self.stats.fallback_blobs >= max_blobs {
                self.skip(path, blob, SkipReason::NotFetched);
                continue;
            }
            self.stats.fallback_blobs += 1;
            match client.blob(&identity.repository, &blob).await {
                Ok(bytes) if bytes.len() as u64 > self.max_file_bytes => {
                    self.skip(path, blob, SkipReason::Oversize);
                }
                Ok(bytes) => self.ingest(path, blob, &bytes),
                Err(GitHubError::ResponseTooLarge { .. }) => {
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

    /// Index verified `bytes` for `path`, or skip it for an LFS pointer or
    /// extractor failure. The bytes are dropped by the caller afterwards.
    fn ingest(&mut self, path: RelativePath, blob: ObjectSha, bytes: &[u8]) {
        if bytes.starts_with(LFS_POINTER_PREFIX) {
            self.skip(path, blob, SkipReason::LfsPointer);
            return;
        }
        match self.context.upsert(path.as_str(), bytes) {
            Ok(_) => {
                self.pending.remove(&path);
                self.indexed += 1;
            }
            Err(error) => self.skip(
                path,
                blob,
                SkipReason::ParseFailed {
                    detail: error.to_string(),
                },
            ),
        }
    }

    /// Record a skip and stop wanting the path.
    fn skip(&mut self, path: RelativePath, blob: ObjectSha, reason: SkipReason) {
        self.pending.remove(&path);
        self.skipped.push(SkippedFile { path, blob, reason });
    }

    /// Coverage, ingest stats, eligible files, and the populated context.
    fn finish(mut self) -> (Coverage, IngestStats, Vec<SnapshotFile>, SymbolicContext) {
        self.skipped.sort_by(|a, b| a.path.cmp(&b.path));
        let mut excluded = BTreeMap::new();
        for exclusion in &self.excluded {
            *excluded.entry(exclusion.reason).or_insert(0) += 1;
        }
        if self.rejected > 0 {
            excluded.insert(ExclusionReason::InvalidEntry, self.rejected);
        }
        let coverage = Coverage {
            truncated_tree: false,
            eligible: self.files.len(),
            indexed: self.indexed,
            skipped: self.skipped,
            excluded,
        };
        (coverage, self.stats, self.files, self.context)
    }
}

#[cfg(test)]
/// Owner tests for the cold build: complete verified snapshots, archive
/// fallback, every incomplete outcome, and crash-safe publication, driven
/// through a routed fake GitHub and in-memory storage and persistence.
mod tests {
    use super::*;
    use crate::github::{Operation, RequestLimits, git_blob_sha};
    use crate::identity::{
        GitHubHost, InstallationId, InstallationToken, RepositoryIdentity, TenantId,
    };
    use crate::testing::{MemorySnapshotStore, MemoryStorage, RoutedTransport};
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::sync::Arc;
    use std::time::{Duration, UNIX_EPOCH};
    use tar::EntryType;

    /// Declares `lookupUser` and `saveUser`.
    const USER: &[u8] = b"export interface User {\n  id: string;\n}\n\nexport function lookupUser(id: string): User {\n  return { id };\n}\n\nexport function saveUser(user: User): void {\n  console.log(user.id);\n}\n";
    /// Declares `formatUser`.
    const FORMAT: &[u8] = b"import { User } from \"./user\";\n\nexport function formatUser(user: User): string {\n  return user.id;\n}\n";
    /// Declares `handleRequest`.
    const HANDLER: &[u8] = b"import { lookupUser } from \"./user\";\n\nexport function handleRequest(id: string): string {\n  return lookupUser(id).id;\n}\n";
    /// Declares `omittedFn`; never placed in the archive.
    const OMITTED: &[u8] = b"export function omittedFn(): number {\n  return 1;\n}\n";
    /// Declares `substitutedFn`; the archive carries other bytes for its path.
    const SUBSTITUTED: &[u8] = b"export function substitutedFn(): number {\n  return 2;\n}\n";
    /// A Git LFS pointer.
    const LFS: &[u8] = b"version https://git-lfs.github.com/spec/v1\noid sha256:0000\nsize 12\n";
    /// A parse-error file the extractor defers.
    const BROKEN: &[u8] = b"export function (\n";

    /// Forty copies of `ch` as an object id.
    fn sha(ch: char) -> ObjectSha {
        ObjectSha::new(&ch.to_string().repeat(40)).expect("sha")
    }

    /// Blob id of `bytes`.
    fn blob_sha(bytes: &[u8]) -> ObjectSha {
        ObjectSha::new(&git_blob_sha(bytes)).expect("blob sha")
    }

    /// The caller identity for the pinned test repository.
    fn identity() -> Identity {
        Identity {
            tenant: TenantId::new("acme").expect("tenant"),
            installation: InstallationId::new(42).expect("installation"),
            repository: RepositoryIdentity::new(1296269, "octocat", "Hello-World").expect("repo"),
        }
    }

    /// One tree entry of the fake repository.
    struct File {
        path: &'static str,
        mode: &'static str,
        content: &'static [u8],
        size: Option<u64>,
    }

    /// A regular file whose listed size is its content length.
    fn file(path: &'static str, content: &'static [u8]) -> File {
        File {
            path,
            mode: "100644",
            content,
            size: None,
        }
    }

    /// A fake repository at commit `c`/tree `7` with routed responses.
    struct FakeRepo {
        transport: Arc<RoutedTransport>,
        tree: ObjectSha,
        commit: ObjectSha,
    }

    impl FakeRepo {
        /// Route repository, branch, tree, every blob, and the tarball
        /// containing `archive` entries under the GitHub prefix.
        fn new(files: &[File], archive: &[(&str, &[u8])]) -> FakeRepo {
            let transport = Arc::new(RoutedTransport::default());
            let (tree, commit) = (sha('7'), sha('c'));
            transport.json(
                "/repos/octocat/Hello-World",
                r#"{"id":1296269,"full_name":"octocat/Hello-World","default_branch":"main","private":true}"#,
            );
            transport.json(
                "/branches/main",
                &format!(
                    r#"{{"name":"main","commit":{{"sha":"{commit}","commit":{{"tree":{{"sha":"{tree}"}}}}}}}}"#
                ),
            );
            let entries: Vec<String> = files
                .iter()
                .map(|f| {
                    let sha = blob_sha(f.content);
                    let kind = match f.mode {
                        "040000" => "tree",
                        "160000" => "commit",
                        _ => "blob",
                    };
                    let size = f.size.unwrap_or(f.content.len() as u64);
                    transport.route(&format!("/git/blobs/{sha}"), 200, f.content);
                    format!(
                        r#"{{"path":"{}","mode":"{}","type":"{kind}","sha":"{sha}","size":{size}}}"#,
                        f.path, f.mode
                    )
                })
                .collect();
            transport.json(
                &format!("/git/trees/{tree}?recursive=1"),
                &format!(
                    r#"{{"sha":"{tree}","truncated":false,"tree":[{}]}}"#,
                    entries.join(",")
                ),
            );
            transport.route(&format!("/tarball/{commit}"), 200, tarball(archive));
            FakeRepo {
                transport,
                tree,
                commit,
            }
        }

        /// Replace the tree route with a truncated listing.
        fn truncate_tree(&self) {
            let tree = &self.tree;
            self.transport.json(
                &format!("/git/trees/{tree}?recursive=1"),
                &format!(r#"{{"sha":"{tree}","truncated":true,"tree":[]}}"#),
            );
        }

        /// A client over this fake.
        fn client(&self) -> GitHubClient {
            GitHubClient::with_transport(
                GitHubHost::public(),
                InstallationToken::new("ghs_token").expect("token"),
                RequestLimits::default(),
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
    fn standard_files() -> Vec<File> {
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
    fn standard_archive() -> Vec<(&'static str, &'static [u8])> {
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
                &repo.tree
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
            result.ingest,
            IngestStats {
                archive_entries: 4,
                fallback_blobs: 2,
                archive_error: None,
            }
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
        for fragment in ["return { id }", "console.log", "return user.id", "stale"] {
            assert!(
                !text.contains(fragment),
                "source fragment {fragment:?} persisted"
            );
        }
        let snapshot = Snapshot::decode(&bytes, &identity, &repo.tree).expect("decode");
        assert_eq!(snapshot.coverage, *coverage);
        let paths: Vec<&str> = snapshot.files.iter().map(|f| f.path.as_str()).collect();
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
        let scope = SnapshotScope {
            identity: identity.clone(),
            extraction_version: EXTRACTION_VERSION.to_owned(),
            format_version: SNAPSHOT_FORMAT_VERSION,
        };
        assert_eq!(
            persistence
                .snapshot_pointer(&scope)
                .await
                .expect("pointer")
                .as_ref(),
            Some(pointer)
        );

        let loaded = Snapshots::new(&storage, &persistence)
            .read(&identity, pointer)
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
        assert_eq!(again.coverage, *coverage);
        assert_eq!(again.ingest, IngestStats::default());
        assert_eq!(calls.get(Operation::Archive), 1);
        assert_eq!(calls.get(Operation::Tree), 1);
        assert_eq!(storage.keys().len(), 1);
    }

    /// Scenario 2: a truncated tree, a parse failure, an exhausted fallback
    /// bound, and an exhausted expansion bound each yield an incomplete
    /// result with exact skips; nothing is written and a prior pointer for
    /// another tree stays in place. A bounded archive failure alone still
    /// completes through fallback.
    #[tokio::test]
    async fn incomplete_attempts_report_coverage_and_keep_the_prior_pointer() {
        let identity = identity();
        let scope = SnapshotScope {
            identity: identity.clone(),
            extraction_version: EXTRACTION_VERSION.to_owned(),
            format_version: SNAPSHOT_FORMAT_VERSION,
        };
        let prior = SnapshotPointer {
            tree: sha('0'),
            commit: sha('1'),
            key: ObjectKey::snapshot(
                &identity,
                EXTRACTION_VERSION,
                SNAPSHOT_FORMAT_VERSION,
                &sha('0'),
            ),
            digest: ObjectDigest::of(b""),
            published_at: SystemTime::UNIX_EPOCH,
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

        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        repo.truncate_tree();
        let (result, keys, pointer, _) = run(&repo, ColdBuildLimits::default()).await;
        assert_eq!(
            result.outcome,
            ColdBuildOutcome::Incomplete(IncompleteReason::TruncatedTree)
        );
        assert!(result.coverage.truncated_tree && !result.coverage.is_complete());
        assert!(keys.is_empty());
        assert_eq!(pointer.as_ref(), Some(&prior));

        let mut files = standard_files();
        files.push(file("src/broken.ts", BROKEN));
        let mut archive = standard_archive();
        archive.push(("src/broken.ts", BROKEN));
        let repo = FakeRepo::new(&files, &archive);
        let (result, keys, pointer, _) = run(&repo, ColdBuildLimits::default()).await;
        assert_eq!(
            result.outcome,
            ColdBuildOutcome::Incomplete(IncompleteReason::UnindexedFiles { count: 1 })
        );
        assert_eq!((result.coverage.eligible, result.coverage.indexed), (7, 5));
        let broken = result
            .coverage
            .skipped
            .iter()
            .find(|f| f.path.as_str() == "src/broken.ts")
            .expect("broken skipped");
        assert_eq!(broken.blob, blob_sha(BROKEN));
        assert!(
            matches!(&broken.reason, SkipReason::ParseFailed { detail } if detail.contains("deferred"))
        );
        assert!(keys.is_empty());
        assert_eq!(pointer.as_ref(), Some(&prior));

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
        assert!(matches!(result.outcome, ColdBuildOutcome::Published(_)));
        assert_eq!(keys.len(), 1);
        assert_eq!(pointer.map(|p| p.tree), Some(repo.tree.clone()));

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
            ColdBuildOutcome::Incomplete(IncompleteReason::UnindexedFiles { count: 6 })
        );
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
        let key = snapshot_key(&identity, &repo.tree);
        let prior = SnapshotPointer {
            tree: sha('0'),
            commit: sha('1'),
            key: snapshot_key(&identity, &sha('0')),
            digest: ObjectDigest::of(b""),
            published_at: UNIX_EPOCH,
        };
        let steps: [(fn(&MemoryStorage, &MemorySnapshotStore), bool); 3] = [
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
                        .read(&identity, &published)
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
        let key = snapshot_key(&identity, &repo.tree);
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
            .read(&identity, &pointer)
            .await
            .expect("read");
        let winner = SnapshotPointer {
            tree: sha('0'),
            key: snapshot_key(&identity, &sha('0')),
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
                .publish(&scope(&identity), Some(&pointer), &repo.commit, &snapshot)
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

    /// The scope of `identity` under the current versions.
    fn scope(identity: &Identity) -> SnapshotScope {
        SnapshotScope {
            identity: identity.clone(),
            extraction_version: EXTRACTION_VERSION.to_owned(),
            format_version: SNAPSHOT_FORMAT_VERSION,
        }
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
        let repo = FakeRepo::new(&standard_files(), &archive);
        let client = repo.client();
        let identity = identity();
        let publish = async |limits: ColdBuildLimits| {
            let storage = MemoryStorage::default();
            let store = MemorySnapshotStore::default();
            let result = ColdBuild::new(&client, &storage, &store, limits)
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

        let (streamed, streamed_bytes) = publish(ColdBuildLimits::default()).await;
        assert_eq!((streamed.archive_entries, streamed.fallback_blobs), (3, 3));
        let (fallback, fallback_bytes) = publish(ColdBuildLimits {
            max_expanded_archive_bytes: 64,
            ..ColdBuildLimits::default()
        })
        .await;
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
                    .read(intruder, &pointer)
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
                .read(&owner, &pointer)
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
        let head = snapshot_key(&owner, &repo.tree);
        let stale = snapshot_key(&owner, &sha('1'));
        let gone = snapshot_key(&owner, &sha('2'));
        let foreign = snapshot_key(&other, &sha('1'));
        for (identity, key) in [
            (&owner, &head),
            (&owner, &stale),
            (&owner, &gone),
            (&other, &foreign),
        ] {
            store
                .upsert_record(identity, key, UNIX_EPOCH)
                .await
                .expect("seed record");
        }
        storage.create(&stale, b"stale").await.expect("seed");
        storage.create(&foreign, b"foreign").await.expect("seed");

        let older_than = SystemTime::now();
        store.stall_before_swap();
        assert!(
            tokio::time::timeout(Duration::from_secs(60), build.run(&owner))
                .await
                .is_err()
        );
        storage.fail_next_delete();
        assert!(matches!(
            snapshots.collect(&owner, older_than).await,
            Err(ColdBuildError::Storage(StorageError::Backend { .. }))
        ));
        assert!(store.records().contains(&(owner.clone(), stale.clone())));
        assert_eq!(
            snapshots
                .collect(&owner, older_than)
                .await
                .expect("collect"),
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
        let later = SystemTime::now() + Duration::from_secs(3600);
        assert_eq!(snapshots.collect(&owner, later).await.expect("collect"), []);
        assert!(snapshots.read(&owner, &pointer).await.is_ok());

        storage.delete(&head).await.expect("lose object");
        let ColdBuildOutcome::Published(rebuilt) =
            build.run(&owner).await.expect("rebuild").outcome
        else {
            panic!("a missing pointed-to object is rebuilt");
        };
        assert_eq!(rebuilt.key, head);
        assert!(snapshots.read(&owner, &rebuilt).await.is_ok());

        assert_eq!(
            snapshots.remove_repository(&owner).await.expect("remove"),
            [head]
        );
        assert_eq!(
            store.snapshot_pointer(&scope(&owner)).await.expect("read"),
            None
        );
        assert_eq!(storage.keys(), [foreign.clone()]);
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
        let key = snapshot_key(&victim, &sha('1'));
        storage.create(&key, b"victim").await.expect("seed");
        store
            .upsert_record(&owner, &key, UNIX_EPOCH)
            .await
            .expect("forge record");
        assert_eq!(
            Snapshots::new(&storage, &store)
                .collect(&owner, SystemTime::now())
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
        assert_send(Snapshots::new(&storage, &store).collect(&identity, SystemTime::now()));
    }

    /// FIND-3: tree entries the listing rejected for an invalid path or an
    /// unknown mode are counted under their own exclusion reason instead of
    /// vanishing, and do not change eligibility or completeness.
    #[tokio::test]
    async fn rejected_tree_entries_are_counted_as_invalid_exclusions() {
        let files = [
            file("src/user.ts", USER),
            file("src/a\\\\b.ts", FORMAT),
            File {
                path: "src/odd.ts",
                mode: "100600",
                content: HANDLER,
                size: None,
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
        assert_eq!((result.coverage.eligible, result.coverage.indexed), (1, 1));
        assert_eq!(
            result.coverage.excluded,
            BTreeMap::from([(ExclusionReason::InvalidEntry, 2)])
        );
    }
}
