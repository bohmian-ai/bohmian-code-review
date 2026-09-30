//! In-memory test doubles shared by facade tests: a fixed response body, a
//! URL-routed fake GitHub transport, create-if-absent object storage, and
//! identity-scoped snapshot and context stores.
//!
//! These prove publication order, retry convergence, and isolation without
//! a network, disk, or database. They are compiled for owner tests and,
//! behind the `testing` feature, for the outer-crate journey and the live
//! pull-request command, which inject them in place of a consumer's stores.

use crate::context::CONTEXT_LIFETIME;
use crate::github::{
    BoxFuture, HttpBody, HttpRequest, HttpResponse, HttpTransport, TransportError,
};
use crate::identity::{Identity, ObjectSha};
use crate::persistence::{
    ContextId, ContextLifetime, ContextLookup, ContextRecord, ContextStore, PersistenceError,
    SnapshotPointer, SnapshotScope, SnapshotStore,
};
use crate::storage::{ObjectKey, ObjectStorage, StorageError};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Response body delivered as one chunk, then end of body.
pub struct FixedBody(Option<Vec<u8>>);

impl FixedBody {
    /// A body carrying exactly `bytes`.
    pub fn new(bytes: Vec<u8>) -> FixedBody {
        FixedBody(Some(bytes))
    }
}

impl HttpBody for FixedBody {
    /// Yields the whole body once, then `None`.
    ///
    /// # Errors
    ///
    /// Never.
    fn chunk(&mut self) -> BoxFuture<'_, Result<Option<Vec<u8>>, TransportError>> {
        let chunk = self.0.take();
        Box::pin(async move { Ok(chunk) })
    }
}

/// One scripted response: status, headers, body.
pub type Response = (u16, Vec<(String, String)>, Vec<u8>);

/// Fake transport answering by URL suffix; unmatched requests get 404.
#[derive(Default)]
pub struct RoutedTransport {
    routes: Mutex<Vec<(String, VecDeque<Response>)>>,
    seen: Mutex<Vec<String>>,
}

impl RoutedTransport {
    /// Answer requests whose URL ends with `suffix` with `status` and
    /// `body`; a later route for the same suffix overrides an earlier one.
    pub fn route(&self, suffix: &str, status: u16, body: impl Into<Vec<u8>>) {
        self.respond(suffix, vec![(status, Vec::new(), body.into())]);
    }

    /// Answer requests whose URL ends with `suffix` with `responses` in
    /// order, repeating the last; overrides earlier routes for `suffix`.
    pub fn respond(&self, suffix: &str, responses: Vec<Response>) {
        self.routes
            .lock()
            .expect("routes lock")
            .insert(0, (suffix.to_owned(), responses.into()));
    }

    /// Answer with JSON 200.
    pub fn json(&self, suffix: &str, json: &str) {
        self.route(suffix, 200, json);
    }

    /// URLs requested so far.
    pub fn seen(&self) -> Vec<String> {
        self.seen.lock().expect("seen lock").clone()
    }
}

impl HttpTransport for RoutedTransport {
    /// Serves the next response of the first matching route.
    ///
    /// # Errors
    ///
    /// Never; unmatched URLs are answered with 404.
    fn get(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
        self.seen
            .lock()
            .expect("seen lock")
            .push(request.url.clone());
        let mut routes = self.routes.lock().expect("routes lock");
        let (status, headers, body) = routes
            .iter_mut()
            .find(|(suffix, _)| request.url.ends_with(suffix.as_str()))
            .and_then(|(_, responses)| {
                if responses.len() > 1 {
                    responses.pop_front()
                } else {
                    responses.front().cloned()
                }
            })
            .unwrap_or((404, Vec::new(), Vec::new()));
        Box::pin(async move {
            Ok(HttpResponse {
                status,
                headers,
                body: Box::new(FixedBody::new(body)),
            })
        })
    }
}

/// Wait forever: a stalled step whose future the test drops with a
/// paused-clock `tokio::time::timeout`.
async fn stall(switch: &AtomicBool) {
    if switch.swap(false, Ordering::SeqCst) {
        std::future::pending::<()>().await;
    }
}

