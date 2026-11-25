//! `workspace.rename` (decision 0111): a name the service chose is replaced, and a worktree's unpublished branch with
//! it. The folder is never moved: a running agent's working directory stays where it is.

use std::path::Path;

use massorca_core_git::BranchError;
use massorca_core_store::{ProjectRow, WorkspaceKind, WorkspaceRow, WorkspaceState};
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceRenameParams};

use crate::branch_prefix;
use crate::create::{invalid, refused_write};
use crate::error::DomainError;
use crate::names;
use crate::unique_names::Folders;
use crate::views;
use crate::workspaces::Workspaces;

impl Workspaces {
    /// Renames a workspace whose name the service chose to the first free of `params.name`, `name-2` .. `name-100`.
    /// A worktree's branch is renamed too, under the branch prefix, unless it moved or was published. The flag stays
    /// set: the new name is still the service's.
    pub fn rename_workspace(
        &self,
        params: WorkspaceRenameParams,
    ) -> Result<Workspace, DomainError> {
        if !params.from_generated {
            return Err(invalid(
                "from_generated must be true: only a name the service chose is renamed",
            ));
        }
        let base = names::clean_workspace_name(&params.name)?;
        let _guard = self.lock_mutations();
        let mut row = self.resolve_workspace(&params.workspace)?;
        if !row.name_generated {
            return Err(DomainError::new(
                codes::WORKSPACE_NAME_NOT_GENERATED,
                format!(
                    "workspace {:?} has a name a person gave; it is never renamed",
                    row.name
                ),
            ));
        }
        if row.state == WorkspaceState::Removing {
            return Err(DomainError::new(
                codes::WORKSPACE_BUSY,
                format!("workspace {:?} is being removed", row.name),
            ));
        }
        if row.name == base {
            return Ok(views::workspace(&row));
        }
        let project = self.project_of(&row)?;
        let name = self.free_name(&project, &base, Folders::Ignored)?;
        let branch = match row.kind {
            WorkspaceKind::Folder => None,
            WorkspaceKind::Worktree => Some(self.rename_branch(
                &project,
                &row,
                &name,
                params.expected_branch.as_deref(),
            )?),
        };
        if let Err(error) = self.store.rename_workspace(
            &row.id,
            &name,
            branch.as_ref().map(|(_, new)| new.as_str()),
        ) {
            let error = refused_write(error);
            return Err(match &branch {
                Some((old, new)) => self.rename_branch_back(&row, old, new, error),
                None => error,
            });
        }
        row.name = name;
        if let Some((_, new)) = branch {
            row.branch = Some(new);
        }
        Ok(views::workspace(&row))
    }

    /// Checks the worktree's branch and renames it for `name`. Returns the old and the new branch.
    fn rename_branch(
        &self,
        project: &ProjectRow,
        row: &WorkspaceRow,
        name: &str,
        expected: Option<&str>,
    ) -> Result<(String, String), DomainError> {
        let dir = Path::new(&row.path);
        let Some(current) = self.git.current_branch(dir)? else {
            return Err(DomainError::new(
                codes::GIT_DETACHED_HEAD,
                format!("workspace {:?} is not on a branch", row.name),
            ));
        };
        if row.branch.as_deref() != Some(current.as_str())
            || expected.is_some_and(|branch| branch != current)
        {
            return Err(DomainError::new(
                codes::WORKSPACE_BRANCH_CHANGED,
                format!(
                    "workspace {:?} is on branch {current:?}, not the one expected",
                    row.name
                ),
            ));
        }
        let repo = Path::new(&project.path);
        if self.git.branch_has_upstream(dir, &current)?
            || self.git.remote_branch_exists(repo, &current)?
        {
            return Err(DomainError::new(
                codes::WORKSPACE_BRANCH_PUBLISHED,
                format!("branch {current:?} is on a remote; renaming it would split it"),
            ));
        }
        let segment = self.branch_prefix.segment(&self.git, repo)?;
        let new = branch_prefix::branch_name(segment.as_deref(), name);
        self.git
            .branch_rename(dir, &current, &new)
            .map_err(branch_refusal)?;
        Ok((current, new))
    }

    /// Undoes a branch rename whose row write failed; a failed undo is named in the error.
    fn rename_branch_back(
        &self,
        row: &WorkspaceRow,
        old: &str,
        new: &str,
        error: DomainError,
    ) -> DomainError {
        match self.git.branch_rename(Path::new(&row.path), new, old) {
            Ok(_) => error,
            Err(undo) => DomainError::new(
                error.code(),
                format!(
                    "{}; branch {new} could not be renamed back to {old}: {undo}",
                    error.message()
                ),
            ),
        }
    }
}

fn branch_refusal(error: BranchError) -> DomainError {
    match error {
        BranchError::Refused { code, message } => DomainError::new(code, message),
        BranchError::WouldOverwrite { message, .. } => DomainError::new(codes::GIT_FAILED, message),
        BranchError::Git(error) => error.into(),
    }
}
