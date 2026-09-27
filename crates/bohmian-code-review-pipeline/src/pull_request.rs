//! The PR-context facade: pin a pull request, overlay its head tree on a
//! private index, rank candidates from its changed lines, pack them under a
//! token allowance, and publish one shared [`ReviewContext`].
//!
//! [`PrContext::run`] reads the PR's base/head before and after listing its
//! files and retries when either moved. A compatible default-branch
//! snapshot is loaded into an invocation-private index and reconciled with
//! the head tree through the snapshot ingest: unchanged facts are carried,
//! changed and drifted blobs are fetched within a bound, and removed paths
//! are never loaded. Without a usable snapshot, or with a truncated head
//! listing, a bounded index of files near the changed paths is built
//! instead and reported as scoped. Candidate blobs are fetched at their head
//! identity only after ranking. Source bytes are dropped once packed.

use crate::context::{
    Budget, CONTEXT_FORMAT_VERSION, ContextCoverage, ContextError, Contexts, NotAnalyzed,
    NotAnalyzedReason, OmittedReason, OmittedSpan, ReviewContext, Scope, SelectedSpan,
};
use crate::eligibility::MAX_ELIGIBLE_FILE_BYTES;
use crate::github::{
    FileStatus, GitHubClient, PR_FILES_PER_PAGE, PullRequest, PullRequestFile, RejectedPath,
    TreeEntry, TreeEntryMode, TreeListing,
};
use crate::identity::{Identity, ObjectSha};
use crate::persistence::{ContextLifetime, ContextRecord, ContextStore, SnapshotStore};
use crate::snapshot::{
    ColdBuildError, Ingest, Snapshot, SnapshotFile, Snapshots, root_attributes, snapshot_scope,
};
use crate::storage::{ObjectStorage, StorageError};
use bohmian_code_review::Error as SymbolicError;
use bohmian_code_review::{
    ChangedBlob, Context, ContextLimits, ContextSpan, EXTRACTION_VERSION, LineRange, Omission,
    OmissionReason, RankedFile, RelativePath, SelectError, SpanKind, SymbolicContext,
    build_candidate_context, parse_hunks,
};
use std::collections::{BTreeMap, BTreeSet};

/// Ceilings for one PR-context invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrContextLimits {
    /// PR files read; the provider's documented listing ceiling.
    pub max_pr_files: usize,
    /// Default-branch drift blobs fetched to overlay a snapshot; past it the
    /// overlay is reported incomplete.
    pub max_overlay_changes: usize,
    /// Files in a scoped index, and blobs fetched to build it.
    pub max_scoped_files: usize,
    /// Selected candidate files.
    pub max_candidate_files: usize,
    /// Review token allocation shared by the diff, prompt, and context.
    pub review_tokens: usize,
    /// Tokens reserved for the downstream prompt.
    pub prompt_reserve: usize,
    /// Times the PR files are listed before a moving PR is reported.
    pub max_pagination_attempts: usize,
    /// Largest eligible file.
    pub max_file_bytes: u64,
}

impl Default for PrContextLimits {
    /// 3,000 PR files, 300 overlay changes, 300 scoped files, 15 candidate
    /// files, 64,000 review tokens with a 4,096 prompt reserve, three
    /// pagination attempts, and 1 MiB files.
    fn default() -> PrContextLimits {
        PrContextLimits {
            max_pr_files: 3000,
            max_overlay_changes: 300,
            max_scoped_files: 300,
            max_candidate_files: 15,
            review_tokens: 64_000,
            prompt_reserve: 4096,
            max_pagination_attempts: 3,
            max_file_bytes: MAX_ELIGIBLE_FILE_BYTES,
        }
    }
}

/// A published context: its record, whose id the caller hands downstream,
/// and the stored object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrContextResult {
    /// Published metadata, including the context id.
    pub record: ContextRecord,
    /// When the context store published the record and when it expires.
    pub lifetime: ContextLifetime,
    /// The object the id names.
    pub context: ReviewContext,
}

/// PR files listed under one unchanged base/head pair.
struct Pinned {
    pull_request: PullRequest,
    files: Vec<PullRequestFile>,
    rejected: Vec<RejectedPath>,
    truncated: bool,
}

/// One changed file's verified head bytes and head-side added lines.
struct Head {
    path: RelativePath,
    bytes: Vec<u8>,
    changed: Vec<LineRange>,
}

/// The PR-context facade over the provider, snapshot storage and store,
/// and context storage and store.
pub struct PrContext<'a, O, S, C> {
    client: &'a GitHubClient,
    snapshot_storage: &'a O,
    snapshot_store: &'a S,
    contexts: Contexts<'a, O, C>,
    limits: PrContextLimits,
}

