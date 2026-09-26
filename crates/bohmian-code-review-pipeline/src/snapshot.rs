//! Cold default-branch snapshot build, publication, read-back, and cleanup.
//!
//! [`ColdBuild::run`] pins the default branch, classifies its tree with
//! [`classify`], streams the commit tarball through bounded decompression,
//! verifies every eligible entry against its Git blob id, falls back to
//! bounded verified blob calls for anything the archive omitted or
//! substituted, and indexes through the public [`SymbolicContext`]. Source
//! bytes live only while one entry is being parsed. A [`Snapshot`] is
//! recorded, then written at its immutable key and compared byte for byte on
//! read-back before the pointer moves; an incomplete or interrupted attempt
//! writes nothing and returns exact coverage. [`Snapshots`] reads published
//! snapshots and deletes recorded snapshots no pointer names.

use crate::eligibility::{
    Exclusion, ExclusionReason, MAX_ELIGIBLE_FILE_BYTES, RootAttributes, classify,
};
use crate::execution::{Execution, Interrupted};
use crate::github::{
    Archive, BoundedReader, GitHubClient, GitHubError, TreeEntry, TreeEntryMode, git_blob_sha,
};
use crate::identity::{Identity, ObjectSha};
use crate::persistence::{PersistenceError, SnapshotPointer, SnapshotScope, SnapshotStore};
use crate::storage::{ObjectDigest, ObjectKey, ObjectStorage, StorageError};
use bohmian_code_review::Error as SymbolicError;
use bohmian_code_review::{EXTRACTION_VERSION, IndexedFileFacts, RelativePath, SymbolicContext};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::time::SystemTime;
use tar::EntryType;
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
    /// The archive omitted it and the fallback bound or deadline was reached.
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
pub struct Snapshots<'a> {
    storage: &'a dyn ObjectStorage,
    store: &'a dyn SnapshotStore,
}

impl<'a> Snapshots<'a> {
    /// Snapshot access over the caller's storage and snapshot store.
    pub fn new(storage: &'a dyn ObjectStorage, store: &'a dyn SnapshotStore) -> Snapshots<'a> {
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
    pub fn read(
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
        let bytes = self.storage.read(&expected)?;
        if ObjectDigest::of(&bytes) != pointer.digest {
            return Err(ColdBuildError::Corrupt {
                detail: "digest mismatch".to_owned(),
            });
        }
        Snapshot::decode(&bytes, identity, &pointer.tree)
    }

    /// Delete up to `limit` recorded snapshots older than `before` that no
    /// pointer names, returning the deleted keys.
    ///
    /// Each object is deleted before its record, so a failure part-way
    /// leaves a record the next call retries rather than an untracked
    /// object. A record whose object was never written deletes cleanly
    /// because deleting an absent object succeeds. Every returned key must
    /// be a snapshot key of the identity it was returned with, so a faulty
    /// store cannot steer deletion into another tenant, installation, or
    /// repository.
    ///
    /// ponytail: a build adopting an object between the store's listing and
    /// the delete can publish a pointer to a just-deleted object; the next
    /// cold build sees the object missing and rebuilds it. A store-side lease
    /// closes the window if that ever matters.
    ///
    /// # Errors
    ///
    /// Returns [`ColdBuildError::ForeignKey`] for a key outside its
    /// identity's snapshot namespace, otherwise the first persistence or
    /// storage failure; keys deleted before it stay deleted.
    pub fn collect(
        &self,
        before: SystemTime,
        limit: usize,
    ) -> Result<Vec<ObjectKey>, ColdBuildError> {
        let mut deleted = Vec::new();
        for (identity, key) in self.store.unreferenced_snapshots(before, limit)? {
            if !key.is_snapshot_of(&identity) {
                return Err(ColdBuildError::ForeignKey { key });
            }
            self.storage.delete(&key)?;
            self.store.delete_snapshot(&identity, &key)?;
            deleted.push(key);
        }
        Ok(deleted)
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
    /// The caller cancelled or the deadline passed during ingest.
    Interrupted(Interrupted),
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
    /// Another publication moved the pointer first; retry the build.
    #[error("snapshot pointer changed during the build")]
    PointerConflict,
}

/// The cold-build facade over injected provider, storage, and snapshot store.
pub struct ColdBuild<'a> {
    client: &'a GitHubClient,
    storage: &'a dyn ObjectStorage,
    store: &'a dyn SnapshotStore,
    limits: ColdBuildLimits,
}

