//! Creating workspaces (ADR 0008): a new worktree and branch in a git project, or a
//! registration of an existing directory in a folder project.

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use massorca_core_git::{Base, WorktreeAdd};
use massorca_core_store::{
    ProjectKind as StoreKind, ProjectRow, StoreError, WorkspaceKind as StoreWorkspaceKind,
    WorkspaceRow, WorkspaceState as StoreWorkspaceState, new_id, now_ms,
};
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{BaseRefresh, Workspace, WorkspaceCreateParams};

use crate::branch_prefix;
use crate::error::DomainError;
use crate::generated_names;
use crate::names;
use crate::paths;
use crate::views;
use crate::workspaces::Workspaces;

/// Where a new workspace's name comes from: the caller's, or a generated creature name.
#[derive(Clone, Copy)]
enum Naming {
    Typed,
    Generated,
}

// Held under the mutation lock, so far below decision 0013's five-minute remote cap.
const BASE_REFRESH_TIMEOUT: Duration = Duration::from_secs(30);

/// A workspace from `create_workspace_once`.
#[derive(Debug, Clone)]
pub struct Created {
    pub workspace: Workspace,
    /// A repeated `request_id` returned the workspace an earlier call stored; this call changed nothing.
    pub repeated: bool,
}

impl Workspaces {
    /// Creates a worktree (git project) or registers a folder (folder project). A repeated
    /// `request_id` returns the workspace the first call stored, whatever its other fields.
    pub fn create_workspace(
        &self,
        params: WorkspaceCreateParams,
    ) -> Result<Workspace, DomainError> {
        self.create(params, Naming::Typed)
            .map(|created| created.workspace)
    }

    /// `create_workspace` under a generated name (ADR 0008); `params.name` is ignored.
    pub fn create_workspace_with_generated_name(
        &self,
        params: WorkspaceCreateParams,
    ) -> Result<Workspace, DomainError> {
        self.create(params, Naming::Generated)
            .map(|created| created.workspace)
    }

    /// Like `create_workspace`, and says whether this call made the workspace or a repeated `request_id` found it.
    /// A blank name asks for a generated one.
    pub fn create_workspace_once(
        &self,
        params: WorkspaceCreateParams,
    ) -> Result<Created, DomainError> {
        let naming = if params.name.trim().is_empty() {
            Naming::Generated
        } else {
            Naming::Typed
        };
        self.create(params, naming)
    }

    fn create(
        &self,
        params: WorkspaceCreateParams,
        naming: Naming,
    ) -> Result<Created, DomainError> {
        let _guard = self.lock_mutations();
        if let Some(request_id) = params.request_id.as_deref() {
            if request_id.is_empty() {
                return Err(invalid("request id must not be empty"));
            }
            if let Some(row) = self.store.find_workspace_by_request_id(request_id)? {
                return Ok(Created {
                    workspace: views::workspace(&row),
                    repeated: true,
                });
            }
        }
        let project = self.resolve_project(&params.project)?;
        let name = match naming {
            Naming::Typed => names::clean_workspace_name(&params.name)?,
            Naming::Generated => self.generated_name(&project)?,
        };
        let params = WorkspaceCreateParams { name, ..params };
        if self
            .store
            .find_workspace_by_name(&project.id, &params.name)?
            .is_some()
        {
            return Err(DomainError::new(
                codes::WORKSPACE_NAME_TAKEN,
                format!(
                    "project {:?} already has a workspace named {:?}",
                    project.name, params.name
                ),
            ));
        }
        let workspace = match project.kind {
            StoreKind::Git => self.create_worktree(&project, &params),
            StoreKind::Folder => self.create_folder(&project, &params),
        }?;
        Ok(Created {
            workspace,
            repeated: false,
        })
    }

    /// A creature name that no workspace, branch or directory of `project` already takes.
    fn generated_name(&self, project: &ProjectRow) -> Result<String, DomainError> {
        let names: HashSet<String> = self
            .store
            .list_workspaces(Some(&project.id))?
            .into_iter()
            .map(|row| row.name.to_lowercase())
            .collect();
        let free = match project.kind {
            StoreKind::Folder => generated_names::generated_name(
                |name| names.contains(name),
                generated_names::random_choice(),
            ),
            StoreKind::Git => {
                let repo = Path::new(&project.path);
                let segment = self.branch_prefix.segment(&self.git, repo)?;
                let branches: HashSet<String> = self
                    .git
                    .branches(repo)?
                    .into_iter()
                    .map(|branch| branch.name.to_lowercase())
                    .collect();
                let root = paths::absolute(&project.workspaces_root)?;
                generated_names::generated_name(
                    |name| {
                        let branch = branch_prefix::branch_name(segment.as_deref(), name);
                        names.contains(name)
                            || branches.contains(&branch.to_lowercase())
                            || is_occupied(&root.join(name))
                    },
                    generated_names::random_choice(),
                )
            }
        };
        free.ok_or_else(|| {
            DomainError::new(
                codes::WORKSPACE_NAME_TAKEN,
                "every generated name is taken; give one",
            )
        })
    }