impl<'a, O: ObjectStorage, S: SnapshotStore, C: ContextStore> PrContext<'a, O, S, C> {
    /// Facade over the caller's capabilities and ceilings; snapshot and
    /// context objects may live in separate storage instances.
    pub fn new(
        client: &'a GitHubClient,
        snapshot_storage: &'a O,
        snapshot_store: &'a S,
        context_storage: &'a O,
        context_store: &'a C,
        limits: PrContextLimits,
    ) -> PrContext<'a, O, S, C> {
        PrContext {
            client,
            snapshot_storage,
            snapshot_store,
            contexts: Contexts::new(context_storage, context_store),
            limits,
        }
    }

    /// Build and publish the review context of pull request `number`.
    ///
    /// Every fact comes from one base/head pair. The overlay and ranking are
    /// private to this call; nothing but the context object and its record
    /// is written. The caller bounds the call by dropping its future; a drop
    /// leaves at most a record whose object is absent, which reads treat as
    /// not found, and a retry converges on a readable context.
    ///
    /// # Errors
    ///
    /// Returns provider errors for the PR, its files, head commit, tree, and
    /// attributes; `PullRequestMoved`, a retry state, when base or head kept
    /// changing; snapshot errors other than a missing object; symbolic,
    /// storage, and persistence failures. Per-file problems are not errors:
    /// they are listed in [`ReviewContext::not_analyzed`] and coverage.
    pub async fn run(
        &self,
        identity: &Identity,
        number: u64,
    ) -> Result<PrContextResult, ContextError> {
        let pinned = self.pin(identity, number).await?;
        let repository = &identity.repository;
        let pull_request = &pinned.pull_request;
        let tree = self
            .client
            .commit(repository, &pull_request.head)
            .await?
            .tree;
        let listing = self.client.tree(repository, &tree).await?;
        let attributes = root_attributes(self.client, identity, &listing.entries).await?;
        let snapshot = match listing.truncated {
            true => None,
            false => self.snapshot(identity).await?,
        };
        let limits = &self.limits;
        let rejected_changed: Vec<RelativePath> = pinned
            .rejected
            .iter()
            .filter_map(|rejected| RelativePath::new(&rejected.path).ok())
            .collect();
        let mut ingest = match snapshot {
            Some(_) => Ingest::new(
                &listing.entries,
                listing.invalid_paths,
                &attributes,
                limits.max_file_bytes,
            ),
            None => {
                let seeds = pinned
                    .files
                    .iter()
                    .filter(|file| file.status != FileStatus::Removed)
                    .map(|file| &file.path)
                    .chain(&rejected_changed)
                    .collect();
                Ingest::new(
                    &near(&listing, &seeds),
                    0,
                    &attributes,
                    limits.max_file_bytes,
                )
            }
        };
        let snapshot_tree = snapshot.map(|snapshot| {
            let tree = snapshot.tree.clone();
            ingest.reuse(snapshot);
            tree
        });

        let pr_files_rejected = pinned.rejected.len();
        let mut not_analyzed: Vec<NotAnalyzed> = pinned
            .rejected
            .into_iter()
            .map(|rejected| NotAnalyzed {
                path: rejected.path,
                reason: NotAnalyzedReason::Rejected {
                    detail: rejected.reason,
                },
            })
            .collect();
        let index_cap = match snapshot_tree {
            Some(_) => usize::MAX,
            None => limits.max_scoped_files,
        };
        let mut heads = Vec::new();
        for file in &pinned.files {
            match self
                .changed_head(&mut ingest, identity, file, index_cap)
                .await
            {
                Ok(head) => heads.push(head),
                Err(reason) => not_analyzed.push(NotAnalyzed {
                    path: file.path.to_string(),
                    reason,
                }),
            }
        }
        let fetch_bound = match snapshot_tree {
            Some(_) => limits.max_overlay_changes,
            None => limits.max_scoped_files.saturating_sub(ingest.indexed()),
        };
        ingest.fallback(self.client, identity, fetch_bound).await;
        let (mut files, _, eligible, index) = ingest.finish();
        files.truncated_tree = listing.truncated;
        let coverage = ContextCoverage {
            scope: match snapshot_tree {
                Some(_) => Scope::Repository,
                None => Scope::Scoped,
            },
            snapshot_complete: snapshot_tree.is_some(),
            snapshot_tree,
            overlay_complete: files.is_complete(),
            pr_files_truncated: pinned.truncated,
            pr_files_rejected,
            files,
        };

        let changed: BTreeSet<&RelativePath> = pinned
            .files
            .iter()
            .map(|file| &file.path)
            .chain(&rejected_changed)
            .collect();
        let mut ranked = rank(&index, &heads, &mut not_analyzed)?;
        ranked.retain(|file| !changed.contains(&file.path));
        drop(heads);
        let diff_tokens = pinned
            .files
            .iter()
            .filter_map(|file| file.patch.as_deref())
            .map(estimate_tokens)
            .sum();
        let context_allowance = limits
            .review_tokens
            .saturating_sub(limits.prompt_reserve)
            .saturating_sub(diff_tokens);
        let packed = match context_allowance {
            0 => Context::default(),
            _ => {
                self.pack(
                    identity,
                    ranked,
                    &eligible,
                    context_allowance,
                    &mut not_analyzed,
                )
                .await?
            }
        };
        let review = ReviewContext {
            format_version: CONTEXT_FORMAT_VERSION,
            extraction_version: EXTRACTION_VERSION.to_owned(),
            identity: identity.clone(),
            pull_request: pull_request.number,
            base: pull_request.base.clone(),
            head: pull_request.head.clone(),
            tree,
            spans: selected_spans(packed.spans, &eligible),
            omissions: packed.omissions.into_iter().map(omitted_span).collect(),
            not_analyzed,
            budget: Budget {
                review_tokens: limits.review_tokens,
                prompt_reserve: limits.prompt_reserve,
                diff_tokens,
                context_allowance,
                selected_tokens: packed.tokens,
                exhausted: context_allowance == 0,
            },
            coverage,
        };
        let (record, lifetime) = self.contexts.publish(identity, &review).await?;
        Ok(PrContextResult {
            record,
            lifetime,
            context: review,
        })
    }

    /// List the PR's files between two reads of its base and head, retrying
    /// while either moves.
    ///
    /// # Errors
    ///
    /// Returns provider errors and `PullRequestMoved` after the last attempt.
    async fn pin(&self, identity: &Identity, number: u64) -> Result<Pinned, ContextError> {
        let repository = &identity.repository;
        let max_files = self.limits.max_pr_files;
        let pages = max_files.div_ceil(PR_FILES_PER_PAGE as usize).max(1);
        for _ in 0..self.limits.max_pagination_attempts {
            let mut pinned = Pinned {
                pull_request: self.client.pull_request(repository, number).await?,
                files: Vec::new(),
                rejected: Vec::new(),
                truncated: false,
            };
            for page in 1..=pages {
                let listed = self
                    .client
                    .pull_request_files(repository, number, page as u32)
                    .await?;
                pinned.files.extend(listed.files);
                pinned.rejected.extend(listed.rejected);
                if !listed.has_next {
                    break;
                }
                pinned.truncated = page == pages;
            }
            if pinned.files.len() > max_files {
                pinned.files.truncate(max_files);
                pinned.truncated = true;
            }
            let after = self.client.pull_request(repository, number).await?;
            let before = &pinned.pull_request;
            if (&after.base, &after.head) == (&before.base, &before.head) {
                return Ok(pinned);
            }
        }
        Err(ContextError::PullRequestMoved {
            attempts: self.limits.max_pagination_attempts,
        })
    }

    /// The current compatible default-branch snapshot, or `None` when no
    /// pointer exists or its object is gone.
    ///
    /// # Errors
    ///
    /// Returns persistence failures and snapshot errors other than a
    /// missing object.
    async fn snapshot(&self, identity: &Identity) -> Result<Option<Snapshot>, ContextError> {
        let scope = snapshot_scope(identity, self.limits.max_file_bytes);
        let Some(pointer) = self.snapshot_store.snapshot_pointer(&scope).await? else {
            return Ok(None);
        };
        match Snapshots::new(self.snapshot_storage, self.snapshot_store)
            .read(&scope, &pointer)
            .await
        {
            Ok(snapshot) => Ok(Some(snapshot)),
            Err(ColdBuildError::Storage(StorageError::NotFound { .. })) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Verify one PR file against the pinned head tree, parse its patch,
    /// and fetch its head blob, indexing it when the overlay still wants it
    /// and holds fewer than `index_cap` indexed files. A head past the cap
    /// is still analyzed and stays pending, so coverage reports it
    /// unindexed.
    ///
    /// # Errors
    ///
    /// Returns why the file cannot be analyzed; it affects no other file.
    async fn changed_head(
        &self,
        ingest: &mut Ingest,
        identity: &Identity,
        file: &PullRequestFile,
        index_cap: usize,
    ) -> Result<Head, NotAnalyzedReason> {
        if file.status == FileStatus::Removed {
            return Err(NotAnalyzedReason::Removed);
        }
        let blob = ingest
            .eligible_blob(&file.path)
            .ok_or(NotAnalyzedReason::NotEligible)?
            .clone();
        if blob != file.sha {
            return Err(NotAnalyzedReason::BlobMismatch);
        }
        let patch = file
            .patch
            .as_deref()
            .ok_or(NotAnalyzedReason::MissingPatch)?;
        let changed = parse_hunks(patch).map_err(|error| NotAnalyzedReason::MalformedPatch {
            detail: error.to_string(),
        })?;
        if changed.is_empty() {
            return Err(NotAnalyzedReason::DeletionOnly);
        }
        let bytes = self
            .client
            .blob(&identity.repository, &blob)
            .await
            .map_err(|error| NotAnalyzedReason::FetchFailed {
                detail: error.to_string(),
            })?;
        if ingest.is_pending(&file.path) && ingest.indexed() < index_cap {
            ingest.ingest(file.path.clone(), blob, &bytes);
        }
        Ok(Head {
            path: file.path.clone(),
            bytes,
            changed,
        })
    }

    /// Pack `ranked` under the candidate file ceiling and `allowance`,
    /// fetching each candidate blob at its head identity only when packing
    /// reaches it. A candidate whose blob cannot be fetched is dropped from
    /// the ranking and listed as not analyzed.
    ///
    /// # Errors
    ///
    /// Returns packing errors other than a blob not yet fetched.
    async fn pack(
        &self,
        identity: &Identity,
        mut ranked: Vec<RankedFile>,
        eligible: &[SnapshotFile],
        allowance: usize,
        not_analyzed: &mut Vec<NotAnalyzed>,
    ) -> Result<Context, ContextError> {
        let limits = ContextLimits {
            max_files: self.limits.max_candidate_files,
            max_tokens: allowance,
        };
        let mut blobs: BTreeMap<RelativePath, Vec<u8>> = BTreeMap::new();
        loop {
            let missing = {
                let view = blobs
                    .iter()
                    .map(|(path, bytes)| (path.clone(), bytes.as_slice()))
                    .collect();
                match build_candidate_context(&ranked, &view, limits, estimate_tokens) {
                    Err(SymbolicError::Select(SelectError::MissingBlob { path })) => path,
                    packed => return Ok(packed?),
                }
            };
            let fetched = match blob_of(eligible, &missing) {
                Some(blob) => self
                    .client
                    .blob(&identity.repository, blob)
                    .await
                    .map_err(|error| error.to_string()),
                None => Err("candidate is not in the head tree".to_owned()),
            };
            match fetched {
                Ok(bytes) => {
                    blobs.insert(missing, bytes);
                }
                Err(detail) => {
                    ranked.retain(|file| file.path != missing);
                    not_analyzed.push(NotAnalyzed {
                        path: missing.to_string(),
                        reason: NotAnalyzedReason::FetchFailed { detail },
                    });
                }
            }
        }
    }
}

/// Rank candidates from every valid changed head, listing a head the
/// symbolic library rejects as not analyzed without dropping the others.
/// Only analyzed heads are excluded here; the caller also removes PR files
/// that were not analyzed, so no changed path is ever a candidate.
///
/// # Errors
///
/// Returns a symbolic error only if the validated heads fail together.
fn rank(
    index: &SymbolicContext,
    heads: &[Head],
    not_analyzed: &mut Vec<NotAnalyzed>,
) -> Result<Vec<RankedFile>, ContextError> {
    let mut valid = Vec::with_capacity(heads.len());
    for head in heads {
        // ponytail: validates by ranking each head alone, so heads are
        // extracted twice; a per-file error from `rank_files` would avoid it.
        match index.rank(head.path.as_str(), &head.bytes, head.changed.clone()) {
            Ok(_) => valid.push(ChangedBlob {
                path: head.path.as_str(),
                head: &head.bytes,
                changed: head.changed.clone(),
            }),
            Err(error) => not_analyzed.push(NotAnalyzed {
                path: head.path.to_string(),
                reason: NotAnalyzedReason::AnalysisFailed {
                    detail: error.to_string(),
                },
            }),
        }
    }
    Ok(index.rank_files(&valid)?)
}

/// Stored form of packed candidate spans, each with its head blob id.
fn selected_spans(spans: Vec<ContextSpan>, eligible: &[SnapshotFile]) -> Vec<SelectedSpan> {
    spans
        .into_iter()
        .filter_map(|span| {
            let SpanKind::Candidate { score, names } = span.kind else {
                return None;
            };
            Some(SelectedSpan {
                blob: blob_of(eligible, &span.path)?.clone(),
                path: span.path,
                start: span.lines.start,
                end: span.lines.end,
                score,
                names,
                text: span.text,
                tokens: span.tokens,
            })
        })
        .collect()
}

/// Stored form of one ceiling omission.
fn omitted_span(omission: Omission) -> OmittedSpan {
    OmittedSpan {
        path: omission.path,
        start: omission.lines.start,
        end: omission.lines.end,
        names: omission.names,
        reason: match omission.reason {
            OmissionReason::FileLimit => OmittedReason::FileLimit,
            OmissionReason::TokenLimit { tokens } => OmittedReason::TokenLimit { tokens },
        },
    }
}

/// Head blob id of eligible `path`.
fn blob_of<'e>(eligible: &'e [SnapshotFile], path: &RelativePath) -> Option<&'e ObjectSha> {
    eligible
        .binary_search_by(|file| file.path.cmp(path))
        .ok()
        .map(|index| &eligible[index].blob)
}

