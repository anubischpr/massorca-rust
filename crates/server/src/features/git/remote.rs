//! `git.fetch`, `git.pull`, `git.push` and `git.abort` (decision 0013). Mutations hold the workspace's lock.

use std::time::Duration;

use massorca_core_git::{FetchTarget, GitError, RemoteError};
use massorca_protocol::methods::git::GitWorkspaceRef;
use massorca_protocol::methods::git::remote::{
    self, GitAborted, GitFetchParams, GitFetched, GitPullParams, GitPulled, GitPushParams,
    GitPushed, MAX_REMOTE_TIMEOUT_MS,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use super::{blocking, git_error, repository};
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(super) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries.methods.register(
            Owner::Core,
            remote::FETCH,
            capabilities::GIT,
            Permission::Workspace,
            fetch,
        )?,
        registries.methods.register(
            Owner::Core,
            remote::PULL,
            capabilities::GIT,
            Permission::Workspace,
            pull,
        )?,
        registries.methods.register(
            Owner::Core,
            remote::PUSH,
            capabilities::GIT,
            Permission::Workspace,
            push,
        )?,
        registries.methods.register(
            Owner::Core,
            remote::ABORT,
            capabilities::GIT,
            Permission::Workspace,
            abort,
        )?,
    ])
}

async fn fetch(ctx: RequestCtx, params: GitFetchParams) -> Result<GitFetched, ErrorBody> {
    let timeout = timeout(params.timeout_ms);
    let repository = repository(&ctx, params.workspace).await?;
    let _guard = repository.lock().await;
    blocking(move || {
        let target = match (&params.remote, params.all) {
            (_, true) => FetchTarget::All,
            (Some(remote), false) => FetchTarget::Remote(remote),
            (None, false) => FetchTarget::Default,
        };
        repository
            .git
            .fetch(&repository.dir, target, timeout)
            .map_err(remote_error)
    })
    .await
}

async fn pull(ctx: RequestCtx, params: GitPullParams) -> Result<GitPulled, ErrorBody> {
    let timeout = timeout(params.timeout_ms);
    let repository = repository(&ctx, params.workspace).await?;
    let _guard = repository.lock().await;
    blocking(move || {
        repository
            .git
            .pull(&repository.dir, params.mode, timeout)
            .map_err(remote_error)
    })
    .await
}

async fn push(ctx: RequestCtx, params: GitPushParams) -> Result<GitPushed, ErrorBody> {
    let timeout = timeout(params.timeout_ms);
    let repository = repository(&ctx, params.workspace).await?;
    let _guard = repository.lock().await;
    blocking(move || {
        repository
            .git
            .push(
                &repository.dir,
                params.remote.as_deref(),
                params.force_with_lease.as_deref(),
                timeout,
            )
            .map_err(remote_error)
    })
    .await
}

async fn abort(ctx: RequestCtx, params: GitWorkspaceRef) -> Result<GitAborted, ErrorBody> {
    let repository = repository(&ctx, params.workspace).await?;
    let _guard = repository.lock().await;
    blocking(move || repository.git.abort(&repository.dir).map_err(remote_error)).await
}

/// The caller's limit, never above the five-minute ceiling of decision 0013.
fn timeout(requested_ms: Option<u64>) -> Duration {
    Duration::from_millis(
        requested_ms
            .unwrap_or(MAX_REMOTE_TIMEOUT_MS)
            .clamp(1_000, MAX_REMOTE_TIMEOUT_MS),
    )
}

fn remote_error(error: RemoteError) -> ErrorBody {
    match error {
        RemoteError::Refused { code, message } => ErrorBody::new(code, message),
        // A remote that never answers looks the same as one that cannot be reached.
        RemoteError::Git(error @ GitError::Timeout { .. }) => {
            ErrorBody::new(codes::GIT_REMOTE_UNREACHABLE, error.to_string())
        }
        RemoteError::Git(error) => git_error(error),
    }
}
