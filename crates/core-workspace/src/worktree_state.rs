//! Git's lock and prune state of workspace worktrees, read from `git worktree list` (ADR 0008).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use massorca_core_git::WorktreeEntry;
use massorca_core_store::{ProjectKind, WorkspaceRow};
use massorca_protocol::methods::workspaces::Workspace;

use crate::error::DomainError;
use crate::paths;
use crate::views;
use crate::workspaces::Workspaces;

impl Workspaces {
    /// The rows as workspaces, each with its worktree's lock and prune state. Git lists each
    /// git project once; a listing that fails leaves those flags unknown instead of failing the read.
    pub(crate) fn workspaces_with_git_state(
        &self,
        rows: &[WorkspaceRow],
    ) -> Result<Vec<Workspace>, DomainError> {
        let mut entries: HashMap<String, WorktreeEntry> = HashMap::new();
        let mut listed: HashSet<&str> = HashSet::new();
        for row in rows {
            if !listed.insert(row.project_id.as_str()) {
                continue;
            }
            let Some(project) = self.store.get_project(&row.project_id)? else {
                continue;
            };
            let repo = Path::new(&project.path);
            if !matches!(project.kind, ProjectKind::Git) || !repo.is_dir() {
                continue;
            }
            if let Ok(found) = self.git.worktree_list(repo) {
                for entry in found {
                    entries.insert(paths::path_key(&entry.path), entry);
                }
            }
        }
        Ok(rows
            .iter()
            .map(|row| {
                let workspace = self.view(row);
                match entries.get(&row.path_key) {
                    Some(entry) => views::with_worktree_state(workspace, entry),
                    None => workspace,
                }
            })
            .collect())
    }
}
