//! Pinned pull-request comparisons: the complete merge-base→head change set
//! and the active baseline→head change set of exactly the commits a caller
//! froze, with verified before/after source for every changed file.
//!
//! [`Comparisons`] never chooses a baseline or reads the live PR's head as
//! a source of truth. The live PR is read once, for its metadata and to
//! confirm the pinned PR and head repository. Every changed file is checked
//! against the complete tree listings of its comparison's base and head, and
//! its source is fetched by the verified blob id. A change the provider
//! omitted or could not describe fails the comparison rather than being
//! guessed: a truncated file list or tree, a rejected path, or a text change
//! without a patch. A baseline the head no longer descends from, or one the
//! provider cannot find, is reported as unusable so the caller can choose a
//! full review; another source is never compared in its place.

use crate::eligibility::{ExclusionReason, RootAttributes, generated_exclusion};
use crate::github::{
    ComparisonStatus, FileComparison, FileStatus, GitHubClient, GitHubError, PullRequest,
    PullRequestFile, PullRequestState, TreeEntry, TreeEntryMode, TreeListing,
};
use crate::identity::{Identity, ObjectSha};
use crate::pull_request::PrContextResult;
use crate::snapshot::{is_oversize, root_attributes};
use bohmian_code_review::RelativePath;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Leading bytes inspected for a NUL, as Git does to call a blob binary.
const BINARY_PROBE_BYTES: usize = 8000;

/// The exact pull request a caller reviews: its number and the base and
/// head commits it froze, with the repository the head commit came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestPins {
    /// PR number in the identity's repository.
    pub number: u64,
    /// Pinned base commit.
    pub base: ObjectSha,
    /// Pinned head commit.
    pub head: ObjectSha,
    /// Numeric id of the repository holding the head, a fork's for a fork PR.
    pub head_repository_id: u64,
}

/// Pinned PR metadata: the caller's pins, the live PR's description, and
/// the verified merge base of the pinned base and head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestSource {
    /// PR number.
    pub number: u64,
    /// Browser URL.
    pub url: String,
    /// Title.
    pub title: String,
    /// Description; empty when the author left none.
    pub body: String,
    /// Open or closed.
    pub state: PullRequestState,
    /// Draft flag.
    pub draft: bool,
    /// Base branch name.
    pub base_ref: String,
    /// Pinned base commit.
    pub base: ObjectSha,
    /// Head branch name.
    pub head_ref: String,
    /// Pinned head commit.
    pub head: ObjectSha,
    /// Repository holding the head commit.
    pub head_repository_id: u64,
    /// Merge base of the pinned base and head.
    pub merge_base: ObjectSha,
}

/// How one file changed between a comparison's base and head.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Present only at the head, including a copy.
    Added,
    /// Content changed in place.
    Modified,
    /// Present only at the base.
    Deleted,
    /// Moved from [`ChangedFile::previous_path`], possibly with edits.
    Renamed,
    /// Mode or object type changed.
    TypeChanged,
}

/// One unified-diff hunk with its header coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hunk {
    /// First base line, 0 when the hunk adds to an empty file.
    pub old_start: u32,
    /// Base lines covered.
    pub old_lines: u32,
    /// First head line, 0 when the hunk empties the file.
    pub new_start: u32,
    /// Head lines covered.
    pub new_lines: u32,
    /// Hunk text, header included.
    pub patch: String,
}

/// Why a file's bytes were not fetched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum UnavailableReason {
    /// The blob is larger than the invocation's file ceiling.
    Oversize {
        /// The ceiling in bytes.
        limit: u64,
    },
}

