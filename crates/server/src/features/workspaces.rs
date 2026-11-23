//! `workspace.create`, `workspace.list`, `workspace.show` and `workspace.remove` (capability `workspaces/1`).
//! Removal publishes `workspace.removing` between its two halves and `workspace.removed` after the row is gone.

use std::sync::Arc;

use massorca_protocol::methods::events::{
    TOPIC_WORKSPACE_CREATED, TOPIC_WORKSPACE_REMOVED, TOPIC_WORKSPACE_REMOVING,
};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceCreateParams, WorkspaceList, WorkspaceListParams, WorkspaceRef,
    WorkspaceRemoveParams, WorkspaceRemoved,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use massorca_core_workspace::RemovalWaivers;

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::{blocking, for_mutation, publish, shared};
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
    ])
}

async fn create(ctx: RequestCtx, params: WorkspaceCreateParams) -> Result<Workspace, ErrorBody> {
    let domain = for_mutation(&ctx).await?;
    let workspace = blocking(move || domain.create_workspace(params)).await?;
    publish(&ctx, TOPIC_WORKSPACE_CREATED, &workspace.id);
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

async fn remove(
    ctx: RequestCtx,
    params: WorkspaceRemoveParams,
) -> Result<WorkspaceRemoved, ErrorBody> {
    let domain = Arc::new(for_mutation(&ctx).await?);
    let waivers = RemovalWaivers {
        force: params.force,
        discard_ignored: params.discard_ignored,
        forget_orphan: params.forget_orphan,
    };
    let selector = params.workspace;
    let begun = {
        let domain = Arc::clone(&domain);
        blocking(move || domain.begin_remove(&selector, waivers)).await?
    };
    publish(&ctx, TOPIC_WORKSPACE_REMOVING, &begun.id);
    stop_runs(&ctx, &begun.id).await?;
    let id = begun.id.clone();
    let removed = blocking(move || domain.finish_remove(&id, waivers)).await?;
    publish(&ctx, TOPIC_WORKSPACE_REMOVED, &removed.workspace.id);
    Ok(removed)
}
