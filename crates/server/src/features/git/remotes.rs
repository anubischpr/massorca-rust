//! `git.remotes`: the hosting identity of each network remote of a workspace or project (decision 0114).

use std::path::PathBuf;
use std::sync::Arc;

use massorca_protocol::methods::git::remotes::{self, GitRemotes, GitRemotesParams};
use massorca_protocol::{ErrorBody, capabilities, codes};

use super::{blocking, git_error, git_for_caller, repository};
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(super) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::GIT_REMOTE_IDENTITY, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            remotes::REMOTES,
            capabilities::GIT_REMOTE_IDENTITY,
            Permission::Read,
            handle,
        )?,
    ])
}

async fn handle(ctx: RequestCtx, params: GitRemotesParams) -> Result<GitRemotes, ErrorBody> {
    let (git, dir) = match (params.workspace, params.project) {
        (Some(workspace), None) => {
            let repository = repository(&ctx, workspace).await?;
            (repository.git, repository.dir)
        }
        (None, Some(project)) => (git_for_caller(&ctx), project_dir(&ctx, project).await?),
        _ => {
            return Err(ErrorBody::new(
                codes::REQUEST_INVALID_PARAMS,
                "git.remotes needs exactly one of workspace or project",
            ));
        }
    };
    let remotes = blocking(move || git.remote_identities(&dir).map_err(git_error)).await?;
    Ok(GitRemotes { remotes })
}

/// The checkout of the project a selector names, which holds its main worktree.
async fn project_dir(ctx: &RequestCtx, selector: String) -> Result<PathBuf, ErrorBody> {
    let workspaces = Arc::clone(&ctx.state.workspaces);
    let project =
        blocking(move || workspaces.show_project(&selector).map_err(ErrorBody::from)).await?;
    Ok(PathBuf::from(project.path))
}
