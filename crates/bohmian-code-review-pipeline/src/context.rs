//! The shared review-context object: publication, authorized read-back by
//! context id, and expiry cleanup.
//!
//! A [`ReviewContext`] is the only source-bearing object the library
//! persists. Its id is the digest of its bytes plus a random value, so each
//! publication has its own record, key, and lifetime while a record's
//! digest still checks the object's bytes. Publication upserts the record,
//! which the context store stamps with `published_at` and `expires_at` on
//! its own clock, before creating the object if absent, so every object has
//! a record; a reader treats a record whose object is absent as not found.
//! Reads require the caller's tenant and installation and are refused once
//! the store judges the record expired, and cleanup deletes each expired
//! object before its record.

use crate::github::GitHubError;
use crate::identity::{Identity, ObjectSha};
use crate::persistence::{
    ContextId, ContextLifetime, ContextLookup, ContextRecord, ContextStore, PersistenceError,
};
use crate::snapshot::{ColdBuildError, Coverage};
use crate::storage::{ObjectDigest, ObjectKey, ObjectStorage, StorageError, hex};
use bohmian_code_review::Error as SymbolicError;
use bohmian_code_review::RelativePath;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

/// Layout version of the serialized [`ReviewContext`].
pub const CONTEXT_FORMAT_VERSION: u32 = 1;

/// How long a published context stays readable; a [`ContextStore`] stamps
/// `expires_at` this long after `published_at`.
pub const CONTEXT_LIFETIME: Duration = Duration::from_secs(24 * 60 * 60);

/// Whether the index covered the whole PR head tree or only files near the
/// changed paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    /// A default-branch snapshot overlaid to the PR head tree.
    Repository,
    /// A bounded index of eligible files near the changed paths.
    Scoped,
}

/// Independent coverage axes of one review context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCoverage {
    /// Repository or scoped index.
    pub scope: Scope,
    /// Tree of the complete snapshot the overlay started from, if any.
    pub snapshot_tree: Option<ObjectSha>,
    /// Whether a complete snapshot was loaded.
    pub snapshot_complete: bool,
    /// Whether every eligible file of the indexed set is at its head blob.
    pub overlay_complete: bool,
    /// Whether the provider's PR-file ceiling hid further changed files.
    pub pr_files_truncated: bool,
    /// PR-file entries rejected by validation, whose changes are unknown.
    pub pr_files_rejected: usize,
    /// Eligible, indexed, skipped, and excluded counts of the indexed set.
    pub files: Coverage,
}

impl ContextCoverage {
    /// Only a repository-scope index from a complete snapshot with a
    /// complete overlay and every PR file listed and validated covers the
    /// full PR head.
    pub fn is_full(&self) -> bool {
        self.scope == Scope::Repository
            && self.snapshot_complete
            && self.overlay_complete
            && !self.pr_files_truncated
            && self.pr_files_rejected == 0
    }
}

/// Why a PR file or ranked candidate contributed nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotAnalyzedReason {
    /// The provider entry failed path, status, or id validation.
    Rejected {
        /// Validation failure; never source text.
        detail: String,
    },
    /// The file was removed, so it has no head-side lines.
    Removed,
    /// The path is not an eligible file in this invocation's index.
    NotEligible,
    /// The provider's blob id differs from the pinned head tree's.
    BlobMismatch,
    /// The provider omitted the patch, as for binary or very large diffs.
    MissingPatch,
    /// The patch could not be parsed into head-side lines.
    MalformedPatch {
        /// Parser error; never source text.
        detail: String,
    },
    /// The patch adds no head-side lines.
    DeletionOnly,
    /// The head or candidate blob could not be fetched.
    FetchFailed {
        /// Provider error; never source text.
        detail: String,
    },
    /// The head blob could not be analyzed with its changed lines.
    AnalysisFailed {
        /// Extraction or validation error; never source text.
        detail: String,
    },
}

