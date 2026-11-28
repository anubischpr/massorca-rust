//! `workspace.create`, `workspace.list`, `workspace.show` and `workspace.remove` (capability `workspaces/1`),
//! `workspace.update` (`workspace-update/1`), `workspace.check` (`workspace-check/1`, decision 0089) and
//! `workspace.current` (`workspace-current/1`), `workspace.rename` (`workspace-rename/1`, decision 0111), `workspace.stop_runs` (`workspace-stop-runs/1`). `workspace-unique-name/1` says `workspace.create` honours `unique_name`. Removal publishes `workspace.removing` between its two halves and `workspace.removed` after the row is gone.
//! A create runs the new worktree's setup script in a service terminal (decision 0086); a removal runs the project's
//! archive script in the worktree first (decision 0095). With `with_children`, the workspaces of delegated children are
//! removed first, each the same way (decision 0110).

use std::sync::Arc;

use massorca_protocol::methods::events::{
    TOPIC_WORKSPACE_CREATED, TOPIC_WORKSPACE_REMOVED, TOPIC_WORKSPACE_REMOVING,
    TOPIC_WORKSPACE_UPDATED,
};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceArchive, WorkspaceCheck, WorkspaceCreateParams,
    WorkspaceCurrentParams, WorkspaceList, WorkspaceListParams, WorkspaceRef,
    WorkspaceRemoveParams, WorkspaceRemoved, WorkspaceRenameParams, WorkspaceRunsStopped,
    WorkspaceUpdateParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};
use serde_json::json;

