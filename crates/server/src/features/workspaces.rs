//! `workspace.create`, `workspace.list`, `workspace.show` and `workspace.remove` (capability `workspaces/1`),
//! `workspace.update` (`workspace-update/1`), `workspace.check` (`workspace-check/1`, decision 0089) and
//! `workspace.current` (`workspace-current/1`), `workspace.rename` (`workspace-rename/1`, decision 0111). `workspace-unique-name/1` says `workspace.create` honours `unique_name`. Removal publishes `workspace.removing` between its two halves and `workspace.removed` after the row is gone.
//! A create runs the new worktree's setup script in a service terminal (decision 0086); a removal runs the project's
//! archive script in the worktree first (decision 0095).

use std::sync::Arc;

use massorca_protocol::methods::events::{
    TOPIC_WORKSPACE_CREATED, TOPIC_WORKSPACE_REMOVED, TOPIC_WORKSPACE_REMOVING,
    TOPIC_WORKSPACE_UPDATED,
};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceCheck, WorkspaceCreateParams, WorkspaceCurrentParams, WorkspaceList,
    WorkspaceListParams, WorkspaceRef, WorkspaceRemoveParams, WorkspaceRemoved,
    WorkspaceRenameParams, WorkspaceUpdateParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use massorca_core_workspace::RemovalWaivers;

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::{blocking, for_mutation, publish, shared};
use crate::features::{workspace_archive, workspace_setup};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::WORKSPACES, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            workspaces::CREATE,
            capabilities::WORKSPACES,
            Permission::Workspace,
            create,
        )?,
        registries.methods.register(
            Owner::Core,
            workspaces::LIST,
            capabilities::WORKSPACES,
            Permission::Read,
            list,
        )?,
        registries.methods.register(
            Owner::Core,
            workspaces::SHOW,
            capabilities::WORKSPACES,
            Permission::Read,
            show,
        )?,
        registries.methods.register(
            Owner::Core,
            workspaces::REMOVE,
            capabilities::WORKSPACES,
            Permission::Workspace,
            remove,
        )?,
        registries
            .capabilities
            .register(capabilities::WORKSPACE_UNIQUE_NAME, Owner::Core)?,
        registries
            .capabilities
            .register(capabilities::WORKSPACE_UPDATE, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            workspaces::UPDATE,
            capabilities::WORKSPACE_UPDATE,
            Permission::Workspace,
            update,
        )?,
        registries
            .capabilities
            .register(capabilities::WORKSPACE_CHECK, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            workspaces::CHECK,
            capabilities::WORKSPACE_CHECK,
            Permission::Read,
            check,
        )?,
        registries
            .capabilities
            .register(capabilities::WORKSPACE_CURRENT, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            workspaces::CURRENT,
            capabilities::WORKSPACE_CURRENT,
            Permission::Read,
            current,
        )?,
        registries
            .capabilities
            .register(capabilities::WORKSPACE_RENAME, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            workspaces::RENAME,
            capabilities::WORKSPACE_RENAME,
            Permission::Workspace,
            rename,
        )?,
    ])
}

async fn create(ctx: RequestCtx, params: WorkspaceCreateParams) -> Result<Workspace, ErrorBody> {
    let no_setup = params.no_setup;
    let domain = for_mutation(&ctx).await?;
    // Why: a blank name asks for a generated one; the domain refuses a blank typed name.
    let created = blocking(move || domain.create_workspace_once(params)).await?;
    let mut workspace = created.workspace;
    publish(&ctx, TOPIC_WORKSPACE_CREATED, &workspace.id);
    // A repeated request id changed nothing, so it neither rewrites the runner nor starts setup again.
    if !created.repeated {
        workspace.setup = workspace_setup::start(&ctx, &workspace, no_setup).await;
    }
    Ok(workspace)
}

async fn list(ctx: RequestCtx, params: WorkspaceListParams) -> Result<WorkspaceList, ErrorBody> {
    let domain = shared(&ctx);
    let project = params.project.clone();
    let workspaces = {
        let domain = Arc::clone(&domain);
        blocking(move || domain.list_workspaces(params.project.as_deref())).await?
    };
    let unregistered = blocking(move || domain.unregistered_worktrees(project.as_deref())).await?;
    Ok(WorkspaceList {
        workspaces,
        unregistered,
    })
}

/// How many passes stop the runs of a workspace being removed. A run that started just before the removal began can
/// appear after the first pass; none can start after it.
const STOP_PASSES: usize = 3;

