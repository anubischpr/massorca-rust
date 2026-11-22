//! `git.status`.

use massorca_protocol::methods::git::status::{
    self, DEFAULT_STATUS_LIMIT, GitStatus, GitStatusParams, MAX_STATUS_LIMIT,
};
use massorca_protocol::{ErrorBody, capabilities};

use super::{blocking, git_error, repository};
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(super) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![registries.methods.register(
        Owner::Core,
        status::STATUS,
        capabilities::GIT,
        Permission::Read,
        handle,
    )?])
}

async fn handle(ctx: RequestCtx, params: GitStatusParams) -> Result<GitStatus, ErrorBody> {
    let limit = params
        .limit
        .unwrap_or(DEFAULT_STATUS_LIMIT)
        .clamp(1, MAX_STATUS_LIMIT);
    let repository = repository(&ctx, params.workspace).await?;
    blocking(move || {
        repository
            .git
            .status(&repository.dir, limit as usize)
            .map_err(git_error)
    })
    .await
}
