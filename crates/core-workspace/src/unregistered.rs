//! Linked worktrees git lists for a project that no workspace row records (ADR 0008). They are reported, with git's
//! lock and prune state, so the user can see what blocks a name or path; none is removed.

use std::path::Path;

use massorca_core_store::ProjectKind as StoreKind;
use massorca_protocol::methods::workspaces::UnregisteredWorktree;

use crate::error::DomainError;
use crate::paths;
use crate::workspaces::Workspaces;

impl Workspaces {
    /// The unregistered worktrees of every git project, or of `project` (a selector), under the workspaces root or
    /// elsewhere. Read under the mutation lock, so a create in progress is never reported half-done. A project whose
    /// directory is gone is skipped; its rows still list as missing.
    pub fn unregistered_worktrees(
        &self,
        project: Option<&str>,
    ) -> Result<Vec<UnregisteredWorktree>, DomainError> {
        let _guard = self.lock_mutations();
        let projects = match project {
            Some(selector) => vec![self.resolve_project(selector)?],
            None => self.store.list_projects()?,
        };
        let mut found = Vec::new();
        for project in projects
            .iter()
            .filter(|project| matches!(project.kind, StoreKind::Git))
        {
            let repo = Path::new(&project.path);
            if !repo.is_dir() {
                continue;
            }
            // Git lists the main worktree first; it is never a workspace.
            for entry in self.git.worktree_list(repo)?.iter().skip(1) {
                let key = paths::path_key(&entry.path);
                if self.store.find_workspace_by_path_key(&key)?.is_some() {
                    continue;
                }
                found.push(UnregisteredWorktree {
                    project_id: project.id.clone(),
                    path: paths::path_text(&paths::canonical(&entry.path))?,
                    branch: entry.branch.clone(),
                    locked: entry.locked.clone(),
                    prunable: entry.prunable.clone(),
                });
            }
        }
        Ok(found)
    }
}
