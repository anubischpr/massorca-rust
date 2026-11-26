//! The Conference room's typing line (decision 0122): the bots working on an open reply or a room task now.

use massorca_core_agents::Agents;
use massorca_core_store::{BotRow, Store, TaskOrigin, TeamRow};
use massorca_protocol::ErrorBody;
use massorca_protocol::methods::agents::ActivityState;
use massorca_protocol::methods::tasks::CardState;
use massorca_protocol::methods::team_room::RoomTyping;

use super::store_error;
use super::task_state::{self, Session};

/// The bots working on an open reply or a room task now, once each, in team order.
pub(super) fn typing(
    store: &Store,
    agents: &Agents,
    team: &TeamRow,
    bots: &[BotRow],
) -> Result<Vec<RoomTyping>, ErrorBody> {
    let mut working: Vec<String> = Vec::new();
    for line in store
        .open_team_room_replies(&team.id)
        .map_err(store_error)?
    {
        let (Some(bot), Some(session)) = (line.reply_bot_id, line.reply_session.as_deref()) else {
            continue;
        };
        let session = task_state::load_id(agents, session)?;
        if task_state::is_running(&session)
            && let Session::Known(shown) = &session
            && shown
                .activity
                .as_ref()
                .is_none_or(|activity| activity.state != ActivityState::Done)
        {
            working.push(bot);
        }
    }
    for task in store.list_tasks(Some(&team.id)).map_err(store_error)? {
        if task.origin != TaskOrigin::Room || task.session_id.is_none() || task.outcome.is_some() {
            continue;
        }
        let shown = task_state::load(agents, &task)?;
        if task_state::standing(&task, &shown).state == CardState::Working
            && let Some(bot) = task.bot_id
        {
            working.push(bot);
        }
    }
    Ok(bots
        .iter()
        .filter(|bot| working.contains(&bot.id))
        .map(|bot| RoomTyping {
            bot_id: bot.id.clone(),
            bot_name: bot.name.clone(),
        })
        .collect())
}