/// One side of a changed file: its exact text, or why there is none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    /// UTF-8 text of the verified blob.
    Text {
        /// Blob id the text hashes to.
        blob: ObjectSha,
        /// Exact text.
        text: String,
    },
    /// The verified blob holds a NUL or is not UTF-8.
    Binary {
        /// Blob id.
        blob: ObjectSha,
    },
    /// The blob was not fetched.
    Unavailable {
        /// Blob id.
        blob: ObjectSha,
        /// Why.
        #[serde(flatten)]
        reason: UnavailableReason,
    },
    /// The entry is not reviewable source: a symlink, submodule, or a
    /// vendored, generated, or minified path.
    Excluded {
        /// Object id of the blob or submodule commit.
        blob: ObjectSha,
        /// Why.
        reason: ExclusionReason,
    },
}

impl Source {
    /// The text when this side is text.
    fn text(&self) -> Option<&str> {
        match self {
            Source::Text { text, .. } => Some(text),
            _ => None,
        }
    }
}

/// One changed file of a comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedFile {
    /// Head-side path, or the deleted path.
    pub path: RelativePath,
    /// Base-side path of a rename.
    pub previous_path: Option<RelativePath>,
    /// How the file changed.
    pub status: ChangeKind,
    /// Base side; absent for an added file.
    pub before: Option<Source>,
    /// Head side; absent for a deleted file.
    pub after: Option<Source>,
    /// The provider's hunks; empty when the content did not change or a
    /// side is not text.
    pub hunks: Vec<Hunk>,
}

/// Every changed file between two pinned commits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSet {
    /// Base commit.
    pub base: ObjectSha,
    /// Head commit.
    pub head: ObjectSha,
    /// Changed files in provider order; empty when nothing changed.
    pub files: Vec<ChangedFile>,
}

/// Why a supplied baseline cannot anchor an incremental comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineProblem {
    /// The head no longer descends from the baseline.
    Rewritten,
    /// The provider cannot find the baseline or relate it to the head.
    Unavailable,
}

/// A pinned review's source evidence and the context selected for its
/// active changes.
#[derive(Debug, Clone, PartialEq)]
pub struct PinnedReview {
    /// Pinned PR metadata.
    pub pull_request: PullRequestSource,
    /// Merge base → head.
    pub full: ChangeSet,
    /// Baseline → head, or the full change set without a baseline.
    pub active: ChangeSet,
    /// The published review context of the active changes.
    pub context: PrContextResult,
}

/// Why a pinned comparison failed. Nothing is published on failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ComparisonError {
    /// A provider read failed.
    #[error(transparent)]
    Provider(#[from] GitHubError),
    /// The live PR names another head repository than the pins.
    #[error("head repository mismatch: pinned {pinned}, provider returned {actual}")]
    HeadRepositoryMismatch {
        /// Pinned head repository.
        pinned: u64,
        /// The live PR's head repository.
        actual: u64,
    },
    /// The supplied baseline cannot anchor an incremental comparison.
    #[error("baseline {baseline} is unusable: {problem:?}")]
    BaselineUnusable {
        /// The supplied baseline.
        baseline: ObjectSha,
        /// Why.
        problem: BaselineProblem,
    },
    /// The provider's file list or a tree listing hit its ceiling.
    #[error("comparison {base}...{head} is truncated")]
    Truncated {
        /// Comparison base.
        base: ObjectSha,
        /// Comparison head.
        head: ObjectSha,
    },
    /// A changed path is not a canonical repository-relative path.
    #[error("changed path {path} rejected: {reason}")]
    RejectedPath {
        /// Raw path text.
        path: String,
        /// Why.
        reason: String,
    },
    /// The provider described a change the pinned trees do not hold.
    #[error("change to {path} does not match the pinned trees: {detail}")]
    Unverified {
        /// Changed path.
        path: RelativePath,
        /// What did not match.
        detail: String,
    },
    /// A text change came without a patch.
    #[error("patch unavailable for {path}")]
    PatchUnavailable {
        /// Changed path.
        path: RelativePath,
    },
    /// A patch was not unified-diff hunks.
    #[error("malformed patch for {path}: {detail}")]
    MalformedPatch {
        /// Changed path.
        path: RelativePath,
        /// Parser detail.
        detail: String,
    },
}

