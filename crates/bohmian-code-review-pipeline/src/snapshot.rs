//! Cold default-branch snapshot build, publication, and read-back.
//!
//! [`ColdBuild::run`] pins the default branch, classifies its tree with
//! [`classify`], streams the commit tarball through bounded decompression,
//! verifies every eligible entry against its Git blob id, falls back to
//! bounded verified blob calls for anything the archive omitted or
//! substituted, and indexes through the public [`SymbolicContext`]. Source
//! bytes live only while one entry is being parsed. A [`Snapshot`] is
//! written at its immutable key and read back before the pointer moves;
//! an incomplete attempt writes nothing and returns exact coverage.

use crate::eligibility::{
    Exclusion, ExclusionReason, MAX_ELIGIBLE_FILE_BYTES, RootAttributes, classify,
};
use crate::execution::{Execution, Interrupted};
use crate::github::{
    Archive, BoundedReader, GitHubClient, GitHubError, TreeEntry, TreeEntryMode, git_blob_sha,
};
use crate::identity::{Identity, ObjectSha};
use crate::persistence::{Persistence, PersistenceError, SnapshotPointer, SnapshotScope};
use crate::storage::{ObjectDigest, ObjectKey, ObjectStorage, StorageError};
use bohmian_code_review::{EXTRACTION_VERSION, IndexedFileFacts, RelativePath, SymbolicContext};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::time::{Duration, SystemTime};
use tar::EntryType;
use thiserror::Error;

/// Layout version of the serialized [`Snapshot`]; part of every snapshot key.
pub const SNAPSHOT_FORMAT_VERSION: u32 = 1;

/// Longest deadline a cold build accepts.
pub const MAX_COLD_BUILD_DEADLINE: Duration = Duration::from_secs(15 * 60);

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
    /// Whether a complete snapshot may still carry this skip.
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

