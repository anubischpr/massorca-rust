//! `git.diff` and `git.diff_file` (decision 0011).

use massorca_protocol::methods::git::diff::{
    self, DEFAULT_DIFF_LIMIT, DEFAULT_PATCH_BYTES, DiffTarget, GitDiff, GitDiffFile,
    GitDiffFileParams, GitDiffParams, MAX_DIFF_LIMIT, MAX_PATCH_BYTES,
};
use massorca_protocol::{ErrorBody, capabilities};

use super::{Repository, blocking, git_error, relative_path, repository, require_commit};
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(super) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries.methods.register(
            Owner::Core,
            diff::DIFF,
            capabilities::GIT,
            Permission::Read,
            list,
        )?,
        registries.methods.register(
            Owner::Core,
            diff::DIFF_FILE,
            capabilities::GIT,
            Permission::Read,
            file,
        )?,
    ])
}

async fn list(ctx: RequestCtx, params: GitDiffParams) -> Result<GitDiff, ErrorBody> {
    let limit = params
        .limit
        .unwrap_or(DEFAULT_DIFF_LIMIT)
        .clamp(1, MAX_DIFF_LIMIT);
    let repository = repository(&ctx, params.workspace).await?;
    let target = params.target;
    blocking(move || {
        check_target(&repository, &target)?;
        repository
            .git
            .diff(&repository.dir, &target, limit as usize)
            .map_err(git_error)
    })
    .await
}

async fn file(ctx: RequestCtx, params: GitDiffFileParams) -> Result<GitDiffFile, ErrorBody> {
    let max_bytes = params
        .max_bytes
        .unwrap_or(DEFAULT_PATCH_BYTES)
        .clamp(1, MAX_PATCH_BYTES);
    relative_path(&params.path)?;
    if let Some(original) = &params.original_path {
        relative_path(original)?;
    }
    let repository = repository(&ctx, params.workspace).await?;
    blocking(move || {
        check_target(&repository, &params.target)?;
        repository
            .git
            .diff_file(
                &repository.dir,
                &params.target,
                &params.path,
                params.original_path.as_deref(),
                max_bytes as usize,
            )
            .map_err(git_error)
    })
    .await
}

/// A range's revisions must name commits; the other modes compare fixed sides.
fn check_target(repository: &Repository, target: &DiffTarget) -> Result<(), ErrorBody> {
    if let DiffTarget::Range { from, to } = target {
        require_commit(repository, from)?;
        if let Some(to) = to {
            require_commit(repository, to)?;
        }
    }
    Ok(())
}
