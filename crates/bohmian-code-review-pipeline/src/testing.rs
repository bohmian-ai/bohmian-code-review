//! In-memory test doubles shared by facade tests: a fixed response body, a
//! URL-routed fake GitHub transport, create-only object storage, and an identity-scoped snapshot store.
//!
//! These prove publication order, retry convergence, and isolation without
//! a network, disk, or database. They are compiled only for tests.

use crate::github::{
    BoxFuture, HttpBody, HttpRequest, HttpResponse, HttpTransport, TransportError,
};
use crate::identity::{Identity, ObjectSha};
use crate::persistence::{PersistenceError, SnapshotPointer, SnapshotScope, SnapshotStore};
use crate::storage::{ObjectKey, ObjectStorage, StorageError};
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

/// Response body delivered as one chunk, then end of body.
pub(crate) struct FixedBody(Option<Vec<u8>>);

impl FixedBody {
    /// A body carrying exactly `bytes`.
    pub(crate) fn new(bytes: Vec<u8>) -> FixedBody {
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

/// Fake transport answering by URL suffix; unmatched requests get 404.
#[derive(Default)]
pub(crate) struct RoutedTransport {
    routes: Mutex<Vec<(String, u16, Vec<u8>)>>,
    seen: Mutex<Vec<String>>,
}

impl RoutedTransport {
    /// Answer requests whose URL ends with `suffix` with `status` and
    /// `body`; a later route for the same suffix overrides an earlier one.
    pub(crate) fn route(&self, suffix: &str, status: u16, body: impl Into<Vec<u8>>) {
        self.routes
            .lock()
            .expect("routes lock")
            .insert(0, (suffix.to_owned(), status, body.into()));
    }

    /// Answer with JSON 200.
    pub(crate) fn json(&self, suffix: &str, json: &str) {
        self.route(suffix, 200, json);
    }

    /// URLs requested so far.
    pub(crate) fn seen(&self) -> Vec<String> {
        self.seen.lock().expect("seen lock").clone()
    }
}

impl HttpTransport for RoutedTransport {
    /// Serves the first matching route.
    ///
    /// # Errors
    ///
    /// Never; unmatched URLs are answered with 404.
    fn get(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
        self.seen
            .lock()
            .expect("seen lock")
            .push(request.url.clone());
        let routes = self.routes.lock().expect("routes lock");
        let (status, body) = routes
            .iter()
            .find(|(suffix, _, _)| request.url.ends_with(suffix))
            .map_or((404, Vec::new()), |(_, status, body)| {
                (*status, body.clone())
            });
        Box::pin(async move {
            Ok(HttpResponse {
                status,
                headers: Vec::new(),
                body: Box::new(FixedBody::new(body)),
            })
        })
    }
}

/// Create-only in-memory storage with one-shot create and read failure
/// switches.
#[derive(Default)]
pub(crate) struct MemoryStorage {
    objects: Mutex<BTreeMap<ObjectKey, Vec<u8>>>,
    fail_next_create: AtomicBool,
    fail_next_read: AtomicBool,
}

impl MemoryStorage {
    /// Make the next `create` fail after nothing was written.
    pub(crate) fn fail_next_create(&self) {
        self.fail_next_create.store(true, Ordering::SeqCst);
    }

    /// Make the next `read` fail with a backend error; stored objects stay.
    pub(crate) fn fail_next_read(&self) {
        self.fail_next_read.store(true, Ordering::SeqCst);
    }

    /// Every stored key in order.
    pub(crate) fn keys(&self) -> Vec<ObjectKey> {
        self.objects
            .lock()
            .expect("objects lock")
            .keys()
            .cloned()
            .collect()
    }
}

impl ObjectStorage for MemoryStorage {
    /// Refuses overwrites and honours the failure switch.
    ///
    /// # Errors
    ///
    /// `AlreadyExists` for a taken key, `Backend` when the switch was set.
    async fn create(&self, key: &ObjectKey, bytes: &[u8]) -> Result<(), StorageError> {
        if self.fail_next_create.swap(false, Ordering::SeqCst) {
            return Err(StorageError::Backend {
                detail: "injected create failure".to_owned(),
            });
        }
        let mut objects = self.objects.lock().expect("objects lock");
        if objects.contains_key(key) {
            return Err(StorageError::AlreadyExists { key: key.clone() });
        }
        objects.insert(key.clone(), bytes.to_vec());
        Ok(())
    }

    /// Exact-key read that honours the read failure switch.
    ///
    /// # Errors
    ///
    /// `Backend` when the switch was set, `NotFound` when absent.
    async fn read(&self, key: &ObjectKey) -> Result<Vec<u8>, StorageError> {
        if self.fail_next_read.swap(false, Ordering::SeqCst) {
            return Err(StorageError::Backend {
                detail: "injected read failure".to_owned(),
            });
        }
        self.objects
            .lock()
            .expect("objects lock")
            .get(key)
            .cloned()
            .ok_or_else(|| StorageError::NotFound { key: key.clone() })
    }

    /// Idempotent delete.
    ///
    /// # Errors
    ///
    /// Never.
    async fn delete(&self, key: &ObjectKey) -> Result<(), StorageError> {
        self.objects.lock().expect("objects lock").remove(key);
        Ok(())
    }
}

/// In-memory snapshot store with conditional pointer publication, snapshot
/// records keyed by identity, and a one-shot publish failure switch.
#[derive(Default)]
pub(crate) struct MemorySnapshotStore {
    pointers: Mutex<HashMap<SnapshotScope, SnapshotPointer>>,
    records: Mutex<BTreeMap<(ObjectKey, Identity), SystemTime>>,
    fail_next_publish: AtomicBool,
}

impl MemorySnapshotStore {
    /// Make the next pointer publication fail without moving the pointer.
    pub(crate) fn fail_next_publish(&self) {
        self.fail_next_publish.store(true, Ordering::SeqCst);
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

    /// Compare-and-set on the current pointer's tree.
    ///
    /// # Errors
    ///
    /// `Backend` when the failure switch was set.
    async fn publish_snapshot_pointer(
        &self,
        scope: &SnapshotScope,
        expected: Option<&ObjectSha>,
        pointer: &SnapshotPointer,
    ) -> Result<bool, PersistenceError> {
        if self.fail_next_publish.swap(false, Ordering::SeqCst) {
            return Err(PersistenceError::Backend {
                detail: "injected publish failure".to_owned(),
            });
        }
        let mut pointers = self.pointers.lock().expect("pointers lock");
        let current = pointers.get(scope).map(|pointer| &pointer.tree);
        if current != expected {
            return Ok(false);
        }
        pointers.insert(scope.clone(), pointer.clone());
        Ok(true)
    }

    /// Insert or refresh the record of `key` under `identity`.
    ///
    /// # Errors
    ///
    /// Never.
    async fn record_snapshot(
        &self,
        identity: &Identity,
        key: &ObjectKey,
        recorded_at: SystemTime,
    ) -> Result<(), PersistenceError> {
        self.records
            .lock()
            .expect("records lock")
            .insert((key.clone(), identity.clone()), recorded_at);
        Ok(())
    }

    /// Records older than `before` whose key no pointer of the same
    /// identity names, in key order.
    ///
    /// # Errors
    ///
    /// Never.
    async fn unreferenced_snapshots(
        &self,
        before: SystemTime,
        limit: usize,
    ) -> Result<Vec<(Identity, ObjectKey)>, PersistenceError> {
        let pointers = self.pointers.lock().expect("pointers lock");
        let referenced = |identity: &Identity, key: &ObjectKey| {
            pointers
                .iter()
                .any(|(scope, pointer)| scope.identity == *identity && pointer.key == *key)
        };
        Ok(self
            .records
            .lock()
            .expect("records lock")
            .iter()
            .filter(|((key, identity), recorded_at)| {
                **recorded_at < before && !referenced(identity, key)
            })
            .take(limit)
            .map(|((key, identity), _)| (identity.clone(), key.clone()))
            .collect())
    }

    /// Idempotent removal of one record under `identity`.
    ///
    /// # Errors
    ///
    /// Never.
    async fn delete_snapshot(
        &self,
        identity: &Identity,
        key: &ObjectKey,
    ) -> Result<(), PersistenceError> {
        self.records
            .lock()
            .expect("records lock")
            .remove(&(key.clone(), identity.clone()));
        Ok(())
    }

    /// Drop every pointer and record for `identity` and return every
    /// recorded key.
    ///
    /// # Errors
    ///
    /// Never.
    async fn remove_repository(
        &self,
        identity: &Identity,
    ) -> Result<Vec<ObjectKey>, PersistenceError> {
        self.pointers
            .lock()
            .expect("pointers lock")
            .retain(|scope, _| scope.identity != *identity);
        let mut keys = Vec::new();
        self.records
            .lock()
            .expect("records lock")
            .retain(|(key, owner), _| {
                let keep = owner != identity;
                if !keep {
                    keys.push(key.clone());
                }
                keep
            });
        Ok(keys)
    }
}
