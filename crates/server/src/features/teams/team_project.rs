//! `team.project_list`, `team.project_add`, `team.project_update` and `team.project_rm` (capability `teams/1`, decision 0072).

use massorca_core_store::{Store, TeamProjectRow, TeamRow, new_id, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::events::{TOPIC_TASK_CHANGED, TOPIC_TEAM_CHANGED};
use massorca_protocol::methods::projects::Project as WorkspaceProject;
use massorca_protocol::methods::teams::{
    self, CodeSource, TeamProject, TeamProjectAddParams, TeamProjectList, TeamProjectRemoveParams,
    TeamProjectShowParams, TeamProjectUpdateParams, TeamRef,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::{publish, shared};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::{blocking, invalid, non_blank, optional_text, selectors, store_error};

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        methods.register(
            Owner::Core,
            teams::PROJECT_LIST,
            capabilities::TEAMS,
            Permission::Read,
            list,
        )?,
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
        methods.register(
            Owner::Core,
            teams::PROJECT_SHOW,
            capabilities::TEAMS,
            Permission::Read,
            show,
        )?,
    ])
}

async fn list(ctx: RequestCtx, params: TeamRef) -> Result<TeamProjectList, ErrorBody> {
    let workspaces = shared(&ctx);
    blocking(&ctx, move |store, _agents| {
        let team = selectors::team(store, &params.team)?;
        let rows = store.list_team_projects(&team.id).map_err(store_error)?;
        let linked = workspace_projects(&workspaces)?;
        Ok(TeamProjectList {
            projects: rows.iter().map(|row| project_view(row, &linked)).collect(),
        })
    })
    .await
}

/// `team.project_show`: one project of a team, by id or exact name. Reads need no Workspaces.
async fn show(ctx: RequestCtx, params: TeamProjectShowParams) -> Result<TeamProject, ErrorBody> {
    let workspaces = shared(&ctx);
    blocking(&ctx, move |store, _agents| {
        let team = selectors::team(store, &params.team)?;
        let row = selectors::team_project(store, &team, &params.project)?;
        Ok(project_view(&row, &workspace_projects(&workspaces)?))
    })
    .await
}

async fn add(ctx: RequestCtx, params: TeamProjectAddParams) -> Result<TeamProject, ErrorBody> {
    let workspaces = shared(&ctx);
    let project = blocking(&ctx, move |store, _agents| {
        add_project(store, &workspaces, params)
    })
    .await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &project.team_id);
    Ok(project)
}

async fn update(
    ctx: RequestCtx,
    params: TeamProjectUpdateParams,
) -> Result<TeamProject, ErrorBody> {
    let workspaces = shared(&ctx);
    let project = blocking(&ctx, move |store, _agents| {
        update_project(store, &workspaces, params)
    })
    .await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &project.team_id);
    Ok(project)
}

/// Removes a project. Its tasks stay on the Board without it, and each one is published as changed.
async fn remove(
    ctx: RequestCtx,
    params: TeamProjectRemoveParams,
) -> Result<TeamProject, ErrorBody> {
    let workspaces = shared(&ctx);
    let (project, tasks) = blocking(&ctx, move |store, _agents| {
        remove_project(store, &workspaces, params)
    })
    .await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &project.team_id);
    for id in &tasks {
        publish(&ctx, TOPIC_TASK_CHANGED, id);
    }
    Ok(project)
}

fn add_project(
    store: &Store,
    workspaces: &Workspaces,
    params: TeamProjectAddParams,
) -> Result<TeamProject, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let code_source = code_source_name(&team, params.code_source)?;
    let source_project_id = source_project_id(workspaces, &team, params.source_project)?;
    one_code_source(source_project_id.as_deref(), code_source.as_deref())?;
    let row = TeamProjectRow {
        id: new_id(),
        team_id: team.id.clone(),
        name: non_blank(&params.name, "the project name")?,
        goal: params.goal.unwrap_or_default().trim().to_owned(),
        brief: params.brief.unwrap_or_default(),
        created_at: now_ms(),
        source_project_id,
        code_source,
    };
    store.insert_team_project(&row).map_err(store_error)?;
    Ok(project_view(&row, &workspace_projects(workspaces)?))
}

fn update_project(
    store: &Store,
    workspaces: &Workspaces,
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
    let code_given = params.source_project.is_some() || params.code_source.is_some();
    if let Some(source) = params.source_project {
        row.source_project_id = source_project_id(workspaces, &team, Some(source))?;
    }
    if params.code_source.is_some() {
        row.code_source = code_source_name(&team, params.code_source)?;
    }
    if code_given {
        one_code_source(row.source_project_id.as_deref(), row.code_source.as_deref())?;
    }
    if !store.update_team_project(&row).map_err(store_error)? {
        return Err(ErrorBody::new(
            codes::INTERNAL,
            "the project was removed by another call",
        ));
    }
    Ok(project_view(&row, &workspace_projects(workspaces)?))
}