/// Yield to other futures while `held` is set: a step paused mid-flight so
/// a test can interleave another facade call, then release it.
async fn hold(held: &AtomicBool) {
    while held.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }
}

/// Create-if-absent in-memory storage with a one-shot stall after a create,
/// a one-shot delete failure, and a releasable hold before deletes.
#[derive(Default)]
pub struct MemoryStorage {
    objects: Mutex<BTreeMap<ObjectKey, Vec<u8>>>,
    stall_after_create: AtomicBool,
    fail_next_delete: AtomicBool,
    pub hold_deletes: AtomicBool,
}

impl MemoryStorage {
    /// Make the next `create` store its object, then never return.
    pub fn stall_after_create(&self) {
        self.stall_after_create.store(true, Ordering::SeqCst);
    }

    /// Make the next `delete` fail with the object still present.
    pub fn fail_next_delete(&self) {
        self.fail_next_delete.store(true, Ordering::SeqCst);
    }

    /// Every stored key in order.
    pub fn keys(&self) -> Vec<ObjectKey> {
        self.objects
            .lock()
            .expect("objects lock")
            .keys()
            .cloned()
            .collect()
    }
}

impl ObjectStorage for MemoryStorage {
    /// Refuses overwrites; honours the stall switch after storing.
    ///
    /// # Errors
    ///
    /// `AlreadyExists` for a taken key.
    async fn create(&self, key: &ObjectKey, bytes: &[u8]) -> Result<(), StorageError> {
        {
            let mut objects = self.objects.lock().expect("objects lock");
            if objects.contains_key(key) {
                return Err(StorageError::AlreadyExists { key: key.clone() });
            }
            objects.insert(key.clone(), bytes.to_vec());
        }
        stall(&self.stall_after_create).await;
        Ok(())
    }

    /// Exact-key read.
    ///
    /// # Errors
    ///
    /// `NotFound` when absent.
    async fn read(&self, key: &ObjectKey) -> Result<Vec<u8>, StorageError> {
        self.objects
            .lock()
            .expect("objects lock")
            .get(key)
            .cloned()
            .ok_or_else(|| StorageError::NotFound { key: key.clone() })
    }

    /// Delete where an absent key succeeds; honours the failure switch.
    ///
    /// # Errors
    ///
    /// `Backend` when the switch was set; the object stays.
    async fn delete(&self, key: &ObjectKey) -> Result<(), StorageError> {
        hold(&self.hold_deletes).await;
        if self.fail_next_delete.swap(false, Ordering::SeqCst) {
            return Err(StorageError::Backend {
                detail: "injected delete failure".to_owned(),
            });
        }
        self.objects.lock().expect("objects lock").remove(key);
        Ok(())
    }
}

/// A store-owned clock that starts at the Unix epoch and moves only when a
/// test advances it, standing in for a database clock.
#[derive(Default)]
pub struct ManualClock(Mutex<Duration>);

impl ManualClock {
    /// The store's current time.
    fn now(&self) -> SystemTime {
        UNIX_EPOCH + *self.0.lock().expect("clock lock")
    }

    /// Move the clock forward by `by`.
    pub fn advance(&self, by: Duration) {
        *self.0.lock().expect("clock lock") += by;
    }
}

/// In-memory snapshot store: pointers by scope with compare-and-swap, and
/// records keyed by identity with `touched_at` and a collection claim, all
/// timed by its own [`ManualClock`], with
/// one-shot stalls after a record upsert and before a swap and a
/// releasable hold before swaps.
#[derive(Default)]
pub struct MemorySnapshotStore {
    pointers: Mutex<HashMap<SnapshotScope, SnapshotPointer>>,
    records: Mutex<BTreeMap<(Identity, ObjectKey), (SystemTime, bool)>>,
    stall_after_upsert: AtomicBool,
    stall_before_swap: AtomicBool,
    pub hold_swaps: AtomicBool,
    pub clock: ManualClock,
}

