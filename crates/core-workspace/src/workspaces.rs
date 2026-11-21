use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use massorca_core_git::Git;
use massorca_core_store::Store;


/// The projects and workspaces domain API. Share one value across threads behind an `Arc`.
#[derive(Debug)]
pub struct Workspaces {
    pub(crate) store: Store,
    pub(crate) git: Git,
    pub(crate) default_root: PathBuf,
    /// Serialises every mutation, so a name, path or branch checked before git runs
    /// cannot be taken by another call before the row is inserted.
    mutations: Mutex<()>,
}

impl Workspaces {
    /// `default_root` is where a project's workspaces go unless the project names another root.
    pub fn new(store: Store, git: Git, default_root: PathBuf) -> Self {
        Self {
            store,
            git,
            default_root,
            mutations: Mutex::new(()),
        }
    }

    /// Holds the mutation lock. A poisoned lock is still usable: it guards no data.
    pub(crate) fn lock_mutations(&self) -> MutexGuard<'_, ()> {
        self.mutations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