/// Blob entries of `listing` sharing a directory with a changed path.
/// Uncapped, so shared eligibility classifies every one before the scoped
/// file bound is spent on eligible files only.
fn near(listing: &TreeListing, changed: &BTreeSet<&RelativePath>) -> Vec<TreeEntry> {
    let directories: BTreeSet<&str> = changed.iter().map(|path| parent(path.as_str())).collect();
    listing
        .entries
        .iter()
        .filter(|entry| matches!(entry.mode, TreeEntryMode::File | TreeEntryMode::Executable))
        .filter(|entry| directories.contains(parent(entry.path.as_str())))
        .cloned()
        .collect()
}

/// Directory part of a repository-relative path; empty at the root.
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(directory, _)| directory)
}

/// Estimated tokens of `text`: one per four bytes, rounded up. An estimate
/// only; the downstream caller counts its real request.
pub(crate) fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

#[cfg(test)]
/// PR-context journeys over the routed fake GitHub, a cold-built snapshot,
/// and separate in-memory snapshot and context capabilities: private and
/// concurrent overlays checked against a from-scratch head parse, stale
/// bases, scoped fallback, moving PRs and bad patches, budget exhaustion,
/// shared read-back, and dropped publication.
mod tests {
    use super::*;
    use crate::context::CONTEXT_LIFETIME;
    use crate::eligibility::ExclusionReason;
    use crate::github::{GitHubError, Operation};
    use crate::snapshot::tests::{
        FORMAT, FakeRepo, File, USER, blob_sha, file, identity, sha, standard_archive,
        standard_files,
    };
    use crate::snapshot::{ColdBuild, ColdBuildLimits, ColdBuildOutcome, SkipReason};
    use crate::testing::{MemoryContextStore, MemorySnapshotStore, MemoryStorage};
    use serde_json::json;
    use std::time::Duration;