/// One PR file or ranked candidate that contributed nothing, with why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotAnalyzed {
    /// Path as the provider reported it; unvalidated for `Rejected`.
    pub path: String,
    /// Why it was not analyzed.
    pub reason: NotAnalyzedReason,
}

/// One selected candidate declaration with its exact source text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedSpan {
    /// Candidate path.
    pub path: RelativePath,
    /// Head blob the text was read from.
    pub blob: ObjectSha,
    /// First one-based line.
    pub start: u32,
    /// Last one-based line, inclusive.
    pub end: u32,
    /// The candidate file's score.
    pub score: usize,
    /// Changed-line names that matched this declaration.
    pub names: Vec<String>,
    /// Exact source lines.
    pub text: String,
    /// Estimated tokens of `text`.
    pub tokens: usize,
}

/// Which ceiling left a ranked candidate span out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OmittedReason {
    /// The candidate file ceiling.
    FileLimit,
    /// The remaining token allowance.
    TokenLimit {
        /// Estimated tokens of the span.
        tokens: usize,
    },
}

/// A ranked candidate span left out by a ceiling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmittedSpan {
    /// Candidate path.
    pub path: RelativePath,
    /// First one-based line.
    pub start: u32,
    /// Last one-based line, inclusive.
    pub end: u32,
    /// Changed-line names that matched it.
    pub names: Vec<String>,
    /// The ceiling that excluded it.
    pub reason: OmittedReason,
}

/// How the review token allocation was divided. Every figure is an
/// estimate; the downstream caller must count its complete model request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    /// Configured review allocation.
    pub review_tokens: usize,
    /// Reserved for the downstream prompt.
    pub prompt_reserve: usize,
    /// Estimated tokens of every PR file patch, delivered separately.
    pub diff_tokens: usize,
    /// What remained for selected context.
    pub context_allowance: usize,
    /// Estimated tokens of the selected spans.
    pub selected_tokens: usize,
    /// The diff and reserve left no context allowance, so nothing was packed.
    pub exhausted: bool,
}

/// The immutable, source-bearing object a context id names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewContext {
    /// [`CONTEXT_FORMAT_VERSION`] this object was written under.
    pub format_version: u32,
    /// Extractor that ranked the candidates.
    pub extraction_version: String,
    /// Caller identity the object is scoped to.
    pub identity: Identity,
    /// Pull request number.
    pub pull_request: u64,
    /// Base commit pinned for the whole invocation.
    pub base: ObjectSha,
    /// Head commit pinned for the whole invocation.
    pub head: ObjectSha,
    /// Root tree of `head`.
    pub tree: ObjectSha,
    /// Selected candidate spans in packing order.
    pub spans: Vec<SelectedSpan>,
    /// Ranked candidate spans a ceiling excluded, in packing order.
    pub omissions: Vec<OmittedSpan>,
    /// PR files and candidates that contributed nothing, with reasons.
    pub not_analyzed: Vec<NotAnalyzed>,
    /// Token allocation.
    pub budget: Budget,
    /// Coverage of the index the ranking used.
    pub coverage: ContextCoverage,
}