impl MemorySnapshotStore {
    /// Make the next `upsert_record` write its record, then never return.
    pub fn stall_after_upsert(&self) {
        self.stall_after_upsert.store(true, Ordering::SeqCst);
    }

    /// Make the next pointer swap never return, without moving the pointer.
    pub fn stall_before_swap(&self) {
        self.stall_before_swap.store(true, Ordering::SeqCst);
    }

    /// Every `(identity, key)` record in order.
    pub fn records(&self) -> Vec<(Identity, ObjectKey)> {
        self.records
            .lock()
            .expect("records lock")
            .keys()
            .cloned()
            .collect()
    }
}

impl SnapshotStore for MemorySnapshotStore {
    /// Pointer for `scope`.
    ///
    /// # Errors
    ///
    /// Never.
    async fn snapshot_pointer(
        &self,
        scope: &SnapshotScope,
    ) -> Result<Option<SnapshotPointer>, PersistenceError> {
        Ok(self
            .pointers
            .lock()
            .expect("pointers lock")
            .get(scope)
            .cloned())
    }

    /// Compare-and-swap on the current pointer's tree; honours the stall
    /// switch before comparing.
    ///
    /// # Errors
    ///
    /// Never.
    async fn publish_snapshot_pointer(
        &self,
        scope: &SnapshotScope,
        expected: Option<&ObjectSha>,
        pointer: &SnapshotPointer,
    ) -> Result<bool, PersistenceError> {
        stall(&self.stall_before_swap).await;
        hold(&self.hold_swaps).await;
        let mut pointers = self.pointers.lock().expect("pointers lock");
        if pointers.get(scope).map(|pointer| &pointer.tree) != expected {
            return Ok(false);
        }
        pointers.insert(scope.clone(), pointer.clone());
        Ok(true)
    }

    /// Insert or re-touch an unclaimed record at the store's now; honours
    /// the stall switch after writing.
    ///
    /// # Errors
    ///
    /// `Collecting` when the record is claimed.
    async fn upsert_record(
        &self,
        identity: &Identity,
        key: &ObjectKey,
    ) -> Result<(), PersistenceError> {
        let touched_at = self.clock.now();
        {
            let mut records = self.records.lock().expect("records lock");
            let record = records
                .entry((identity.clone(), key.clone()))
                .or_insert((touched_at, false));
            if record.1 {
                return Err(PersistenceError::Collecting { key: key.clone() });
            }
            record.0 = touched_at;
        }
        stall(&self.stall_after_upsert).await;
        Ok(())
    }

    /// Claim `identity`'s records touched at or before the store's now
    /// minus `grace` that none of its pointers names, in key order, under
    /// both locks.
    ///
    /// # Errors
    ///
    /// Never.
    async fn unreferenced(
        &self,
        identity: &Identity,
        grace: Duration,
    ) -> Result<Vec<ObjectKey>, PersistenceError> {
        let now = self.clock.now();
        let pointers = self.pointers.lock().expect("pointers lock");
        let referenced = |key: &ObjectKey| {
            pointers
                .iter()
                .any(|(scope, pointer)| scope.identity == *identity && pointer.key == *key)
        };
        let mut claimed = Vec::new();
        for ((owner, key), (touched_at, claim)) in
            self.records.lock().expect("records lock").iter_mut()
        {
            if owner == identity && *touched_at + grace <= now && !referenced(key) {
                *claim = true;
                claimed.push(key.clone());
            }
        }
        Ok(claimed)
    }

    /// Idempotent removal of one record under `identity`.
    ///
    /// # Errors
    ///
    /// Never.
    async fn delete_record(
        &self,
        identity: &Identity,
        key: &ObjectKey,
    ) -> Result<(), PersistenceError> {
        self.records
            .lock()
            .expect("records lock")
            .remove(&(identity.clone(), key.clone()));
        Ok(())
    }

    /// Drop every pointer of `identity`.
    ///
    /// # Errors
    ///
    /// Never.
    async fn remove_pointers(&self, identity: &Identity) -> Result<(), PersistenceError> {
        self.pointers
            .lock()
            .expect("pointers lock")
            .retain(|scope, _| scope.identity != *identity);
        Ok(())
    }
}

