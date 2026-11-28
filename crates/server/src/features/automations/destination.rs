//! Where an automation's runs work (decision 0127): the destination an automation sets, checked when it is set, and the
//! refusals a run meets when its destination can no longer be used. A refusal is recorded as a run row with its fixed
//! sentence, and repeats fold into one row (core-store). A workspace the service cannot reach is not evidence that it
//! is gone, so such a run is not refused here.

use std::path::Path;

use massorca_core_git::Git;
use massorca_core_store::{AutomationRow, Store, destination};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::automations::{AutomationDestination, AutomationDestinationKind};
use massorca_protocol::methods::projects::ProjectKind;
use massorca_protocol::methods::workspaces::WorkspaceState;
use massorca_protocol::{ErrorBody, codes};

use crate::features::git::git_error;

/// A run that cannot start: its protocol code, and the fixed sentence recorded as its reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Refusal {
    pub(super) code: &'static str,
    pub(super) sentence: &'static str,
}

pub(super) const NO_PROJECT: Refusal = Refusal {
    code: codes::AUTOMATION_NO_PROJECT,
    sentence: "The automation has no project, so the run did not start.",
};
const REMOVED: Refusal = Refusal {
    code: codes::AUTOMATION_DESTINATION_REMOVED,
    sentence: "The destination workspace was removed, so the run did not start.",
};
const DETACHED: Refusal = Refusal {
    code: codes::AUTOMATION_REPOSITORY_DETACHED,
    sentence: "The repository is no longer attached to this host, so the run did not start.",
};
const PATH_CHANGED: Refusal = Refusal {
    code: codes::AUTOMATION_PATH_CHANGED,
    sentence: "The destination path changed or is missing, so the run did not start.",
};

/// The refusal a run of `row` meets, judged from the service's own records and the folders on this host. An automation
/// without a project is left to the run itself, which records it as such.
pub(super) fn refusal(workspaces: &Workspaces, row: &AutomationRow) -> Option<Refusal> {
    let project_id = row.project_id.as_deref()?;
    match row.destination.as_str() {
        destination::EXISTING_WORKSPACE => existing_refusal(workspaces, row),
        destination::PROJECT_WORKTREE_PER_RUN => project_refusal(workspaces, project_id),
        _ => None,
    }
}

fn existing_refusal(workspaces: &Workspaces, row: &AutomationRow) -> Option<Refusal> {
    let Some(id) = row.destination_workspace_id.as_deref() else {
        return Some(REMOVED);
    };
    let found = match workspaces.show_workspace(id) {
        Ok(found) => found,
        Err(error) if error.code() == codes::WORKSPACE_NOT_FOUND => return Some(REMOVED),
        Err(_) => return None,
    };
    if found.state == WorkspaceState::Removing {
        return Some(REMOVED);
    }
    if found.missing || row.destination_path.as_deref() != Some(found.path.as_str()) {
        return Some(PATH_CHANGED);
    }
    None
}

/// A new worktree per run needs its project's folder still attached, and still a git repository.
fn project_refusal(workspaces: &Workspaces, project_id: &str) -> Option<Refusal> {
    let project = match workspaces.show_project(project_id) {
        Ok(project) => project,
        Err(error) if error.code() == codes::PROJECT_NOT_FOUND => return Some(NO_PROJECT),
        Err(_) => return None,
    };
    if project.kind != ProjectKind::Git || !Path::new(&project.path).is_dir() {
        return Some(DETACHED);
    }
    None
}

/// A destination as the store keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Stored {
    pub(super) kind: &'static str,
    pub(super) workspace_id: Option<String>,
    pub(super) path: Option<String>,
    pub(super) base_branch: Option<String>,
}

impl Stored {
    /// The project's own place: what automations stored before decision 0127 use.
    pub(super) fn project_default() -> Self {
        Self {
            kind: destination::PROJECT_DEFAULT,
            workspace_id: None,
            path: None,
            base_branch: None,
        }
    }
}

