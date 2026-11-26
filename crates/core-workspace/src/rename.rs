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
use crate::unique_names::Search;
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
        self.rename_generated(params, false)
    }

    /// The first-prompt trigger's rename (decision 0118): [`Workspaces::rename_workspace`], and the name is then the
    /// person's, so no later run renames it. Not offered on the wire.
    pub fn rename_from_first_prompt(
        &self,
        params: WorkspaceRenameParams,
    ) -> Result<Workspace, DomainError> {
        self.rename_generated(params, true)
    }

    fn rename_generated(
        &self,
        params: WorkspaceRenameParams,
        settle: bool,
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
        let project = self.project_of(&row)?;
        // The branch checks run before the no-op below, so a stale `expected_branch` is still refused.
        let current = match row.kind {
            WorkspaceKind::Folder => None,
            WorkspaceKind::Worktree => {
                Some(self.checked_branch(&project, &row, params.expected_branch.as_deref())?)
            }
        };
        let name = self.free_name(&project, &base, Search::Rename(&row))?;
        // A settling rename still writes the flag when the name stays, so the trigger's name is the person's.
        if name == row.name && !settle {
            return Ok(views::workspace(&row));
        }
        let moved = match current {
            Some(current) => {
                let segment = self
                    .branch_prefix
                    .segment(&self.git, Path::new(&project.path))?;
                let new = branch_prefix::branch_name(segment.as_deref(), &name);
                if new == current {
                    None
                } else {
                    self.move_branch(&row, &current, &new)?;
                    Some((current, new))
                }
            }
            None => None,
        };
        let branch = moved
            .as_ref()
            .map(|(_, new)| new.clone())
            .or_else(|| row.branch.clone());
        if let Err(error) = self
            .store
            .rename_workspace(&row.id, &name, branch.as_deref(), !settle)
        {
            let error = refused_write(error);
            return Err(match &moved {
                Some((old, new)) => self.rename_branch_back(&row, old, new, error),
                None => error,
            });
        }
        row.name = name;
        row.branch = branch;
        row.name_generated = !settle;
        Ok(views::workspace(&row))
    }

    /// The worktree's checked-out branch, once it is the recorded one, the expected one, and on no remote.
    fn checked_branch(
        &self,
        project: &ProjectRow,
        row: &WorkspaceRow,
        expected: Option<&str>,
    ) -> Result<String, DomainError> {
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
        if self.git.branch_has_upstream(dir, &current)?
            || self
                .git
                .remote_branch_exists(Path::new(&project.path), &current)?
        {
            return Err(DomainError::new(
                codes::WORKSPACE_BRANCH_PUBLISHED,
                format!("branch {current:?} is on a remote; renaming it would split it"),
            ));
        }
        Ok(current)
    }

    /// Renames the branch `from` to `to` in the worktree. A case-only change goes through a temporary name, because a
    /// case-insensitive filesystem sees the new loose ref as the old one.
    fn move_branch(&self, row: &WorkspaceRow, from: &str, to: &str) -> Result<(), DomainError> {
        let dir = Path::new(&row.path);
        if !from.eq_ignore_ascii_case(to) {
            self.git
                .branch_rename(dir, from, to)
                .map_err(branch_refusal)?;
            return Ok(());
        }
        let temporary = format!("{from}-massorca-rename");
        self.git
            .branch_rename(dir, from, &temporary)
            .map_err(branch_refusal)?;
        if let Err(error) = self.git.branch_rename(dir, &temporary, to) {
            // Best effort: the first step is undone so the row's branch still names it.
            let _undone = self.git.branch_rename(dir, &temporary, from);
            return Err(branch_refusal(error));
        }
        Ok(())
    }

    /// Undoes a branch rename whose row write failed; a failed undo is named in the error.
    fn rename_branch_back(
        &self,
        row: &WorkspaceRow,
        old: &str,
        new: &str,
        error: DomainError,
    ) -> DomainError {
        match self.move_branch(row, new, old) {
            Ok(()) => error,
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