/// One pinned commit's root tree, its complete listing, and its root
/// attributes.
pub(crate) struct PinnedTree {
    /// Root tree id.
    pub(crate) tree: ObjectSha,
    /// The complete listing.
    pub(crate) listing: TreeListing,
    /// Root `.gitattributes` of that tree.
    attributes: RootAttributes,
    /// Entry index into the listing by path.
    paths: BTreeMap<RelativePath, usize>,
}

impl PinnedTree {
    /// The entry at `path`.
    fn entry(&self, path: &RelativePath) -> Option<&TreeEntry> {
        self.paths
            .get(path)
            .map(|&index| &self.listing.entries[index])
    }
}

/// The comparison stage over one provider client and identity.
///
/// Fetched sources are cached by blob id for the stage's lifetime, so a
/// file in both the full and the active change set is fetched once.
pub(crate) struct Comparisons<'a> {
    client: &'a GitHubClient,
    identity: &'a Identity,
    max_file_bytes: u64,
    sources: BTreeMap<ObjectSha, Source>,
}

impl<'a> Comparisons<'a> {
    /// Stage over `client` for `identity` with the invocation file ceiling.
    pub(crate) fn new(
        client: &'a GitHubClient,
        identity: &'a Identity,
        max_file_bytes: u64,
    ) -> Comparisons<'a> {
        Comparisons {
            client,
            identity,
            max_file_bytes,
            sources: BTreeMap::new(),
        }
    }

    /// Read the live PR for its metadata, confirm the pins name it, and
    /// compare the pinned base with the pinned head.
    ///
    /// Returns the PR as the context stage should see it, at exactly the
    /// pins, its metadata, and the merge-base comparison.
    ///
    /// # Errors
    ///
    /// Provider errors, including the PR's own identity checks, and
    /// `HeadRepositoryMismatch` when the live PR names another existing
    /// head repository. A deleted fork is accepted: the pinned commits are
    /// verified by the comparison and tree reads.
    pub(crate) async fn pull_request(
        &self,
        pins: &PullRequestPins,
    ) -> Result<(PullRequest, PullRequestSource, FileComparison), ComparisonError> {
        let repository = &self.identity.repository;
        let live = self.client.pull_request(repository, pins.number).await?;
        if let Some(actual) = live.head_repository_id
            && actual != pins.head_repository_id
        {
            return Err(ComparisonError::HeadRepositoryMismatch {
                pinned: pins.head_repository_id,
                actual,
            });
        }
        let full = self
            .client
            .compare_files(repository, &pins.base, &pins.head)
            .await?;
        let source = PullRequestSource {
            number: pins.number,
            url: live.url.clone(),
            title: live.title.clone(),
            body: live.body.clone(),
            state: live.state,
            draft: live.draft,
            base_ref: live.base_ref.clone(),
            base: pins.base.clone(),
            head_ref: live.head_ref.clone(),
            head: pins.head.clone(),
            head_repository_id: pins.head_repository_id,
            merge_base: full.merge_base.clone(),
        };
        let pinned = PullRequest {
            base: pins.base.clone(),
            head: pins.head.clone(),
            head_repository_id: Some(pins.head_repository_id),
            ..live
        };
        Ok((pinned, source, full))
    }

    /// Compare `baseline` with `head` for an incremental review.
    ///
    /// # Errors
    ///
    /// `BaselineUnusable` as `Rewritten` when the head does not descend
    /// from the baseline and as `Unavailable` when the provider cannot find
    /// either; other provider errors.
    pub(crate) async fn baseline(
        &self,
        baseline: &ObjectSha,
        head: &ObjectSha,
    ) -> Result<FileComparison, ComparisonError> {
        let unusable = |problem| ComparisonError::BaselineUnusable {
            baseline: baseline.clone(),
            problem,
        };
        let comparison = match self
            .client
            .compare_files(&self.identity.repository, baseline, head)
            .await
        {
            Err(GitHubError::NotFound { .. }) => {
                return Err(unusable(BaselineProblem::Unavailable));
            }
            other => other?,
        };
        let descends = matches!(
            comparison.status,
            ComparisonStatus::Ahead | ComparisonStatus::Identical
        );
        if !descends || comparison.merge_base != *baseline {
            return Err(unusable(BaselineProblem::Rewritten));
        }
        Ok(comparison)
    }

    /// The root tree of `commit`, listed completely.
    ///
    /// # Errors
    ///
    /// Provider errors, and `Truncated` naming `commit` when the listing
    /// is incomplete.
    pub(crate) async fn tree(&self, commit: &ObjectSha) -> Result<PinnedTree, ComparisonError> {
        let repository = &self.identity.repository;
        let tree = self.client.commit(repository, commit).await?.tree;
        let listing = self.client.tree(repository, &tree).await?;
        if listing.truncated {
            return Err(ComparisonError::Truncated {
                base: commit.clone(),
                head: commit.clone(),
            });
        }
        let attributes = root_attributes(
            self.client,
            self.identity,
            &listing.entries,
            self.max_file_bytes,
        )
        .await?;
        let paths = listing
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.path.clone(), index))
            .collect();
        Ok(PinnedTree {
            tree,
            listing,
            attributes,
            paths,
        })
    }

    /// Describe `comparison`, listed between commits `base` and `head`
    /// whose trees are `before` and `after`, as a verified change set.
    ///
    /// # Errors
    ///
    /// `Truncated` and `RejectedPath` for an incomplete provider list,
    /// `Unverified` when a change disagrees with the trees, patch errors,
    /// and provider errors fetching a source blob.
    pub(crate) async fn change_set(
        &mut self,
        (base, before): (&ObjectSha, &PinnedTree),
        (head, after): (&ObjectSha, &PinnedTree),
        comparison: &FileComparison,
    ) -> Result<ChangeSet, ComparisonError> {
        if comparison.truncated {
            return Err(ComparisonError::Truncated {
                base: base.clone(),
                head: head.clone(),
            });
        }
        if let Some(rejected) = comparison.rejected.first() {
            return Err(ComparisonError::RejectedPath {
                path: rejected.path.clone(),
                reason: rejected.reason.clone(),
            });
        }
        let mut files = Vec::with_capacity(comparison.files.len());
        for file in &comparison.files {
            if let Some(changed) = self.changed_file(file, before, after).await? {
                files.push(changed);
            }
        }
        Ok(ChangeSet {
            base: base.clone(),
            head: head.clone(),
            files,
        })
    }

    /// One provider file as a verified change, or `None` when unchanged.
    ///
    /// # Errors
    ///
    /// As [`Comparisons::change_set`].
    async fn changed_file(
        &mut self,
        file: &PullRequestFile,
        before: &PinnedTree,
        after: &PinnedTree,
    ) -> Result<Option<ChangedFile>, ComparisonError> {
        let (status, previous_path) = match file.status {
            FileStatus::Unchanged => return Ok(None),
            FileStatus::Added | FileStatus::Copied => (ChangeKind::Added, None),
            FileStatus::Removed => (ChangeKind::Deleted, None),
            FileStatus::Modified => (ChangeKind::Modified, None),
            FileStatus::Changed => (ChangeKind::TypeChanged, None),
            FileStatus::Renamed => (ChangeKind::Renamed, file.previous_path.clone()),
        };
        let path = &file.path;
        let unverified = |detail: &str| ComparisonError::Unverified {
            path: path.clone(),
            detail: detail.to_owned(),
        };
        let before_entry = match status {
            ChangeKind::Added => None,
            _ => Some(
                before
                    .entry(previous_path.as_ref().unwrap_or(path))
                    .ok_or_else(|| unverified("absent from the base tree"))?,
            ),
        };
        let after_entry = match status {
            ChangeKind::Deleted => None,
            _ => {
                let entry = after
                    .entry(path)
                    .ok_or_else(|| unverified("absent from the head tree"))?;
                if entry.sha != file.sha {
                    return Err(unverified("head blob differs from the head tree"));
                }
                Some(entry)
            }
        };
        let before_source = match before_entry {
            Some(entry) => Some(self.source(entry, &before.attributes).await?),
            None => None,
        };
        let after_source = match after_entry {
            Some(entry) => Some(self.source(entry, &after.attributes).await?),
            None => None,
        };
        let hunks = match file.patch.as_deref() {
            Some(patch) => hunks(patch).map_err(|detail| ComparisonError::MalformedPatch {
                path: path.clone(),
                detail,
            })?,
            None if needs_patch(before_source.as_ref(), after_source.as_ref()) => {
                return Err(ComparisonError::PatchUnavailable { path: path.clone() });
            }
            None => Vec::new(),
        };
        Ok(Some(ChangedFile {
            path: path.clone(),
            previous_path,
            status,
            before: before_source,
            after: after_source,
            hunks,
        }))
    }

    /// The source of tree `entry` under that tree's `attributes`, fetched
    /// by its verified blob id at most once per stage.
    ///
    /// # Errors
    ///
    /// Provider errors other than the file ceiling's refusal, which is
    /// reported as [`Source::Unavailable`].
    async fn source(
        &mut self,
        entry: &TreeEntry,
        attributes: &RootAttributes,
    ) -> Result<Source, ComparisonError> {
        let blob = entry.sha.clone();
        let excluded = match entry.mode {
            TreeEntryMode::Symlink => Some(ExclusionReason::Symlink),
            TreeEntryMode::Submodule => Some(ExclusionReason::Submodule),
            _ => generated_exclusion(entry.path.as_str(), attributes),
        };
        if let Some(reason) = excluded {
            return Ok(Source::Excluded { blob, reason });
        }
        let limit = self.max_file_bytes;
        let oversize = Source::Unavailable {
            blob: blob.clone(),
            reason: UnavailableReason::Oversize { limit },
        };
        if entry.size.is_some_and(|size| size > limit) {
            return Ok(oversize);
        }
        if let Some(source) = self.sources.get(&blob) {
            return Ok(source.clone());
        }
        let source = match self
            .client
            .blob(&self.identity.repository, &blob, limit)
            .await
        {
            Ok(bytes) => text_source(blob.clone(), bytes),
            Err(error) if is_oversize(&error, limit) => oversize,
            Err(error) => return Err(error.into()),
        };
        self.sources.insert(blob, source.clone());
        Ok(source)
    }
}