/// Why a PR-context build, read, or cleanup failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ContextError {
    /// A provider call for the PR, its head, or its tree failed.
    #[error(transparent)]
    GitHub(#[from] GitHubError),
    /// Loading the default-branch snapshot failed.
    #[error(transparent)]
    Snapshot(#[from] ColdBuildError),
    /// Object storage failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// Metadata persistence failed.
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    /// The symbolic library rejected validated input.
    #[error(transparent)]
    Symbolic(#[from] SymbolicError),
    /// The PR's base or head changed during every pagination attempt; a
    /// retry may succeed.
    #[error("pull request moved during {attempts} pagination attempts")]
    PullRequestMoved {
        /// Attempts made.
        attempts: usize,
    },
    /// No readable context exists for this id and identity.
    #[error("context not found: {id}")]
    NotFound {
        /// The requested id.
        id: ContextId,
    },
    /// The context's `expires_at` has passed.
    #[error("context expired: {id}")]
    Expired {
        /// The requested id.
        id: ContextId,
    },
    /// A record names a key other than the caller's own context key.
    #[error("context record names a foreign key: {key}")]
    ForeignKey {
        /// The refused key.
        key: ObjectKey,
    },
    /// The object does not match its record.
    #[error("context object is corrupt: {detail}")]
    Corrupt {
        /// What failed validation; never source text.
        detail: String,
    },
    /// The operating system's random source failed while minting a context
    /// id; nothing was written.
    #[error("random context id unavailable: {detail}")]
    Random {
        /// Operating system error.
        detail: String,
    },
}

/// Outcome of one expiry sweep.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextCleanup {
    /// Contexts whose object and record were deleted.
    pub deleted: Vec<(Identity, ContextId)>,
    /// Expired contexts still recorded after a failure, for the next sweep.
    pub lagging: Vec<(Identity, ContextId, String)>,
}

/// The context-read facade, and the publication and cleanup it pairs with,
/// over the caller's context object storage and context store.
pub struct Contexts<'a, O, C> {
    storage: &'a O,
    store: &'a C,
}

impl<'a, O: ObjectStorage, C: ContextStore> Contexts<'a, O, C> {
    /// Context access over the caller's context storage and store.
    pub fn new(storage: &'a O, store: &'a C) -> Contexts<'a, O, C> {
        Contexts { storage, store }
    }

    /// Publish `context` for `identity`, readable until the `expires_at`
    /// the context store stamps.
    ///
    /// Upserts the record, then creates the object if absent. The id joins
    /// the object digest and a random value, so a later publication of
    /// identical bytes gets its own id, key, and expiry, and cleanup of one
    /// never touches another. Dropped after the upsert, the record names an
    /// absent object that reads treat as not found; a retry publishes a new
    /// readable id.
    ///
    /// # Errors
    ///
    /// Returns `Random` when no id can be minted, `Corrupt` if the context
    /// does not serialize, and persistence and storage failures, including
    /// `AlreadyExists` should the fresh key already hold an object.
    pub(crate) async fn publish(
        &self,
        identity: &Identity,
        context: &ReviewContext,
    ) -> Result<(ContextRecord, ContextLifetime), ContextError> {
        let bytes = serde_json::to_vec(context).map_err(|error| ContextError::Corrupt {
            detail: error.to_string(),
        })?;
        let digest = ObjectDigest::of(&bytes);
        let id = context_id(&digest)?;
        let record = ContextRecord {
            key: ObjectKey::context(identity, &id),
            id,
            digest,
            pull_request: context.pull_request,
            base: context.base.clone(),
            head: context.head.clone(),
        };
        let lifetime = self.store.upsert_context(identity, &record).await?;
        self.storage.create(&record.key, &bytes).await?;
        Ok((record, lifetime))
    }

    /// Read the context `id` published for `identity`, if the context store
    /// still judges it live.
    ///
    /// Only an exact `(identity, id)` record is used; its key must be the
    /// caller's own context key before storage is touched. The object is
    /// verified against the record's digest and identity and is never
    /// rebuilt from current branch content.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` without a record or object, `Expired` once the
    /// store's clock reaches `expires_at`, `ForeignKey` for a record naming another key,
    /// `Corrupt` when the bytes do not match the record, and persistence or
    /// storage backend failures.
    pub async fn read(
        &self,
        identity: &Identity,
        id: &ContextId,
    ) -> Result<ReviewContext, ContextError> {
        let not_found = || ContextError::NotFound { id: id.clone() };
        let record = match self.store.context(identity, id).await? {
            ContextLookup::Live(record) if record.id == *id => record,
            ContextLookup::Expired => return Err(ContextError::Expired { id: id.clone() }),
            _ => return Err(not_found()),
        };
        let key = ObjectKey::context(identity, id);
        if record.key != key {
            return Err(ContextError::ForeignKey { key: record.key });
        }
        let bytes = match self.storage.read(&key).await {
            Ok(bytes) => bytes,
            Err(StorageError::NotFound { .. }) => return Err(not_found()),
            Err(error) => return Err(error.into()),
        };
        let corrupt = |detail: &str| ContextError::Corrupt {
            detail: detail.to_owned(),
        };
        if ObjectDigest::of(&bytes) != record.digest {
            return Err(corrupt("digest mismatch"));
        }
        let context: ReviewContext =
            serde_json::from_slice(&bytes).map_err(|error| corrupt(&error.to_string()))?;
        if context.format_version != CONTEXT_FORMAT_VERSION
            || context.identity != *identity
            || (context.pull_request, &context.base, &context.head)
                != (record.pull_request, &record.base, &record.head)
        {
            return Err(corrupt("object does not match its record"));
        }
        Ok(context)
    }

    /// Delete up to `limit` contexts the context store reports expired on
    /// its clock: each object, where a missing object is success, then its
    /// record.
    ///
    /// A failed object delete, or a record naming another identity's key,
    /// keeps the record and is reported as lag for the next sweep; the
    /// sweep continues with the other contexts.
    ///
    /// # Errors
    ///
    /// Returns the persistence failure listing expired records.
    pub async fn collect_expired(&self, limit: usize) -> Result<ContextCleanup, ContextError> {
        let mut cleanup = ContextCleanup::default();
        for (identity, record) in self.store.expired_contexts(limit).await? {
            let deleted = if record.key != ObjectKey::context(&identity, &record.id) {
                Err(format!("record names a foreign key: {}", record.key))
            } else {
                match self.storage.delete(&record.key).await {
                    Ok(()) => self
                        .store
                        .delete_context(&identity, &record.id)
                        .await
                        .map_err(|error| error.to_string()),
                    Err(error) => Err(error.to_string()),
                }
            };
            match deleted {
                Ok(()) => cleanup.deleted.push((identity, record.id)),
                Err(error) => cleanup.lagging.push((identity, record.id, error)),
            }
        }
        Ok(cleanup)
    }
}

