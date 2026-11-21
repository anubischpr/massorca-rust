//! `git.commit` (decisions 0010 and 0013). Holds the workspace's mutation lock.

use massorca_core_git::{CommitError, CommitRequest};
use massorca_protocol::methods::git::commit::{self, GitCommitParams, GitCommitted};
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
        commit::COMMIT,
        capabilities::GIT,
        Permission::Workspace,
        handle,
    )?])
}

async fn handle(ctx: RequestCtx, params: GitCommitParams) -> Result<GitCommitted, ErrorBody> {
    let repository = repository(&ctx, params.workspace).await?;
    let _guard = repository.lock().await;
    blocking(move || {
        let request = CommitRequest {
            message: params.message.as_deref(),
            amend: params.amend,
            allow_published: params.allow_published,
        };
        repository
            .git
            .commit(&repository.dir, request)
            .map_err(|error| match error {
                CommitError::Refused { code, message } => ErrorBody::new(code, message),
                CommitError::Git(error) => git_error(error),
            })
    })
    .await
}