/// Checks a destination an automation sets in `project_id`. An absent one keeps the project's own place. An existing
/// workspace must belong to the project and be usable; a worktree per run needs a git project.
pub(super) fn check(
    workspaces: &Workspaces,
    git: &Git,
    project_id: &str,
    destination: Option<&AutomationDestination>,
) -> Result<Stored, ErrorBody> {
    let Some(destination) = destination else {
        return Ok(Stored::project_default());
    };
    match destination.kind {
        AutomationDestinationKind::ProjectDefault => {
            if destination.workspace.is_some() || destination.base_branch.is_some() {
                return Err(invalid(
                    "the project's own place takes no workspace or base branch",
                ));
            }
            Ok(Stored::project_default())
        }
        AutomationDestinationKind::ProjectWorktreePerRun => {
            if destination.workspace.is_some() {
                return Err(invalid("a new worktree per run takes no workspace"));
            }
            let project = workspaces.show_project(project_id)?;
            if project.kind != ProjectKind::Git {
                return Err(invalid(format!(
                    "a new worktree per run needs a git repository, and {} is a folder",
                    project.name
                )));
            }
            let base = base_branch(destination.base_branch.as_deref())?;
            if let Some(name) = &base {
                known_base(git, &project.path, name)?;
            }
            Ok(Stored {
                kind: destination::PROJECT_WORKTREE_PER_RUN,
                workspace_id: None,
                path: None,
                base_branch: base,
            })
        }
        AutomationDestinationKind::ExistingWorkspace => {
            if destination.base_branch.is_some() {
                return Err(invalid("an existing workspace takes no base branch"));
            }
            let id = destination
                .workspace
                .as_deref()
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .ok_or_else(|| invalid("an existing workspace needs its workspace id"))?;
            let found = workspaces.show_workspace(id)?;
            if found.project_id != project_id {
                return Err(invalid(format!(
                    "workspace {} belongs to another project; choose one of this project's workspaces",
                    found.name
                )));
            }
            if found.state == WorkspaceState::Removing || found.missing {
                return Err(invalid(format!(
                    "workspace {} is not usable: its folder is missing or it is being removed",
                    found.name
                )));
            }
            Ok(Stored {
                kind: destination::EXISTING_WORKSPACE,
                workspace_id: Some(found.id),
                path: Some(found.path),
                base_branch: None,
            })
        }
        AutomationDestinationKind::Unknown => Err(invalid(
            "the destination must be project_default, project_worktree_per_run or existing_workspace",
        )),
    }
}

/// A base branch must be a commit of the project's repository, so a run is not refused later for one that never existed.
fn known_base(git: &Git, repo: &str, name: &str) -> Result<(), ErrorBody> {
    let repo = Path::new(repo);
    let base = git.qualify_base(repo, name).map_err(git_error)?;
    if git.ref_exists(repo, &base.revision).map_err(git_error)? {
        return Ok(());
    }
    Err(ErrorBody::new(
        codes::AUTOMATION_BASE_BRANCH_UNKNOWN,
        format!("the base branch {name} is not a commit of this repository"),
    ))
}

fn base_branch(given: Option<&str>) -> Result<Option<String>, ErrorBody> {
    match given.map(str::trim) {
        None => Ok(None),
        Some("") => Err(invalid("the base branch must not be blank")),
        Some(branch) => Ok(Some(branch.to_owned())),
    }
}

/// The destination an automation shows: its kind, its workspace and that workspace's name while it exists.
pub(super) fn view(store: &Store, row: &AutomationRow) -> AutomationDestination {
    let kind = match row.destination.as_str() {
        destination::PROJECT_DEFAULT => AutomationDestinationKind::ProjectDefault,
        destination::PROJECT_WORKTREE_PER_RUN => AutomationDestinationKind::ProjectWorktreePerRun,
        destination::EXISTING_WORKSPACE => AutomationDestinationKind::ExistingWorkspace,
        _ => AutomationDestinationKind::Unknown,
    };
    let workspace_name = row
        .destination_workspace_id
        .as_deref()
        .and_then(|id| store.get_workspace(id).ok().flatten())
        .map(|workspace| workspace.name);
    AutomationDestination {
        kind,
        workspace: row.destination_workspace_id.clone(),
        workspace_name,
        base_branch: row.base_branch.clone(),
    }
}

fn invalid(message: impl Into<String>) -> ErrorBody {
    ErrorBody::new(codes::AUTOMATION_DESTINATION_INVALID, message)
}