/// A fresh context id: `digest`, a dash, and 128 random bits in hex.
///
/// # Errors
///
/// Returns `Random` when the operating system's random source fails.
fn context_id(digest: &ObjectDigest) -> Result<ContextId, ContextError> {
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|error| ContextError::Random {
        detail: error.to_string(),
    })?;
    ContextId::new(&format!("{}-{}", digest.as_str(), hex(&random))).map_err(|error| {
        ContextError::Corrupt {
            detail: error.to_string(),
        }
    })
}

#[cfg(test)]
/// Shared-read contract over the in-memory context store: repeated reads,
/// expiry, identity isolation, integrity, absent objects, and cleanup.
mod tests {
    use super::*;
    use crate::identity::{InstallationId, RepositoryIdentity, TenantId};
    use crate::testing::{MemoryContextStore, MemoryStorage};
    use std::sync::atomic::Ordering;

    /// An identity for `tenant` under `installation`.
    fn identity(tenant: &str, installation: u64) -> Identity {
        Identity {
            tenant: TenantId::new(tenant).expect("tenant"),
            installation: InstallationId::new(installation).expect("installation"),
            repository: RepositoryIdentity::new(7, "o", "r").expect("repo"),
        }
    }

    /// Forty copies of `ch` as an object id.
    fn sha(ch: char) -> ObjectSha {
        ObjectSha::new(&ch.to_string().repeat(40)).expect("sha")
    }

