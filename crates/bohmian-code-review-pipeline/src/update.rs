//! Incremental default-branch snapshot updates and review-time staleness.
//!
//! [`Update::run`] pins the default branch and takes the changed files
//! between the pointer's commit and the head from the compare API, without
//! listing the tree. Each changed path is classified from a non-recursive
//! listing of its parent directory at the head; the prior snapshot's facts
//! are carried for every other path, and changed eligible blobs are fetched
//! as verified blobs. The result is published through the cold build's
//! record-upsert, create-if-absent, pointer compare-and-swap path. A root
//! `.gitattributes` change, a head that does not descend from the pointer's
//! commit, or more changed files than compare lists or
//! [`ColdBuildLimits::max_changed_blobs`] allows rebuilds from the archive.
//! An incomplete attempt writes nothing, so the prior pointer and the
//! immutable prior object stay valid for reviews already using them.
//!
//! Every update, incremental or rebuilt, extends the prior snapshot's
//! co-change history with the PRs merged since its commit, reading one
//! listing per merged PR; at the same tree the pointer and its history are
//! reused unchanged.

use crate::github::{BranchHead, GitHubClient, GitHubError, TreeEntry, TreeEntryMode, TreeListing};
use crate::identity::{Identity, ObjectSha};
use crate::persistence::{SnapshotPointer, SnapshotStore};
use crate::snapshot::{
    ColdBuild, ColdBuildError, ColdBuildLimits, ColdBuildResult, Ingest, Snapshots,
    root_attributes, snapshot_scope,
};
use crate::storage::{ObjectStorage, StorageError};
use bohmian_code_review::RelativePath;
use std::collections::BTreeMap;

/// Whether the current pointer covers the default branch head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Freshness {
    /// The pointer names the head's tree.
    Current(SnapshotPointer),
    /// The head moved past the pointer, or no pointer exists; the caller may
    /// run [`Update::run`] while a review keeps using the old snapshot.
    Stale {
        /// Current pointer, if any.
        pointer: Option<SnapshotPointer>,
        /// Branch head it does not cover.
        head: BranchHead,
    },
}

/// The update facade over injected provider, storage, and snapshot store.
pub struct Update<'a, O, S> {
    cold: ColdBuild<'a, O, S>,
}

