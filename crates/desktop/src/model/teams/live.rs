//! The Bot Teams a service read reports, as the Teams pages draw them: the teams of the connected service,
//! their Boards and the bots on them. The pages key on numbers; `TeamIds` gives each protocol id its number.

use massorca_protocol::methods::tasks::Column as WireColumn;

use super::state::TeamsState;
use super::types::{Bot, BotId, Column, HostChoice, Task, TaskId, Team, TeamId};
use crate::model::shell::HostReach;
use crate::service::{TeamIds, TeamRecord};

/// The teams a read named, on one host. A record or card whose id `ids` does not number is left out.
pub fn live_teams(ids: &TeamIds, host: &str, records: &[TeamRecord]) -> TeamsState {
    let hosts = vec![HostChoice {
        name: host.to_owned(),
        reach: HostReach::Connected,
    }];
    let teams = records
        .iter()
        .filter_map(|record| live_team(ids, host, record))
        .collect();
    TeamsState::connected(hosts, teams)
}

fn live_team(ids: &TeamIds, host: &str, record: &TeamRecord) -> Option<Team> {
    let mut bots: Vec<Bot> = Vec::new();
    let mut tasks = Vec::new();
    for board_column in &record.board.columns {
        // A column this build does not know has no page, so its cards are not drawn.
        let Some(column) = column_of(board_column.column) else {
            continue;
        };
        for card in &board_column.cards {
            let bot = card.bot_id.as_deref().and_then(|id| ids.get(id)).map(BotId);
            // The cards are the only place the protocol names a bot, so the bots are listed from them.
            if let (Some(id), Some(name)) = (bot, card.bot.as_ref())
                && !bots.iter().any(|known| known.id == id)
            {
                bots.push(Bot {
                    id,
                    name: name.clone(),
                    role: String::new(),
                    agent: None,
                });
            }
            if let Some(id) = ids.get(&card.id) {
                tasks.push(Task {
                    id: TaskId(id),
                    title: card.title.clone(),
                    column,
                    bot,
                    time: None,
                    reach: HostReach::Connected,
                    note: card.error.clone(),
                });
            }
        }
    }
    Some(Team {
        id: TeamId(ids.get(&record.team.id)?),
        name: record.team.name.clone(),
        purpose: record.team.purpose.clone(),
        host: host.to_owned(),
        reach: HostReach::Connected,
        bots,
        lead: None,
        workspaces: record.team.workspaces,
        tasks,
        projects: Vec::new(),
        routines: Vec::new(),
        addresses: Vec::new(),
        memory: Vec::new(),
        files: Vec::new(),
    })
}

/// The page's column for a protocol column.
fn column_of(column: WireColumn) -> Option<Column> {
    match column {
        WireColumn::Open => Some(Column::Open),
        WireColumn::Running => Some(Column::Running),
        WireColumn::Waiting => Some(Column::Waiting),
        WireColumn::Done => Some(Column::Done),
        WireColumn::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::tasks::CardState;
    use massorca_protocol::methods::teams::{
        Board, BoardCard, BoardColumn, Team as WireTeam, TeamAccess, TeamCharacter,
    };

    use super::*;

    fn card(id: &str, title: &str, bot: Option<(&str, &str)>, error: Option<&str>) -> BoardCard {
        BoardCard {
            id: id.into(),
            title: title.into(),
            bot_id: bot.map(|(id, _)| id.into()),
            bot: bot.map(|(_, name)| name.into()),
            state: CardState::Working,
            last_known: false,
            error: error.map(Into::into),
            at_ms: 1,
        }
    }

    fn record(team: &str, columns: Vec<(WireColumn, Vec<BoardCard>)>) -> TeamRecord {
        TeamRecord {
            team: WireTeam {
                id: team.into(),
                name: "Desarrollo".into(),
                purpose: "Ship".into(),
                character: TeamCharacter::Triangle,
                access: TeamAccess::Full,
                workspaces: true,
                created_at_ms: 1,
            },
            board: Board {
                team_id: team.into(),
                project_id: None,
                columns: columns
                    .into_iter()
                    .map(|(column, cards)| BoardColumn { column, cards })
                    .collect(),
            },
        }
    }

    fn numbered(records: &[TeamRecord]) -> TeamIds {
        let mut ids = TeamIds::default();
        for record in records {
            ids.register(record);
        }
        ids
    }

    #[test]
    fn the_cards_of_each_column_become_its_tasks_and_their_bots_are_named() {
        let records = [record(
            "t1",
            vec![
                (
                    WireColumn::Open,
                    vec![card("k1", "Write the spec", None, None)],
                ),
                (
                    WireColumn::Done,
                    vec![card(
                        "k2",
                        "Ship it",
                        Some(("b1", "Mara")),
                        Some("Push was rejected"),
                    )],
                ),
                (WireColumn::Unknown, vec![card("k3", "Hidden", None, None)]),
            ],
        )];
        let view = live_teams(&numbered(&records), "This PC", &records);
        let team = &view.teams()[0];
        let tasks: Vec<(&str, Column)> = team
            .tasks
            .iter()
            .map(|task| (task.title.as_str(), task.column))
            .collect();
        assert_eq!(
            tasks,
            [("Write the spec", Column::Open), ("Ship it", Column::Done)]
        );
        let done = &team.tasks[1];
        assert_eq!(done.note.as_deref(), Some("Push was rejected"));
        let bot = done.bot.expect("the card names its bot");
        assert_eq!(team.bot(bot).map(|bot| bot.name.as_str()), Some("Mara"));
        assert_eq!(team.bots.len(), 1);
        assert_eq!(
            team.bots[0].agent, None,
            "a card does not say which agent runs the bot"
        );
        assert_eq!(team.lead_name(), None);
    }

    #[test]
    fn a_team_takes_its_host_and_settings_from_the_read() {
        let records = [record("t1", Vec::new())];
        let view = live_teams(&numbered(&records), "This PC", &records);
        let team = &view.teams()[0];
        assert_eq!(
            (
                team.name.as_str(),
                team.purpose.as_str(),
                team.host.as_str()
            ),
            ("Desarrollo", "Ship", "This PC")
        );
        assert!(team.workspaces);
        assert_eq!(team.reach, HostReach::Connected);
        assert_eq!(view.hosts()[0].name, "This PC");
        assert!(view.is_connected());
    }
}