/// `bytes` of verified blob `blob` as text, or binary when a NUL appears in
/// the leading [`BINARY_PROBE_BYTES`] or the bytes are not UTF-8.
fn text_source(blob: ObjectSha, bytes: Vec<u8>) -> Source {
    if bytes[..bytes.len().min(BINARY_PROBE_BYTES)].contains(&0) {
        return Source::Binary { blob };
    }
    match String::from_utf8(bytes) {
        Ok(text) => Source::Text { blob, text },
        Err(_) => Source::Binary { blob },
    }
}

/// Whether a change between `before` and `after` must carry a patch: every
/// present side is text, an absent side counts as empty text, and the
/// texts differ. A binary, unavailable, or excluded side has no text diff.
fn needs_patch(before: Option<&Source>, after: Option<&Source>) -> bool {
    fn text(side: Option<&Source>) -> Option<&str> {
        side.map_or(Some(""), Source::text)
    }
    matches!((text(before), text(after)), (Some(old), Some(new)) if old != new)
}

/// Split unified-diff `patch` into hunks at each `@@ -a,b +c,d @@` header.
///
/// # Errors
///
/// Returns a detail when text precedes the first header or a header's
/// coordinates do not parse.
fn hunks(patch: &str) -> Result<Vec<Hunk>, String> {
    let mut hunks: Vec<Hunk> = Vec::new();
    if patch.is_empty() {
        return Ok(hunks);
    }
    for line in patch.split('\n') {
        if let Some(header) = line.strip_prefix("@@ -") {
            let (old, rest) = header
                .split_once(" +")
                .ok_or_else(|| format!("hunk header `{line}`"))?;
            let (new, _) = rest
                .split_once(" @@")
                .ok_or_else(|| format!("hunk header `{line}`"))?;
            let (old_start, old_lines) =
                range(old).ok_or_else(|| format!("hunk header `{line}`"))?;
            let (new_start, new_lines) =
                range(new).ok_or_else(|| format!("hunk header `{line}`"))?;
            hunks.push(Hunk {
                old_start,
                old_lines,
                new_start,
                new_lines,
                patch: line.to_owned(),
            });
        } else {
            let hunk = hunks
                .last_mut()
                .ok_or_else(|| "text before the first hunk header".to_owned())?;
            hunk.patch.push('\n');
            hunk.patch.push_str(line);
        }
    }
    Ok(hunks)
}