impl<'a, O: ObjectStorage, S: SnapshotStore> Update<'a, O, S> {
    /// Facade over the caller's capabilities and ceilings.
    pub fn new(
        client: &'a GitHubClient,
        storage: &'a O,
        store: &'a S,
        limits: ColdBuildLimits,
    ) -> Update<'a, O, S> {
        Update {
            cold: ColdBuild::new(client, storage, store, limits),
        }
    }

    /// Advance `identity`'s pointer to a complete snapshot of the current
    /// default-branch tree.
    ///
    /// Without a pointer under the current versions (first build, or an
    /// extraction or format version change) or with its object missing,
    /// this is a cold build; at the same tree the pointer is validated and
    /// reused with its history unchanged. Otherwise the pointer's
    /// co-change history is extended with PRs merged since its commit, and
    /// only the paths compare reports changed are listed and fetched,
    /// unless a REQ-007 trigger rebuilds from the archive. The caller
    /// bounds the call by dropping its future; a drop at any `.await` leaves the
    /// pointer on a complete snapshot and every written object recorded,
    /// and a retry converges. No persistence transaction spans provider or
    /// storage IO.
    ///
    /// # Errors
    ///
    /// As [`ColdBuild::run`], including [`ColdBuildError::History`] from
    /// the history read, plus the compare and directory provider errors, and `MalformedResponse` when a changed path is missing from
    /// the tree compare says holds it. `PointerConflict` or
    /// `Persistence(Collecting)` is a retry state; a changed blob that
    /// cannot be fetched is not an error but an `Incomplete` outcome
    /// leaving the pointer in place.
    pub async fn run(&self, identity: &Identity) -> Result<ColdBuildResult, ColdBuildError> {
        let cold = &self.cold;
        let scope = snapshot_scope(identity, cold.limits.max_file_bytes);
        let current = cold.store.snapshot_pointer(&scope).await?;
        let head = cold.pin(identity).await?;
        let Some(pointer) = current.as_ref().filter(|pointer| pointer.tree != head.tree) else {
            return cold.build(&scope, current.as_ref(), &head, None).await;
        };
        let prior = match Snapshots::new(cold.storage, cold.store)
            .read(&scope, pointer)
            .await
        {
            Ok(prior) => prior,
            Err(ColdBuildError::Storage(StorageError::NotFound { .. })) => {
                return cold.build(&scope, current.as_ref(), &head, None).await;
            }
            Err(error) => return Err(error),
        };
        let changes = cold
            .client
            .compare(&identity.repository, &pointer.commit, &head.commit)
            .await?;
        let attributes_changed = changes
            .removed
            .iter()
            .chain(&changes.added)
            .any(|path| path.as_str() == ".gitattributes");
        if !changes.ahead
            || changes.truncated
            || changes.files > cold.limits.max_changed_blobs
            || attributes_changed
        {
            return cold
                .build(&scope, current.as_ref(), &head, Some(&prior.history))
                .await;
        }
        let history = cold.history(identity, &head, Some(&prior.history)).await?;

        let mut head_tree = Directories::new(cold.client, identity, &head.tree);
        let added = head_tree.entries(&changes.added).await?;
        let root = head_tree
            .listing("")
            .await?
            .map_or(&[][..], |root| &root.entries);
        let attributes =
            root_attributes(cold.client, identity, root, cold.limits.max_file_bytes).await?;
        if head_tree.truncated {
            return Ok(ColdBuildResult::truncated(&head));
        }

        let mut ingest = Ingest::new(
            &added,
            changes.invalid_added,
            &attributes,
            cold.limits.max_file_bytes,
        );
        ingest.carry(prior, &changes.removed);
        ingest
            .fallback(cold.client, identity, cold.limits.max_changed_blobs)
            .await;
        cold.complete(&scope, current.as_ref(), &head, ingest, false, history)
            .await
    }

    /// Compare the current pointer with the default branch head without
    /// reading snapshots or waiting on any update, so a review can proceed
    /// on its loaded snapshot and report staleness.
    ///
    /// # Errors
    ///
    /// Returns the pointer read or the repository or branch provider error.
    pub async fn freshness(&self, identity: &Identity) -> Result<Freshness, ColdBuildError> {
        let pointer = self
            .cold
            .store
            .snapshot_pointer(&snapshot_scope(identity, self.cold.limits.max_file_bytes))
            .await?;
        let head = self.cold.pin(identity).await?;
        Ok(match pointer {
            Some(pointer) if pointer.tree == head.tree => Freshness::Current(pointer),
            pointer => Freshness::Stale { pointer, head },
        })
    }
}

/// Non-recursive listings of one tree's directories, each listed at most
/// once and reached from the root through verified subtree ids.
struct Directories<'a> {
    client: &'a GitHubClient,
    identity: &'a Identity,
    root: &'a ObjectSha,
    /// Listings by path prefix: empty for the root, else ending in `/`.
    listed: BTreeMap<String, TreeListing>,
    /// Some listing was truncated, so an entry may be missing from it.
    truncated: bool,
}