    /// A new branch checked out in a new worktree, or an existing local branch when asked.
    fn create_worktree(
        &self,
        project: &ProjectRow,
        params: &WorkspaceCreateParams,
    ) -> Result<Workspace, DomainError> {
        let repo = Path::new(&project.path);
        // Why: a named or existing branch is used as given; only a new one takes the prefix.
        let branch = match params.branch.as_deref() {
            Some(explicit) => explicit.to_owned(),
            None if params.existing_branch => params.name.clone(),
            None => {
                let segment = self.branch_prefix.segment(&self.git, repo)?;
                branch_prefix::branch_name(segment.as_deref(), &params.name)
            }
        };
        let branch = branch.as_str();
        if !self.git.check_branch_name(repo, branch)? {
            return Err(DomainError::new(
                codes::WORKSPACE_NAME_INVALID,
                format!("branch {branch:?} is not a valid git branch name"),
            ));
        }
        let target = match params.path.as_deref() {
            Some(text) => paths::absolute(text)?,
            None => paths::absolute(&project.workspaces_root)?.join(&params.name),
        };
        if let Some(adopted) = self.adopt_interrupted_worktree(project, &target, branch, params)? {
            return Ok(adopted);
        }
        self.ensure_path_free(&target)?;
        let mut base = Base {
            name: "HEAD".to_owned(),
            revision: "HEAD".to_owned(),
        };
        let mut refresh = None;
        if params.existing_branch {
            if params.base.is_some() {
                return Err(invalid(
                    "base does not apply when an existing branch is checked out",
                ));
            }
            if params.refresh_base {
                return Err(invalid(
                    "refresh_base does not apply when an existing branch is checked out",
                ));
            }
            if !self.git.branch_exists(repo, branch)? {
                return Err(DomainError::new(
                    codes::BRANCH_NOT_FOUND,
                    format!("there is no local branch {branch:?}"),
                ));
            }
        } else {
            if self.git.branch_exists(repo, branch)?
                || self.git.remote_branch_exists(repo, branch)?
            {
                return Err(DomainError::new(
                    codes::BRANCH_EXISTS,
                    format!("branch {branch:?} already exists"),
                ));
            }
            // Unnamed: the project's base (WS-04), else the default branch (decision 0008), else HEAD.
            let named = params.base.as_deref().or(project.base_ref.as_deref());
            if let Some(found) = match named {
                Some(name) => Some(self.git.qualify_base(repo, name)?),
                None => self.git.default_base(repo)?,
            } {
                base = found;
            }
            if params.refresh_base {
                // A failed refresh does not fail the create: it goes on from the local copy, and the reply says so.
                refresh = Some(
                    match self
                        .git
                        .fetch_tracking_base(repo, &base.revision, BASE_REFRESH_TIMEOUT)
                    {
                        Ok(Some(fetched)) => BaseRefresh::Fetched {
                            moved: !fetched.updated.is_empty(),
                        },
                        Ok(None) => BaseRefresh::NotRemote,
                        Err(error) => BaseRefresh::Failed {
                            reason: error.to_string(),
                        },
                    },
                );
            }
            if !self.git.ref_exists(repo, &base.revision)? {
                return Err(DomainError::new(
                    codes::BASE_NOT_FOUND,
                    format!("base {:?} does not resolve to a commit", base.name),
                ));
            }
        }

        // Everything the row needs is computed before git runs, so nothing after the
        // worktree exists can fail except the insert, which has its own rollback.
        let created_branch = !params.existing_branch;
        let stored_path = paths::path_text(&paths::canonical(&target))?;
        let path_key = paths::path_key(&target);
        let add = if params.existing_branch {
            WorktreeAdd::ExistingBranch { branch }
        } else {
            WorktreeAdd::NewBranch {
                branch,
                base: Some(base.revision.as_str()),
            }
        };
        self.git.worktree_add(repo, &target, add)?;

        let row = WorkspaceRow {
            id: new_id(),
            project_id: project.id.clone(),
            kind: StoreWorkspaceKind::Worktree,
            name: params.name.clone(),
            path: stored_path,
            path_key,
            branch: Some(branch.to_owned()),
            base_ref: created_branch.then(|| base.name.clone()),
            created_branch,
            state: StoreWorkspaceState::Ready,
            request_id: params.request_id.clone(),
            created_at: now_ms(),
            comment: None,
        };
        if let Err(error) = self.store.insert_workspace(&row) {
            let leftovers =
                self.roll_back_worktree(repo, &target, created_branch.then_some(branch));
            return Err(with_leftovers(refused_write(error), &leftovers));
        }
        Ok(Workspace {
            base_refresh: refresh,
            ..views::workspace(&row)
        })
    }