/// Stops every agent run, then every terminal, of the workspace `id`, so its folder is removed with nothing running in
/// it. A run that does not stop refuses the removal, which stays `removing`, so `rm` again retries it.
async fn stop_runs(ctx: &RequestCtx, id: &str) -> Result<(), ErrorBody> {
    let agents = ctx.state.agents.clone();
    let terminals = ctx.state.terminals.clone();
    let id = id.to_owned();
    tokio::task::spawn_blocking(move || {
        for _ in 0..STOP_PASSES {
            let live: Vec<_> = agents
                .list(Some(&id))
                .map_err(ErrorBody::from)?
                .into_iter()
                .filter(|session| session.execution.state != TerminalState::Exited)
                .collect();
            for session in &live {
                agents.stop(&session.id, false).map_err(ErrorBody::from)?;
            }
            let running: Vec<_> = terminals
                .list(Some(&id))
                .into_iter()
                .filter(|terminal| terminal.exit.is_none())
                .collect();
            for terminal in &running {
                let info = match terminals.kill(&terminal.id, false) {
                    Ok(info) if info.exit.is_none() => terminals.kill(&terminal.id, true),
                    other => other,
                };
                if let Ok(info) = info
                    && info.exit.is_none()
                {
                    return Err(ErrorBody::new(
                        codes::WORKSPACE_BUSY,
                        format!(
                            "terminal {} in the workspace did not stop; try again",
                            info.id
                        ),
                    ));
                }
            }
            if live.is_empty() && running.is_empty() {
                return Ok(());
            }
        }
        Err(ErrorBody::new(
            codes::WORKSPACE_BUSY,
            "runs kept starting in the workspace while it was being removed; try again",
        ))
    })
    .await
    .unwrap_or_else(|error| {
        Err(ErrorBody::new(
            codes::INTERNAL,
            format!("stopping the workspace's runs failed: {error}"),
        ))
    })
}

async fn show(ctx: RequestCtx, params: WorkspaceRef) -> Result<Workspace, ErrorBody> {
    let domain = shared(&ctx);
    blocking(move || domain.show_workspace_with_git_state(&params.workspace)).await
}

async fn current(ctx: RequestCtx, params: WorkspaceCurrentParams) -> Result<Workspace, ErrorBody> {
    let domain = shared(&ctx);
    blocking(move || domain.current_workspace(&params.path)).await
}

async fn remove(
    ctx: RequestCtx,
    params: WorkspaceRemoveParams,
) -> Result<WorkspaceRemoved, ErrorBody> {
    let waivers = RemovalWaivers {
        force: params.force,
        discard_ignored: params.discard_ignored,
        forget_orphan: params.forget_orphan,
    };
    remove_workspace(&ctx, params.workspace, waivers, params.allow_failed_archive).await
}

/// Finishes a removal that `begun` (marked `removing`) started elsewhere: `workspace.removing`, then the removal and
/// `workspace.removed`. Nothing is stopped: the caller checked under the mutation lock that nothing runs in it, and a
/// `removing` workspace takes no new start, resume or terminal (decision 0163).
pub(crate) async fn finish_begun_removal(
    ctx: &RequestCtx,
    begun: String,
    waivers: RemovalWaivers,
) -> Result<WorkspaceRemoved, ErrorBody> {
    let domain = for_mutation(ctx).await?;
    publish(ctx, TOPIC_WORKSPACE_REMOVING, &begun);
    let removed = blocking(move || domain.finish_remove(&begun, waivers)).await?;
    publish(ctx, TOPIC_WORKSPACE_REMOVED, &removed.workspace.id);
    Ok(removed)
}

/// Removes workspace `selector` with `waivers`: the refusals, `workspace.removing`, every run in it stopped, then the
/// removal and `workspace.removed`. `agent.forget` with `remove_workspace` (decision 0163) begins its removal itself,
/// under the start gate, and finishes it with [`finish_begun_removal`]; both meet the same removal guards and codes.
pub(crate) async fn remove_workspace(
    ctx: &RequestCtx,
    selector: String,
    waivers: RemovalWaivers,
    allow_failed_archive: bool,
) -> Result<WorkspaceRemoved, ErrorBody> {
    let domain = Arc::new(for_mutation(ctx).await?);
    let begun = {
        let domain = Arc::clone(&domain);
        blocking(move || domain.begin_remove(&selector, waivers)).await?
    };
    publish(ctx, TOPIC_WORKSPACE_REMOVING, &begun.id);
    // Before anything is stopped: the archive may save work from the running checkout.
    let archive = match workspace_archive::run(ctx, &begun, allow_failed_archive).await {
        Ok(archive) => archive,
        Err(refusal) => {
            let id = begun.id.clone();
            let cancel = Arc::clone(&domain);
            if blocking(move || cancel.cancel_remove(&id)).await.is_ok() {
                publish(ctx, TOPIC_WORKSPACE_UPDATED, &begun.id);
            }
            return Err(refusal);
        }
    };
    stop_runs(ctx, &begun.id).await?;
    let id = begun.id.clone();
    let mut removed = blocking(move || domain.finish_remove(&id, waivers)).await?;
    removed.archive = archive;
    publish(ctx, TOPIC_WORKSPACE_REMOVED, &removed.workspace.id);
    Ok(removed)
}

async fn update(ctx: RequestCtx, params: WorkspaceUpdateParams) -> Result<Workspace, ErrorBody> {
    let domain = shared(&ctx);
    let workspace = blocking(move || domain.update_workspace(params)).await?;
    publish(&ctx, TOPIC_WORKSPACE_UPDATED, &workspace.id);
    Ok(workspace)
}

/// Renames a generated name and its branch; the prefix setting applies as at create, so it takes the mutation handle.
async fn rename(ctx: RequestCtx, params: WorkspaceRenameParams) -> Result<Workspace, ErrorBody> {
    let domain = for_mutation(&ctx).await?;
    let workspace = blocking(move || domain.rename_workspace(params)).await?;
    publish(&ctx, TOPIC_WORKSPACE_UPDATED, &workspace.id);
    Ok(workspace)
}

async fn check(ctx: RequestCtx, params: WorkspaceRef) -> Result<WorkspaceCheck, ErrorBody> {
    let domain = shared(&ctx);
    blocking(move || domain.check_workspace(&params.workspace)).await
}
