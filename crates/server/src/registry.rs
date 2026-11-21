//! Named contributions of one kind, each with an owner. Registering returns a handle; dropping the handle removes the entry.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak};

use crate::owner::Owner;

/// One registered contribution.
#[derive(Debug)]
pub struct Entry<K> {
    pub key: String,
    pub owner: Owner,
    pub value: K,
}

type Entries<K> = BTreeMap<String, Arc<Entry<K>>>;

/// Contributions of one kind, keyed by name. Clones share the table.
pub struct Registry<K> {
    entries: Arc<RwLock<Entries<K>>>,
}

impl<K> Registry<K> {
    /// The entry registered under `key`.
    pub fn get(&self, key: &str) -> Option<Arc<Entry<K>>> {
        read(&self.entries).get(key).cloned()
    }

    /// Every entry, ordered by key.
    pub fn list(&self) -> Vec<Arc<Entry<K>>> {
        read(&self.entries).values().cloned().collect()
    }
}

impl<K: Send + Sync + 'static> Registry<K> {
    /// Registers `value` under `key`. A key that is taken is refused, whoever owns it.
    pub fn register(
        &self,
        owner: Owner,
        key: impl Into<String>,
        value: K,
    ) -> Result<RegistrationHandle, RegistrationError> {
        let key = key.into();
        let mut entries = write(&self.entries);
        if entries.contains_key(&key) {
            return Err(RegistrationError::Duplicate { key });
        }
        entries.insert(
            key.clone(),
            Arc::new(Entry {
                key: key.clone(),
                owner,
                value,
            }),
        );
        drop(entries);

        // Weak, so a handle that outlives its registry does nothing when dropped.
        let table: Weak<RwLock<Entries<K>>> = Arc::downgrade(&self.entries);
        Ok(RegistrationHandle {
            unregister: Some(Box::new(move || {
                if let Some(table) = table.upgrade() {
                    write(&table).remove(&key);
                }
            })),
        })
    }
}

impl<K> Clone for Registry<K> {
    fn clone(&self) -> Self {
        Self {
            entries: Arc::clone(&self.entries),
        }
    }
}

impl<K> Default for Registry<K> {
    fn default() -> Self {
        Self {
            entries: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

impl<K> fmt::Debug for Registry<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("entries", &read(&self.entries).len())
            .finish()
    }
}

/// Keeps a registration in place: dropping it removes the entry. Hold it for as long as the contribution should be offered.
#[must_use = "dropping the handle at once removes the registration"]
pub struct RegistrationHandle {
    unregister: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl Drop for RegistrationHandle {
    fn drop(&mut self) {
        if let Some(unregister) = self.unregister.take() {
            unregister();
        }
    }
}

impl fmt::Debug for RegistrationHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegistrationHandle").finish_non_exhaustive()
    }
}

/// Why a registration was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistrationError {
    #[error("{key} is already registered")]
    Duplicate { key: String },
}

fn read<K>(table: &RwLock<Entries<K>>) -> RwLockReadGuard<'_, Entries<K>> {
    table.read().unwrap_or_else(PoisonError::into_inner)
}

fn write<K>(table: &RwLock<Entries<K>>) -> RwLockWriteGuard<'_, Entries<K>> {
    table.write().unwrap_or_else(PoisonError::into_inner)
}