fn remove_project(
    store: &Store,
    workspaces: &Workspaces,
    params: TeamProjectRemoveParams,
) -> Result<(TeamProject, Vec<String>), ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let row = selectors::team_project(store, &team, &params.project)?;
    let linked_projects = workspace_projects(workspaces)?;
    let linked = store.list_tasks_of_project(&row.id).map_err(store_error)?;
    if !store.delete_team_project(&row.id).map_err(store_error)? {
        return Err(ErrorBody::new(
            codes::INTERNAL,
            "the project was removed by another call",
        ));
    }
    let tasks = linked.into_iter().map(|task| task.id).collect();
    Ok((project_view(&row, &linked_projects), tasks))
}

/// The stored name of a recorded code source. Only a team with Workspaces on records one; clearing needs none.
fn code_source_name(
    team: &TeamRow,
    source: Option<CodeSource>,
) -> Result<Option<String>, ErrorBody> {
    let name = match source {
        None | Some(CodeSource::None) => return Ok(None),
        Some(CodeSource::ExistingRepository) => CODE_SOURCE_EXISTING,
        Some(CodeSource::NewFolderWithGit) => CODE_SOURCE_NEW_FOLDER,
        Some(CodeSource::Unknown) => return Err(invalid("the code source is not a known kind")),
    };
    require_workspaces(team, "record a code source")?;
    Ok(Some(name.to_owned()))
}

/// Connecting a project to code needs the team's Workspaces option (decision 0120, pv96); it gates nothing else.
fn require_workspaces(team: &TeamRow, action: &str) -> Result<(), ErrorBody> {
    if team.workspaces {
        return Ok(());
    }
    Err(ErrorBody::new(
        codes::TEAM_WORKSPACES_OFF,
        format!(
            "team {} has Workspaces off, so it cannot {action}; turn it on with: massorca team update TEAM --workspaces true",
            team.name
        ),
    ))
}

const CODE_SOURCE_EXISTING: &str = "existing_repository";
const CODE_SOURCE_NEW_FOLDER: &str = "new_folder_with_git";

fn code_source_of(name: &str) -> CodeSource {
    match name {
        CODE_SOURCE_EXISTING => CodeSource::ExistingRepository,
        CODE_SOURCE_NEW_FOLDER => CodeSource::NewFolderWithGit,
        _ => CodeSource::Unknown,
    }
}

/// The Workspaces project `selector` names by id, exact name or path; a blank selector links none, which a team with
/// Workspaces off may still do.
fn source_project_id(
    workspaces: &Workspaces,
    team: &TeamRow,
    selector: Option<String>,
) -> Result<Option<String>, ErrorBody> {
    match optional_text(selector) {
        Some(selector) => {
            require_workspaces(team, "link a Workspaces project")?;
            Ok(Some(workspaces.show_project(&selector)?.id))
        }
        None => Ok(None),
    }
}

/// A project has one code source: a linked Workspaces project or a new folder with git, never both (decision 0126).
fn one_code_source(linked: Option<&str>, code_source: Option<&str>) -> Result<(), ErrorBody> {
    if linked.is_some() && code_source == Some(CODE_SOURCE_NEW_FOLDER) {
        return Err(invalid(
            "a project has one code source: link a Workspaces project or record a new folder with git, not both",
        ));
    }
    Ok(())
}

fn workspace_projects(workspaces: &Workspaces) -> Result<Vec<WorkspaceProject>, ErrorBody> {
    Ok(workspaces.list_projects()?)
}

/// The wire view of a project. Its linked project's name and kind come from `linked` while that project exists.
fn project_view(row: &TeamProjectRow, linked: &[WorkspaceProject]) -> TeamProject {
    let source = row
        .source_project_id
        .as_deref()
        .and_then(|id| linked.iter().find(|project| project.id == id));
    TeamProject {
        id: row.id.clone(),
        team_id: row.team_id.clone(),
        name: row.name.clone(),
        goal: row.goal.clone(),
        brief: row.brief.clone(),
        created_at_ms: row.created_at,
        source_project: row.source_project_id.clone(),
        source_project_name: source.map(|project| project.name.clone()),
        source_kind: source.map(|project| project.kind),
        code_source: row.code_source.as_deref().map(code_source_of),
    }
}
