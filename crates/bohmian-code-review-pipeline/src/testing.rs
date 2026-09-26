//! In-memory test doubles shared by facade tests: a URL-routed fake GitHub
//! transport, create-only object storage, and identity-scoped persistence.
//!
//! These prove publication order, retry convergence, and isolation without
//! a network, disk, or database. They are compiled only for tests.

use crate::github::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use crate::identity::{Identity, ObjectSha};
use crate::persistence::{
    ContextId, ContextRecord, Persistence, PersistenceError, SnapshotPointer, SnapshotScope,
};
use crate::storage::{ObjectKey, ObjectStorage, StorageError};
use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

/// Fake transport answering by URL suffix; unmatched requests get 404.
#[derive(Default)]
pub(crate) struct RoutedTransport {
    routes: Mutex<Vec<(String, u16, Vec<u8>)>>,
    seen: Mutex<Vec<String>>,
    on_request: Mutex<Option<Box<dyn Fn(&str) + Send>>>,
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

    /// Run `hook` with every requested URL before answering it.
    pub(crate) fn on_request(&self, hook: impl Fn(&str) + Send + 'static) {
        *self.on_request.lock().expect("hook lock") = Some(Box::new(hook));
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
    fn get(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.seen
            .lock()
            .expect("seen lock")
            .push(request.url.clone());
        if let Some(hook) = self.on_request.lock().expect("hook lock").as_ref() {
            hook(&request.url);
        }
        let routes = self.routes.lock().expect("routes lock");
        let (status, body) = routes
            .iter()
            .find(|(suffix, _, _)| request.url.ends_with(suffix))
            .map_or((404, Vec::new()), |(_, status, body)| (*status, body.clone()));
        Ok(HttpResponse {
            status,
            headers: Vec::new(),
            body: Box::new(Cursor::new(body)),
        })
    }
}

/// Create-only in-memory storage with a one-shot create failure switch.
#[derive(Default)]
pub(crate) struct MemoryStorage {
    objects: Mutex<BTreeMap<ObjectKey, Vec<u8>>>,
    fail_next_create: AtomicBool,
}

impl MemoryStorage {
    /// Make the next `create` fail after nothing was written.
    pub(crate) fn fail_next_create(&self) {
        self.fail_next_create.store(true, Ordering::SeqCst);
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
    fn create(&self, key: &ObjectKey, bytes: &[u8]) -> Result<(), StorageError> {
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

    /// Exact-key read.
    ///
    /// # Errors
    ///
    /// `NotFound` when absent.
    fn read(&self, key: &ObjectKey) -> Result<Vec<u8>, StorageError> {
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
    fn delete(&self, key: &ObjectKey) -> Result<(), StorageError> {
        self.objects.lock().expect("objects lock").remove(key);
        Ok(())
    }
}

/// In-memory persistence with conditional pointer publication and a
/// one-shot publish failure switch.
#[derive(Default)]
pub(crate) struct MemoryPersistence {
    pointers: Mutex<HashMap<SnapshotScope, SnapshotPointer>>,
    contexts: Mutex<BTreeMap<(Identity, ContextId), ContextRecord>>,
    fail_next_publish: AtomicBool,
}

impl MemoryPersistence {
    /// Make the next pointer publication fail without moving the pointer.
    pub(crate) fn fail_next_publish(&self) {
        self.fail_next_publish.store(true, Ordering::SeqCst);
    }
}

impl Persistence for MemoryPersistence {
    /// Pointer for `scope`.
    ///
    /// # Errors
    ///
    /// Never.
    fn snapshot_pointer(
        &self,
        scope: &SnapshotScope,
    ) -> Result<Option<SnapshotPointer>, PersistenceError> {
        Ok(self.pointers.lock().expect("pointers lock").get(scope).cloned())
    }

    /// Compare-and-set on the current pointer's tree.
    ///
    /// # Errors
    ///
    /// `Backend` when the failure switch was set.
    fn publish_snapshot_pointer(
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

    /// Insert once per identity and id.
    ///
    /// # Errors
    ///
    /// `AlreadyPublished` for a duplicate id.
    fn publish_context(
        &self,
        identity: &Identity,
        record: &ContextRecord,
    ) -> Result<(), PersistenceError> {
        let mut contexts = self.contexts.lock().expect("contexts lock");
        let key = (identity.clone(), record.id.clone());
        if contexts.contains_key(&key) {
            return Err(PersistenceError::AlreadyPublished {
                id: record.id.clone(),
            });
        }
        contexts.insert(key, record.clone());
        Ok(())
    }

    /// Exact identity-scoped lookup.
    ///
    /// # Errors
    ///
    /// Never.
    fn context(
        &self,
        identity: &Identity,
        id: &ContextId,
    ) -> Result<Option<ContextRecord>, PersistenceError> {
        Ok(self
            .contexts
            .lock()
            .expect("contexts lock")
            .get(&(identity.clone(), id.clone()))
            .cloned())
    }

    /// Records expired at `now`, oldest first by map order.
    ///
    /// # Errors
    ///
    /// Never.
    fn expired_contexts(
        &self,
        now: SystemTime,
        limit: usize,
    ) -> Result<Vec<(Identity, ContextRecord)>, PersistenceError> {
        Ok(self
            .contexts
            .lock()
            .expect("contexts lock")
            .iter()
            .filter(|(_, record)| record.expires_at <= now)
            .take(limit)
            .map(|((identity, _), record)| (identity.clone(), record.clone()))
            .collect())
    }

    /// Idempotent delete.
    ///
    /// # Errors
    ///
    /// Never.
    fn delete_context(&self, identity: &Identity, id: &ContextId) -> Result<(), PersistenceError> {
        self.contexts
            .lock()
            .expect("contexts lock")
            .remove(&(identity.clone(), id.clone()));
        Ok(())
    }

    /// Drop every row for `identity` and return the object keys they named.
    ///
    /// # Errors
    ///
    /// Never.
    fn remove_repository(&self, identity: &Identity) -> Result<Vec<ObjectKey>, PersistenceError> {
        let mut keys = Vec::new();
        self.pointers
            .lock()
            .expect("pointers lock")
            .retain(|scope, pointer| {
                let keep = scope.identity != *identity;
                if !keep {
                    keys.push(pointer.key.clone());
                }
                keep
            });
        self.contexts
            .lock()
            .expect("contexts lock")
            .retain(|(owner, _), record| {
                let keep = owner != identity;
                if !keep {
                    keys.push(record.key.clone());
                }
                keep
            });
        Ok(keys)
    }
}