/// In-memory context store keyed by exact identity and id, stamping and
/// judging expiry on its own [`ManualClock`], with a one-shot stall after
/// an upsert.
#[derive(Default)]
pub struct MemoryContextStore {
    records: Mutex<BTreeMap<(Identity, ContextId), (ContextRecord, ContextLifetime)>>,
    stall_after_upsert: AtomicBool,
    pub clock: ManualClock,
}

impl MemoryContextStore {
    /// Make the next `upsert_context` write its record, then never return.
    pub fn stall_after_upsert(&self) {
        self.stall_after_upsert.store(true, Ordering::SeqCst);
    }

    /// Every `(identity, id)` record in order.
    pub fn ids(&self) -> Vec<(Identity, ContextId)> {
        self.records
            .lock()
            .expect("records lock")
            .keys()
            .cloned()
            .collect()
    }
}

impl ContextStore for MemoryContextStore {
    /// Insert stamped at the store's now, or keep an existing record's
    /// stamps; honours the stall switch after writing.
    ///
    /// # Errors
    ///
    /// Never.
    async fn upsert_context(
        &self,
        identity: &Identity,
        record: &ContextRecord,
    ) -> Result<ContextLifetime, PersistenceError> {
        let published_at = self.clock.now();
        let lifetime = self
            .records
            .lock()
            .expect("records lock")
            .entry((identity.clone(), record.id.clone()))
            .or_insert_with(|| {
                let lifetime = ContextLifetime {
                    published_at,
                    expires_at: published_at + CONTEXT_LIFETIME,
                };
                (record.clone(), lifetime)
            })
            .1;
        stall(&self.stall_after_upsert).await;
        Ok(lifetime)
    }

    /// Exact `(identity, id)` lookup, expired at or after `expires_at` on
    /// the store's clock; another identity's id is absent.
    ///
    /// # Errors
    ///
    /// Never.
    async fn context(
        &self,
        identity: &Identity,
        id: &ContextId,
    ) -> Result<ContextLookup, PersistenceError> {
        let now = self.clock.now();
        Ok(
            match self
                .records
                .lock()
                .expect("records lock")
                .get(&(identity.clone(), id.clone()))
            {
                None => ContextLookup::Absent,
                Some((_, lifetime)) if now >= lifetime.expires_at => ContextLookup::Expired,
                Some((record, _)) => ContextLookup::Live(record.clone()),
            },
        )
    }

    /// Up to `limit` records expired at or before the store's now, in key
    /// order.
    ///
    /// # Errors
    ///
    /// Never.
    async fn expired_contexts(
        &self,
        limit: usize,
    ) -> Result<Vec<(Identity, ContextRecord)>, PersistenceError> {
        let now = self.clock.now();
        Ok(self
            .records
            .lock()
            .expect("records lock")
            .iter()
            .filter(|(_, (_, lifetime))| lifetime.expires_at <= now)
            .take(limit)
            .map(|((identity, _), (record, _))| (identity.clone(), record.clone()))
            .collect())
    }

    /// Idempotent removal of one record under `identity`.
    ///
    /// # Errors
    ///
    /// Never.
    async fn delete_context(
        &self,
        identity: &Identity,
        id: &ContextId,
    ) -> Result<(), PersistenceError> {
        self.records
            .lock()
            .expect("records lock")
            .remove(&(identity.clone(), id.clone()));
        Ok(())
    }

    /// Drop every record of `identity`, returning their object keys.
    ///
    /// # Errors
    ///
    /// Never.
    async fn remove_repository(
        &self,
        identity: &Identity,
    ) -> Result<Vec<ObjectKey>, PersistenceError> {
        let mut records = self.records.lock().expect("records lock");
        let keys = records
            .iter()
            .filter(|((owner, _), _)| owner == identity)
            .map(|(_, (record, _))| record.key.clone())
            .collect();
        records.retain(|(owner, _), _| owner != identity);
        Ok(keys)
    }
}

