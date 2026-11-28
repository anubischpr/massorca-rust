//! `git.rebase_base` (decision 0181): rebasing a worktree's branch onto its base, and the refusals before git runs.

use std::path::Path;

use massorca_core_git::{RebaseOutcome, RemoteError};
use massorca_core_store::{WorkspaceKind, WorkspaceState};
use massorca_protocol::codes;

use crate::create::BASE_REFRESH_TIMEOUT;
use crate::error::DomainError;
use crate::paths;
use crate::workspaces::Workspaces;

impl Workspaces {
    /// Rebases the worktree `selector` names onto its base: the branch's recorded base, else the project's, else the
    /// repository default, as a create resolves them. With `refresh`, the base's remote-tracking branch is fetched
    /// first (no prune, 30 seconds at most), and a failed fetch refuses: the rebase would otherwise use the copy the
    /// person asked to refresh. Refused before git changes anything: a folder, a workspace being removed, the main
    /// worktree (an in-place checkout included), a detached HEAD, an operation in progress, and local changes.
    pub fn rebase_workspace_base(
        &self,
        selector: &str,
        refresh: bool,
    ) -> Result<RebaseOutcome, DomainError> {
        let row = self.resolve_workspace(selector)?;
        if row.kind == WorkspaceKind::Folder {
            return Err(DomainError::new(
                codes::GIT_NOT_A_REPOSITORY,
                format!("workspace {:?} is a folder, not a git worktree", row.name),
            ));
        }
        if row.state == WorkspaceState::Removing {
            return Err(DomainError::new(
                codes::WORKSPACE_BUSY,
                format!("workspace {:?} is being removed", row.name),
            ));
        }
        if row.in_place {
            return Err(DomainError::new(
                codes::WORKSPACE_MAIN_WORKTREE,
                format!(
                    "workspace {:?} is the project's own checkout, in place, and is never rebased",
                    row.name
                ),
            ));
        }
        let project = self.project_of(&row)?;
        let repo = Path::new(&project.path);
        let dir = Path::new(&row.path);
        let entries = self.git.worktree_list(repo)?;
        if entries
            .first()
            .is_some_and(|main| paths::path_key(&main.path) == paths::path_key(dir))
        {
            return Err(DomainError::new(
                codes::WORKSPACE_MAIN_WORKTREE,
                format!("{} is the main worktree and is never rebased", row.path),
            ));
        }
        if self.git.current_branch(dir)?.is_none() {
            return Err(DomainError::new(
                codes::GIT_DETACHED_HEAD,
                format!("workspace {:?} is not on a branch", row.name),
            ));
        }
        if self.git.operation_in_progress(dir)?.is_some() {
            return Err(DomainError::new(
                codes::WORKSPACE_BUSY,
                format!(
                    "workspace {:?} has a merge or rebase in progress; finish or abort it first",
                    row.name
                ),
            ));
        }
        if self.git.status_dirty(dir)? {
            return Err(DomainError::new(
                codes::WORKSPACE_DIRTY,
                format!(
                    "workspace {:?} has local changes; commit or discard them first",
                    row.name
                ),
            ));
        }
        let named = row.base_ref.as_deref().or(project.base_ref.as_deref());
        let Some(base) = self.git.base_or_default(repo, named)? else {
            return Err(DomainError::new(
                codes::BASE_NOT_FOUND,
                "there is no base branch to rebase onto",
            ));
        };
        if refresh {
            self.refresh_base(repo, &base.revision)?;
        }
        if !self.git.ref_exists(repo, &base.revision)? {
            return Err(DomainError::new(
                codes::BASE_NOT_FOUND,
                format!("base {:?} does not resolve to a commit", base.name),
            ));
        }
        Ok(self.git.rebase_onto(dir, &base.revision)?)
    }

    /// Fetches the remote-tracking branch `revision` names; a base that is not one fetches nothing.
    fn refresh_base(&self, repo: &Path, revision: &str) -> Result<(), DomainError> {
        match self
            .git
            .fetch_tracking_base(repo, revision, BASE_REFRESH_TIMEOUT)
        {
            Ok(_) => Ok(()),
            Err(RemoteError::Refused { code, message }) => Err(DomainError::new(code, message)),
            Err(RemoteError::Git(error)) => Err(error.into()),
        }
    }
}
