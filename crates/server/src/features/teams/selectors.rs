//! Resolves the selectors of Bot Teams calls: an id first, then an exact name within the scope the call names.

use massorca_core_store::{BotRow, Store, TeamProjectRow, TeamRow};
use massorca_protocol::{ErrorBody, codes};

use super::store_error;

/// The team `selector` names: its id, then its exact name.
pub(crate) fn team(store: &Store, selector: &str) -> Result<TeamRow, ErrorBody> {
    if let Some(row) = store.get_team(selector).map_err(store_error)? {
        return Ok(row);
    }
    store
        .find_team_by_name(selector)
        .map_err(store_error)?
        .ok_or_else(|| ErrorBody::new(codes::TEAM_NOT_FOUND, format!("no team named {selector:?}")))
}

/// The bot `selector` names within `team`: its id, then its exact name.
pub(crate) fn bot(store: &Store, team: &TeamRow, selector: &str) -> Result<BotRow, ErrorBody> {
    if let Some(row) = store.get_bot(selector).map_err(store_error)?
        && row.team_id == team.id
    {
        return Ok(row);
    }
    store
        .find_bot_by_name(&team.id, selector)
        .map_err(store_error)?
        .ok_or_else(|| {
            ErrorBody::new(
                codes::BOT_NOT_FOUND,
                format!("team {} has no bot {selector:?}", team.name),
            )
        })
}

/// The project `selector` names within `team`: its id, then its exact name.
pub(crate) fn team_project(
    store: &Store,
    team: &TeamRow,
    selector: &str,
) -> Result<TeamProjectRow, ErrorBody> {
    if let Some(row) = store.get_team_project(selector).map_err(store_error)?
        && row.team_id == team.id
    {
        return Ok(row);
    }
    store
        .find_team_project_by_name(&team.id, selector)
        .map_err(store_error)?
        .ok_or_else(|| {
            ErrorBody::new(
                codes::TEAM_PROJECT_NOT_FOUND,
                format!("team {} has no project {selector:?}", team.name),
            )
        })
}