impl<'a> Directories<'a> {
    /// Nothing listed yet under `root`.
    fn new(
        client: &'a GitHubClient,
        identity: &'a Identity,
        root: &'a ObjectSha,
    ) -> Directories<'a> {
        Directories {
            client,
            identity,
            root,
            listed: BTreeMap::new(),
            truncated: false,
        }
    }

    /// The listing of directory `dir` (empty for the root), listing each
    /// ancestor on the way, or `None` when some ancestor lacks it.
    ///
    /// # Errors
    ///
    /// The directory provider error.
    async fn listing(&mut self, dir: &str) -> Result<Option<&TreeListing>, GitHubError> {
        let mut tree = self.root.clone();
        let mut prefix = String::new();
        let mut names = dir.split('/').filter(|name| !name.is_empty());
        loop {
            if !self.listed.contains_key(&prefix) {
                let listing = self
                    .client
                    .directory(&self.identity.repository, &tree, &prefix)
                    .await?;
                self.truncated |= listing.truncated;
                self.listed.insert(prefix.clone(), listing);
            }
            let Some(name) = names.next() else {
                return Ok(self.listed.get(&prefix));
            };
            let child = format!("{prefix}{name}");
            let Some(subtree) = self.listed[&prefix].entries.iter().find(|entry| {
                entry.mode == TreeEntryMode::Directory && entry.path.as_str() == child
            }) else {
                return Ok(None);
            };
            tree = subtree.sha.clone();
            prefix = child + "/";
        }
    }

    /// The tree entry of every path in `paths`, from its parent's listing.
    ///
    /// # Errors
    ///
    /// The directory provider error, or `MalformedResponse` when a path is
    /// missing from a complete listing of its parent.
    async fn entries(&mut self, paths: &[RelativePath]) -> Result<Vec<TreeEntry>, GitHubError> {
        let mut entries = Vec::with_capacity(paths.len());
        for path in paths {
            let dir = path.as_str().rsplit_once('/').map_or("", |(dir, _)| dir);
            let entry = self
                .listing(dir)
                .await?
                .and_then(|listing| listing.entries.iter().find(|entry| entry.path == *path))
                .cloned();
            match entry {
                Some(entry) => entries.push(entry),
                None if self.truncated => {}
                None => {
                    return Err(GitHubError::MalformedResponse {
                        resource: format!("tree {}", self.root),
                        detail: format!("compared path {path} is not in the tree"),
                    });
                }
            }
        }
        Ok(entries)
    }
}

#[cfg(test)]
/// Update journeys over the cold build's routed fake GitHub and in-memory
/// storage: a from-scratch oracle, failed and dropped attempts, moving and
/// racing heads, and a version change.
mod tests {
    use super::*;
    use crate::eligibility::ExclusionReason;
    use crate::github::{GitHubError, Operation};
    use crate::persistence::SnapshotScope;
    use crate::snapshot::tests::{
        FakeRepo, File, HANDLER, LFS, OMITTED, bad_sha, blob_sha, file, identity, scope, sha,
        standard_archive, standard_files,
    };
    use crate::snapshot::{ColdBuildOutcome, IncompleteReason, SkipReason};
    use crate::storage::ObjectKey;
    use crate::testing::{MemorySnapshotStore, MemoryStorage};
    use bohmian_code_review::EXTRACTION_VERSION;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    /// `src/user.ts` after the push: `saveUser` removed, `renameUser` added.
    const USER_V2: &[u8] = b"export function lookupUser(id: string): string {\n  return id;\n}\n\nexport function renameUser(id: string): string {\n  return id;\n}\n";
    /// Declares `extraFn`; added by a later push.
    const EXTRA: &[u8] = b"export function extraFn(): number {\n  return 3;\n}\n";

    /// The push after the standard tree: `src/user.ts` changed,
    /// `src/format.ts` deleted, `src/handler.ts` renamed to
    /// `src/api/handler.ts`, `src/substituted.ts` made ineligible by turning
    /// into a symlink, and the excluded `node_modules/x.ts` deleted;
    /// `src/omitted.ts`, the LFS pointer, and `.gitattributes` unchanged.
    fn pushed_files() -> Vec<File> {
        let mut files: Vec<File> = standard_files()
            .into_iter()
            .filter(|f| {
                !matches!(
                    f.path,
                    "src/user.ts"
                        | "src/format.ts"
                        | "src/handler.ts"
                        | "src/substituted.ts"
                        | "node_modules/x.ts"
                )
            })
            .collect();
        files.push(file("src/user.ts", USER_V2));
        files.push(file("src/api/handler.ts", HANDLER));
        files.push(File {
            path: "src/substituted.ts",
            mode: "120000",
            content: b"user.ts",
            size: None,
        });
        files
    }

