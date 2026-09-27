//! Incremental default-branch snapshot updates and review-time staleness.
//!
//! [`Update::run`] pins the default branch, reclassifies its tree, carries
//! the prior snapshot's facts for every path whose blob is unchanged,
//! fetches changed blobs as verified blobs (or through the archive past
//! [`ColdBuildLimits::max_changed_blobs`]), and publishes through the cold
//! build's record-upsert, create-if-absent, pointer compare-and-swap path.
//! Deleted, renamed-away, and now-ineligible paths simply are not carried.
//! An incomplete attempt writes nothing, so the prior pointer and the
//! immutable prior object stay valid for reviews already using them.

use crate::github::{BranchHead, GitHubClient};
use crate::identity::Identity;
use crate::persistence::{SnapshotPointer, SnapshotStore};
use crate::snapshot::{
    ColdBuild, ColdBuildError, ColdBuildLimits, ColdBuildResult, Snapshots, snapshot_scope,
};
use crate::storage::{ObjectStorage, StorageError};

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
    /// reused. Otherwise only changed paths are fetched. The caller bounds
    /// the call by dropping its future; a drop at any `.await` leaves the
    /// pointer on a complete snapshot and every written object recorded,
    /// and a retry converges. No persistence transaction spans provider or
    /// storage IO.
    ///
    /// # Errors
    ///
    /// As [`ColdBuild::run`]: `PointerConflict` or `Persistence(Collecting)`
    /// is a retry state; a changed blob that cannot be fetched is not an
    /// error but an `Incomplete` outcome leaving the pointer in place.
    pub async fn run(&self, identity: &Identity) -> Result<ColdBuildResult, ColdBuildError> {
        let cold = &self.cold;
        let scope = snapshot_scope(identity);
        let current = cold.store.snapshot_pointer(&scope).await?;
        let head = cold.pin(identity).await?;
        let Some(pointer) = current.as_ref().filter(|pointer| pointer.tree != head.tree) else {
            return cold.build(&scope, current.as_ref(), &head).await;
        };
        let prior = match Snapshots::new(cold.storage, cold.store)
            .read(identity, pointer)
            .await
        {
            Ok(prior) => prior,
            Err(ColdBuildError::Storage(StorageError::NotFound { .. })) => {
                return cold.build(&scope, current.as_ref(), &head).await;
            }
            Err(error) => return Err(error),
        };
        let mut ingest = match cold.start(identity, &head).await? {
            Ok(ingest) => ingest,
            Err(truncated) => return Ok(truncated),
        };
        ingest.reuse(prior);
        let archive_too_large = if ingest.pending() > cold.limits.max_changed_blobs {
            cold.fetch(&mut ingest, identity, &head).await
        } else {
            ingest
                .fallback(cold.client, identity, cold.limits.max_changed_blobs)
                .await;
            false
        };
        cold.complete(&scope, current.as_ref(), &head, ingest, archive_too_large)
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
            .snapshot_pointer(&snapshot_scope(identity))
            .await?;
        let head = self.cold.pin(identity).await?;
        Ok(match pointer {
            Some(pointer) if pointer.tree == head.tree => Freshness::Current(pointer),
            pointer => Freshness::Stale { pointer, head },
        })
    }
}

#[cfg(test)]
/// Update journeys over the cold build's routed fake GitHub and in-memory
/// storage: a from-scratch oracle, failed and dropped attempts, moving and
/// racing heads, and a version change.
mod tests {
    use super::*;
    use crate::github::Operation;
    use crate::persistence::SnapshotScope;
    use crate::snapshot::tests::{
        FakeRepo, File, HANDLER, LFS, OMITTED, SUBSTITUTED, bad_sha, blob_sha, file, identity, sha,
        standard_archive, standard_files,
    };
    use crate::snapshot::{ColdBuildOutcome, IncompleteReason, SkipReason};
    use crate::storage::ObjectKey;
    use crate::testing::{MemorySnapshotStore, MemoryStorage};
    use bohmian_code_review::EXTRACTION_VERSION;
    use std::sync::atomic::Ordering;
    use std::time::{Duration, UNIX_EPOCH};

    /// `src/user.ts` after the push: `saveUser` removed, `renameUser` added.
    const USER_V2: &[u8] = b"export function lookupUser(id: string): string {\n  return id;\n}\n\nexport function renameUser(id: string): string {\n  return id;\n}\n";
    /// Declares `extraFn`; added by a later push.
    const EXTRA: &[u8] = b"export function extraFn(): number {\n  return 3;\n}\n";

    /// The push after the standard tree: `src/user.ts` changed,
    /// `src/format.ts` deleted, `src/handler.ts` renamed to
    /// `src/api/handler.ts`, and `src/substituted.ts` made ineligible by an
    /// attributes change; `src/omitted.ts` and the LFS pointer unchanged.
    fn pushed_files() -> Vec<File> {
        let mut files: Vec<File> = standard_files()
            .into_iter()
            .filter(|f| {
                !matches!(
                    f.path,
                    "src/user.ts" | "src/format.ts" | "src/handler.ts" | ".gitattributes"
                )
            })
            .collect();
        files.push(file("src/user.ts", USER_V2));
        files.push(file("src/api/handler.ts", HANDLER));
        files.push(file(
            ".gitattributes",
            b"generated/* linguist-generated\nsrc/substituted.ts linguist-generated\n",
        ));
        files
    }