/// One in-memory store serving snapshots and contexts, as a consumer's
/// single database-backed type does for [`crate::ReviewPipeline`]; each
/// half keeps its own clock and switches.
#[derive(Default)]
pub struct MemoryStore {
    /// Snapshot pointers and records.
    pub snapshots: MemorySnapshotStore,
    /// Context records.
    pub contexts: MemoryContextStore,
}

impl SnapshotStore for MemoryStore {
    /// As [`MemorySnapshotStore`].
    ///
    /// # Errors
    ///
    /// Never.
    async fn snapshot_pointer(
        &self,
        scope: &SnapshotScope,
    ) -> Result<Option<SnapshotPointer>, PersistenceError> {
        self.snapshots.snapshot_pointer(scope).await
    }

    /// As [`MemorySnapshotStore`].
    ///
    /// # Errors
    ///
    /// Never.
    async fn publish_snapshot_pointer(
        &self,
        scope: &SnapshotScope,
        expected: Option<&ObjectSha>,
        pointer: &SnapshotPointer,
    ) -> Result<bool, PersistenceError> {
        self.snapshots
            .publish_snapshot_pointer(scope, expected, pointer)
            .await
    }

    /// As [`MemorySnapshotStore`].
    ///
    /// # Errors
    ///
    /// `Collecting` for a claimed record.
    async fn upsert_record(
        &self,
        identity: &Identity,
        key: &ObjectKey,
    ) -> Result<(), PersistenceError> {
        self.snapshots.upsert_record(identity, key).await
    }

    /// As [`MemorySnapshotStore`].
    ///
    /// # Errors
    ///
    /// Never.
    async fn unreferenced(
        &self,
        identity: &Identity,
        grace: Duration,
    ) -> Result<Vec<ObjectKey>, PersistenceError> {
        self.snapshots.unreferenced(identity, grace).await
    }

    /// As [`MemorySnapshotStore`].
    ///
    /// # Errors
    ///
    /// Never.
    async fn delete_record(
        &self,
        identity: &Identity,
        key: &ObjectKey,
    ) -> Result<(), PersistenceError> {
        self.snapshots.delete_record(identity, key).await
    }

    /// As [`MemorySnapshotStore`].
    ///
    /// # Errors
    ///
    /// Never.
    async fn remove_pointers(&self, identity: &Identity) -> Result<(), PersistenceError> {
        self.snapshots.remove_pointers(identity).await
    }
}

impl ContextStore for MemoryStore {
    /// As [`MemoryContextStore`].
    ///
    /// # Errors
    ///
    /// Never.
    async fn upsert_context(
        &self,
        identity: &Identity,
        record: &ContextRecord,
    ) -> Result<ContextLifetime, PersistenceError> {
        self.contexts.upsert_context(identity, record).await
    }

    /// As [`MemoryContextStore`].
    ///
    /// # Errors
    ///
    /// Never.
    async fn context(
        &self,
        identity: &Identity,
        id: &ContextId,
    ) -> Result<ContextLookup, PersistenceError> {
        self.contexts.context(identity, id).await
    }

    /// As [`MemoryContextStore`].
    ///
    /// # Errors
    ///
    /// Never.
    async fn expired_contexts(
        &self,
        limit: usize,
    ) -> Result<Vec<(Identity, ContextRecord)>, PersistenceError> {
        self.contexts.expired_contexts(limit).await
    }

    /// As [`MemoryContextStore`].
    ///
    /// # Errors
    ///
    /// Never.
    async fn delete_context(
        &self,
        identity: &Identity,
        id: &ContextId,
    ) -> Result<(), PersistenceError> {
        self.contexts.delete_context(identity, id).await
    }

    /// As [`MemoryContextStore`].
    ///
    /// # Errors
    ///
    /// Never.
    async fn remove_repository(
        &self,
        identity: &Identity,
    ) -> Result<Vec<ObjectKey>, PersistenceError> {
        self.contexts.remove_repository(identity).await
    }
}
