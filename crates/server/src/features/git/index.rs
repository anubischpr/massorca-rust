//! `git.stage`, `git.unstage` and `git.discard` (decisions 0010 and 0013). Each holds the workspace's mutation lock.

use massorca_core_git::{DiscardError, DiscardWaivers, PathSet};
use massorca_protocol::methods::git::index::{
    self, GitDiscardParams, GitDiscarded, GitPathsParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};
use serde_json::json;

use super::{blocking, git_error, relative_path, repository};
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(super) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries.methods.register(
            Owner::Core,
            index::STAGE,
            capabilities::GIT,
            Permission::Workspace,
            stage,
        )?,
        registries.methods.register(
            Owner::Core,
            index::UNSTAGE,
            capabilities::GIT,
            Permission::Workspace,
            unstage,
        )?,
        registries.methods.register(
            Owner::Core,
            index::DISCARD,
            capabilities::GIT,
            Permission::Workspace,
            discard,
        )?,
    ])
}

async fn stage(ctx: RequestCtx, params: GitPathsParams) -> Result<(), ErrorBody> {
    check_paths(&params.paths, params.all)?;
    let repository = repository(&ctx, params.workspace).await?;
    let _guard = repository.lock().await;
    blocking(move || {
        let paths = path_set(&params.paths, params.all);
        repository
            .git
            .stage(&repository.dir, paths)
            .map_err(git_error)
    })
    .await
}

async fn unstage(ctx: RequestCtx, params: GitPathsParams) -> Result<(), ErrorBody> {
    check_paths(&params.paths, params.all)?;
    let repository = repository(&ctx, params.workspace).await?;
    let _guard = repository.lock().await;
    blocking(move || {
        let paths = path_set(&params.paths, params.all);
        repository
            .git
            .unstage(&repository.dir, paths)
            .map_err(git_error)
    })
    .await
}

async fn discard(ctx: RequestCtx, params: GitDiscardParams) -> Result<GitDiscarded, ErrorBody> {
    check_paths(&params.paths, params.all)?;
    let repository = repository(&ctx, params.workspace).await?;
    let _guard = repository.lock().await;
    let waivers = DiscardWaivers {
        delete_untracked: params.delete_untracked,
        include_staged: params.include_staged,
    };
    blocking(move || {
        let paths = path_set(&params.paths, params.all);
        repository
            .git
            .discard(&repository.dir, paths, waivers, params.dry_run)
            .map_err(|error| match error {
                DiscardError::Refused {
                    code,
                    message,
                    paths,
                } => {
                    let mut body = ErrorBody::new(code, message);
                    body.data = Some(json!({ "paths": paths }));
                    body
                }
                DiscardError::Git(error) => git_error(error),
            })
    })
    .await
}

/// Exactly one of `paths` and `all`, and every path inside the workspace.
fn check_paths(paths: &[String], all: bool) -> Result<(), ErrorBody> {
    if paths.is_empty() == !all {
        return Err(ErrorBody::new(
            codes::GIT_PATHS_REQUIRED,
            "name the files in `paths`, or set `all`, but not both",
        ));
    }
    for path in paths {
        relative_path(path)?;
    }
    Ok(())
}

fn path_set(paths: &[String], all: bool) -> PathSet<'_> {
    if all {
        PathSet::All
    } else {
        PathSet::Paths(paths)
    }
}