    /// A one-span context for `identity`.
    fn context(identity: &Identity) -> ReviewContext {
        ReviewContext {
            format_version: CONTEXT_FORMAT_VERSION,
            extraction_version: bohmian_code_review::EXTRACTION_VERSION.to_owned(),
            identity: identity.clone(),
            pull_request: 5,
            base: sha('a'),
            head: sha('b'),
            tree: sha('c'),
            spans: vec![SelectedSpan {
                path: RelativePath::new("src/user.ts").expect("path"),
                blob: sha('d'),
                start: 1,
                end: 3,
                score: 1,
                names: vec!["lookupUser".to_owned()],
                text: "export function lookupUser() {}\n".to_owned(),
                tokens: 8,
            }],
            omissions: Vec::new(),
            not_analyzed: Vec::new(),
            budget: Budget {
                review_tokens: 100,
                prompt_reserve: 10,
                diff_tokens: 10,
                context_allowance: 80,
                selected_tokens: 8,
                exhausted: false,
            },
            coverage: ContextCoverage {
                scope: Scope::Repository,
                snapshot_tree: Some(sha('e')),
                snapshot_complete: true,
                overlay_complete: true,
                pr_files_truncated: false,
                pr_files_rejected: 0,
                files: Coverage::default(),
            },
        }
    }

    /// Scenarios 14–15: repeated reads before expiry return the same
    /// context; once the store's clock reaches `expires_at`, or for another
    /// tenant or installation, a read fails; identical content under two
    /// identities lives at separate keys; the record carries no source text.
    #[tokio::test]
    async fn reads_are_identity_scoped_and_expire() {
        let storage = MemoryStorage::default();
        let store = MemoryContextStore::default();
        let contexts = Contexts::new(&storage, &store);
        let (acme, other_tenant, other_installation) = (
            identity("acme", 1),
            identity("zeta", 1),
            identity("acme", 2),
        );
        store.clock.advance(Duration::from_secs(1_000_000));
        let (record, lifetime) = contexts
            .publish(&acme, &context(&acme))
            .await
            .expect("publish");
        assert_eq!(
            lifetime.expires_at,
            lifetime.published_at + CONTEXT_LIFETIME
        );
        let first = contexts.read(&acme, &record.id).await.expect("read");
        store
            .clock
            .advance(CONTEXT_LIFETIME - Duration::from_secs(1));
        assert_eq!(contexts.read(&acme, &record.id).await, Ok(first.clone()));
        assert_eq!(first, context(&acme));
        for stranger in [&other_tenant, &other_installation] {
            assert_eq!(
                contexts.read(stranger, &record.id).await,
                Err(ContextError::NotFound {
                    id: record.id.clone()
                })
            );
        }
        for step in [Duration::from_secs(1), CONTEXT_LIFETIME] {
            store.clock.advance(step);
            assert_eq!(
                contexts.read(&acme, &record.id).await,
                Err(ContextError::Expired {
                    id: record.id.clone()
                })
            );
        }
        let (theirs, _) = contexts
            .publish(&other_tenant, &context(&other_tenant))
            .await
            .expect("publish");
        assert_ne!(theirs.key, record.key);
        assert_eq!(storage.keys().len(), 2);
        let metadata = serde_json::to_string(&record).expect("record");
        assert!(!metadata.contains("lookupUser()"));
    }

    /// A record whose object is absent reads as not found; a corrupted
    /// object, a record naming another identity's key, and an unknown id
    /// fail explicitly.
    #[tokio::test]
    async fn absent_corrupt_and_foreign_objects_fail() {
        let storage = MemoryStorage::default();
        let store = MemoryContextStore::default();
        let contexts = Contexts::new(&storage, &store);
        let acme = identity("acme", 1);
        let (record, _) = contexts
            .publish(&acme, &context(&acme))
            .await
            .expect("publish");
        storage.delete(&record.key).await.expect("delete");
        assert_eq!(
            contexts.read(&acme, &record.id).await,
            Err(ContextError::NotFound {
                id: record.id.clone()
            })
        );
        storage.create(&record.key, b"{}").await.expect("corrupt");
        assert!(matches!(
            contexts.read(&acme, &record.id).await,
            Err(ContextError::Corrupt { .. })
        ));
        let other = identity("zeta", 1);
        let foreign_id = ContextId::new("foreign").expect("id");
        let foreign = ContextRecord {
            id: foreign_id.clone(),
            key: ObjectKey::context(&other, &foreign_id),
            ..record.clone()
        };
        store.upsert_context(&acme, &foreign).await.expect("seed");
        assert_eq!(
            contexts.read(&acme, &foreign_id).await,
            Err(ContextError::ForeignKey { key: foreign.key })
        );
        let unknown = ContextId::new("unknown").expect("id");
        assert_eq!(
            contexts.read(&acme, &unknown).await,
            Err(ContextError::NotFound { id: unknown })
        );
    }

