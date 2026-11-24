use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use massorca_core_git::Git;
use massorca_core_store::Store;
use massorca_protocol::methods::workspaces::Workspace;

use crate::branch_prefix::BranchPrefix;
use crate::error::DomainError;
use crate::layout::WorkspaceLayout;
use crate::views;

/// The projects and workspaces domain API. Share one value across threads behind an `Arc`.
#[derive(Debug)]
pub struct Workspaces {
    pub(crate) store: Store,
    pub(crate) git: Git,
    pub(crate) default_root: PathBuf,
    /// How new branches are prefixed; `none` unless a caller asks otherwise.
    pub(crate) branch_prefix: BranchPrefix,
    /// The layout a project added through this handle gets under the default root.
    pub(crate) layout: WorkspaceLayout,
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
            branch_prefix: BranchPrefix::None,
            layout: WorkspaceLayout::default(),
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
            branch_prefix: self.branch_prefix.clone(),
            layout: self.layout,
            mutations: Arc::clone(&self.mutations),
        }
    }

    /// A handle over the same store, git and root that prefixes the branches it creates with
    /// `branch_prefix`. It shares the mutation lock, as `with_default_root` does.
    pub fn with_branch_prefix(&self, branch_prefix: BranchPrefix) -> Self {
        Self {
            store: self.store.clone(),
            git: self.git.clone(),
            default_root: self.default_root.clone(),
            branch_prefix,
            layout: self.layout,
            mutations: Arc::clone(&self.mutations),
        }
    }

    /// A handle over the same store and git that gives projects added through it `layout`.
    /// It shares the mutation lock, like `with_default_root`.
    pub fn with_layout(&self, layout: WorkspaceLayout) -> Self {
        Self {
            store: self.store.clone(),
            git: self.git.clone(),
            default_root: self.default_root.clone(),
            layout,
            branch_prefix: self.branch_prefix.clone(),
            mutations: Arc::clone(&self.mutations),
        }
    }

    /// Lets several agents write in a folder workspace at once (a Bot Teams team folder, decision 0072 amendment).
    /// A worktree is refused: its git index is never shared.
    pub fn share_writers(&self, id: &str) -> Result<(), DomainError> {
        if self.store.share_workspace_writers(id)? {
            return Ok(());
        }
        Err(DomainError::new(
            massorca_protocol::codes::REQUEST_INVALID_PARAMS,
            format!(
                "workspace {id} is not a folder workspace; only a folder can have several writers"
            ),
        ))
    }

    /// Whether several agents may write in the workspace at once. Every worktree, and every folder not marked, has one.
    pub fn shares_writers(&self, id: &str) -> Result<bool, DomainError> {
        Ok(self.store.workspace_shares_writers(id)?)
    }

    /// Where new workspace roots go unless a project names its own.
    pub fn default_root(&self) -> &std::path::Path {
        &self.default_root
    }

    /// The git runner the domain uses; git features run their commands through it.
    pub fn git(&self) -> &Git {
        &self.git
    }

    /// Every workspace, or only those of `project` (a selector), oldest first, with git's lock
    /// and prune state of each worktree.
    pub fn list_workspaces(&self, project: Option<&str>) -> Result<Vec<Workspace>, DomainError> {
        let project_id = match project {
            Some(selector) => Some(self.resolve_project(selector)?.id),
            None => None,
        };
        let rows = self.store.list_workspaces(project_id.as_deref())?;
        self.workspaces_with_git_state(&rows)
    }

    /// The workspace `selector` names: id, then exact name (unique within a project only),
    /// then path.
    pub fn show_workspace(&self, selector: &str) -> Result<Workspace, DomainError> {
        Ok(views::workspace(&self.resolve_workspace(selector)?))
    }

    /// Like `show_workspace`, with git's lock and prune state of the worktree. It runs git, so only
    /// `workspace.show` asks for it; every other lookup takes `show_workspace`.
    pub fn show_workspace_with_git_state(&self, selector: &str) -> Result<Workspace, DomainError> {
        let row = self.resolve_workspace(selector)?;
        // One row in, one workspace out.
        Ok(self
            .workspaces_with_git_state(std::slice::from_ref(&row))?
            .swap_remove(0))
    }

    /// Holds the mutation lock. A poisoned lock is still usable: it guards no data.
    pub(crate) fn lock_mutations(&self) -> MutexGuard<'_, ()> {
        self.mutations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