/// What one build attempt covered, distinguishing excluded paths from
/// eligible files that were not indexed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Coverage {
    /// The provider truncated the recursive listing; nothing was indexed.
    pub truncated_tree: bool,
    /// Eligible files in the listed tree.
    pub eligible: usize,
    /// Eligible files whose facts are in the index.
    pub indexed: usize,
    /// Eligible files not indexed, with reasons.
    pub skipped: Vec<SkippedFile>,
    /// Excluded entries counted by reason.
    pub excluded: BTreeMap<ExclusionReason, usize>,
    /// Entries verified from the archive stream.
    pub archive_entries: usize,
    /// Blobs fetched individually after the archive.
    pub fallback_blobs: usize,
    /// Why the archive stream stopped early, if it did; never source text.
    pub archive_error: Option<String>,
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
/// Creation time and branch commit live in the [`SnapshotPointer`], not
/// here, so the same tree always serializes to the same bytes.
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
    /// Coverage of the attempt that produced this object; always complete.
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
    pub fn decode(bytes: &[u8], identity: &Identity, tree: &ObjectSha) -> Result<Snapshot, ColdBuildError> {
        let corrupt = |detail: String| ColdBuildError::Corrupt { detail };
        let snapshot: Snapshot =
            serde_json::from_slice(bytes).map_err(|error| corrupt(error.to_string()))?;
        if snapshot.format_version != SNAPSHOT_FORMAT_VERSION {
            return Err(corrupt(format!("format version {}", snapshot.format_version)));
        }
        if snapshot.extraction_version != EXTRACTION_VERSION {
            return Err(corrupt(format!("extraction version {}", snapshot.extraction_version)));
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
    /// Returns the facade's error for a fact stamped by another extractor
    /// or violating a declaration invariant.
    pub fn into_context(self) -> Result<SymbolicContext, bohmian_code_review::Error> {
        let mut context = SymbolicContext::new();
        for facts in self.facts {
            context.load(facts)?;
        }
        Ok(context)
    }
}

/// Read and validate the snapshot a pointer names.
///
/// # Errors
///
/// Returns [`ColdBuildError::Storage`] when the object is missing or the
/// backend fails and [`ColdBuildError::Corrupt`] when its bytes do not match
/// the pointer's digest or fail [`Snapshot::decode`].
pub fn read_snapshot(
    storage: &dyn ObjectStorage,
    identity: &Identity,
    pointer: &SnapshotPointer,
) -> Result<Snapshot, ColdBuildError> {
    let bytes = storage.read(&pointer.key)?;
    if ObjectDigest::of(&bytes) != pointer.digest {
        return Err(ColdBuildError::Corrupt {
            detail: "digest mismatch".to_owned(),
        });
    }
    Snapshot::decode(&bytes, identity, &pointer.tree)
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

/// Result of one cold build invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColdBuildResult {
    /// Default-branch commit that was pinned.
    pub commit: ObjectSha,
    /// Its root tree.
    pub tree: ObjectSha,
    /// Exact coverage of this attempt, or of the reused snapshot.
    pub coverage: Coverage,
    /// Publication outcome.
    pub outcome: ColdBuildOutcome,
}

/// Why a cold build could not produce a result at all.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ColdBuildError {
    /// The execution allows more than [`MAX_COLD_BUILD_DEADLINE`].
    #[error("cold build deadline exceeds {max:?}")]
    DeadlineTooLong {
        /// The permitted maximum.
        max: Duration,
    },
    /// A repository-level provider call failed.
    #[error(transparent)]
    GitHub(#[from] GitHubError),
    /// Object storage failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// Metadata persistence failed.
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
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

/// The cold-build facade over injected provider, storage, and persistence.
pub struct ColdBuild<'a> {
    client: &'a GitHubClient,
    storage: &'a dyn ObjectStorage,
    persistence: &'a dyn Persistence,
    limits: ColdBuildLimits,
}

impl<'a> ColdBuild<'a> {
    /// Facade over the caller's capabilities and ceilings.
    pub fn new(
        client: &'a GitHubClient,
        storage: &'a dyn ObjectStorage,
        persistence: &'a dyn Persistence,
        limits: ColdBuildLimits,
    ) -> ColdBuild<'a> {
        ColdBuild {
            client,
            storage,
            persistence,
            limits,
        }
    }

    /// Build or reuse the complete default-branch snapshot for `identity`.
    ///
    /// Publication order is fixed: the object is created, read back, and
    /// validated before the pointer is published conditionally on the
    /// pointer observed at the start. An existing object at the key is
    /// validated and adopted so a retry after an interrupted publication
    /// converges.
    ///
    /// # Errors
    ///
    /// Returns [`ColdBuildError::DeadlineTooLong`] before any call,
    /// provider errors for the repository, branch, tree, attributes, and
    /// archive-open calls, storage and persistence errors, `Corrupt` for an
    /// unusable existing object, and `PointerConflict` when the conditional
    /// publication lost. Per-file failures never error; they appear in
    /// coverage.
    pub fn run(
        &self,
        execution: &Execution,
        identity: &Identity,
    ) -> Result<ColdBuildResult, ColdBuildError> {
        if execution.remaining().is_some_and(|left| left > MAX_COLD_BUILD_DEADLINE) {
            return Err(ColdBuildError::DeadlineTooLong {
                max: MAX_COLD_BUILD_DEADLINE,
            });
        }
        let scope = SnapshotScope {
            identity: identity.clone(),
            extraction_version: EXTRACTION_VERSION.to_owned(),
            format_version: SNAPSHOT_FORMAT_VERSION,
        };
        let current = self.persistence.snapshot_pointer(&scope)?;
        let repository = self.client.repository(execution, &identity.repository)?;
        let head =
            self.client
                .branch_head(execution, &identity.repository, &repository.default_branch)?;
        if let Some(pointer) = current.as_ref().filter(|pointer| pointer.tree == head.tree) {
            let snapshot = read_snapshot(self.storage, identity, pointer)?;
            return Ok(ColdBuildResult {
                commit: head.commit,
                tree: head.tree,
                coverage: snapshot.coverage,
                outcome: ColdBuildOutcome::Reused(pointer.clone()),
            });
        }
        let result = |coverage, outcome| ColdBuildResult {
            commit: head.commit.clone(),
            tree: head.tree.clone(),
            coverage,
            outcome,
        };

        let listing = self.client.tree(execution, &identity.repository, &head.tree)?;
        if listing.truncated {
            let coverage = Coverage {
                truncated_tree: true,
                ..Coverage::default()
            };
            return Ok(result(
                coverage,
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
        let mut ingest = Ingest::new(&listing.entries, &attributes, self.limits.max_file_bytes);
        match self
            .client
            .archive(execution, &identity.repository, &head.commit)
        {
            Ok(archive) => ingest.consume_archive(archive, self.limits.max_expanded_archive_bytes),
            Err(error) => ingest.archive_error = Some(error.to_string()),
        }
        let interrupted = ingest.fallback(self.client, execution, identity, self.limits.max_fallback_blobs);
        let (coverage, files, context) = ingest.finish();
        if let Some(interrupted) = interrupted {
            return Ok(result(
                coverage,
                ColdBuildOutcome::Incomplete(IncompleteReason::Interrupted(interrupted)),
            ));
        }
        if !coverage.is_complete() {
            let count = coverage.unindexed();
            return Ok(result(
                coverage,
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
        let pointer = self.publish(&scope, current.as_ref(), &head.commit, snapshot)?;
        Ok(result(coverage, ColdBuildOutcome::Published(pointer)))
    }

    /// Write `snapshot` at its key, adopt a valid existing object, read the
    /// bytes back, then move the pointer.
    ///
    /// # Errors
    ///
    /// Storage, persistence, `Corrupt`, and `PointerConflict` as in [`ColdBuild::run`].
    fn publish(
        &self,
        scope: &SnapshotScope,
        current: Option<&SnapshotPointer>,
        commit: &ObjectSha,
        snapshot: Snapshot,
    ) -> Result<SnapshotPointer, ColdBuildError> {
        let identity = &scope.identity;
        let key = ObjectKey::snapshot(
            identity,
            EXTRACTION_VERSION,
            SNAPSHOT_FORMAT_VERSION,
            &snapshot.tree,
        );
        let bytes = serde_json::to_vec(&snapshot).map_err(|error| ColdBuildError::Corrupt {
            detail: error.to_string(),
        })?;
        match self.storage.create(&key, &bytes) {
            Ok(()) => {}
            Err(StorageError::AlreadyExists { .. }) => {}
            Err(error) => return Err(error.into()),
        }
        let stored = self.storage.read(&key)?;
        Snapshot::decode(&stored, identity, &snapshot.tree)?;
        let pointer = SnapshotPointer {
            tree: snapshot.tree,
            commit: commit.clone(),
            key,
            digest: ObjectDigest::of(&stored),
            published_at: SystemTime::now(),
        };
        let expected = current.map(|pointer| &pointer.tree);
        if !self
            .persistence
            .publish_snapshot_pointer(scope, expected, &pointer)?
        {
            return Err(ColdBuildError::PointerConflict);
        }
        Ok(pointer)
    }
}

/// In-memory ingest state for one attempt: eligible entries still wanted,
/// what was indexed, and what was skipped.
struct Ingest {
    context: SymbolicContext,
    max_file_bytes: u64,
    files: Vec<SnapshotFile>,
    pending: BTreeMap<RelativePath, ObjectSha>,
    skipped: Vec<SkippedFile>,
    excluded: Vec<Exclusion>,
    indexed: usize,
    archive_entries: usize,
    fallback_blobs: usize,
    archive_error: Option<String>,
}

impl Ingest {
    /// Classify `entries` and start with every eligible file pending.
    fn new(
        entries: &[TreeEntry],
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
            indexed: 0,
            archive_entries: 0,
            fallback_blobs: 0,
            archive_error: None,
        }
    }

    /// Stream the tarball, indexing every pending entry whose bytes hash to
    /// the listed blob id. Entries outside the repository, non-regular
    /// entries, unknown paths, and mismatched bytes are left pending for
    /// fallback. A stream error stops the archive and is recorded.
    fn consume_archive(&mut self, archive: Archive, max_expanded_bytes: u64) {
        let expanded = BoundedReader::new(
            GzDecoder::new(archive.reader),
            max_expanded_bytes,
            "archive expansion",
        );
        let mut tar = tar::Archive::new(expanded);
        let outcome: Result<(), io::Error> = (|| {
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
                let size = entry.header().size()?;
                if size > self.max_file_bytes {
                    self.skip(path, blob, SkipReason::Oversize);
                    continue;
                }
                let mut bytes = Vec::with_capacity(size as usize);
                entry.read_to_end(&mut bytes)?;
                if git_blob_sha(&bytes) != blob.as_str() {
                    continue;
                }
                self.archive_entries += 1;
                self.ingest(path, blob, &bytes);
            }
            Ok(())
        })();
        if let Err(error) = outcome {
            self.archive_error = Some(error.to_string());
        }
    }

    /// Fetch every still-pending blob individually, up to `max_blobs`, and
    /// index it. Returns the interruption that stopped the loop, if any;
    /// everything still pending afterwards is skipped as not fetched.
    fn fallback(
        &mut self,
        client: &GitHubClient,
        execution: &Execution,
        identity: &Identity,
        max_blobs: usize,
    ) -> Option<Interrupted> {
        let mut interrupted = None;
        while let Some((path, blob)) = self.pending.pop_first() {
            if interrupted.is_some() || self.fallback_blobs >= max_blobs {
                self.skipped.push(SkippedFile {
                    path,
                    blob,
                    reason: SkipReason::NotFetched,
                });
                continue;
            }
            if let Err(reason) = execution.check() {
                interrupted = Some(reason);
                self.skipped.push(SkippedFile {
                    path,
                    blob,
                    reason: SkipReason::NotFetched,
                });
                continue;
            }
            self.fallback_blobs += 1;
            match client.blob(execution, &identity.repository, &blob) {
                Ok(bytes) if bytes.len() as u64 > self.max_file_bytes => {
                    self.skip(path, blob, SkipReason::Oversize);
                }
                Ok(bytes) => self.ingest(path, blob, &bytes),
                Err(GitHubError::Interrupted(reason)) => {
                    interrupted = Some(reason);
                    self.skipped.push(SkippedFile {
                        path,
                        blob,
                        reason: SkipReason::NotFetched,
                    });
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

    /// Coverage, eligible files, and the populated context.
    fn finish(mut self) -> (Coverage, Vec<SnapshotFile>, SymbolicContext) {
        self.skipped.sort_by(|a, b| a.path.cmp(&b.path));
        let mut excluded = BTreeMap::new();
        for exclusion in &self.excluded {
            *excluded.entry(exclusion.reason).or_insert(0) += 1;
        }
        let coverage = Coverage {
            truncated_tree: false,
            eligible: self.files.len(),
            indexed: self.indexed,
            skipped: self.skipped,
            excluded,
            archive_entries: self.archive_entries,
            fallback_blobs: self.fallback_blobs,
            archive_error: self.archive_error,
        };
        (coverage, self.files, self.context)
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
    use crate::identity::{GitHubHost, InstallationId, InstallationToken, RepositoryIdentity, TenantId};
    use crate::testing::{MemoryPersistence, MemoryStorage, RoutedTransport};
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;
    use std::sync::Arc;
    use std::time::Instant;

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
                GitHubHost::Public,
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
                .append_data(&mut header, format!("octocat-Hello-World-ccccccc/{path}"), *bytes)
                .expect("entry");
        }
        let mut link = tar::Header::new_gnu();
        link.set_entry_type(EntryType::Symlink);
        link.set_size(0);
        link.set_mode(0o777);
        link.set_cksum();
        builder
            .append_link(&mut link, "octocat-Hello-World-ccccccc/link.ts", "src/user.ts")
            .expect("link");
        let mut escape = tar::Header::new_gnu();
        escape.set_size(USER.len() as u64);
        escape.set_mode(0o644);
        escape.set_cksum();
        builder
            .append_data(&mut escape, "octocat-Hello-World-ccccccc/../escape.ts", USER)
            .expect("escape");
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
        let persistence = MemoryPersistence::default();
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
            ObjectKey::snapshot(&identity, EXTRACTION_VERSION, SNAPSHOT_FORMAT_VERSION, &repo.tree)
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
        assert_eq!((coverage.archive_entries, coverage.fallback_blobs), (4, 2));
        assert_eq!(coverage.archive_error, None);

        let calls = client.calls();
        assert_eq!(calls.get(Operation::Archive), 1);
        assert_eq!(calls.get(Operation::Tree), 1);
        assert_eq!(calls.get(Operation::Blob), 3, "attributes + two fallbacks");
        let seen = repo.transport.seen();
        assert!(seen.iter().any(|url| url.ends_with(&format!("/git/blobs/{}", blob_sha(OMITTED)))));
        assert!(seen.iter().any(|url| url.ends_with(&format!("/git/blobs/{}", blob_sha(SUBSTITUTED)))));
        assert!(!seen.iter().any(|url| url.ends_with(&format!("/git/blobs/{}", blob_sha(USER)))));

        assert_eq!(storage.keys(), [pointer.key.clone()]);
        let bytes = storage.read(&pointer.key).expect("object");
        assert_eq!(ObjectDigest::of(&bytes), pointer.digest);
        let text = String::from_utf8(bytes.clone()).expect("json");
        assert!(text.contains("lookupUser") && text.contains("omittedFn"));
        for fragment in ["return { id }", "console.log", "return user.id", "stale"] {
            assert!(!text.contains(fragment), "source fragment {fragment:?} persisted");
        }
        let snapshot = Snapshot::decode(&bytes, &identity, &repo.tree).expect("decode");
        assert_eq!(snapshot.coverage, *coverage);
        let paths: Vec<&str> = snapshot.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            ["src/format.ts", "src/handler.ts", "src/lfs.ts", "src/omitted.ts", "src/substituted.ts", "src/user.ts"]
        );
        let scope = SnapshotScope {
            identity: identity.clone(),
            extraction_version: EXTRACTION_VERSION.to_owned(),
            format_version: SNAPSHOT_FORMAT_VERSION,
        };
        assert_eq!(persistence.snapshot_pointer(&scope).expect("pointer").as_ref(), Some(pointer));

        let loaded = read_snapshot(&storage, &identity, pointer)
            .expect("read")
            .into_context()
            .expect("load");
        assert_eq!(loaded.file_count(), 5);
        let hit = loaded.lookup("lookupUser");
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].path.as_str(), "src/user.ts");
        assert_eq!(loaded.lookup("substitutedFn")[0].path.as_str(), "src/substituted.ts");
        assert_eq!(calls.get(Operation::Blob), 3, "reload fetched nothing");

        let again = build.run(&execution(), &identity).expect("second build");
        assert_eq!(again.outcome, ColdBuildOutcome::Reused(pointer.clone()));
        assert_eq!(again.coverage, *coverage);
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
            key: ObjectKey::snapshot(&identity, EXTRACTION_VERSION, SNAPSHOT_FORMAT_VERSION, &sha('0')),
            digest: ObjectDigest::of(b""),
            published_at: SystemTime::UNIX_EPOCH,
        };
        let run = |repo: &FakeRepo, limits: ColdBuildLimits| {
            let client = repo.client();
            let storage = MemoryStorage::default();
            let persistence = MemoryPersistence::default();
            assert!(persistence.publish_snapshot_pointer(&scope, None, &prior).expect("seed"));
            let result = ColdBuild::new(&client, &storage, &persistence, limits)
                .run(&execution(), &identity)
                .expect("build result");
            let pointer = persistence.snapshot_pointer(&scope).expect("pointer");
            (result, storage.keys(), pointer, client.calls().get(Operation::Blob))
        };

        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        repo.truncate_tree();
        let (result, keys, pointer, _) = run(&repo, ColdBuildLimits::default());
        assert_eq!(result.outcome, ColdBuildOutcome::Incomplete(IncompleteReason::TruncatedTree));
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
        assert!(matches!(&broken.reason, SkipReason::ParseFailed { detail } if detail.contains("deferred")));
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
        assert!(result.coverage.archive_error.as_deref().is_some_and(|e| e.contains("archive expansion")));
        assert_eq!(result.coverage.archive_entries, 0);
        assert_eq!(result.coverage.fallback_blobs, 6);
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
        assert!(result.coverage.skipped.iter().all(|f| f.reason == SkipReason::NotFetched));
        assert!(keys.is_empty());
        assert_eq!(pointer.as_ref(), Some(&prior));
    }

    /// Cancellation during fallback stops fetching, marks the remaining
    /// eligible files as not fetched, and reports the interruption; a
    /// deadline longer than the facade allows is refused before any call.
    #[test]
    fn interruption_and_overlong_deadline_are_explicit() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let client = repo.client();
        let storage = MemoryStorage::default();
        let persistence = MemoryPersistence::default();
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
        assert_eq!(result.coverage.archive_entries, 4);
        assert_eq!(result.coverage.fallback_blobs, 0);
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
                ("src/omitted.ts", &SkipReason::NotFetched),
                ("src/substituted.ts", &SkipReason::NotFetched),
            ]
        );
        assert!(storage.keys().is_empty());

        let long = Execution::with_timeout(MAX_COLD_BUILD_DEADLINE + Duration::from_secs(1));
        let before = client.calls().get(Operation::Repository);
        assert_eq!(
            build.run(&long, &identity),
            Err(ColdBuildError::DeadlineTooLong {
                max: MAX_COLD_BUILD_DEADLINE
            })
        );
        assert_eq!(client.calls().get(Operation::Repository), before);
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
        let persistence = MemoryPersistence::default();
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
            Err(ColdBuildError::Persistence(PersistenceError::Backend { .. }))
        ));
        assert_eq!(storage.keys().len(), 1);
        assert_eq!(persistence.snapshot_pointer(&scope).expect("pointer"), None);

        let archives = client.calls().get(Operation::Archive);
        let result = build.run(&execution(), &identity).expect("retry");
        let ColdBuildOutcome::Published(pointer) = result.outcome else {
            panic!("retry must publish");
        };
        assert_eq!(client.calls().get(Operation::Archive), archives + 1, "whole-build retry");
        assert_eq!(storage.keys(), [pointer.key.clone()]);
        assert_eq!(
            persistence.snapshot_pointer(&scope).expect("pointer").as_ref(),
            Some(&pointer)
        );

        let mut tampered = pointer.clone();
        tampered.digest = ObjectDigest::of(b"other");
        assert!(matches!(
            read_snapshot(&storage, &identity, &tampered),
            Err(ColdBuildError::Corrupt { detail }) if detail == "digest mismatch"
        ));
        let other = Identity {
            tenant: TenantId::new("other").expect("tenant"),
            ..identity.clone()
        };
        assert!(matches!(
            read_snapshot(&storage, &other, &pointer),
            Err(ColdBuildError::Corrupt { detail }) if detail == "identity mismatch"
        ));

        let corrupt_storage = MemoryStorage::default();
        corrupt_storage
            .create(&pointer.key, b"{\"not\":\"a snapshot\"}")
            .expect("seed corrupt object");
        let corrupt_persistence = MemoryPersistence::default();
        let build = ColdBuild::new(&client, &corrupt_storage, &corrupt_persistence, ColdBuildLimits::default());
        assert!(matches!(
            build.run(&execution(), &identity),
            Err(ColdBuildError::Corrupt { .. })
        ));
        assert_eq!(corrupt_persistence.snapshot_pointer(&scope).expect("pointer"), None);
    }
}