impl<'a> ColdBuild<'a> {
    /// Facade over the caller's capabilities and ceilings.
    pub fn new(
        client: &'a GitHubClient,
        storage: &'a dyn ObjectStorage,
        store: &'a dyn SnapshotStore,
        limits: ColdBuildLimits,
    ) -> ColdBuild<'a> {
        ColdBuild {
            client,
            storage,
            store,
            limits,
        }
    }

    /// Build or reuse the complete default-branch snapshot for `identity`.
    ///
    /// The caller's `execution` bounds every provider call; the library
    /// imposes no maximum of its own. A current pointer at the branch tree
    /// is validated and reused; if its object has gone missing the snapshot
    /// is rebuilt. Cancellation and the deadline are checked before every
    /// archive entry and fallback blob and once more before publication, so
    /// an interrupted build never writes. Publication order is fixed: the
    /// key is recorded, the object is created (or an existing one adopted),
    /// read back and compared byte for byte, and only then is the pointer
    /// published conditionally on the pointer observed at the start. A retry
    /// after any interruption converges, and every object written is
    /// tracked for collection and namespace removal.
    ///
    /// # Errors
    ///
    /// Returns provider errors for the repository, branch, tree, and
    /// attributes calls, storage and persistence errors, `ForeignKey` or
    /// `Corrupt` for an unusable current pointer or existing object, and
    /// `PointerConflict` when the conditional publication lost. Per-file
    /// failures and an unusable archive never error; they appear in
    /// coverage and ingest stats.
    pub fn run(
        &self,
        execution: &Execution,
        identity: &Identity,
    ) -> Result<ColdBuildResult, ColdBuildError> {
        let scope = SnapshotScope {
            identity: identity.clone(),
            extraction_version: EXTRACTION_VERSION.to_owned(),
            format_version: SNAPSHOT_FORMAT_VERSION,
        };
        let current = self.store.snapshot_pointer(&scope)?;
        let repository = self.client.repository(execution, &identity.repository)?;
        let head =
            self.client
                .branch_head(execution, &identity.repository, &repository.default_branch)?;
        let result = |coverage, ingest, outcome| ColdBuildResult {
            commit: head.commit.clone(),
            tree: head.tree.clone(),
            coverage,
            ingest,
            outcome,
        };
        if let Some(pointer) = current.as_ref().filter(|pointer| pointer.tree == head.tree) {
            match Snapshots::new(self.storage, self.store).read(identity, pointer) {
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

        let listing = self
            .client
            .tree(execution, &identity.repository, &head.tree)?;
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
            Some(entry) => RootAttributes::parse(&self.client.blob(
                execution,
                &identity.repository,
                &entry.sha,
            )?),
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
            .archive(execution, &identity.repository, &head.commit)
        {
            Ok(archive) => {
                ingest.consume_archive(archive, execution, self.limits.max_expanded_archive_bytes)
            }
            Err(error) => ingest.stats.archive_error = Some(error.to_string()),
        }
        let interrupted = ingest
            .fallback(
                self.client,
                execution,
                identity,
                self.limits.max_fallback_blobs,
            )
            .or_else(|| execution.check().err());
        let (coverage, stats, files, context) = ingest.finish();
        if let Some(interrupted) = interrupted {
            return Ok(result(
                coverage,
                stats,
                ColdBuildOutcome::Incomplete(IncompleteReason::Interrupted(interrupted)),
            ));
        }
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
        let pointer = self.publish(&scope, current.as_ref(), &head.commit, &snapshot)?;
        Ok(result(
            coverage,
            stats,
            ColdBuildOutcome::Published(pointer),
        ))
    }

    /// Record `snapshot`'s key, write it there or adopt the object already
    /// there when its bytes are identical, then move the pointer.
    ///
    /// Recording first means a failure at any later step leaves a tracked
    /// key that collection or namespace removal deletes.
    ///
    /// # Errors
    ///
    /// Storage, persistence, `Corrupt`, and `PointerConflict` as in [`ColdBuild::run`].
    fn publish(
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
        let now = SystemTime::now();
        self.store.record_snapshot(identity, &key, now)?;
        match self.storage.create(&key, &bytes) {
            Ok(()) | Err(StorageError::AlreadyExists { .. }) => {}
            Err(error) => return Err(error.into()),
        }
        if self.storage.read(&key)? != bytes {
            return Err(ColdBuildError::Corrupt {
                detail: "stored object differs from the built snapshot".to_owned(),
            });
        }
        let pointer = SnapshotPointer {
            tree: snapshot.tree.clone(),
            commit: commit.clone(),
            key,
            digest: ObjectDigest::of(&bytes),
            published_at: now,
        };
        let expected = current.map(|pointer| &pointer.tree);
        if !self
            .store
            .publish_snapshot_pointer(scope, expected, &pointer)?
        {
            return Err(ColdBuildError::PointerConflict);
        }
        Ok(pointer)
    }
}

/// Reader that checks the caller's cancellation and deadline before every
/// read of `inner`, so no archive bytes are consumed once interrupted.
struct Interruptible<'a, R> {
    inner: R,
    execution: &'a Execution,
}

impl<R: Read> Read for Interruptible<'_, R> {
    /// Reads from `inner` while the execution may continue.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error::other`] wrapping the [`Interrupted`] reason once
    /// cancelled or past the deadline (never [`io::ErrorKind::Interrupted`],
    /// which readers retry), and propagates `inner`'s errors.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.execution.check().map_err(io::Error::other)?;
        self.inner.read(buf)
    }
}