    /// Identical content published twice gets two random ids, each with
    /// its own store-stamped expiry; a sweep that listed the first before
    /// the second was published removes only the first.
    #[tokio::test]
    async fn identical_publications_expire_and_clean_up_independently() {
        let storage = MemoryStorage::default();
        let store = MemoryContextStore::default();
        let contexts = Contexts::new(&storage, &store);
        let acme = identity("acme", 1);
        let (first, _) = contexts
            .publish(&acme, &context(&acme))
            .await
            .expect("publish");
        store.clock.advance(CONTEXT_LIFETIME);
        storage.hold_deletes.store(true, Ordering::SeqCst);
        let (sweep, second) = tokio::join!(contexts.collect_expired(10), async {
            let (second, _) = contexts
                .publish(&acme, &context(&acme))
                .await
                .expect("publish");
            assert_eq!(
                contexts.read(&acme, &first.id).await,
                Err(ContextError::Expired {
                    id: first.id.clone()
                })
            );
            storage.hold_deletes.store(false, Ordering::SeqCst);
            second
        });
        assert_ne!(first.id, second.id);
        assert_ne!(first.key, second.key);
        assert_eq!(first.digest, second.digest);
        assert_eq!(
            sweep.expect("sweep").deleted,
            [(acme.clone(), first.id.clone())]
        );
        assert_eq!(
            contexts.read(&acme, &first.id).await,
            Err(ContextError::NotFound {
                id: first.id.clone()
            })
        );
        assert_eq!(contexts.read(&acme, &second.id).await, Ok(context(&acme)));
        assert_eq!(store.ids(), [(acme.clone(), second.id.clone())]);
        assert_eq!(storage.keys(), [second.key]);
    }

    /// Cleanup deletes each expired object then its record, treats a
    /// missing object as success, keeps a record whose delete failed as
    /// lag, and converges on the next sweep; unexpired contexts stay.
    #[tokio::test]
    async fn expired_cleanup_deletes_object_then_record_and_reports_lag() {
        let storage = MemoryStorage::default();
        let store = MemoryContextStore::default();
        let contexts = Contexts::new(&storage, &store);
        let (acme, zeta) = (identity("acme", 1), identity("zeta", 1));
        let (old, _) = contexts
            .publish(&acme, &context(&acme))
            .await
            .expect("publish");
        let (missing, _) = contexts
            .publish(&zeta, &context(&zeta))
            .await
            .expect("publish");
        storage.delete(&missing.key).await.expect("delete");
        store.clock.advance(CONTEXT_LIFETIME);
        let mut fresh_context = context(&acme);
        fresh_context.pull_request = 6;
        let (fresh, _) = contexts
            .publish(&acme, &fresh_context)
            .await
            .expect("publish");

        storage.fail_next_delete();
        let sweep = contexts.collect_expired(10).await.expect("sweep");
        assert_eq!(sweep.deleted, [(zeta.clone(), missing.id.clone())]);
        assert_eq!(sweep.lagging.len(), 1);
        assert_eq!((&sweep.lagging[0].0, &sweep.lagging[0].1), (&acme, &old.id));
        assert!(storage.keys().contains(&old.key));
        let mut recorded = vec![
            (acme.clone(), old.id.clone()),
            (acme.clone(), fresh.id.clone()),
        ];
        recorded.sort();
        assert_eq!(store.ids(), recorded);

        let retry = contexts.collect_expired(10).await.expect("retry");
        assert_eq!(retry.deleted, [(acme.clone(), old.id.clone())]);
        assert!(retry.lagging.is_empty());
        assert_eq!(store.ids(), [(acme.clone(), fresh.id.clone())]);
        assert_eq!(storage.keys(), [fresh.key]);
    }
}
