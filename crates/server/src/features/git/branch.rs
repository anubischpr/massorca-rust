//! `git.branches`, `git.branch_create`, `git.branch_switch`, `git.branch_rename` and `git.branch_delete`
//! (decisions 0010 and 0013). Mutations hold the workspace's mutation lock.

use std::path::PathBuf;
use std::sync::Arc;

use massorca_core_git::{BranchError, Git};
use massorca_protocol::methods::git::branch::{
    self, GitBranch, GitBranchCreateParams, GitBranchDeleteParams, GitBranchDeleted,
    GitBranchRenameParams, GitBranchSwitchParams, GitBranchSwitched, GitBranches,
    GitBranchesParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};
use serde_json::json;

use super::{Repository, blocking, git_error, git_runner, repository};
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(super) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries.methods.register(
            Owner::Core,
            branch::BRANCHES,
            capabilities::GIT,
            Permission::Read,
            branches,
        )?,
        registries.methods.register(
            Owner::Core,
            branch::BRANCH_CREATE,
            capabilities::GIT,
            Permission::Workspace,
            create,
        )?,
        registries.methods.register(
            Owner::Core,
            branch::BRANCH_SWITCH,
            capabilities::GIT,
            Permission::Workspace,
            switch,
        )?,
        registries.methods.register(
            Owner::Core,
            branch::BRANCH_RENAME,
            capabilities::GIT,
            Permission::Workspace,
            rename,
        )?,
        registries.methods.register(
            Owner::Core,
            branch::BRANCH_DELETE,
            capabilities::GIT,
            Permission::Workspace,
            delete,
        )?,
        registries
            .capabilities
            .register(capabilities::GIT_PROJECT_BRANCHES, Owner::Core)?,
    ])
}

async fn branches(ctx: RequestCtx, params: GitBranchesParams) -> Result<GitBranches, ErrorBody> {
    let (git, dir) = match (params.workspace, params.project) {
        (Some(workspace), None) => {
            let repository = repository(&ctx, workspace).await?;
            (repository.git, repository.dir)
        }
        (None, Some(project)) => project_checkout(&ctx, project).await?,
        _ => {
            return Err(ErrorBody::new(
                codes::REQUEST_INVALID_PARAMS,
                "git.branches takes exactly one of workspace and project",
            ));
        }
    };
    blocking(move || {
        let branches = git.branches(&dir).map_err(git_error)?;
        Ok(GitBranches { branches })
    })
    .await
}

/// The git runner and the checkout of project `selector`, for a read that names no workspace (decision 0112).
async fn project_checkout(ctx: &RequestCtx, selector: String) -> Result<(Git, PathBuf), ErrorBody> {
    let workspaces = Arc::clone(&ctx.state.workspaces);
    let dir = blocking(move || {
        workspaces
            .project_checkout(&selector)
            .map_err(ErrorBody::from)
    })
    .await?;
    Ok((git_runner(ctx), dir))
}

async fn create(ctx: RequestCtx, params: GitBranchCreateParams) -> Result<GitBranch, ErrorBody> {
    let repository = repository(&ctx, params.workspace).await?;
    let _guard = repository.lock().await;
    blocking(move || {
        repository
            .git
            .branch_create(
                &repository.dir,
                &params.name,
                params.start.as_deref(),
                params.switch,
            )
            .map_err(branch_error)
    })
    .await
}

async fn switch(
    ctx: RequestCtx,
    params: GitBranchSwitchParams,
) -> Result<GitBranchSwitched, ErrorBody> {
    let repository = repository(&ctx, params.workspace).await?;
    let _guard = repository.lock().await;
    blocking(move || {
        repository
            .git
            .branch_switch(&repository.dir, &params.branch)
            .map_err(branch_error)?;
        Ok(GitBranchSwitched {
            branch: params.branch,
        })
    })
    .await
}

async fn rename(ctx: RequestCtx, params: GitBranchRenameParams) -> Result<GitBranch, ErrorBody> {
    let repository = repository(&ctx, params.workspace).await?;
    let _guard = repository.lock().await;
    blocking(move || {
        repository
            .git
            .branch_rename(&repository.dir, &params.from, &params.to)
            .map_err(branch_error)
    })
    .await
}

async fn delete(
    ctx: RequestCtx,
    params: GitBranchDeleteParams,
) -> Result<GitBranchDeleted, ErrorBody> {
    let repository = repository(&ctx, params.workspace).await?;
    let _guard = repository.lock().await;
    let in_use = workspace_branches(&ctx, &repository).await?;
    blocking(move || {
        let commit = repository
            .git
            .branch_delete(&repository.dir, &params.branch, params.force, &in_use)
            .map_err(branch_error)?;
        Ok(GitBranchDeleted {
            branch: params.branch,
            commit,
        })
    })
    .await
}

/// The branches workspaces of the same project were created on; they share its repository.
async fn workspace_branches(
    ctx: &RequestCtx,
    repository: &Repository,
) -> Result<Vec<String>, ErrorBody> {
    let workspaces = Arc::clone(&ctx.state.workspaces);
    let project = repository.project_id.clone();
    blocking(move || {
        Ok(workspaces
            .list_workspaces(Some(&project))
            .map_err(ErrorBody::from)?
            .into_iter()
            .filter_map(|workspace| workspace.branch)
            .collect())
    })
    .await
}

fn branch_error(error: BranchError) -> ErrorBody {
    match error {
        BranchError::Refused { code, message } => ErrorBody::new(code, message),
        BranchError::WouldOverwrite { message, paths } => {
            let mut body = ErrorBody::new(
                massorca_protocol::codes::GIT_CHECKOUT_WOULD_OVERWRITE,
                message,
            );
            body.data = Some(json!({ "paths": paths }));
            body
        }
        BranchError::Git(error) => git_error(error),
    }
}