    /// Archive of [`pushed_files`], used only by the from-scratch oracle.
    fn pushed_archive() -> Vec<(&'static str, &'static [u8])> {
        vec![
            ("src/user.ts", USER_V2),
            ("src/api/handler.ts", HANDLER),
            ("src/omitted.ts", OMITTED),
            ("src/lfs.ts", LFS),
        ]
    }

    /// Current pointer of `identity`.
    async fn pointer(store: &MemorySnapshotStore, identity: &Identity) -> Option<SnapshotPointer> {
        store
            .snapshot_pointer(&scope(identity))
            .await
            .expect("pointer")
    }

    /// Publish the standard tree with a cold build and return its pointer.
    async fn cold(
        repo: &FakeRepo,
        storage: &MemoryStorage,
        store: &MemorySnapshotStore,
    ) -> SnapshotPointer {
        repo.push(
            &repo.commit,
            &repo.tree,
            &standard_files(),
            &standard_archive(),
        );
        let client = repo.client();
        let result = ColdBuild::new(&client, storage, store, ColdBuildLimits::default())
            .run(&identity())
            .await
            .expect("cold build");
        let ColdBuildOutcome::Published(pointer) = result.outcome else {
            panic!("expected publication, got {:?}", result.outcome);
        };
        pointer
    }

    /// The snapshot object key of `commit`.
    fn key(commit: &crate::identity::ObjectSha) -> ObjectKey {
        ObjectKey::snapshot(
            &identity(),
            EXTRACTION_VERSION,
            crate::snapshot::SNAPSHOT_FORMAT_VERSION,
            crate::eligibility::MAX_ELIGIBLE_FILE_BYTES,
            commit,
        )
    }

    /// Scenario 1: a push changing, deleting, renaming, and de-eligibling
    /// files takes its changes from compare without a recursive listing,
    /// lists only the changed paths' head directories, fetches only the changed
    /// blobs, leaves the old object's bytes untouched, moves the pointer,
    /// and writes the same bytes as a cold build of the new tree; a repeat
    /// reuses it.
    #[tokio::test]
    async fn change_delete_and_rename_match_a_fresh_parse() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let identity = identity();
        let old = cold(&repo, &storage, &store).await;
        let old_bytes = storage.read(&old.key).await.expect("old object");

        let (tree, commit) = (sha('8'), sha('d'));
        repo.push(&commit, &tree, &pushed_files(), &pushed_archive());
        let before = repo.transport.seen().len();
        let client = repo.client();
        let update = Update::new(&client, &storage, &store, ColdBuildLimits::default());
        let result = update.run(&identity).await.expect("update");
        let ColdBuildOutcome::Published(new) = &result.outcome else {
            panic!("expected publication, got {:?}", result.outcome);
        };
        assert_eq!(
            (&new.tree, &new.commit, &new.key),
            (&tree, &commit, &key(&commit))
        );
        assert_eq!(pointer(&store, &identity).await.as_ref(), Some(new));
        assert_eq!(
            (
                result.ingest.reused_files,
                result.ingest.fallback_blobs,
                result.ingest.archive_entries
            ),
            (2, 2, 0),
            "omitted.ts and the LFS pointer reused; user.ts and api/handler.ts fetched"
        );
        let calls = client.calls();
        assert_eq!(calls.get(Operation::Archive), 0);
        assert_eq!(calls.get(Operation::Blob), 3, "attributes + two changed");
        assert_eq!(calls.get(Operation::Compare), 1);
        assert_eq!(calls.get(Operation::Tree), 3, "root, src, src/api");
        let seen = repo.transport.seen();
        assert!(!seen[before..].iter().any(|url| url.contains("recursive")));
        assert!(
            !seen[before..]
                .iter()
                .any(|url| url.ends_with(&format!("/git/blobs/{}", blob_sha(OMITTED))))
        );
        assert_eq!(storage.read(&old.key).await.expect("old object"), old_bytes);
        assert_eq!(storage.keys().len(), 2);