/// In-memory ingest state for one attempt: eligible entries still wanted,
/// what was indexed and skipped, and the interruption that stopped it.
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
    interrupted: Option<Interrupted>,
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
            interrupted: None,
        }
    }

    /// Stream the tarball through bounded decompression; a stream error
    /// stops the archive and is recorded, and an interruption stops it and
    /// is kept for [`Ingest::fallback`], leaving the rest pending.
    ///
    /// Both the compressed body and the expanded stream are
    /// [`Interruptible`], so cancellation and the deadline are checked
    /// before every source read and every decompressed read, including tar
    /// headers, eligible entry bodies, and unselected entries the tar
    /// iterator drains. A read already blocked in the transport still ends
    /// at the transport's own timeout.
    fn consume_archive(
        &mut self,
        archive: Archive,
        execution: &Execution,
        max_expanded_bytes: u64,
    ) {
        let expanded = Interruptible {
            inner: BoundedReader::new(
                GzDecoder::new(Interruptible {
                    inner: archive.reader,
                    execution,
                }),
                max_expanded_bytes,
                "archive expansion",
            ),
            execution,
        };
        if let Err(error) = self.stream_entries(expanded) {
            // Cancellation and an elapsed deadline never revert, so a check
            // that fails now names the interruption that failed the read.
            match execution.check() {
                Err(interrupted) => self.interrupted = Some(interrupted),
                Ok(_) => self.stats.archive_error = Some(error.to_string()),
            }
        }
    }

    /// Index every pending tar entry whose bytes hash to the listed blob id.
    ///
    /// Non-regular entries, paths outside the repository prefix or not
    /// pending, entries larger than the file bound, and mismatched bytes are
    /// left pending for verified blob fallback, so an archive can never
    /// substitute content or turn an eligible file into a benign skip.
    ///
    /// # Errors
    ///
    /// Returns the first tar, decompression, or read error, including the
    /// expansion bound and an [`Interruptible`] interruption; entries
    /// indexed before it stay indexed.
    fn stream_entries<R: Read>(&mut self, expanded: R) -> io::Result<()> {
        let mut tar = tar::Archive::new(expanded);
        for entry in tar.entries()? {
            let mut entry = entry?;
            if !matches!(
                entry.header().entry_type(),
                EntryType::Regular | EntryType::Continuous
            ) {
                continue;
            }
            let Some(path) = entry
                .path()
                .ok()
                .and_then(|path| path.to_str().map(str::to_owned))
                .and_then(|raw| raw.split_once('/').map(|(_, rest)| rest.to_owned()))
                .and_then(|rest| RelativePath::new(&rest).ok())
            else {
                continue;
            };
            let Some(blob) = self.pending.get(&path).cloned() else {
                continue;
            };
            if entry.header().size()? > self.max_file_bytes {
                continue;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            if git_blob_sha(&bytes) != blob.as_str() {
                continue;
            }
            self.stats.archive_entries += 1;
            self.ingest(path, blob, &bytes);
        }
        Ok(())
    }

    /// Fetch every still-pending blob individually, up to `max_blobs`, and
    /// index it. Returns the interruption that stopped the archive or this
    /// loop, if any; everything still pending afterwards is skipped as not
    /// fetched.
    fn fallback(
        &mut self,
        client: &GitHubClient,
        execution: &Execution,
        identity: &Identity,
        max_blobs: usize,
    ) -> Option<Interrupted> {
        let mut interrupted = self.interrupted;
        while let Some((path, blob)) = self.pending.pop_first() {
            if interrupted.is_none() && self.stats.fallback_blobs < max_blobs {
                interrupted = execution.check().err();
            }
            if interrupted.is_some() || self.stats.fallback_blobs >= max_blobs {
                self.skip(path, blob, SkipReason::NotFetched);
                continue;
            }
            self.stats.fallback_blobs += 1;
            match client.blob(execution, &identity.repository, &blob) {
                Ok(bytes) if bytes.len() as u64 > self.max_file_bytes => {
                    self.skip(path, blob, SkipReason::Oversize);
                }
                Ok(bytes) => self.ingest(path, blob, &bytes),
                Err(GitHubError::Interrupted(reason)) => {
                    interrupted = Some(reason);
                    self.skip(path, blob, SkipReason::NotFetched);
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
        interrupted
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
    use crate::execution::CancelToken;
    use crate::github::{Operation, RequestLimits};
    use crate::identity::{
        GitHubHost, InstallationId, InstallationToken, RepositoryIdentity, TenantId,
    };
    use crate::testing::{MemorySnapshotStore, MemoryStorage, RoutedTransport};
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::time::{Duration, Instant, UNIX_EPOCH};

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
        ) -> Result<crate::github::HttpResponse, crate::github::TransportError> {
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

    /// A one-minute cold-build execution.
    fn execution() -> Execution {
        Execution::with_timeout(Duration::from_secs(60))
    }

    /// Scenario 1: the cold build verifies archive entries, falls back to
    /// blobs for omitted and substituted entries, skips the LFS pointer by
    /// content, writes a source-free object, publishes the pointer only
    /// after read-back, reproduces facts on reload without source calls, and
    /// reuses the pointer on a second invocation without another archive.
    #[test]
    fn complete_cold_snapshot_is_verified_source_free_and_reloadable() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let storage = MemoryStorage::default();
        let persistence = MemorySnapshotStore::default();
        let identity = identity();
        let build = ColdBuild::new(&client, &storage, &persistence, ColdBuildLimits::default());

        let result = build.run(&execution(), &identity).expect("cold build");
        let review = Execution::with_timeout(Duration::from_secs(30 * 60));
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
        let bytes = storage.read(&pointer.key).expect("object");
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
                .expect("pointer")
                .as_ref(),
            Some(pointer)
        );

        let loaded = Snapshots::new(&storage, &persistence)
            .read(&identity, pointer)
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

        let again = build.run(&execution(), &identity).expect("second build");
        assert_eq!(again.outcome, ColdBuildOutcome::Reused(pointer.clone()));
        assert_eq!(again.coverage, *coverage);
        assert_eq!(again.ingest, IngestStats::default());
        assert_eq!(calls.get(Operation::Archive), 1);
        assert_eq!(calls.get(Operation::Tree), 1);
        assert_eq!(storage.keys().len(), 1);
        assert!(review.remaining().expect("review clock") > Duration::from_secs(29 * 60));
    }

    /// Scenario 2: a truncated tree, a parse failure, an exhausted fallback
    /// bound, and an exhausted expansion bound each yield an incomplete
    /// result with exact skips; nothing is written and a prior pointer for
    /// another tree stays in place. A bounded archive failure alone still
    /// completes through fallback.
    #[test]
    fn incomplete_attempts_report_coverage_and_keep_the_prior_pointer() {
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
        let run = |repo: &FakeRepo, limits: ColdBuildLimits| {
            let client = repo.client();
            let storage = MemoryStorage::default();
            let persistence = MemorySnapshotStore::default();
            assert!(
                persistence
                    .publish_snapshot_pointer(&scope, None, &prior)
                    .expect("seed")
            );
            let result = ColdBuild::new(&client, &storage, &persistence, limits)
                .run(&execution(), &identity)
                .expect("build result");
            let pointer = persistence.snapshot_pointer(&scope).expect("pointer");
            (
                result,
                storage.keys(),
                pointer,
                client.calls().get(Operation::Blob),
            )
        };

        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        repo.truncate_tree();
        let (result, keys, pointer, _) = run(&repo, ColdBuildLimits::default());
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
        let (result, keys, pointer, _) = run(&repo, ColdBuildLimits::default());
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
        );
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
        );
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
        );
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

    /// Cancellation once the archive request starts stops ingest before any
    /// archive entry or fallback blob, marks every eligible file as not
    /// fetched, and reports the interruption; the library imposes no
    /// deadline maximum, so a day-long caller deadline then publishes.
    #[test]
    fn interruption_is_explicit_and_deadlines_are_caller_owned() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let storage = MemoryStorage::default();
        let persistence = MemorySnapshotStore::default();
        let identity = identity();
        let build = ColdBuild::new(&client, &storage, &persistence, ColdBuildLimits::default());

        let cancel = CancelToken::new();
        let execution = Execution::new(Instant::now() + Duration::from_secs(60), cancel.clone());
        let hook_cancel = cancel.clone();
        repo.transport.on_request(move |url| {
            if url.contains("/tarball/") {
                hook_cancel.cancel();
            }
        });
        let result = build.run(&execution, &identity).expect("result");
        assert_eq!(
            result.outcome,
            ColdBuildOutcome::Incomplete(IncompleteReason::Interrupted(Interrupted::Cancelled))
        );
        assert_eq!(result.ingest.archive_entries, 0);
        assert_eq!(result.ingest.fallback_blobs, 0);
        assert_eq!(result.coverage.indexed, 0);
        assert_eq!(result.coverage.skipped.len(), 6);
        assert!(
            result
                .coverage
                .skipped
                .iter()
                .all(|f| f.reason == SkipReason::NotFetched)
        );
        assert!(storage.keys().is_empty());

        let long = Execution::with_timeout(Duration::from_secs(24 * 60 * 60));
        let result = build.run(&long, &identity).expect("long deadline");
        assert!(matches!(result.outcome, ColdBuildOutcome::Published(_)));
    }

    /// S17: a storage failure before the object exists surfaces and leaves
    /// no pointer; a persistence failure after the object was written leaves
    /// the object without a pointer, and the retry adopts the validated
    /// existing object and publishes. A corrupt object at the key is refused
    /// rather than adopted, and a tampered digest fails read-back.
    #[test]
    fn publication_is_crash_safe_and_retry_converges() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let storage = MemoryStorage::default();
        let persistence = MemorySnapshotStore::default();
        let identity = identity();
        let build = ColdBuild::new(&client, &storage, &persistence, ColdBuildLimits::default());
        let scope = SnapshotScope {
            identity: identity.clone(),
            extraction_version: EXTRACTION_VERSION.to_owned(),
            format_version: SNAPSHOT_FORMAT_VERSION,
        };

        storage.fail_next_create();
        assert!(matches!(
            build.run(&execution(), &identity),
            Err(ColdBuildError::Storage(StorageError::Backend { .. }))
        ));
        assert!(storage.keys().is_empty());
        assert_eq!(persistence.snapshot_pointer(&scope).expect("pointer"), None);

        persistence.fail_next_publish();
        assert!(matches!(
            build.run(&execution(), &identity),
            Err(ColdBuildError::Persistence(
                PersistenceError::Backend { .. }
            ))
        ));
        assert_eq!(storage.keys().len(), 1);
        assert_eq!(persistence.snapshot_pointer(&scope).expect("pointer"), None);

        let archives = client.calls().get(Operation::Archive);
        let result = build.run(&execution(), &identity).expect("retry");
        let ColdBuildOutcome::Published(pointer) = result.outcome else {
            panic!("retry must publish");
        };
        assert_eq!(
            client.calls().get(Operation::Archive),
            archives + 1,
            "whole-build retry"
        );
        assert_eq!(storage.keys(), std::slice::from_ref(&pointer.key));
        assert_eq!(
            persistence
                .snapshot_pointer(&scope)
                .expect("pointer")
                .as_ref(),
            Some(&pointer)
        );

        let mut tampered = pointer.clone();
        tampered.digest = ObjectDigest::of(b"other");
        assert!(matches!(
            Snapshots::new(&storage, &persistence).read(&identity, &tampered),
            Err(ColdBuildError::Corrupt { detail }) if detail == "digest mismatch"
        ));

        let corrupt_storage = MemoryStorage::default();
        corrupt_storage
            .create(&pointer.key, b"{\"not\":\"a snapshot\"}")
            .expect("seed corrupt object");
        let corrupt_persistence = MemorySnapshotStore::default();
        let build = ColdBuild::new(
            &client,
            &corrupt_storage,
            &corrupt_persistence,
            ColdBuildLimits::default(),
        );
        assert!(matches!(
            build.run(&execution(), &identity),
            Err(ColdBuildError::Corrupt { .. })
        ));
        assert_eq!(
            corrupt_persistence
                .snapshot_pointer(&scope)
                .expect("pointer"),
            None
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
    #[test]
    fn same_tree_serializes_identically_across_ingest_paths() {
        let big: &'static [u8] =
            Box::leak(vec![b'x'; MAX_ELIGIBLE_FILE_BYTES as usize + 1].into_boxed_slice());
        let mut archive = standard_archive();
        archive.retain(|(path, _)| *path != "src/format.ts");
        archive.push(("src/format.ts", big));
        let repo = FakeRepo::new(&standard_files(), &archive);
        let client = repo.client();
        let identity = identity();
        let publish = |limits: ColdBuildLimits| {
            let storage = MemoryStorage::default();
            let store = MemorySnapshotStore::default();
            let result = ColdBuild::new(&client, &storage, &store, limits)
                .run(&execution(), &identity)
                .expect("build");
            let ColdBuildOutcome::Published(pointer) = &result.outcome else {
                panic!("expected publication, got {:?}", result.outcome);
            };
            (result.ingest, storage.read(&pointer.key).expect("object"))
        };

        let (streamed, streamed_bytes) = publish(ColdBuildLimits::default());
        assert_eq!((streamed.archive_entries, streamed.fallback_blobs), (3, 3));
        let (fallback, fallback_bytes) = publish(ColdBuildLimits {
            max_expanded_archive_bytes: 64,
            ..ColdBuildLimits::default()
        });
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
    #[test]
    fn identities_cannot_read_or_advance_each_others_snapshots() {
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

        let result = build.run(&execution(), &owner).expect("owner build");
        let ColdBuildOutcome::Published(pointer) = result.outcome else {
            panic!("owner must publish");
        };
        for intruder in [&other_tenant, &other_installation] {
            assert_eq!(
                store.snapshot_pointer(&scope(intruder)).expect("read"),
                None
            );
            assert_eq!(
                Snapshots::new(&storage, &store).read(intruder, &pointer),
                Err(ColdBuildError::ForeignKey {
                    key: pointer.key.clone()
                })
            );

            let own = build.run(&execution(), intruder).expect("intruder build");
            let ColdBuildOutcome::Published(own) = own.outcome else {
                panic!("intruder builds its own snapshot");
            };
            assert_ne!(own.key, pointer.key);
            assert_eq!(
                store.snapshot_pointer(&scope(&owner)).expect("read"),
                Some(pointer.clone())
            );
        }
        assert_eq!(storage.keys().len(), 3);
        assert!(
            Snapshots::new(&storage, &store)
                .read(&owner, &pointer)
                .is_ok()
        );
    }

    /// REQ-006 cleanup and retry: an object left without a pointer by an
    /// interrupted publication is recorded, so collection removes it once
    /// expired and never touches a pointed-to snapshot; a pointer whose
    /// object went missing is rebuilt; namespace removal returns every
    /// recorded key of that identity only.
    #[test]
    fn collection_removes_only_expired_unreferenced_snapshots() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let build = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default());
        let owner = identity();
        let orphaned = Identity {
            tenant: TenantId::new("orphaned").expect("tenant"),
            ..owner.clone()
        };
        let ColdBuildOutcome::Published(pointer) =
            build.run(&execution(), &owner).expect("owner").outcome
        else {
            panic!("owner must publish");
        };
        store.fail_next_publish();
        assert!(build.run(&execution(), &orphaned).is_err());
        let orphan = ObjectKey::snapshot(
            &orphaned,
            EXTRACTION_VERSION,
            SNAPSHOT_FORMAT_VERSION,
            &repo.tree,
        );
        assert_eq!(storage.keys().len(), 2);

        assert_eq!(
            Snapshots::new(&storage, &store)
                .collect(UNIX_EPOCH, 10)
                .expect("nothing expired"),
            []
        );
        let later = SystemTime::now() + Duration::from_secs(3600);
        assert_eq!(
            Snapshots::new(&storage, &store)
                .collect(later, 10)
                .expect("collect"),
            [orphan]
        );
        assert_eq!(storage.keys(), std::slice::from_ref(&pointer.key));
        assert_eq!(
            Snapshots::new(&storage, &store)
                .collect(later, 10)
                .expect("idempotent"),
            []
        );
        assert!(
            Snapshots::new(&storage, &store)
                .read(&owner, &pointer)
                .is_ok()
        );

        let faulty = MemorySnapshotStore::default();
        faulty
            .record_snapshot(&orphaned, &pointer.key, UNIX_EPOCH)
            .expect("forge record");
        assert_eq!(
            Snapshots::new(&storage, &faulty).collect(later, 10),
            Err(ColdBuildError::ForeignKey {
                key: pointer.key.clone()
            })
        );
        assert!(
            Snapshots::new(&storage, &store)
                .read(&owner, &pointer)
                .is_ok()
        );

        storage.delete(&pointer.key).expect("lose object");
        let rebuilt = build.run(&execution(), &owner).expect("rebuild");
        let ColdBuildOutcome::Published(rebuilt) = rebuilt.outcome else {
            panic!("a missing pointed-to object is rebuilt");
        };
        assert_eq!(rebuilt.key, pointer.key);
        assert!(
            Snapshots::new(&storage, &store)
                .read(&owner, &rebuilt)
                .is_ok()
        );

        assert!(matches!(
            build.run(&execution(), &orphaned).expect("retry").outcome,
            ColdBuildOutcome::Published(_)
        ));
        assert_eq!(
            store.remove_repository(&owner).expect("remove"),
            [pointer.key]
        );
        assert_eq!(store.snapshot_pointer(&scope(&owner)).expect("read"), None);
        assert!(
            store
                .snapshot_pointer(&scope(&orphaned))
                .expect("read")
                .is_some()
        );
    }

    /// FIND-2: when the archive supplies every eligible file, a cancellation
    /// once the archive request starts still stops the build before
    /// publication: the outcome reports the interruption, nothing is
    /// written, and the prior pointer stays.
    #[test]
    fn interruption_during_a_complete_archive_never_publishes() {
        let files = [file("src/user.ts", USER), file("src/format.ts", FORMAT)];
        let archive = [("src/user.ts", USER), ("src/format.ts", FORMAT)];
        let identity = identity();
        let prior = SnapshotPointer {
            tree: sha('0'),
            commit: sha('1'),
            key: snapshot_key(&identity, &sha('0')),
            digest: ObjectDigest::of(b""),
            published_at: UNIX_EPOCH,
        };
        let run = |execution: &Execution, hook: Box<dyn Fn() + Send + Sync>| {
            let repo = FakeRepo::new(&files, &archive);
            repo.transport.on_request(move |url| {
                if url.contains("/tarball/") {
                    hook();
                }
            });
            let client = repo.client();
            let storage = MemoryStorage::default();
            let store = MemorySnapshotStore::default();
            assert!(
                store
                    .publish_snapshot_pointer(&scope(&identity), None, &prior)
                    .expect("seed")
            );
            let result = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default())
                .run(execution, &identity)
                .expect("result");
            assert!(storage.keys().is_empty(), "no object written");
            assert_eq!(
                store.snapshot_pointer(&scope(&identity)).expect("pointer"),
                Some(prior.clone())
            );
            result
        };

        let cancel = CancelToken::new();
        let cancelled = Execution::new(Instant::now() + Duration::from_secs(60), cancel.clone());
        let result = run(&cancelled, Box::new(move || cancel.cancel()));
        assert_eq!(
            result.outcome,
            ColdBuildOutcome::Incomplete(IncompleteReason::Interrupted(Interrupted::Cancelled))
        );
        assert_eq!(result.ingest.archive_entries, 0);
    }

    /// Transport over a shared fake that serves the tarball body in
    /// `CHUNK`-byte reads, trips `cancel` once `cancel_at` body bytes were
    /// read, and counts every body read that starts after that.
    struct Streamed {
        transport: Arc<RoutedTransport>,
        cancel: CancelToken,
        cancel_at: usize,
        reads_after_cancel: Arc<AtomicUsize>,
    }

    impl crate::github::HttpTransport for Streamed {
        /// Delegates to the shared fake and wraps a tarball body.
        ///
        /// # Errors
        ///
        /// Never.
        fn get(
            &self,
            request: crate::github::HttpRequest,
        ) -> Result<crate::github::HttpResponse, crate::github::TransportError> {
            let tarball = request.url.contains("/tarball/");
            let mut response = self.transport.get(request)?;
            if tarball {
                response.body = Box::new(Chunked {
                    inner: response.body,
                    read: 0,
                    cancel: self.cancel.clone(),
                    cancel_at: self.cancel_at,
                    reads_after_cancel: Arc::clone(&self.reads_after_cancel),
                });
            }
            Ok(response)
        }
    }

    /// Body bytes per read of a [`Chunked`] body.
    const CHUNK: usize = 256;

    /// Streaming body behind [`Streamed`].
    struct Chunked {
        inner: Box<dyn Read + Send>,
        read: usize,
        cancel: CancelToken,
        cancel_at: usize,
        reads_after_cancel: Arc<AtomicUsize>,
    }

    impl Read for Chunked {
        /// Reads at most [`CHUNK`] bytes, counting reads after cancellation
        /// and cancelling once `cancel_at` bytes were delivered.
        ///
        /// # Errors
        ///
        /// Propagates the inner reader's error.
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.cancel.is_cancelled() {
                self.reads_after_cancel.fetch_add(1, AtomicOrdering::SeqCst);
            }
            let want = buf.len().min(CHUNK);
            let read = self.inner.read(&mut buf[..want])?;
            self.read += read;
            if self.read >= self.cancel_at {
                self.cancel.cancel();
            }
            Ok(read)
        }
    }

    /// Roughly `len` bytes of valid, poorly compressible TypeScript, so its
    /// gzipped archive spans many body chunks.
    fn noise(len: usize) -> Vec<u8> {
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut source = String::new();
        let mut index = 0;
        while source.len() < len {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            source.push_str(&format!("export const v{index} = \"{state:016x}\";\n"));
            index += 1;
        }
        source.into_bytes()
    }

    /// FIND-2: cancellation after the first body chunks of a large archive
    /// entry stops source reads at the archive read chain, whether the
    /// entry is eligible (read in full) or unselected (drained by the tar
    /// iterator): no later body chunk is read, the build reports the
    /// cancellation, every eligible file is not fetched, nothing is
    /// written, and the prior pointer stays.
    #[test]
    fn cancellation_mid_archive_entry_stops_reading_the_body() {
        let big: &'static [u8] = Box::leak(noise(96 * 1024).into_boxed_slice());
        let identity = identity();
        let prior = SnapshotPointer {
            tree: sha('0'),
            commit: sha('1'),
            key: snapshot_key(&identity, &sha('0')),
            digest: ObjectDigest::of(b""),
            published_at: UNIX_EPOCH,
        };
        let eligible_entry = (vec![file("src/big.ts", big)], vec![("src/big.ts", big)], 1);
        let unselected_entry = (
            vec![file("src/user.ts", USER)],
            vec![("vendor/big.ts", big), ("src/user.ts", USER)],
            1,
        );
        for (files, archive, eligible) in [eligible_entry, unselected_entry] {
            let repo = FakeRepo::new(&files, &archive);
            let cancel = CancelToken::new();
            let reads_after_cancel = Arc::new(AtomicUsize::new(0));
            let client = GitHubClient::with_transport(
                GitHubHost::public(),
                InstallationToken::new("ghs_token").expect("token"),
                RequestLimits::default(),
                Box::new(Streamed {
                    transport: Arc::clone(&repo.transport),
                    cancel: cancel.clone(),
                    cancel_at: 16 * CHUNK,
                    reads_after_cancel: Arc::clone(&reads_after_cancel),
                }),
            );
            let storage = MemoryStorage::default();
            let store = MemorySnapshotStore::default();
            assert!(
                store
                    .publish_snapshot_pointer(&scope(&identity), None, &prior)
                    .expect("seed")
            );
            let execution = Execution::new(Instant::now() + Duration::from_secs(60), cancel);
            let result = ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default())
                .run(&execution, &identity)
                .expect("result");

            assert_eq!(reads_after_cancel.load(AtomicOrdering::SeqCst), 0);
            assert_eq!(
                result.outcome,
                ColdBuildOutcome::Incomplete(IncompleteReason::Interrupted(Interrupted::Cancelled))
            );
            assert_eq!(result.ingest.archive_entries, 0);
            assert_eq!(result.ingest.archive_error, None);
            assert_eq!(result.ingest.fallback_blobs, 0);
            assert_eq!(result.coverage.skipped.len(), eligible);
            assert!(
                result
                    .coverage
                    .skipped
                    .iter()
                    .all(|f| f.reason == SkipReason::NotFetched)
            );
            assert!(storage.keys().is_empty(), "no object written");
            assert_eq!(
                store.snapshot_pointer(&scope(&identity)).expect("pointer"),
                Some(prior.clone())
            );
        }
    }

    /// FIND-2: the archive ingest owner recognizes an already-expired
    /// deadline at its read chain without waiting for wall time: no entry
    /// is indexed, no archive error is recorded, and the fallback reports
    /// the deadline and fetches nothing, which [`ColdBuild::run`] turns into
    /// an unpublished interrupted outcome.
    #[test]
    fn expired_deadline_stops_archive_ingest() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let identity = identity();
        let listing = client
            .tree(&execution(), &identity.repository, &repo.tree)
            .expect("tree");
        let mut ingest = Ingest::new(
            &listing.entries,
            0,
            &RootAttributes::default(),
            MAX_ELIGIBLE_FILE_BYTES,
        );
        let expired = Execution::new(Instant::now(), CancelToken::new());
        let archive = Archive {
            reader: Box::new(io::Cursor::new(tarball(&standard_archive()))),
        };
        ingest.consume_archive(archive, &expired, u64::MAX);

        assert_eq!(ingest.interrupted, Some(Interrupted::DeadlineExceeded));
        assert_eq!(ingest.stats.archive_entries, 0);
        assert_eq!(ingest.stats.archive_error, None);
        assert_eq!(
            ingest.fallback(&client, &expired, &identity, 10),
            Some(Interrupted::DeadlineExceeded)
        );
        assert_eq!(ingest.stats.fallback_blobs, 0);
        assert_eq!(ingest.indexed, 0);
        assert!(ingest.pending.is_empty());
        assert!(
            ingest
                .skipped
                .iter()
                .all(|f| f.reason == SkipReason::NotFetched)
        );
    }

    /// FIND-1: the key is recorded before the object is written, so after an
    /// injected create, read-back, or pointer-publish failure the snapshot
    /// is tracked: expired collection leaves storage empty, and namespace
    /// removal returns the key for deletion.
    #[test]
    fn every_interrupted_publication_leaves_a_tracked_key() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let identity = identity();
        let key = snapshot_key(&identity, &repo.tree);
        let failures: [fn(&MemoryStorage, &MemorySnapshotStore); 3] = [
            |storage, _| storage.fail_next_create(),
            |storage, _| storage.fail_next_read(),
            |_, store| store.fail_next_publish(),
        ];
        let later = SystemTime::now() + Duration::from_secs(3600);
        for inject in failures {
            let interrupted = || {
                let storage = MemoryStorage::default();
                let store = MemorySnapshotStore::default();
                inject(&storage, &store);
                assert!(
                    ColdBuild::new(&client, &storage, &store, ColdBuildLimits::default())
                        .run(&execution(), &identity)
                        .is_err()
                );
                assert_eq!(
                    store.snapshot_pointer(&scope(&identity)).expect("read"),
                    None
                );
                (storage, store)
            };

            let (storage, store) = interrupted();
            Snapshots::new(&storage, &store)
                .collect(later, 10)
                .expect("collect");
            assert!(storage.keys().is_empty());

            let (_storage, store) = interrupted();
            assert_eq!(
                store.remove_repository(&identity).expect("remove"),
                std::slice::from_ref(&key)
            );
        }
    }

    /// FIND-3: tree entries the listing rejected for an invalid path or an
    /// unknown mode are counted under their own exclusion reason instead of
    /// vanishing, and do not change eligibility or completeness.
    #[test]
    fn rejected_tree_entries_are_counted_as_invalid_exclusions() {
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
            .run(&execution(), &identity())
            .expect("build");
        assert!(matches!(result.outcome, ColdBuildOutcome::Published(_)));
        assert_eq!((result.coverage.eligible, result.coverage.indexed), (1, 1));
        assert_eq!(
            result.coverage.excluded,
            BTreeMap::from([(ExclusionReason::InvalidEntry, 2)])
        );
    }
}
