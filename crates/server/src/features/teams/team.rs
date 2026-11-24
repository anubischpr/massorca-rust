//! `team.create`, `team.list`, `team.show`, `team.update` and `team.rm` (capability `teams/1`, decision 0072).

use massorca_core_agents::Agents;
use massorca_core_store::{Store, TeamRow, new_id, now_ms};
use massorca_protocol::methods::events::{TOPIC_TASK_CHANGED, TOPIC_TEAM_CHANGED};
use massorca_protocol::methods::teams::{
    self, Team, TeamAccess, TeamCharacter, TeamCreateParams, TeamList, TeamRef, TeamRemoveParams,
    TeamUpdateParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::features::agent_redaction::is_host_admin;
use crate::features::workspace_domain::publish;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::task;
use super::{blocking, invalid, non_blank, optional_text, selectors, store_error};

/// The six shapes a team can take, with the name each one has in the store.
const CHARACTERS: [(TeamCharacter, &str); 6] = [
    (TeamCharacter::Triangle, "triangle"),
    (TeamCharacter::Square, "square"),
    (TeamCharacter::Circle, "circle"),
    (TeamCharacter::Diamond, "diamond"),
    (TeamCharacter::Hexagon, "hexagon"),
    (TeamCharacter::Star, "star"),
];

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        methods.register(
            Owner::Core,
            teams::CREATE,
            capabilities::TEAMS,
            Permission::Workspace,
            create,
        )?,
        methods.register(
            Owner::Core,
            teams::LIST,
            capabilities::TEAMS,
            Permission::Read,
            list,
        )?,
        methods.register(
            Owner::Core,
            teams::SHOW,
            capabilities::TEAMS,
            Permission::Read,
            show,
        )?,
        methods.register(
            Owner::Core,
            teams::UPDATE,
            capabilities::TEAMS,
            Permission::Workspace,
            update,
        )?,
        methods.register(
            Owner::Core,
            teams::REMOVE,
            capabilities::TEAMS,
            Permission::Workspace,
            remove,
        )?,
    ])
}

async fn create(ctx: RequestCtx, mut params: TeamCreateParams) -> Result<Team, ErrorBody> {
    // Only a host-admin caller makes a full-access team; another caller's team defaults to `default`.
    params.access = Some(match params.access {
        None if !is_host_admin(&ctx) => TeamAccess::Default,
        access => checked_access(&ctx, access.unwrap_or_default())?,
    });
    let team = blocking(&ctx, move |store, _agents| create_team(store, params)).await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &team.id);
    Ok(team)
}

async fn list(ctx: RequestCtx, _params: NoParams) -> Result<TeamList, ErrorBody> {
    let teams = blocking(&ctx, |store, _agents| {
        let rows = store.list_teams().map_err(store_error)?;
        Ok(rows.iter().map(team_view).collect())
    })
    .await?;
    Ok(TeamList { teams })
}

async fn show(ctx: RequestCtx, params: TeamRef) -> Result<Team, ErrorBody> {
    blocking(&ctx, move |store, _agents| {
        Ok(team_view(&selectors::team(store, &params.team)?))
    })
    .await
}

async fn update(ctx: RequestCtx, params: TeamUpdateParams) -> Result<Team, ErrorBody> {
    if let Some(access) = params.access {
        checked_access(&ctx, access)?;
    }
    let team = blocking(&ctx, move |store, _agents| update_team(store, params)).await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &team.id);
    Ok(team)
}

/// Removes the team with its bots, projects and tasks. Each removed task is published as changed.
async fn remove(ctx: RequestCtx, params: TeamRemoveParams) -> Result<Team, ErrorBody> {
    let (team, tasks) = blocking(&ctx, move |store, agents| {
        remove_team(store, agents, params)
    })
    .await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &team.id);
    for id in &tasks {
        publish(&ctx, TOPIC_TASK_CHANGED, id);
    }
    Ok(team)
}

fn create_team(store: &Store, params: TeamCreateParams) -> Result<Team, ErrorBody> {
    let row = TeamRow {
        id: new_id(),
        name: non_blank(&params.name, "the team name")?,
        purpose: optional_text(params.purpose).unwrap_or_default(),
        character: character_name(params.character.unwrap_or_default())?.to_owned(),
        access: access_name(params.access.unwrap_or_default())?.to_owned(),
        workspaces: params.workspaces.unwrap_or(false),
        created_at: now_ms(),
    };
    store.insert_team(&row).map_err(store_error)?;
    Ok(team_view(&row))
}