    /// Archive of [`pushed_files`], used only by the from-scratch oracle.
    fn pushed_archive() -> Vec<(&'static str, &'static [u8])> {
        vec![
            ("src/user.ts", USER_V2),
            ("src/api/handler.ts", HANDLER),
            ("src/omitted.ts", OMITTED),
            ("src/substituted.ts", SUBSTITUTED),
            ("src/lfs.ts", LFS),
        ]
    }

    /// Current pointer of `identity`.
    async fn pointer(store: &MemorySnapshotStore, identity: &Identity) -> Option<SnapshotPointer> {
        store
            .snapshot_pointer(&snapshot_scope(identity))
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

    /// The snapshot object key of `tree`.
    fn key(tree: &crate::identity::ObjectSha) -> ObjectKey {
        ObjectKey::snapshot(
            &identity(),
            EXTRACTION_VERSION,
            crate::snapshot::SNAPSHOT_FORMAT_VERSION,
            tree,
        )
    }

    /// Scenario 1: a push changing, deleting, renaming, and de-eligibling
    /// files fetches only the changed blobs, leaves the old object's bytes
    /// untouched, moves the pointer, and writes the same bytes as a cold
    /// build of the new tree; a repeat reuses it.
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
            (&tree, &commit, &key(&tree))
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
        let seen = repo.transport.seen();
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
            .read(&identity, new)
            .await
            .expect("read")
            .into_context()
            .expect("load");
        assert_eq!(context.lookup("renameUser")[0].path.as_str(), "src/user.ts");
        assert_eq!(
            context.lookup("handleRequest")[0].path.as_str(),
            "src/api/handler.ts"
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
        assert_eq!(oracle.coverage, *coverage);
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

    /// A push adding a rejected entry at an unsupported path advances the
    /// pointer; a later push adding one at a supported path keeps it.
    #[tokio::test]
    async fn rejected_entry_keeps_the_pointer_only_when_it_may_be_eligible() {
        let repo = FakeRepo::new(&standard_files(), &standard_archive());
        let storage = MemoryStorage::default();
        let store = MemorySnapshotStore::default();
        let identity = identity();
        cold(&repo, &storage, &store).await;
        let client = repo.client();
        let update = Update::new(&client, &storage, &store, ColdBuildLimits::default());

        let mut files = pushed_files();
        files.push(bad_sha("docs/notes.md"));
        repo.push(&sha('d'), &sha('8'), &files, &pushed_archive());
        let proven = update.run(&identity).await.expect("update");
        let ColdBuildOutcome::Published(advanced) = proven.outcome else {
            panic!("expected publication, got {:?}", proven.outcome);
        };
        assert_eq!(advanced.tree, sha('8'));

        files.push(bad_sha("src/bad.ts"));
        repo.push(&sha('e'), &sha('9'), &files, &pushed_archive());
        let unknown = update.run(&identity).await.expect("update");
        assert_eq!(
            unknown.outcome,
            ColdBuildOutcome::Incomplete(IncompleteReason::UnindexedFiles { count: 1 })
        );
        assert_eq!(pointer(&store, &identity).await, Some(advanced));
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
                (identity.clone(), key(&tree)),
            ];
            recorded.sort();
            assert_eq!(store.records(), recorded);
            let mut objects = vec![old.key.clone()];
            if object_written {
                objects.push(key(&tree));
            }
            objects.sort();
            assert_eq!(storage.keys(), objects);

            let retry = update.run(&identity).await.expect("retry");
            let ColdBuildOutcome::Published(published) = retry.outcome else {
                panic!("retry must publish, got {:?}", retry.outcome);
            };
            assert_eq!(published.key, key(&tree));
            assert_eq!(pointer(&store, &identity).await, Some(published.clone()));
            assert!(
                Snapshots::new(&storage, &store)
                    .read(&identity, &published)
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
            while !storage.keys().contains(&key(&tree_b)) {
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
            while !storage.keys().contains(&key(&tree_c)) {
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
            ..snapshot_scope(&identity)
        };
        let old = SnapshotPointer {
            tree: repo.tree.clone(),
            commit: repo.commit.clone(),
            key: ObjectKey::snapshot(&identity, "0-previous", 1, &repo.tree),
            digest: crate::storage::ObjectDigest::of(b"old"),
            published_at: UNIX_EPOCH,
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
        assert_eq!(new.key, key(&repo.tree));
        assert_ne!(new.key, old.key);
        assert_eq!(client.calls().get(Operation::Archive), 1);
        assert_eq!(result.ingest.reused_files, 0);
        assert_eq!(
            store.snapshot_pointer(&old_scope).await.expect("old"),
            Some(old)
        );
    }

    /// Changed blobs beyond the ceiling stream through the archive instead
    /// of individual blob calls, still publishing a complete snapshot.
    #[tokio::test]
    async fn changes_past_the_ceiling_use_the_archive() {
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
            (result.ingest.archive_entries, result.ingest.fallback_blobs),
            (2, 0)
        );
    }
}
