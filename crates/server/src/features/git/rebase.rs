//! `git.rebase_base` (capability `git-rebase-base/1`, decision 0181): a worktree's branch rebased onto its base, under
//! the workspace's lock.

use std::sync::Arc;

use massorca_core_git::RebaseOutcome;
use massorca_protocol::methods::git::rebase::{self, GitRebaseBaseParams, GitRebased};
use massorca_protocol::{ErrorBody, capabilities};

use super::{blocking, repository};
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(super) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::GIT_REBASE_BASE, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            rebase::REBASE_BASE,
            capabilities::GIT_REBASE_BASE,
            Permission::Workspace,
            rebase_base,
        )?,
    ])
}

async fn rebase_base(
    ctx: RequestCtx,
    params: GitRebaseBaseParams,
) -> Result<GitRebased, ErrorBody> {
    let repository = repository(&ctx, params.workspace.clone()).await?;
    let _guard = repository.lock().await;
    let workspaces = Arc::clone(&ctx.state.workspaces);
    blocking(move || {
        workspaces
            .rebase_workspace_base(&params.workspace, params.refresh)
            .map(outcome)
            .map_err(ErrorBody::from)
    })
    .await
}

fn outcome(outcome: RebaseOutcome) -> GitRebased {
    match outcome {
        RebaseOutcome::Rebased { head } => GitRebased::Rebased { head },
        RebaseOutcome::UpToDate => GitRebased::UpToDate,
        RebaseOutcome::Conflict { paths } => GitRebased::Conflict { paths },
    }
}
