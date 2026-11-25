//! `project.add`, `project.list`, `project.show`, `project.inspect`, `project.remove` and `project.set_base` (capability `projects/1`), `project.update` (capability `project-update/1`) and `project.init_git` (capability `project-init-git/1`), over the workspace domain.

use std::sync::Arc;

use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::events::{
    TOPIC_PROJECT_ADDED, TOPIC_PROJECT_CHANGED, TOPIC_PROJECT_REMOVED, TOPIC_WORKSPACE_REMOVED,
};
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectInspectParams, ProjectInspection, ProjectList,
    ProjectRef, ProjectRemoveParams, ProjectSetBaseParams, ProjectUpdateParams,
};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceState};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{AppState, Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::features::config::workspace_layout;
use crate::features::workspace_domain::{blocking, for_mutation, publish, shared};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::PROJECTS, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            projects::ADD,
            capabilities::PROJECTS,
            Permission::Workspace,
            add,
        )?,
        registries.methods.register(
            Owner::Core,
            projects::LIST,
            capabilities::PROJECTS,
            Permission::Read,
            list,
        )?,
        registries.methods.register(
            Owner::Core,
            projects::SHOW,
            capabilities::PROJECTS,
            Permission::Read,
            show,
        )?,
        registries.methods.register(
            Owner::Core,
            projects::INSPECT,
            capabilities::PROJECTS,
            Permission::Read,
            inspect,
        )?,
        registries.methods.register(
            Owner::Core,
            projects::REMOVE,
            capabilities::PROJECTS,
            Permission::Workspace,
            remove,
        )?,
        registries.methods.register(
            Owner::Core,
            projects::SET_BASE,
            capabilities::PROJECTS,
            Permission::Workspace,
            set_base,
        )?,
        registries
            .capabilities
            .register(capabilities::PROJECT_UPDATE, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            projects::UPDATE,
            capabilities::PROJECT_UPDATE,
            Permission::Workspace,
            update,
        )?,
        registries
            .capabilities
            .register(capabilities::PROJECT_INIT_GIT, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            projects::INIT_GIT,
            capabilities::PROJECT_INIT_GIT,
            Permission::Workspace,
            init_git,
        )?,
    ])
}

async fn add(ctx: RequestCtx, params: ProjectAddParams) -> Result<Project, ErrorBody> {
    let layout = workspace_layout(&ctx).await?;
    let domain = for_mutation(&ctx).await?.with_layout(layout);
    let project = blocking(move || domain.add_project(params)).await?;
    publish(&ctx, TOPIC_PROJECT_ADDED, &project.id);
    Ok(project)
}

async fn list(ctx: RequestCtx, _params: NoParams) -> Result<ProjectList, ErrorBody> {
    let domain = shared(&ctx);
    let projects = blocking(move || domain.list_projects()).await?;
    Ok(ProjectList { projects })
}

async fn show(ctx: RequestCtx, params: ProjectRef) -> Result<Project, ErrorBody> {
    let domain = shared(&ctx);
    blocking(move || domain.show_project(&params.project)).await
}

async fn inspect(
    ctx: RequestCtx,
    params: ProjectInspectParams,
) -> Result<ProjectInspection, ErrorBody> {
    let domain = shared(&ctx);
    blocking(move || domain.inspect_project(&params.path)).await
}

async fn remove(ctx: RequestCtx, params: ProjectRemoveParams) -> Result<Project, ErrorBody> {
    let domain = Arc::new(for_mutation(&ctx).await?);
    if params.forget_workspaces {
        ensure_no_live_work(&ctx, Arc::clone(&domain), params.project.clone()).await?;
    }
    let forget = params.forget_workspaces;
    let removed = blocking(move || domain.remove_project(&params.project, forget)).await?;
    publish(&ctx, TOPIC_PROJECT_REMOVED, &removed.project.id);
    for workspace in &removed.workspaces {
        publish(&ctx, TOPIC_WORKSPACE_REMOVED, workspace);
    }
    Ok(removed.project)
}

/// Refuses to forget a project while one of its workspaces has a live terminal or agent run, or is being removed:
/// forgetting the records would orphan those runs. Best effort: a terminal started after this check is not seen.
async fn ensure_no_live_work(
    ctx: &RequestCtx,
    domain: Arc<Workspaces>,
    project: String,
) -> Result<(), ErrorBody> {
    let state = Arc::clone(&ctx.state);
    tokio::task::spawn_blocking(move || {
        for workspace in domain.list_workspaces(Some(&project))? {
            if has_live_work(&state, &workspace)? {
                return Err(busy(&workspace));
            }
        }
        Ok(())
    })
    .await
    .unwrap_or_else(|error| {
        Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the workspace task failed: {error}"),
        ))
    })
}

fn has_live_work(state: &AppState, workspace: &Workspace) -> Result<bool, ErrorBody> {
    if workspace.state == WorkspaceState::Removing {
        return Ok(true);
    }
    let live_terminal = state
        .terminals
        .list(Some(&workspace.id))
        .iter()
        .any(|terminal| terminal.exit.is_none());
    let live_run = state
        .agents
        .list(Some(&workspace.id))?
        .iter()
        .any(|session| session.execution.state != TerminalState::Exited);
    Ok(live_terminal || live_run)
}

fn busy(workspace: &Workspace) -> ErrorBody {
    ErrorBody::new(
        codes::WORKSPACE_BUSY,
        format!(
            "workspace {:?} has a live terminal or agent run, or is being removed; stop it first",
            workspace.name
        ),
    )
}

async fn set_base(ctx: RequestCtx, params: ProjectSetBaseParams) -> Result<Project, ErrorBody> {
    let domain = for_mutation(&ctx).await?;
    let project = blocking(move || domain.set_project_base(params)).await?;
    publish(&ctx, TOPIC_PROJECT_CHANGED, &project.id);
    Ok(project)
}

async fn update(ctx: RequestCtx, params: ProjectUpdateParams) -> Result<Project, ErrorBody> {
    let domain = for_mutation(&ctx).await?;
    let project = blocking(move || domain.update_project(params)).await?;
    publish(&ctx, TOPIC_PROJECT_CHANGED, &project.id);
    Ok(project)
}

async fn init_git(ctx: RequestCtx, params: ProjectRef) -> Result<Project, ErrorBody> {
    let layout = workspace_layout(&ctx).await?;
    let domain = for_mutation(&ctx).await?.with_layout(layout);
    let project = blocking(move || domain.init_project_git(&params.project)).await?;
    publish(&ctx, TOPIC_PROJECT_CHANGED, &project.id);
    Ok(project)
}
