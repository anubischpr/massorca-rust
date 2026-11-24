//! `workspace.check`: whether a workspace's folder is there and readable and, for a worktree, whether git still lists it.

use std::path::Path;

use massorca_core_store::{WorkspaceKind, WorkspaceRow};
use massorca_protocol::methods::workspaces::WorkspaceCheck;

use crate::error::DomainError;
use crate::paths;
use crate::views;
use crate::workspaces::Workspaces;

impl Workspaces {
    /// Reports what the workspace's folder shows now and, for a worktree, whether git still lists it. Changes nothing,
    /// and a failed check is a report, not an error.
    pub fn check_workspace(&self, selector: &str) -> Result<WorkspaceCheck, DomainError> {
        let row = self.resolve_workspace(selector)?;
        let folder = Path::new(&row.path);
        let exists = folder.is_dir();
        let readable = exists && std::fs::read_dir(folder).is_ok();
        let registered = match row.kind {
            WorkspaceKind::Folder => None,
            WorkspaceKind::Worktree => self.worktree_registered(&row)?,
        };
        let registration_ok = match row.kind {
            WorkspaceKind::Folder => true,
            WorkspaceKind::Worktree => registered == Some(true),
        };
        Ok(WorkspaceCheck {
            id: row.id.clone(),
            name: row.name.clone(),
            kind: views::workspace_kind(row.kind),
            path: row.path.clone(),
            exists,
            readable,
            registered,
            ok: exists && readable && registration_ok,
            reason: failure_reason(exists, readable, row.kind, registered),
        })
    }

    /// Whether git lists the worktree under its project's repository; `None` when git cannot answer.
    fn worktree_registered(&self, row: &WorkspaceRow) -> Result<Option<bool>, DomainError> {
        let Some(project) = self.store.get_project(&row.project_id)? else {
            return Ok(None);
        };
        let repo = Path::new(&project.path);
        if !repo.is_dir() {
            return Ok(None);
        }
        Ok(self.git.worktree_list(repo).ok().map(|entries| {
            entries
                .iter()
                .any(|entry| paths::path_key(&entry.path) == row.path_key)
        }))
    }
}

/// The first check that failed, in words; `None` when every check passed.
fn failure_reason(
    exists: bool,
    readable: bool,
    kind: WorkspaceKind,
    registered: Option<bool>,
) -> Option<String> {
    if !exists {
        return Some("the folder is missing".to_owned());
    }
    if !readable {
        return Some("the folder cannot be read".to_owned());
    }
    match (kind, registered) {
        (WorkspaceKind::Worktree, Some(false)) => {
            Some("git no longer lists this worktree".to_owned())
        }
        (WorkspaceKind::Worktree, None) => {
            Some("git could not list the worktrees of its project".to_owned())
        }
        _ => None,
    }
}