fn update_team(store: &Store, params: TeamUpdateParams) -> Result<Team, ErrorBody> {
    let mut row = selectors::team(store, &params.team)?;
    if let Some(name) = params.name {
        row.name = non_blank(&name, "the team name")?;
    }
    if let Some(purpose) = params.purpose {
        row.purpose = purpose.trim().to_owned();
    }
    if let Some(character) = params.character {
        row.character = character_name(character)?.to_owned();
    }
    if let Some(access) = params.access {
        row.access = access_name(access)?.to_owned();
    }
    if let Some(workspaces) = params.workspaces {
        row.workspaces = workspaces;
    }
    if !store.update_team(&row).map_err(store_error)? {
        return Err(removed_meanwhile("team"));
    }
    Ok(team_view(&row))
}

/// Refused while a task of the team runs, unless `force` is given. Returns the team and the ids of its removed tasks.
fn remove_team(
    store: &Store,
    agents: &Agents,
    params: TeamRemoveParams,
) -> Result<(Team, Vec<String>), ErrorBody> {
    let row = selectors::team(store, &params.team)?;
    let tasks = store.list_tasks(Some(&row.id)).map_err(store_error)?;
    if !params.force && task::any_running(agents, &tasks)? {
        return Err(ErrorBody::new(
            codes::TASK_RUNNING,
            format!(
                "team {} has a running task; stop it or remove the team with force",
                row.name
            ),
        ));
    }
    if !store.delete_team(&row.id).map_err(store_error)? {
        return Err(removed_meanwhile("team"));
    }
    let removed_tasks = tasks.into_iter().map(|task| task.id).collect();
    Ok((team_view(&row), removed_tasks))
}

fn removed_meanwhile(what: &str) -> ErrorBody {
    ErrorBody::new(
        codes::INTERNAL,
        format!("the {what} was removed by another call"),
    )
}

/// The store name of a shape. A shape from a newer peer is refused.
fn character_name(character: TeamCharacter) -> Result<&'static str, ErrorBody> {
    CHARACTERS
        .iter()
        .find(|(known, _)| *known == character)
        .map(|(_, name)| *name)
        .ok_or_else(|| invalid("the team character is not one of the six shapes"))
}

fn character_of(name: &str) -> TeamCharacter {
    CHARACTERS
        .iter()
        .find(|(_, known)| *known == name)
        .map_or(TeamCharacter::Unknown, |(character, _)| *character)
}

/// Refuses full access to a caller below host-admin scope: a full-access team's bots skip permission prompts, and runs
/// of routines a host-admin set up also hold vault secrets (decisions 0071 and 0083).
fn checked_access(ctx: &RequestCtx, access: TeamAccess) -> Result<TeamAccess, ErrorBody> {
    if access == TeamAccess::Full && !is_host_admin(ctx) {
        return Err(ErrorBody::new(
            codes::PERMISSION_DENIED,
            "full team access needs a host-admin caller, because the team's bots then run without asking for permission; use --access default, or ask the host's owner",
        ));
    }
    Ok(access)
}

/// The store name of an access mode.
fn access_name(access: TeamAccess) -> Result<&'static str, ErrorBody> {
    match access {
        TeamAccess::Full => Ok(ACCESS_FULL),
        TeamAccess::Default => Ok("default"),
        TeamAccess::Unknown => Err(invalid("the team access is not a known mode")),
    }
}

/// The store name of full access: bots run in their kind's bypass mode.
pub(super) const ACCESS_FULL: &str = "full";

fn access_of(name: &str) -> TeamAccess {
    match name {
        ACCESS_FULL => TeamAccess::Full,
        "default" => TeamAccess::Default,
        _ => TeamAccess::Unknown,
    }
}

fn team_view(row: &TeamRow) -> Team {
    Team {
        id: row.id.clone(),
        name: row.name.clone(),
        purpose: row.purpose.clone(),
        character: character_of(&row.character),
        access: access_of(&row.access),
        workspaces: row.workspaces,
        created_at_ms: row.created_at,
    }
}
