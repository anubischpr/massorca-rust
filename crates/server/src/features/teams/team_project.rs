//! `team.project_add`, `team.project_update` and `team.project_rm` (capability `teams/1`, decision 0072).

use massorca_core_store::{Store, TeamProjectRow, new_id, now_ms};
use massorca_protocol::methods::events::{TOPIC_TASK_CHANGED, TOPIC_TEAM_CHANGED};
use massorca_protocol::methods::teams::{
    self, TeamProject, TeamProjectAddParams, TeamProjectRemoveParams, TeamProjectUpdateParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::publish;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::{blocking, non_blank, selectors, store_error};

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        methods.register(
            Owner::Core,
            teams::PROJECT_ADD,
            capabilities::TEAMS,
            Permission::Workspace,
            add,
        )?,
        methods.register(
            Owner::Core,
            teams::PROJECT_UPDATE,
            capabilities::TEAMS,
            Permission::Workspace,
            update,
        )?,
        methods.register(
            Owner::Core,
            teams::PROJECT_REMOVE,
            capabilities::TEAMS,
            Permission::Workspace,
            remove,
        )?,
    ])
}

async fn add(ctx: RequestCtx, params: TeamProjectAddParams) -> Result<TeamProject, ErrorBody> {
    let project = blocking(&ctx, move |store, _agents| add_project(store, params)).await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &project.team_id);
    Ok(project)
}

async fn update(
    ctx: RequestCtx,
    params: TeamProjectUpdateParams,
) -> Result<TeamProject, ErrorBody> {
    let project = blocking(&ctx, move |store, _agents| update_project(store, params)).await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &project.team_id);
    Ok(project)
}

/// Removes a project. Its tasks stay on the Board without it, and each one is published as changed.
async fn remove(
    ctx: RequestCtx,
    params: TeamProjectRemoveParams,
) -> Result<TeamProject, ErrorBody> {
    let (project, tasks) =
        blocking(&ctx, move |store, _agents| remove_project(store, params)).await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &project.team_id);
    for id in &tasks {
        publish(&ctx, TOPIC_TASK_CHANGED, id);
    }
    Ok(project)
}

fn add_project(store: &Store, params: TeamProjectAddParams) -> Result<TeamProject, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let row = TeamProjectRow {
        id: new_id(),
        team_id: team.id.clone(),
        name: non_blank(&params.name, "the project name")?,
        goal: params.goal.unwrap_or_default().trim().to_owned(),
        brief: params.brief.unwrap_or_default(),
        created_at: now_ms(),
    };
    store.insert_team_project(&row).map_err(store_error)?;
    Ok(project_view(&row))
}

fn update_project(
    store: &Store,
    params: TeamProjectUpdateParams,
) -> Result<TeamProject, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let mut row = selectors::team_project(store, &team, &params.project)?;
    if let Some(name) = params.name {
        row.name = non_blank(&name, "the project name")?;
    }
    if let Some(goal) = params.goal {
        row.goal = goal.trim().to_owned();
    }
    if let Some(brief) = params.brief {
        row.brief = brief;
    }
    if !store.update_team_project(&row).map_err(store_error)? {
        return Err(ErrorBody::new(
            codes::INTERNAL,
            "the project was removed by another call",
        ));
    }
    Ok(project_view(&row))
}

fn remove_project(
    store: &Store,
    params: TeamProjectRemoveParams,
) -> Result<(TeamProject, Vec<String>), ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let row = selectors::team_project(store, &team, &params.project)?;
    let linked = store.list_tasks_of_project(&row.id).map_err(store_error)?;
    if !store.delete_team_project(&row.id).map_err(store_error)? {
        return Err(ErrorBody::new(
            codes::INTERNAL,
            "the project was removed by another call",
        ));
    }
    let tasks = linked.into_iter().map(|task| task.id).collect();
    Ok((project_view(&row), tasks))
}

fn project_view(row: &TeamProjectRow) -> TeamProject {
    TeamProject {
        id: row.id.clone(),
        team_id: row.team_id.clone(),
        name: row.name.clone(),
        goal: row.goal.clone(),
        brief: row.brief.clone(),
        created_at_ms: row.created_at,
    }
}
