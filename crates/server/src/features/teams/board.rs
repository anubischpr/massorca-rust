//! `team.board`: a team's Board, its tasks in four columns, each card in its column (capability `teams/1`, decision 0072).

use massorca_core_agents::Agents;
use massorca_core_store::Store;
use massorca_protocol::methods::tasks::Column;
use massorca_protocol::methods::teams::{self, Board, BoardCard, BoardColumn, BoardParams};
use massorca_protocol::{ErrorBody, capabilities};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::task_state;
use super::{blocking, selectors, store_error};

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![registries.methods.register(
        Owner::Core,
        teams::BOARD,
        capabilities::TEAMS,
        Permission::Read,
        board,
    )?])
}

async fn board(ctx: RequestCtx, params: BoardParams) -> Result<Board, ErrorBody> {
    blocking(&ctx, move |store, agents| board_of(store, agents, params)).await
}

/// The Board of one team, or of one of its projects: every column, in order, with its cards oldest first.
fn board_of(store: &Store, agents: &Agents, params: BoardParams) -> Result<Board, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let project = params
        .project
        .as_deref()
        .map(|selector| selectors::team_project(store, &team, selector))
        .transpose()?;
    let bots = store.list_bots(&team.id).map_err(store_error)?;
    let mut columns: Vec<BoardColumn> = Column::ALL
        .into_iter()
        .map(|column| BoardColumn {
            column,
            cards: Vec::new(),
        })
        .collect();
    for row in store.list_tasks(Some(&team.id)).map_err(store_error)? {
        if project
            .as_ref()
            .is_some_and(|project| row.project_id.as_deref() != Some(project.id.as_str()))
        {
            continue;
        }
        let standing = task_state::standing(&row, &task_state::load(agents, &row)?);
        let card = BoardCard {
            id: row.id.clone(),
            title: row.title.clone(),
            bot: row
                .bot_id
                .as_ref()
                .and_then(|id| bots.iter().find(|bot| &bot.id == id))
                .map(|bot| bot.name.clone()),
            bot_id: row.bot_id.clone(),
            state: standing.state,
            last_known: standing.last_known,
            error: row.error.clone(),
            error_code: row.error_code.clone(),
            workspace_id: row.workspace_id.clone(),
            branch: row.branch.clone(),
            at_ms: row.updated_at,
        };
        if let Some(column) = columns
            .iter_mut()
            .find(|column| column.column == standing.column)
        {
            column.cards.push(card);
        }
    }
    Ok(Board {
        team_id: team.id.clone(),
        project_id: project.map(|project| project.id),
        columns,
    })
}