    /// Added by PR 1; declares `pr1Only`.
    const PR1_FILE: &[u8] = b"export function pr1Only(): number {\n  return 1;\n}\n";
    /// PR 1's handler: adds a `formatUser` import and calls `pr1Only`.
    const PR1_HANDLER: &[u8] = b"import { lookupUser } from \"./user\";\nimport { formatUser } from \"./format\";\n\nexport function handleRequest(id: string): string {\n  return formatUser(lookupUser(id)) + pr1Only();\n}\n";
    /// Patch of [`PR1_HANDLER`]: head lines 2 and 5 added.
    const PR1_PATCH: &str = "@@ -1,5 +1,6 @@\n import { lookupUser } from \"./user\";\n+import { formatUser } from \"./format\";\n \n export function handleRequest(id: string): string {\n-  return lookupUser(id).id;\n+  return formatUser(lookupUser(id)) + pr1Only();\n }";
    /// PR 2's handler: calls `saveUser` and PR 1's unmerged `pr1Only`.
    const PR2_HANDLER: &[u8] = b"import { lookupUser } from \"./user\";\n\nexport function handleRequest(id: string): string {\n  saveUser(lookupUser(id)); pr1Only();\n  return id;\n}\n";
    /// Patch of [`PR2_HANDLER`]: head lines 4–5 added.
    const PR2_PATCH: &str = "@@ -1,5 +1,6 @@\n import { lookupUser } from \"./user\";\n \n export function handleRequest(id: string): string {\n-  return lookupUser(id).id;\n+  saveUser(lookupUser(id)); pr1Only();\n+  return id;\n }";
    /// The default branch's `src/user.ts` after the snapshot: `saveUser`
    /// became `renameUser`.
    const USER_V2: &[u8] = b"export interface User {\n  id: string;\n}\n\nexport function lookupUser(id: string): User {\n  return { id };\n}\n\nexport function renameUser(user: User): void {\n  console.log(user.id);\n}\n";
    /// PR 3's handler: calls `renameUser` and the removed `saveUser`.
    const PR3_HANDLER: &[u8] = b"import { lookupUser } from \"./user\";\n\nexport function handleRequest(id: string): string {\n  renameUser(lookupUser(id)); saveUser(id);\n  return id;\n}\n";
    /// Patch of [`PR3_HANDLER`].
    const PR3_PATCH: &str = "@@ -1,5 +1,6 @@\n import { lookupUser } from \"./user\";\n \n export function handleRequest(id: string): string {\n-  return lookupUser(id).id;\n+  renameUser(lookupUser(id)); saveUser(id);\n+  return id;\n }";
    /// Outside `src/`; also declares `formatUser`.
    const FAR: &[u8] = b"export function formatUser(): string {\n  return \"far\";\n}\n";

    /// Eligible `.ts` paths of the standard tree an oracle parses.
    const ORACLE_PATHS: [&str; 5] = [
        "src/user.ts",
        "src/format.ts",
        "src/handler.ts",
        "src/omitted.ts",
        "src/substituted.ts",
    ];

    /// One PR file entry: path, status, head content, patch.
    type Change = (
        &'static str,
        &'static str,
        &'static [u8],
        Option<&'static str>,
    );

    /// The standard tree with `src/handler.ts` replaced and `extra` added.
    fn head_files(handler: &'static [u8], extra: Vec<File>) -> Vec<File> {
        let mut files: Vec<File> = standard_files()
            .into_iter()
            .filter(|f| f.path != "src/handler.ts")
            .collect();
        files.push(file("src/handler.ts", handler));
        files.extend(extra);
        files
    }

    /// PR JSON for `number` at `base`/`head`.
    fn pr_json(number: u64, base: &ObjectSha, head: &ObjectSha) -> Vec<u8> {
        json!({
            "number": number, "state": "open", "draft": false, "merged": false,
            "base": {"ref": "main", "sha": base.as_str(), "repo": {"id": 1296269}},
            "head": {"ref": "feature", "sha": head.as_str(), "repo": {"id": 1296269}},
        })
        .to_string()
        .into_bytes()
    }

    /// One PR files page body.
    fn files_json(changes: &[Change]) -> String {
        let files: Vec<_> = changes
            .iter()
            .map(|(path, status, content, patch)| {
                json!({
                    "filename": path, "status": status,
                    "sha": blob_sha(content).as_str(), "patch": patch,
                })
            })
            .collect();
        serde_json::Value::from(files).to_string()
    }

    /// Route PR `number` whose head commit `head` has `tree` listing
    /// `files`, with `changes` on one page.
    fn route_pr(
        repo: &FakeRepo,
        number: u64,
        (head, tree): (&ObjectSha, &ObjectSha),
        files: &[File],
        changes: &[Change],
    ) {
        let transport = &repo.transport;
        transport.route(
            &format!("/pulls/{number}"),
            200,
            pr_json(number, &repo.commit, head),
        );
        transport.json(
            &format!("/pulls/{number}/files?per_page=100&page=1"),
            &files_json(changes),
        );
        transport.json(
            &format!("/git/commits/{head}"),
            &json!({"sha": head.as_str(), "tree": {"sha": tree.as_str()}}).to_string(),
        );
        repo.route_tree(tree, files);
    }

    /// Cold-build the standard default branch into `storage`/`store`.
    async fn cold(repo: &FakeRepo, storage: &MemoryStorage, store: &MemorySnapshotStore) {
        let client = repo.client();
        let result = ColdBuild::new(&client, storage, store, ColdBuildLimits::default())
            .run(&identity())
            .await
            .expect("cold build");
        assert!(matches!(result.outcome, ColdBuildOutcome::Published(_)));
    }

    /// Separate snapshot and context capabilities.
    #[derive(Default)]
    struct Stores {
        snapshot_storage: MemoryStorage,
        snapshot_store: MemorySnapshotStore,
        context_storage: MemoryStorage,
        context_store: MemoryContextStore,
    }