use massorca_core_workspace::{RemovalWaivers, Workspaces};

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
        registries
            .capabilities
            .register(capabilities::WORKSPACE_IN_PLACE, Owner::Core)?,
        registries
            .capabilities
            .register(capabilities::WORKSPACE_STOP_RUNS, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            workspaces::STOP_RUNS,
            capabilities::WORKSPACE_STOP_RUNS,
            Permission::Workspace,
            stop_runs_request,
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
    // A repeated request id changed nothing, so it neither rewrites the runner nor starts setup again. An in-place
    // workspace has no new worktree to set up (decision 0112).
    if !created.repeated && !workspace.in_place {
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

/// `workspace.stop_runs`: stops the runs of the workspace `params` names, which stays. See [`stop_runs`].
async fn stop_runs_request(
    ctx: RequestCtx,
    params: WorkspaceRef,
) -> Result<WorkspaceRunsStopped, ErrorBody> {
    let domain = shared(&ctx);
    let id = blocking(move || {
        domain
            .show_workspace(&params.workspace)
            .map(|shown| shown.id)
    })
    .await?;
    stop_runs(&ctx, &id).await
}

/// Stops every agent run, then every terminal, of the workspace `id`, so its folder is removed with nothing running in
/// it. A run that does not stop refuses the removal, which stays `removing`, so `rm` again retries it. Returns the ids
/// it asked to stop.
async fn stop_runs(ctx: &RequestCtx, id: &str) -> Result<WorkspaceRunsStopped, ErrorBody> {
    let agents = ctx.state.agents.clone();
    let terminals = ctx.state.terminals.clone();
    let id = id.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut stopped = WorkspaceRunsStopped::default();
        for _ in 0..STOP_PASSES {
            let live: Vec<_> = agents
                .list(Some(&id))
                .map_err(ErrorBody::from)?
                .into_iter()
                .filter(|session| session.execution.state != TerminalState::Exited)
                .collect();
            // Listed before any run stops, so an agent's own terminal counts as running here, as it is.
            let running: Vec<_> = terminals
                .list(Some(&id))
                .into_iter()
                .filter(|terminal| terminal.exit.is_none())
                .collect();
            for session in &live {
                agents.stop(&session.id, false).map_err(ErrorBody::from)?;
                if !stopped.agents.contains(&session.id) {
                    stopped.agents.push(session.id.clone());
                }
            }
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
                if !stopped.terminals.contains(&terminal.id) {
                    stopped.terminals.push(terminal.id.clone());
                }
            }
            if live.is_empty() && running.is_empty() {
                return Ok(stopped);
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

/// Forgets the terminals of the removed workspace `id` with their output, exited ones and those a retained session kept
/// included; the workspace's session rows and their archived output go with its row. `keep` is the archive script's
/// terminal, whose output the removal names. Files on disk are not touched.
async fn forget_terminals(ctx: &RequestCtx, id: &str, keep: Option<String>) {
    let terminals = ctx.state.terminals.clone();
    let id = id.to_owned();
    let forgotten = tokio::task::spawn_blocking(move || {
        for terminal in terminals.list(Some(&id)) {
            if keep.as_deref() != Some(terminal.id.as_str()) {
                // Already exited after `stop_runs`; one gone meanwhile needs nothing more.
                let _ = terminals.close(&terminal.id);
            }
        }
    })
    .await;
    if let Err(error) = forgotten {
        tracing::warn!(%error, "forgetting a removed workspace's terminals failed");
    }
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
    let domain = Arc::new(for_mutation(&ctx).await?);
    let (target, children) = {
        let domain = Arc::clone(&domain);
        let selector = params.workspace.clone();
        blocking(move || domain.delegated_child_workspaces(&selector, waivers)).await?
    };
    if !children.is_empty() && !params.with_children {
        return Err(has_children(&target, &children));
    }
    let mut removed_children = Vec::new();
    for child in &children {
        match remove_workspace(&ctx, child.id.clone(), waivers, params.allow_failed_archive).await {
            Ok(removed) => removed_children.push(removed),
            // A child removed or forgotten since the list was read is skipped.
            Err(error) if error.code == codes::WORKSPACE_NOT_FOUND => {}
            Err(error) => return Err(child_refused(error, &target, child, &removed_children)),
        }
    }
    let mut removed =
        remove_workspace(&ctx, target.id, waivers, params.allow_failed_archive).await?;
    removed.removed_children = removed_children;
    Ok(removed)
}

/// Finishes a removal that `begun` (marked `removing`) started elsewhere: `workspace.removing`, the archive script
/// (decision 0095), then the removal and `workspace.removed`. Nothing is stopped: the caller checked under the mutation
/// lock that nothing runs in it, and a `removing` workspace takes no new start, resume or terminal (decision 0163).
pub(crate) async fn finish_begun_removal(
    ctx: &RequestCtx,
    begun: Workspace,
    waivers: RemovalWaivers,
    allow_failed_archive: bool,
) -> Result<WorkspaceRemoved, ErrorBody> {
    let domain = Arc::new(for_mutation(ctx).await?);
    publish(ctx, TOPIC_WORKSPACE_REMOVING, &begun.id);
    let archive = archive_before_removal(ctx, &domain, &begun, allow_failed_archive).await?;
    let id = begun.id.clone();
    let mut removed = blocking(move || domain.finish_remove(&id, waivers)).await?;
    let archive_terminal = match &archive {
        Some(WorkspaceArchive::Succeeded { terminal })
        | Some(WorkspaceArchive::Failed {
            terminal: Some(terminal),
            ..
        }) => Some(terminal.clone()),
        _ => None,
    };
    forget_terminals(ctx, &begun.id, archive_terminal).await;
    removed.archive = archive;
    publish(ctx, TOPIC_WORKSPACE_REMOVED, &removed.workspace.id);
    Ok(removed)
}

/// Runs the archive script of the `removing` workspace `begun`. A refusal returns the row to `ready` before the
/// refusal is returned, so nothing is removed or stopped (decision 0095).
async fn archive_before_removal(
    ctx: &RequestCtx,
    domain: &Arc<Workspaces>,
    begun: &Workspace,
    allow_failed_archive: bool,
) -> Result<Option<WorkspaceArchive>, ErrorBody> {
    match workspace_archive::run(ctx, begun, allow_failed_archive).await {
        Ok(archive) => Ok(archive),
        Err(refusal) => {
            let id = begun.id.clone();
            let cancel = Arc::clone(domain);
            if blocking(move || cancel.cancel_remove(&id)).await.is_ok() {
                publish(ctx, TOPIC_WORKSPACE_UPDATED, &begun.id);
            }
            Err(refusal)
        }
    }
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
    let archive = archive_before_removal(ctx, &domain, &begun, allow_failed_archive).await?;
    stop_runs(ctx, &begun.id).await?;
    let id = begun.id.clone();
    let mut removed = blocking(move || domain.finish_remove(&id, waivers)).await?;
    let archive_terminal = match &archive {
        Some(WorkspaceArchive::Succeeded { terminal })
        | Some(WorkspaceArchive::Failed {
            terminal: Some(terminal),
            ..
        }) => Some(terminal.clone()),
        _ => None,
    };
    forget_terminals(ctx, &begun.id, archive_terminal).await;
    removed.archive = archive;
    publish(ctx, TOPIC_WORKSPACE_REMOVED, &removed.workspace.id);
    Ok(removed)
}

/// `workspace.has_children`: the message and `data.children` name the child workspaces, deepest first.
fn has_children(target: &Workspace, children: &[Workspace]) -> ErrorBody {
    let names: Vec<&str> = children.iter().map(|child| child.name.as_str()).collect();
    ErrorBody {
        data: Some(json!({ "children": names })),
        ..ErrorBody::new(
            codes::WORKSPACE_HAS_CHILDREN,
            format!(
                "{} has workspaces of delegated children: {}; remove with with_children to remove them first",
                target.name,
                names.join(", ")
            ),
        )
    }
}

/// A child's refusal keeps its code; the message and data say which child refused and which were removed before it.
fn child_refused(
    error: ErrorBody,
    target: &Workspace,
    child: &Workspace,
    removed: &[WorkspaceRemoved],
) -> ErrorBody {
    let removed: Vec<&str> = removed
        .iter()
        .map(|done| done.workspace.name.as_str())
        .collect();
    let before = if removed.is_empty() {
        String::new()
    } else {
        format!("; removed before it: {}", removed.join(", "))
    };
    let mut data = json!({ "child": child.name, "removed_children": removed });
    if let Some(cause) = error.data {
        data["cause"] = cause;
    }
    ErrorBody {
        code: error.code,
        message: format!(
            "child workspace {}: {}; {} is kept{before}",
            child.name, error.message, target.name
        ),
        data: Some(data),
    }
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