        let coverage = &result.coverage;
        assert!(coverage.is_complete());
        assert_eq!((coverage.eligible, coverage.indexed), (4, 3));
        let context = Snapshots::new(&storage, &store)
            .read(&scope(&identity), new)
            .await
            .expect("read")
            .into_context()
            .expect("load");
        assert_eq!(context.lookup("renameUser")[0].path.as_str(), "src/user.ts");
        assert_eq!(
            context.lookup("handleRequest")[0].path.as_str(),
            "src/api/handler.ts"
        );
        assert_eq!(
            (
                coverage.excluded.get(&ExclusionReason::Symlink),
                coverage.excluded.get(&ExclusionReason::VendoredSegment)
            ),
            (Some(&1), None),
            "only the changed symlink is observed"
        );
        for gone in ["saveUser", "formatUser", "substitutedFn"] {
            assert!(context.lookup(gone).is_empty(), "{gone} survived");
        }

        let oracle_storage = MemoryStorage::default();
        let oracle_store = MemorySnapshotStore::default();
        let oracle = ColdBuild::new(
            &client,
            &oracle_storage,
            &oracle_store,
            ColdBuildLimits::default(),
        )
        .run(&identity)
        .await
        .expect("oracle");
        assert!(matches!(oracle.outcome, ColdBuildOutcome::Published(_)));
        assert_eq!(
            oracle_storage.read(&new.key).await.expect("oracle object"),
            storage.read(&new.key).await.expect("updated object")
        );

