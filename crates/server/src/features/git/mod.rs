//! The `git/1` capability (decisions 0010-0015): one module per command group, sharing workspace resolution here.

mod status;

use std::path::PathBuf;

use massorca_core_git::Git;
use massorca_core_workspace::DomainError;
use massorca_protocol::methods::workspaces::{WorkspaceKind, WorkspaceState};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let mut handles = vec![
        registries
            .capabilities
            .register(capabilities::GIT, Owner::Core)?,
    ];
    handles.extend(status::register(registries)?);
    Ok(handles)
}

/// The git runner and the work tree a workspace selector names.
#[derive(Debug, Clone)]
pub(crate) struct Repository {
    pub(crate) git: Git,
    pub(crate) dir: PathBuf,
}

/// Resolves `selector` to a git work tree. A folder workspace, a missing directory and a workspace being removed are refused.
pub(crate) async fn repository(
    ctx: &RequestCtx,
    selector: String,
) -> Result<Repository, ErrorBody> {
    let workspaces = std::sync::Arc::clone(&ctx.state.workspaces);
    let git = workspaces.git().clone();
    let workspace = blocking(move || {
        workspaces
            .show_workspace(&selector)
            .map_err(ErrorBody::from)
    })
    .await?;
    match workspace.kind {
        WorkspaceKind::Worktree => {}
        _ => {
            return Err(ErrorBody::new(
                codes::GIT_NOT_A_REPOSITORY,
                format!(
                    "workspace `{}` is a folder, not a git worktree",
                    workspace.name
                ),
            ));
        }
    }
    if workspace.state == WorkspaceState::Removing {
        return Err(ErrorBody::new(
            codes::WORKSPACE_NOT_FOUND,
            format!("workspace `{}` is being removed", workspace.name),
        ));
    }
    if workspace.missing {
        return Err(ErrorBody::new(
            codes::PATH_NOT_FOUND,
            format!(
                "the directory of workspace `{}` is missing: {}",
                workspace.name, workspace.path
            ),
        ));
    }
    Ok(Repository {
        git,
        dir: PathBuf::from(workspace.path),
    })
}

/// Runs one git call on the blocking pool and maps its error to the protocol.
pub(crate) async fn blocking<T, F>(call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ErrorBody> + Send + 'static,
{
    tokio::task::spawn_blocking(call)
        .await
        .unwrap_or_else(|error| {
            Err(ErrorBody::new(
                codes::INTERNAL,
                format!("the git task failed: {error}"),
            ))
        })
}

/// A git failure as the protocol reports it.
pub(crate) fn git_error(error: massorca_core_git::GitError) -> ErrorBody {
    DomainError::from(error).into()
}
