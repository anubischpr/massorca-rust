use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use massorca_core_git::Git;
use massorca_core_store::Store;
use massorca_protocol::methods::workspaces::Workspace;

use crate::error::DomainError;
use crate::views;

/// The projects and workspaces domain API. Share one value across threads behind an `Arc`.
#[derive(Debug)]
pub struct Workspaces {
    pub(crate) store: Store,
    pub(crate) git: Git,
    pub(crate) default_root: PathBuf,
    /// Serialises every mutation, so a name, path or branch checked before git runs
    /// cannot be taken by another call before the row is inserted.
    mutations: Arc<Mutex<()>>,
}

impl Workspaces {
    /// `default_root` is where a project's workspaces go unless the project names another root.
    pub fn new(store: Store, git: Git, default_root: PathBuf) -> Self {
        Self {
            store,
            git,
            default_root,
            mutations: Arc::new(Mutex::new(())),
        }
    }

    /// A handle over the same store and git that uses `default_root` for new workspaces roots.
    /// It shares the mutation lock, so handles made this way serialise with each other. The
    /// service makes one per call, so a changed `workspaces.default_root` applies at once.
    pub fn with_default_root(&self, default_root: PathBuf) -> Self {
        Self {
            store: self.store.clone(),
            git: self.git.clone(),
            default_root,
            mutations: Arc::clone(&self.mutations),
        }
    }

    /// The git runner the domain uses; git features run their commands through it.
    pub fn git(&self) -> &Git {
        &self.git
    }

    /// Every workspace, or only those of `project` (a selector), oldest first.
    pub fn list_workspaces(&self, project: Option<&str>) -> Result<Vec<Workspace>, DomainError> {
        let project_id = match project {
            Some(selector) => Some(self.resolve_project(selector)?.id),
            None => None,
        };
        Ok(self
            .store
            .list_workspaces(project_id.as_deref())?
            .iter()
            .map(views::workspace)
            .collect())
    }

    /// The workspace `selector` names: id, then exact name (unique within a project only),
    /// then path.
    pub fn show_workspace(&self, selector: &str) -> Result<Workspace, DomainError> {
        Ok(views::workspace(&self.resolve_workspace(selector)?))
    }

    /// Holds the mutation lock. A poisoned lock is still usable: it guards no data.
    pub(crate) fn lock_mutations(&self) -> MutexGuard<'_, ()> {
        self.mutations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