        let blobs = calls.get(Operation::Blob);
        let again = update.run(&identity).await.expect("repeat");
        assert_eq!(again.outcome, ColdBuildOutcome::Reused(new.clone()));
        assert_eq!(calls.get(Operation::Blob), blobs);
    }

    /// Scenario 2: a changed blob that cannot be fetched reports the exact
    /// skip, writes nothing, and keeps the prior pointer; once the blob is
    /// available a whole retry publishes.
    #[tokio::test]
    async fn failed_changed_blob_keeps_the_pointer_until_a_retry() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let identity = identity();
        let old = cold(&repo, &storage, &store).await;

        let tree = sha('8');
        repo.push(&sha('d'), &tree, &pushed_files(), &pushed_archive());
        let missing = format!("/git/blobs/{}", blob_sha(USER_V2));
        repo.transport.route(&missing, 404, "");
        let client = repo.client();
        let update = Update::new(&client, &storage, &store, ColdBuildLimits::default());
        let result = update.run(&identity).await.expect("update");
        assert_eq!(
            result.outcome,
            ColdBuildOutcome::Incomplete(IncompleteReason::UnindexedFiles { count: 1 })
        );
        let skipped = &result.coverage.skipped;
        assert_eq!(skipped.len(), 2);
        assert!(skipped.iter().any(|f| f.path.as_str() == "src/user.ts"
            && matches!(f.reason, SkipReason::FetchFailed { .. })));
        assert_eq!(pointer(&store, &identity).await, Some(old.clone()));
        assert_eq!(storage.keys(), std::slice::from_ref(&old.key));
        assert_eq!(store.records(), [(identity.clone(), old.key.clone())]);

        repo.transport.route(&missing, 200, USER_V2);
        let retry = update.run(&identity).await.expect("retry");
        assert!(matches!(retry.outcome, ColdBuildOutcome::Published(_)));
        assert_eq!(pointer(&store, &identity).await.map(|p| p.tree), Some(tree));
    }

    /// A push whose changed path's directory listing has a malformed entry
    /// fails the update with a provider error and keeps the prior pointer.
    #[tokio::test]
    async fn broken_tree_entry_keeps_the_pointer() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let identity = identity();
        let old = cold(&repo, &storage, &store).await;
        let client = repo.client();
        let update = Update::new(&client, &storage, &store, ColdBuildLimits::default());

        let mut files = pushed_files();
        files.push(bad_sha("docs/notes.md"));
        repo.push(&sha('d'), &sha('8'), &files, &pushed_archive());
        let error = update.run(&identity).await.expect_err("broken listing");
        assert!(matches!(
            error,
            ColdBuildError::GitHub(GitHubError::MalformedResponse { .. })
        ));
        assert_eq!(pointer(&store, &identity).await, Some(old));
    }

    /// Scenario 2: the update future is dropped by a paused-clock timeout
    /// after the record upsert, after the object create, and before the
    /// pointer swap. Each drop keeps the prior pointer and records every
    /// object; the retry converges on the new tree.
    #[tokio::test(start_paused = true)]
    async fn dropped_update_keeps_the_pointer_and_retry_converges() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let identity = identity();
        let tree = sha('8');
        type Stall = fn(&MemoryStorage, &MemorySnapshotStore);
        let steps: [(Stall, bool); 3] = [
            (|_, store| store.stall_after_upsert(), false),
            (|storage, _| storage.stall_after_create(), true),
            (|_, store| store.stall_before_swap(), true),
        ];
        for (stall, object_written) in steps {
            let storage = MemoryStorage::default();
            let store = MemorySnapshotStore::default();
            let old = cold(&repo, &storage, &store).await;
            repo.push(&sha('d'), &tree, &pushed_files(), &pushed_archive());
            let client = repo.client();
            let update = Update::new(&client, &storage, &store, ColdBuildLimits::default());
            stall(&storage, &store);
            let dropped =
                tokio::time::timeout(Duration::from_secs(60), update.run(&identity)).await;
            assert!(dropped.is_err(), "the stalled update must be dropped");
            assert_eq!(pointer(&store, &identity).await, Some(old.clone()));
            let mut recorded = vec![
                (identity.clone(), old.key.clone()),
                (identity.clone(), key(&sha('d'))),
            ];
            recorded.sort();
            assert_eq!(store.records(), recorded);
            let mut objects = vec![old.key.clone()];
            if object_written {
                objects.push(key(&sha('d')));
            }
            objects.sort();
            assert_eq!(storage.keys(), objects);

            let retry = update.run(&identity).await.expect("retry");
            let ColdBuildOutcome::Published(published) = retry.outcome else {
                panic!("retry must publish, got {:?}", retry.outcome);
            };
            assert_eq!(published.key, key(&sha('d')));
            assert_eq!(pointer(&store, &identity).await, Some(published.clone()));
            assert!(
                Snapshots::new(&storage, &store)
                    .read(&scope(&identity), &published)
                    .await
                    .is_ok()
            );
        }
    }

    /// Scenarios 4, 9, 16: a review-time check reports a missed push as
    /// stale without building; a head that moves while an update publishes
    /// leaves the check stale so another update converges; and an update
    /// that loses the pointer swap to one pinned at a newer head returns
    /// the retry state, whose retry reuses the winner.
    #[tokio::test]
    async fn moving_head_is_reported_stale_and_updates_converge() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let identity = identity();
        let client = repo.client();
        let update = Update::new(&client, &storage, &store, ColdBuildLimits::default());
        assert!(matches!(
            update.freshness(&identity).await.expect("fresh"),
            Freshness::Stale { pointer: None, .. }
        ));
        let old = cold(&repo, &storage, &store).await;
        assert_eq!(
            update.freshness(&identity).await.expect("fresh"),
            Freshness::Current(old.clone())
        );

        let (tree_b, tree_c) = (sha('8'), sha('9'));
        repo.push(&sha('d'), &tree_b, &pushed_files(), &pushed_archive());
        let Freshness::Stale {
            pointer: Some(stale),
            head,
        } = update.freshness(&identity).await.expect("fresh")
        else {
            panic!("missed push must be stale");
        };
        assert_eq!((stale, &head.tree), (old.clone(), &tree_b));
        assert_eq!(
            client.calls().get(Operation::Tree),
            0,
            "check built nothing"
        );

        let mut moved = pushed_files();
        moved.push(file("src/extra.ts", EXTRA));
        store.hold_swaps.store(true, Ordering::SeqCst);
        let (first, ()) = tokio::join!(update.run(&identity), async {
            while !storage.keys().contains(&key(&sha('d'))) {
                tokio::task::yield_now().await;
            }
            repo.push(&sha('e'), &tree_c, &moved, &[]);
            store.hold_swaps.store(false, Ordering::SeqCst);
        });
        let ColdBuildOutcome::Published(first) = first.expect("first").outcome else {
            panic!("first update must publish its pinned head");
        };
        assert_eq!(first.tree, tree_b);
        assert!(matches!(
            update.freshness(&identity).await.expect("fresh"),
            Freshness::Stale { head, .. } if head.tree == tree_c
        ));

        let tree_d = sha('a');
        store.hold_swaps.store(true, Ordering::SeqCst);
        let (lost, won) = tokio::join!(update.run(&identity), async {
            while !storage.keys().contains(&key(&sha('e'))) {
                tokio::task::yield_now().await;
            }
            repo.push(&sha('f'), &tree_d, &pushed_files(), &[]);
            store.hold_swaps.store(false, Ordering::SeqCst);
            update.run(&identity).await
        });
        assert_eq!(lost, Err(ColdBuildError::PointerConflict));
        let ColdBuildOutcome::Published(won) = won.expect("winner").outcome else {
            panic!("newer head must publish");
        };
        assert_eq!(won.tree, tree_d);
        let retry = update.run(&identity).await.expect("retry");
        assert_eq!(retry.outcome, ColdBuildOutcome::Reused(won.clone()));
        assert_eq!(
            update.freshness(&identity).await.expect("fresh"),
            Freshness::Current(won)
        );
    }

    /// Scenario 8: a pointer written under another extraction version is
    /// invisible to the current scope, so the update cold-builds a new key
    /// from the archive and leaves the old-version pointer untouched.
    #[tokio::test]
    async fn version_change_rebuilds_under_a_new_key() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let identity = identity();
        let old_scope = SnapshotScope {
            extraction_version: "0-previous".to_owned(),
            ..scope(&identity)
        };
        let old = SnapshotPointer {
            tree: repo.tree.clone(),
            commit: repo.commit.clone(),
            key: ObjectKey::snapshot(
                &identity,
                "0-previous",
                1,
                crate::eligibility::MAX_ELIGIBLE_FILE_BYTES,
                &repo.tree,
            ),
            digest: crate::storage::ObjectDigest::of(b"old"),
        };
        assert!(
            store
                .publish_snapshot_pointer(&old_scope, None, &old)
                .await
                .expect("seed")
        );
        let client = repo.client();
        let result = Update::new(&client, &storage, &store, ColdBuildLimits::default())
            .run(&identity)
            .await
            .expect("update");
        let ColdBuildOutcome::Published(new) = result.outcome else {
            panic!("expected publication, got {:?}", result.outcome);
        };
        assert_eq!(new.key, key(&repo.commit));
        assert_ne!(new.key, old.key);
        assert_eq!(client.calls().get(Operation::Archive), 1);
        assert_eq!(result.ingest.reused_files, 0);
        assert_eq!(
            store.snapshot_pointer(&old_scope).await.expect("old"),
            Some(old)
        );
    }

    /// Scenario 2: more changed files than the changed-blob bound rebuild
    /// from the archive instead of individual blob calls, still publishing
    /// a complete snapshot.
    #[tokio::test]
    async fn changes_past_the_ceiling_rebuild_from_the_archive() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        cold(&repo, &storage, &store).await;
        repo.push(&sha('d'), &sha('8'), &pushed_files(), &pushed_archive());
        let client = repo.client();
        let limits = ColdBuildLimits {
            max_changed_blobs: 1,
            ..ColdBuildLimits::default()
        };
        let result = Update::new(&client, &storage, &store, limits)
            .run(&identity())
            .await
            .expect("update");
        assert!(matches!(result.outcome, ColdBuildOutcome::Published(_)));
        assert_eq!(client.calls().get(Operation::Archive), 1);
        assert_eq!(
            (result.ingest.archive_entries, result.ingest.reused_files),
            (4, 0)
        );
    }

    /// Scenario 2: a root `.gitattributes` change, a head that does not
    /// descend from the pointer's commit, and a compare at its file ceiling
    /// each rebuild from the archive and publish the complete snapshot a
    /// cold build of the head would.
    #[tokio::test]
    async fn rebuild_triggers_use_the_archive() {
        let attributes_changed = |repo: &FakeRepo| {
            let mut files = pushed_files();
            files.retain(|f| f.path != ".gitattributes");
            files.push(file(
                ".gitattributes",
                b"generated/* linguist-generated\nsrc/omitted.ts linguist-generated\n",
            ));
            repo.push(&sha('d'), &sha('8'), &files, &pushed_archive());
        };
        let diverged = |repo: &FakeRepo| {
            repo.push(&sha('d'), &sha('8'), &pushed_files(), &pushed_archive());
            repo.transport.json(
                &format!("/compare/{}...{}", repo.commit, sha('d')),
                &format!(
                    r#"{{"status":"diverged","base_commit":{{"sha":"{}"}},"commits":[],"files":[]}}"#,
                    repo.commit
                ),
            );
        };
        let at_ceiling = |repo: &FakeRepo| {
            repo.push(&sha('d'), &sha('8'), &pushed_files(), &pushed_archive());
            let files: Vec<String> = (0..crate::github::COMPARE_MAX_FILES)
                .map(|i| format!(r#"{{"filename":"f{i}.ts","status":"added"}}"#))
                .collect();
            repo.transport.json(
                &format!("/compare/{}...{}", repo.commit, sha('d')),
                &format!(
                    r#"{{"status":"ahead","base_commit":{{"sha":"{}"}},"commits":[{{"sha":"{}"}}],"files":[{}]}}"#,
                    repo.commit,
                    sha('d'),
                    files.join(",")
                ),
            );
        };
        type Trigger = fn(&FakeRepo);
        let triggers: [(&str, Trigger); 3] = [
            ("attributes", attributes_changed),
            ("diverged", diverged),
            ("ceiling", at_ceiling),
        ];
        for (name, trigger) in triggers {
            let repo = FakeRepo::new(&standard_files(), &standard_archive());
            let storage = MemoryStorage::default();
            let store = MemorySnapshotStore::default();
            cold(&repo, &storage, &store).await;
            trigger(&repo);
            let client = repo.client();
            let result = Update::new(&client, &storage, &store, ColdBuildLimits::default())
                .run(&identity())
                .await
                .expect(name);
            let ColdBuildOutcome::Published(new) = &result.outcome else {
                panic!("{name}: expected publication, got {:?}", result.outcome);
            };
            assert_eq!(client.calls().get(Operation::Archive), 1, "{name}");
            assert_eq!(result.ingest.reused_files, 0, "{name}");

            let oracle_storage = MemoryStorage::default();
            let oracle = ColdBuild::new(
                &client,
                &oracle_storage,
                &MemorySnapshotStore::default(),
                ColdBuildLimits::default(),
            )
            .run(&identity())
            .await
            .expect("oracle");
            assert!(matches!(oracle.outcome, ColdBuildOutcome::Published(_)));
            assert_eq!(
                oracle_storage.read(&new.key).await.expect("oracle object"),
                storage.read(&new.key).await.expect("updated object"),
                "{name}"
            );
        }
    }
}