    impl Stores {
        /// The PR facade over these stores and `client`.
        fn facade<'a>(
            &'a self,
            client: &'a GitHubClient,
            limits: PrContextLimits,
        ) -> PrContext<'a, MemoryStorage, MemorySnapshotStore, MemoryContextStore> {
            PrContext::new(
                client,
                &self.snapshot_storage,
                &self.snapshot_store,
                &self.context_storage,
                &self.context_store,
                limits,
            )
        }

        /// The read facade over the context stores.
        fn contexts(&self) -> Contexts<'_, MemoryStorage, MemoryContextStore> {
            Contexts::new(&self.context_storage, &self.context_store)
        }
    }

    /// `(path, start, end, text)` of every selected span.
    fn spans(context: &ReviewContext) -> Vec<(String, u32, u32, String)> {
        context
            .spans
            .iter()
            .map(|s| (s.path.to_string(), s.start, s.end, s.text.clone()))
            .collect()
    }

    /// From-scratch oracle: parse every eligible head file, rank the
    /// changed heads, and pack with the facade's limits and estimator.
    fn oracle(
        files: &[File],
        changed: &[(&str, &[u8], Vec<LineRange>)],
    ) -> Vec<(String, u32, u32, String)> {
        let mut index = SymbolicContext::new();
        let mut blobs = BTreeMap::new();
        for f in files
            .iter()
            .filter(|f| ORACLE_PATHS.contains(&f.path) || f.path.starts_with("src/pr"))
        {
            index.upsert(f.path, f.content).expect("upsert");
            blobs.insert(RelativePath::new(f.path).expect("path"), f.content);
        }
        let heads: Vec<ChangedBlob<'_>> = changed
            .iter()
            .map(|(path, head, lines)| ChangedBlob {
                path,
                head,
                changed: lines.clone(),
            })
            .collect();
        let ranked = index.rank_files(&heads).expect("rank");
        let limits = PrContextLimits::default();
        let context = build_candidate_context(
            &ranked,
            &blobs,
            ContextLimits {
                max_files: limits.max_candidate_files,
                max_tokens: 1_000_000,
            },
            estimate_tokens,
        )
        .expect("pack");
        context
            .spans
            .into_iter()
            .map(|s| (s.path.to_string(), s.lines.start, s.lines.end, s.text))
            .collect()
    }

    /// Scenarios 5–6 and 15: two concurrent PRs editing the same path each
    /// see only their own head, equal their own from-scratch oracle, fetch
    /// no archive, and publish one context each that reads back identically
    /// by id without provider calls until it expires.
    #[tokio::test]
    async fn concurrent_prs_see_only_their_own_heads() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let stores = Stores::default();
        cold(&repo, &stores.snapshot_storage, &stores.snapshot_store).await;
        let pr1_files = head_files(PR1_HANDLER, vec![file("src/pr1.ts", PR1_FILE)]);
        let pr2_files = head_files(PR2_HANDLER, Vec::new());
        route_pr(
            &repo,
            1,
            (&sha('1'), &sha('2')),
            &pr1_files,
            &[
                ("src/handler.ts", "modified", PR1_HANDLER, Some(PR1_PATCH)),
                (
                    "src/pr1.ts",
                    "added",
                    PR1_FILE,
                    Some("@@ -0,0 +1,3 @@\n+export function pr1Only(): number {\n+  return 1;\n+}"),
                ),
            ],
        );
        route_pr(
            &repo,
            2,
            (&sha('3'), &sha('4')),
            &pr2_files,
            &[("src/handler.ts", "modified", PR2_HANDLER, Some(PR2_PATCH))],
        );
        let client = repo.client();
        let facade = stores.facade(&client, PrContextLimits::default());
        let caller = identity();
        let (one, two) = tokio::join!(facade.run(&caller, 1), facade.run(&caller, 2));
        let (one, two) = (one.expect("pr 1"), two.expect("pr 2"));
        assert_eq!(client.calls().get(Operation::Archive), 0);

        let range = |start, end| LineRange { start, end };
        assert_eq!(
            spans(&one.context),
            oracle(
                &pr1_files,
                &[
                    (
                        "src/handler.ts",
                        PR1_HANDLER,
                        vec![range(2, 2), range(5, 5)]
                    ),
                    ("src/pr1.ts", PR1_FILE, vec![range(1, 3)]),
                ]
            )
        );
        assert_eq!(
            spans(&two.context),
            oracle(
                &pr2_files,
                &[("src/handler.ts", PR2_HANDLER, vec![range(4, 5)])]
            )
        );
        let paths = |c: &ReviewContext| -> BTreeSet<String> {
            c.spans.iter().map(|s| s.path.to_string()).collect()
        };
        assert_eq!(
            paths(&one.context),
            BTreeSet::from(["src/format.ts".to_owned(), "src/user.ts".to_owned()])
        );
        assert_eq!(
            paths(&two.context),
            BTreeSet::from(["src/user.ts".to_owned()])
        );
        let user = &two.context.spans;
        assert!(
            user.iter()
                .all(|s| !s.names.contains(&"pr1Only".to_owned()))
        );
        for result in [&one, &two] {
            let context = &result.context;
            assert!(context.coverage.is_full(), "{:?}", context.coverage);
            assert_eq!(context.base, repo.commit);
            assert!(
                context.not_analyzed.is_empty(),
                "{:?}",
                context.not_analyzed
            );
            assert!(!context.budget.exhausted);
        }
        assert_eq!(
            (one.context.head.clone(), two.context.head.clone()),
            (sha('1'), sha('3'))
        );

        let before = client.calls().snapshot();
        let contexts = stores.contexts();
        for _ in 0..2 {
            assert_eq!(
                contexts.read(&identity(), &one.record.id).await,
                Ok(one.context.clone())
            );
        }
        assert_eq!(client.calls().snapshot(), before);
        assert_eq!(
            one.lifetime.expires_at,
            one.lifetime.published_at + CONTEXT_LIFETIME
        );
        stores.context_store.clock.advance(CONTEXT_LIFETIME);
        assert_eq!(
            contexts.read(&identity(), &one.record.id).await,
            Err(ContextError::Expired {
                id: one.record.id.clone()
            })
        );
        assert_eq!(stores.context_storage.keys().len(), 2);
        assert_eq!(stores.snapshot_storage.keys().len(), 1);
    }

    /// Scenario 7: a PR based on a default branch that moved past the
    /// snapshot overlays the drift, so the renamed declaration ranks and
    /// the removed one does not; with no overlay budget the drifted file is
    /// reported unfetched, its stale facts are gone, and coverage is not
    /// full.
    #[tokio::test]
    async fn stale_base_overlays_drift_or_reports_incomplete() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let stores = Stores::default();
        cold(&repo, &stores.snapshot_storage, &stores.snapshot_store).await;
        let mut files = head_files(PR3_HANDLER, Vec::new());
        files.retain(|f| f.path != "src/user.ts");
        files.push(file("src/user.ts", USER_V2));
        route_pr(
            &repo,
            3,
            (&sha('5'), &sha('6')),
            &files,
            &[("src/handler.ts", "modified", PR3_HANDLER, Some(PR3_PATCH))],
        );
        let client = repo.client();
        let full = stores
            .facade(&client, PrContextLimits::default())
            .run(&identity(), 3)
            .await
            .expect("full overlay");
        assert!(full.context.coverage.is_full());
        let names: BTreeSet<&str> = full
            .context
            .spans
            .iter()
            .filter(|s| s.path.as_str() == "src/user.ts")
            .flat_map(|s| s.names.iter().map(String::as_str))
            .collect();
        assert_eq!(names, BTreeSet::from(["lookupUser", "renameUser"]));
        assert!(full.context.spans.iter().all(|s| s.blob != blob_sha(USER)));

        let limits = PrContextLimits {
            max_overlay_changes: 0,
            ..PrContextLimits::default()
        };
        let partial = stores
            .facade(&client, limits)
            .run(&identity(), 3)
            .await
            .expect("partial overlay");
        let coverage = &partial.context.coverage;
        assert!(!coverage.overlay_complete && !coverage.is_full());
        assert_eq!(coverage.scope, Scope::Repository);
        assert!(
            coverage
                .files
                .skipped
                .iter()
                .any(|f| f.path.as_str() == "src/user.ts")
        );
        assert!(
            partial
                .context
                .spans
                .iter()
                .all(|s| s.path.as_str() != "src/user.ts")
        );
    }

    /// Scenario 10: after a cold build fails on a truncated tree no pointer
    /// exists, so the PR indexes only files near its changed paths, reports
    /// scoped coverage, and ignores a matching file elsewhere.
    #[tokio::test]
    async fn failed_cold_build_falls_back_to_scoped_context() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        repo.truncate_tree();
        let stores = Stores::default();
        let client = repo.client();
        let result = ColdBuild::new(
            &client,
            &stores.snapshot_storage,
            &stores.snapshot_store,
            ColdBuildLimits::default(),
        )
        .run(&identity())
        .await
        .expect("cold build");
        assert!(matches!(result.outcome, ColdBuildOutcome::Incomplete(_)));
        let files = head_files(PR1_HANDLER, vec![file("lib/far.ts", FAR)]);
        route_pr(
            &repo,
            1,
            (&sha('1'), &sha('2')),
            &files,
            &[("src/handler.ts", "modified", PR1_HANDLER, Some(PR1_PATCH))],
        );
        let scoped = stores
            .facade(&client, PrContextLimits::default())
            .run(&identity(), 1)
            .await
            .expect("scoped");
        let coverage = &scoped.context.coverage;
        assert_eq!(
            (
                coverage.scope,
                coverage.snapshot_complete,
                coverage.snapshot_tree.clone()
            ),
            (Scope::Scoped, false, None)
        );
        assert!(!coverage.is_full());
        assert_eq!(coverage.files.eligible, 6, "src/ files only");
        let paths: BTreeSet<&str> = scoped
            .context
            .spans
            .iter()
            .map(|s| s.path.as_str())
            .collect();
        assert_eq!(paths, BTreeSet::from(["src/format.ts", "src/user.ts"]));
        assert!(stores.snapshot_storage.keys().is_empty());
    }

    /// Without a snapshot, the scoped bound is spent on eligible files:
    /// unsupported, generated, vendored, minified, and oversize neighbors
    /// ahead of `src/user.ts` in the listing are excluded with reasons
    /// instead of taking its slot, and the eligible neighbor past the bound
    /// is reported as not fetched.
    #[tokio::test]
    async fn scoped_bound_skips_excluded_neighbors() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let files = vec![
            file(
                ".gitattributes",
                b"src/a_gen.ts linguist-generated\nsrc/a_vendor.ts linguist-vendored\n",
            ),
            file("src/a.txt", b"notes\n"),
            file("src/a_gen.ts", FORMAT),
            file("src/a_vendor.ts", FORMAT),
            file("src/b.min.js", b"var a=1;\n"),
            File {
                path: "src/c.ts",
                mode: "100644",
                content: FORMAT,
                size: Some(MAX_ELIGIBLE_FILE_BYTES + 1),
            },
            file("src/handler.ts", PR1_HANDLER),
            file("src/user.ts", USER),
            file("src/zz.ts", FORMAT),
        ];
        route_pr(
            &repo,
            1,
            (&sha('1'), &sha('2')),
            &files,
            &[("src/handler.ts", "modified", PR1_HANDLER, Some(PR1_PATCH))],
        );
        let stores = Stores::default();
        let client = repo.client();
        let limits = PrContextLimits {
            max_scoped_files: 2,
            ..PrContextLimits::default()
        };
        let scoped = stores
            .facade(&client, limits)
            .run(&identity(), 1)
            .await
            .expect("scoped");
        let coverage = &scoped.context.coverage.files;
        assert_eq!((coverage.eligible, coverage.indexed), (3, 2));
        assert_eq!(
            coverage
                .skipped
                .iter()
                .map(|file| (file.path.as_str(), &file.reason))
                .collect::<Vec<_>>(),
            [("src/zz.ts", &SkipReason::NotFetched)]
        );
        assert_eq!(
            coverage.excluded,
            BTreeMap::from([
                (ExclusionReason::Unsupported, 1),
                (ExclusionReason::MinifiedOutput, 1),
                (ExclusionReason::GeneratedAttribute, 1),
                (ExclusionReason::VendoredAttribute, 1),
                (ExclusionReason::Oversize, 1),
            ])
        );
        assert!(
            scoped
                .context
                .spans
                .iter()
                .any(|span| span.path.as_str() == "src/user.ts"),
            "{:?}",
            scoped.context
        );
    }

    /// A PR-file entry the provider listing rejected makes PR-file coverage
    /// partial with its reason, while the valid file still yields context.
    #[tokio::test]
    async fn rejected_pr_file_is_not_full_coverage() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let stores = Stores::default();
        cold(&repo, &stores.snapshot_storage, &stores.snapshot_store).await;
        route_pr(
            &repo,
            1,
            (&sha('1'), &sha('2')),
            &head_files(PR1_HANDLER, Vec::new()),
            &[
                ("src/handler.ts", "modified", PR1_HANDLER, Some(PR1_PATCH)),
                ("src/other.ts", "bogus", FORMAT, None),
            ],
        );
        let client = repo.client();
        let result = stores
            .facade(&client, PrContextLimits::default())
            .run(&identity(), 1)
            .await
            .expect("context");
        let coverage = &result.context.coverage;
        assert_eq!(coverage.pr_files_rejected, 1);
        assert!(coverage.overlay_complete && !coverage.is_full());
        assert!(matches!(
            result.context.not_analyzed.as_slice(),
            [NotAnalyzed { path, reason: NotAnalyzedReason::Rejected { .. } }]
                if path == "src/other.ts"
        ));
        assert!(!result.context.spans.is_empty());
    }

    /// A head tree with an unknown-mode entry is a broken listing: the run
    /// fails with a provider error and publishes nothing.
    #[tokio::test]
    async fn broken_head_tree_entry_fails_the_run() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let stores = Stores::default();
        cold(&repo, &stores.snapshot_storage, &stores.snapshot_store).await;
        let odd = File {
            path: "src/odd.ts",
            mode: "100600",
            content: FORMAT,
            size: None,
        };
        route_pr(
            &repo,
            1,
            (&sha('1'), &sha('2')),
            &head_files(PR1_HANDLER, vec![odd]),
            &[("src/handler.ts", "modified", PR1_HANDLER, Some(PR1_PATCH))],
        );
        let client = repo.client();
        let error = stores
            .facade(&client, PrContextLimits::default())
            .run(&identity(), 1)
            .await
            .expect_err("broken listing");
        assert!(matches!(
            error,
            ContextError::GitHub(GitHubError::MalformedResponse { .. })
        ));
        assert!(stores.context_storage.keys().is_empty());
    }

    /// A head-tree entry at a non-canonical path is an unsupported
    /// exclusion: the overlay stays complete and coverage full.
    #[tokio::test]
    async fn non_canonical_head_tree_path_keeps_full_coverage() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let stores = Stores::default();
        cold(&repo, &stores.snapshot_storage, &stores.snapshot_store).await;
        route_pr(
            &repo,
            1,
            (&sha('1'), &sha('2')),
            &head_files(PR1_HANDLER, vec![file("src/a\\\\b.ts", FORMAT)]),
            &[("src/handler.ts", "modified", PR1_HANDLER, Some(PR1_PATCH))],
        );
        let client = repo.client();
        let result = stores
            .facade(&client, PrContextLimits::default())
            .run(&identity(), 1)
            .await
            .expect("context");
        let coverage = &result.context.coverage;
        assert!(
            coverage.overlay_complete && coverage.is_full(),
            "{coverage:?}"
        );
        assert_eq!(
            coverage.files.excluded.get(&ExclusionReason::Unsupported),
            Some(&3),
            "README.md, .gitattributes, and the non-canonical path"
        );
        assert!(!result.context.spans.is_empty());
    }

    /// Without a snapshot, changed heads share the scoped index ceiling
    /// with fallback: with one slot and two valid changed heads both are
    /// fetched and ranked, only the first is indexed, the second is
    /// reported not fetched, and fallback fetches nothing more.
    #[tokio::test]
    async fn changed_heads_share_the_scoped_index_ceiling() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        route_pr(
            &repo,
            1,
            (&sha('1'), &sha('2')),
            &head_files(PR1_HANDLER, vec![file("src/pr1.ts", PR1_FILE)]),
            &[
                ("src/handler.ts", "modified", PR1_HANDLER, Some(PR1_PATCH)),
                (
                    "src/pr1.ts",
                    "added",
                    PR1_FILE,
                    Some("@@ -0,0 +1,3 @@\n+export function pr1Only(): number {\n+  return 1;\n+}"),
                ),
            ],
        );
        let stores = Stores::default();
        let client = repo.client();
        let limits = PrContextLimits {
            max_scoped_files: 1,
            ..PrContextLimits::default()
        };
        let scoped = stores
            .facade(&client, limits)
            .run(&identity(), 1)
            .await
            .expect("scoped");
        let context = &scoped.context;
        let files = &context.coverage.files;
        assert_eq!(context.coverage.scope, Scope::Scoped);
        assert_eq!((files.eligible, files.indexed), (7, 1));
        assert!(
            files
                .skipped
                .iter()
                .any(|f| f.path.as_str() == "src/pr1.ts" && f.reason == SkipReason::NotFetched),
            "{files:?}"
        );
        assert!(!context.coverage.is_full());
        assert!(
            context.not_analyzed.is_empty(),
            "{:?}",
            context.not_analyzed
        );
        assert_eq!(
            client.calls().get(Operation::Blob),
            3,
            "root attributes and the two heads"
        );
    }

    /// A rejected PR-file record with a canonical filename is still a
    /// changed path: it is never selected as secondary context, and without
    /// a snapshot it seeds nearby selection. A non-canonical filename stays
    /// a diagnostic only.
    #[tokio::test]
    async fn rejected_changed_path_is_never_secondary() {
        let changes: [Change; 4] = [
            ("src/handler.ts", "modified", PR1_HANDLER, Some(PR1_PATCH)),
            ("src/user.ts", "bogus", USER, None),
            ("lib/gone.ts", "bogus", FAR, None),
            ("src/../x.ts", "modified", FAR, None),
        ];
        for warm in [true, false] {
            let repo = FakeRepo::new(&standard_files(), &standard_archive());
            let stores = Stores::default();
            if warm {
                cold(&repo, &stores.snapshot_storage, &stores.snapshot_store).await;
            }
            let files = head_files(PR1_HANDLER, vec![file("lib/far.ts", FAR)]);
            route_pr(&repo, 1, (&sha('1'), &sha('2')), &files, &changes);
            let client = repo.client();
            let result = stores
                .facade(&client, PrContextLimits::default())
                .run(&identity(), 1)
                .await
                .expect("context");
            let context = &result.context;
            let paths: BTreeSet<&str> = context.spans.iter().map(|s| s.path.as_str()).collect();
            assert!(paths.contains("src/format.ts"), "{warm}: {paths:?}");
            assert!(!paths.contains("src/user.ts"), "{warm}: {paths:?}");
            assert!(!paths.contains("src/handler.ts"), "{warm}: {paths:?}");
            let rejected: BTreeSet<&str> = context
                .not_analyzed
                .iter()
                .filter(|n| matches!(n.reason, NotAnalyzedReason::Rejected { .. }))
                .map(|n| n.path.as_str())
                .collect();
            assert_eq!(
                rejected,
                BTreeSet::from(["src/user.ts", "lib/gone.ts", "src/../x.ts"])
            );
            assert!(!context.coverage.is_full());
            if !warm {
                assert_eq!(context.coverage.files.eligible, 7, "src/ and lib/");
            }
        }
    }

    /// Scenario 18: a 400-file PR is read across four pages; its head moves
    /// during the first listing, so every page is re-read under the new
    /// head. Ineligible, patchless, and malformed files are listed with
    /// reasons while the valid file is still analyzed.
    #[tokio::test]
    async fn moving_pr_retries_and_bad_patches_stay_file_local() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let stores = Stores::default();
        cold(&repo, &stores.snapshot_storage, &stores.snapshot_store).await;
        let (head, tree) = (sha('1'), sha('2'));
        let files = head_files(PR1_HANDLER, vec![file("src/pr1.ts", PR1_FILE)]);
        route_pr(&repo, 9, (&head, &tree), &files, &[]);
        let transport = &repo.transport;
        transport.respond(
            "/pulls/9",
            vec![
                (200, Vec::new(), pr_json(9, &repo.commit, &sha('0'))),
                (200, Vec::new(), pr_json(9, &repo.commit, &head)),
            ],
        );
        let mut changes: Vec<Change> = vec![
            ("src/handler.ts", "modified", PR1_HANDLER, Some(PR1_PATCH)),
            ("src/format.ts", "modified", FORMAT, None),
            ("src/user.ts", "modified", USER, Some("not a patch")),
        ];
        let docs: Vec<&'static str> = (0..397)
            .map(|i| &*Box::leak(format!("docs/f{i:03}.md").into_boxed_str()))
            .collect();
        changes.extend(
            docs.iter()
                .map(|path| (*path, "modified", &b"x"[..], Some("@@ -1 +1 @@\n-a\n+b"))),
        );
        for (page, chunk) in changes.chunks(100).enumerate() {
            let headers = if page < 3 {
                vec![("link".to_owned(), "<x>; rel=\"next\"".to_owned())]
            } else {
                Vec::new()
            };
            transport.respond(
                &format!("/pulls/9/files?per_page=100&page={}", page + 1),
                vec![(200, headers, files_json(chunk).into_bytes())],
            );
        }
        let client = repo.client();
        let result = stores
            .facade(&client, PrContextLimits::default())
            .run(&identity(), 9)
            .await
            .expect("pr");
        let calls = client.calls();
        assert_eq!(
            (
                calls.get(Operation::PullRequest),
                calls.get(Operation::PullRequestFiles)
            ),
            (4, 8)
        );
        let context = &result.context;
        assert_eq!(context.head, head);
        let reason_of = |path: &str| {
            context
                .not_analyzed
                .iter()
                .find(|n| n.path == path)
                .map(|n| n.reason.clone())
        };
        assert_eq!(
            reason_of("src/format.ts"),
            Some(NotAnalyzedReason::MissingPatch)
        );
        assert!(matches!(
            reason_of("src/user.ts"),
            Some(NotAnalyzedReason::MalformedPatch { .. })
        ));
        assert_eq!(
            reason_of("docs/f000.md"),
            Some(NotAnalyzedReason::NotEligible)
        );
        assert_eq!(context.not_analyzed.len(), 399);
        let paths: BTreeSet<&str> = context.spans.iter().map(|s| s.path.as_str()).collect();
        assert_eq!(
            paths,
            BTreeSet::from(["src/pr1.ts"]),
            "changed paths never selected"
        );
    }

    /// A PR whose base or head never settles is a retry state, not a context.
    #[tokio::test]
    async fn perpetually_moving_pr_publishes_nothing() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let stores = Stores::default();
        route_pr(&repo, 9, (&sha('1'), &sha('2')), &standard_files(), &[]);
        repo.transport.respond(
            "/pulls/9",
            (0..7)
                .map(|i| {
                    (
                        200,
                        Vec::new(),
                        pr_json(9, &repo.commit, &sha(char::from(b'0' + i))),
                    )
                })
                .collect(),
        );
        let client = repo.client();
        let result = stores
            .facade(&client, PrContextLimits::default())
            .run(&identity(), 9)
            .await;
        assert_eq!(result, Err(ContextError::PullRequestMoved { attempts: 3 }));
        assert!(stores.context_store.ids().is_empty());
    }

    /// Scenario 19: a diff and prompt reserve filling the allocation leave
    /// an explicit exhausted budget, an empty candidate context, and no
    /// candidate blob fetches.
    #[tokio::test]
    async fn exhausted_budget_packs_nothing_and_says_so() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let stores = Stores::default();
        cold(&repo, &stores.snapshot_storage, &stores.snapshot_store).await;
        let files = head_files(PR1_HANDLER, Vec::new());
        route_pr(
            &repo,
            1,
            (&sha('1'), &sha('2')),
            &files,
            &[("src/handler.ts", "modified", PR1_HANDLER, Some(PR1_PATCH))],
        );
        let client = repo.client();
        let limits = PrContextLimits {
            review_tokens: 100,
            prompt_reserve: 100 - estimate_tokens(PR1_PATCH),
            ..PrContextLimits::default()
        };
        let result = stores
            .facade(&client, limits)
            .run(&identity(), 1)
            .await
            .expect("pr");
        let budget = &result.context.budget;
        assert!(budget.exhausted);
        assert_eq!(
            (
                budget.diff_tokens,
                budget.context_allowance,
                budget.selected_tokens
            ),
            (estimate_tokens(PR1_PATCH), 0, 0)
        );
        assert!(result.context.spans.is_empty() && result.context.omissions.is_empty());
        assert_eq!(
            client.calls().get(Operation::Blob),
            2,
            "attributes and the changed head only"
        );
    }

    /// Dropping the PR future after the record upsert leaves a record with
    /// no object, which reads as not found; after the object create both
    /// exist. Either way every object has a record and the retry publishes
    /// a new id that reads back.
    #[tokio::test(start_paused = true)]
    async fn dropped_publication_leaves_no_readable_half_context() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let files = head_files(PR1_HANDLER, Vec::new());
        route_pr(
            &repo,
            1,
            (&sha('1'), &sha('2')),
            &files,
            &[("src/handler.ts", "modified", PR1_HANDLER, Some(PR1_PATCH))],
        );
        type Stall = fn(&Stores);
        let steps: [(Stall, bool); 2] = [
            (|stores| stores.context_store.stall_after_upsert(), false),
            (|stores| stores.context_storage.stall_after_create(), true),
        ];
        for (stall, object_written) in steps {
            let stores = Stores::default();
            cold(&repo, &stores.snapshot_storage, &stores.snapshot_store).await;
            let client = repo.client();
            let facade = stores.facade(&client, PrContextLimits::default());
            stall(&stores);
            let dropped =
                tokio::time::timeout(Duration::from_secs(60), facade.run(&identity(), 1)).await;
            assert!(dropped.is_err(), "the stalled publication must be dropped");
            let ids = stores.context_store.ids();
            assert_eq!(ids.len(), 1);
            let id = &ids[0].1;
            assert_eq!(
                stores.context_storage.keys().len(),
                usize::from(object_written)
            );
            let read = stores.contexts().read(&identity(), id).await;
            if !object_written {
                assert_eq!(read, Err(ContextError::NotFound { id: id.clone() }));
            }

            let retry = facade.run(&identity(), 1).await.expect("retry");
            assert_ne!(&retry.record.id, id);
            assert_eq!(stores.context_store.ids().len(), 2);
            assert_eq!(
                stores.context_storage.keys().len(),
                1 + usize::from(object_written)
            );
            assert_eq!(
                stores.contexts().read(&identity(), &retry.record.id).await,
                Ok(retry.context)
            );
        }
    }
}