/// `start[,count]` of a hunk header; a missing count is 1.
fn range(text: &str) -> Option<(u32, u32)> {
    match text.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
        None => Some((text.parse().ok()?, 1)),
    }
}

#[cfg(test)]
/// Owner tests for hunk parsing, text classification, and patch need.
mod tests {
    use super::*;

    /// Forty copies of `ch` as an object id.
    fn sha(ch: char) -> ObjectSha {
        ObjectSha::new(&ch.to_string().repeat(40)).expect("sha")
    }

    /// Headers split hunks and keep their text; a missing count is one.
    #[test]
    fn hunks_split_at_headers() {
        let patch = "@@ -1,2 +1,3 @@ fn a\n a\n+b\n c\n@@ -9 +10,0 @@\n-z";
        let parsed = hunks(patch).expect("hunks");
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            (
                parsed[0].old_start,
                parsed[0].old_lines,
                parsed[0].new_start,
                parsed[0].new_lines
            ),
            (1, 2, 1, 3)
        );
        assert_eq!(parsed[0].patch, "@@ -1,2 +1,3 @@ fn a\n a\n+b\n c");
        assert_eq!(
            (
                parsed[1].old_start,
                parsed[1].old_lines,
                parsed[1].new_start,
                parsed[1].new_lines
            ),
            (9, 1, 10, 0)
        );
        assert!(hunks("+orphan").is_err());
        assert!(hunks("@@ -x +1 @@").is_err());
        assert_eq!(hunks("").expect("empty").len(), 0);
    }

    /// A NUL or invalid UTF-8 is binary; other bytes are exact text.
    #[test]
    fn blobs_classify_as_text_or_binary() {
        assert_eq!(
            text_source(sha('a'), b"a\nb".to_vec()),
            Source::Text {
                blob: sha('a'),
                text: "a\nb".to_owned()
            }
        );
        assert_eq!(
            text_source(sha('a'), b"a\0b".to_vec()),
            Source::Binary { blob: sha('a') }
        );
        assert_eq!(
            text_source(sha('a'), vec![0xff, 0xfe]),
            Source::Binary { blob: sha('a') }
        );
    }

    /// Only a differing text change needs a patch.
    #[test]
    fn only_text_changes_need_patches() {
        let text = |t: &str| Source::Text {
            blob: sha('a'),
            text: t.to_owned(),
        };
        let binary = Source::Binary { blob: sha('b') };
        assert!(needs_patch(Some(&text("a")), Some(&text("b"))));
        assert!(needs_patch(None, Some(&text("b"))));
        assert!(!needs_patch(None, Some(&text(""))));
        assert!(!needs_patch(Some(&text("a")), Some(&text("a"))));
        assert!(!needs_patch(Some(&binary), Some(&text("b"))));
        assert!(needs_patch(Some(&text("a")), None));
    }
}