    /// Adopts the worktree an interrupted create left at `target`: git registers it on `branch`
    /// and no row records it (ADR 0008). That create's branch cannot be proven to be this
    /// request's, so the row says it was not created here and removal keeps the branch.
    /// The main worktree and a path that a row already owns are never adopted.
    fn adopt_interrupted_worktree(
        &self,
        project: &ProjectRow,
        target: &Path,
        branch: &str,
        params: &WorkspaceCreateParams,
    ) -> Result<Option<Workspace>, DomainError> {
        let key = paths::path_key(target);
        let entries = self.git.worktree_list(Path::new(&project.path))?;
        let registered = entries
            .iter()
            .skip(1)
            .find(|entry| paths::path_key(&entry.path) == key);
        let Some(entry) = registered else {
            return Ok(None);
        };
        if entry.branch.as_deref() != Some(branch)
            || self.store.find_workspace_by_path_key(&key)?.is_some()
        {
            return Ok(None);
        }
        let row = WorkspaceRow {
            id: new_id(),
            project_id: project.id.clone(),
            kind: StoreWorkspaceKind::Worktree,
            name: params.name.clone(),
            path: paths::path_text(&paths::canonical(target))?,
            path_key: key,
            branch: Some(branch.to_owned()),
            base_ref: None,
            created_branch: false,
            state: StoreWorkspaceState::Ready,
            request_id: params.request_id.clone(),
            created_at: now_ms(),
            comment: None,
        };
        self.store.insert_workspace(&row).map_err(refused_write)?;
        Ok(Some(views::workspace(&row)))
    }

    /// Registers an existing directory: the project's by default. Creates no files.
    pub(crate) fn create_folder(
        &self,
        project: &ProjectRow,
        params: &WorkspaceCreateParams,
    ) -> Result<Workspace, DomainError> {
        if params.branch.is_some()
            || params.base.is_some()
            || params.existing_branch
            || params.refresh_base
        {
            return Err(invalid(
                "branch, base, existing_branch and refresh_base apply only to worktrees",
            ));
        }
        let directory = match params.path.as_deref() {
            Some(text) => paths::existing_directory(&paths::absolute(text)?)?,
            None => paths::existing_directory(Path::new(&project.path))?,
        };
        let row = WorkspaceRow {
            id: new_id(),
            project_id: project.id.clone(),
            kind: StoreWorkspaceKind::Folder,
            name: params.name.clone(),
            path: paths::path_text(&directory)?,
            path_key: paths::path_key(&directory),
            branch: None,
            base_ref: None,
            created_branch: false,
            state: StoreWorkspaceState::Ready,
            request_id: params.request_id.clone(),
            created_at: now_ms(),
            comment: None,
        };
        self.store.insert_workspace(&row).map_err(refused_write)?;
        Ok(views::workspace(&row))
    }

    /// `target` must be absent or an empty directory, and no workspace may own its path key.
    fn ensure_path_free(&self, target: &Path) -> Result<(), DomainError> {
        let claimed = self
            .store
            .find_workspace_by_path_key(&paths::path_key(target))?
            .is_some();
        if claimed || is_occupied(target) {
            return Err(DomainError::new(
                codes::WORKSPACE_PATH_TAKEN,
                format!("{} is already in use", target.display()),
            ));
        }
        Ok(())
    }

    /// Undoes a worktree whose row could not be stored: removes it and, when this request
    /// created the branch, deletes that branch if git proves it merged. Returns what is left.
    fn roll_back_worktree(
        &self,
        repo: &Path,
        target: &Path,
        created_branch: Option<&str>,
    ) -> Vec<String> {
        let mut leftovers = Vec::new();
        if let Err(error) = self.git.worktree_remove(repo, target, true) {
            leftovers.push(format!("worktree {} ({error})", target.display()));
        }
        if let Some(branch) = created_branch {
            match self.git.delete_branch_if_merged(repo, branch) {
                Ok(true) => {}
                Ok(false) => leftovers.push(format!("branch {branch} (not merged)")),
                Err(error) => leftovers.push(format!("branch {branch} ({error})")),
            }
        }
        leftovers
    }
}

fn invalid(message: &str) -> DomainError {
    DomainError::new(codes::REQUEST_INVALID_PARAMS, message)
}

/// A file, a link or a non-empty directory occupies a path; an absent path or an empty
/// directory does not.
fn is_occupied(target: &Path) -> bool {
    match std::fs::symlink_metadata(target) {
        Err(_) => false,
        Ok(metadata) if metadata.is_dir() => match std::fs::read_dir(target) {
            Ok(mut entries) => entries.next().is_some(),
            Err(_) => true,
        },
        Ok(_) => true,
    }
}

/// The domain meaning of a row write (an insert or a rename) that the store refused.
pub(crate) fn refused_write(error: StoreError) -> DomainError {
    match error {
        StoreError::Conflict { constraint } if constraint.contains("path_key") => DomainError::new(
            codes::WORKSPACE_PATH_TAKEN,
            "that path was claimed by another workspace",
        ),
        StoreError::Conflict { constraint } if constraint.contains("name") => DomainError::new(
            codes::WORKSPACE_NAME_TAKEN,
            "that name was claimed by another workspace",
        ),
        other => other.into(),
    }
}

/// Names what a failed rollback left behind, so the caller can finish the cleanup.
fn with_leftovers(error: DomainError, leftovers: &[String]) -> DomainError {
    if leftovers.is_empty() {
        error
    } else {
        DomainError::new(
            error.code(),
            format!("{}; left behind: {}", error.message(), leftovers.join(", ")),
        )
    }
}
