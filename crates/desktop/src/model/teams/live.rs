//! The Bot Teams a service read reports, as the Teams pages draw them: the teams of the connected service,
//! their bots and Boards. The pages key on numbers; `TeamIds` gives each protocol id its number.

use massorca_protocol::methods::agents::AgentKind;
use massorca_protocol::methods::tasks::Column as WireColumn;
use massorca_protocol::methods::teams::{Board, Bot as WireBot};

use super::state::TeamsState;
use super::types::{
    Bot, BotId, Column, HostChoice, Project, ProjectId, Task, TaskId, Team, TeamId,
};
use crate::model::agent_kinds::kind_unavailable_reason;
use crate::model::shell::{Agent, HostReach};
use crate::service::{ProjectRecord, TeamIds, TeamRecord};

/// The teams a read named, on one host. A record or card whose id `ids` does not number is left out. `kinds`
/// are the service's agent kinds, which say whether a bot's kind runs here.
pub fn live_teams(
    ids: &TeamIds,
    host: &str,
    records: &[TeamRecord],
    kinds: &[AgentKind],
) -> TeamsState {
    let hosts = vec![HostChoice {
        name: host.to_owned(),
        reach: HostReach::Connected,
    }];
    let teams = records
        .iter()
        .filter_map(|record| live_team(ids, host, record, kinds))
        .collect();
    TeamsState::connected(hosts, teams)
}

fn live_team(ids: &TeamIds, host: &str, record: &TeamRecord, kinds: &[AgentKind]) -> Option<Team> {
    let bots: Vec<Bot> = record
        .bots
        .iter()
        .filter_map(|bot| live_bot(ids, bot, kinds))
        .collect();
    let lead = record
        .bots
        .iter()
        .find(|bot| bot.lead)
        .and_then(|bot| ids.get(&bot.id))
        .map(BotId);
    Some(Team {
        id: TeamId(ids.get(&record.team.id)?),
        name: record.team.name.clone(),
        character: record.team.character,
        purpose: record.team.purpose.clone(),
        host: host.to_owned(),
        reach: HostReach::Connected,
        bots,
        lead,
        access: record.team.access,
        workspaces: record.team.workspaces,
        tasks: live_tasks(ids, &record.board),
        projects: record
            .projects
            .iter()
            .filter_map(|project| live_project(ids, project))
            .collect(),
        routines: Vec::new(),
        addresses: Vec::new(),
        memory: Vec::new(),
        files: Vec::new(),
    })
}

/// A bot of a team's roster as the page draws it; `None` when the service's id for it is not numbered.
fn live_bot(ids: &TeamIds, bot: &WireBot, kinds: &[AgentKind]) -> Option<Bot> {
    Some(Bot {
        id: BotId(ids.get(&bot.id)?),
        name: bot.name.clone(),
        role: bot.job.clone(),
        agent: agent_of_kind(&bot.kind),
        kind: bot.kind.clone(),
        model: bot.model.clone(),
        effort: bot.effort.clone(),
        kind_unavailable: kind_unavailable_reason(kinds, &bot.kind),
    })
}

/// A project of a team as the page draws it, with the cards of its own Board; `None` when its id is not numbered.
fn live_project(ids: &TeamIds, record: &ProjectRecord) -> Option<Project> {
    Some(Project {
        id: ProjectId(ids.get(&record.project.id)?),
        name: record.project.name.clone(),
        brief: record.project.brief.clone(),
        link: None,
        link_saved: false,
        tasks: live_tasks(ids, &record.board),
    })
}

/// The cards of a Board as the page's tasks, in column order.
fn live_tasks(ids: &TeamIds, board: &Board) -> Vec<Task> {
    let mut tasks = Vec::new();
    for board_column in &board.columns {
        // A column this build does not know has no page, so its cards are not drawn.
        let Some(column) = column_of(board_column.column) else {
            continue;
        };
        for card in &board_column.cards {
            if let Some(id) = ids.get(&card.id) {
                tasks.push(Task {
                    id: TaskId(id),
                    title: card.title.clone(),
                    column,
                    bot: card
                        .bot_id
                        .as_deref()
                        .and_then(|bot| ids.get(bot))
                        .map(BotId),
                    time: None,
                    reach: HostReach::Connected,
                    note: card.error.clone(),
                });
            }
        }
    }
    tasks
}

/// The agent a bot's kind names, when this build knows it.
pub fn agent_of_kind(kind: &str) -> Option<Agent> {
    let agent = crate::model::agent_sessions::agent_of_kind(kind);
    (agent != Agent::Unknown).then_some(agent)
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
        TeamProject as WireProject,
    };

    use super::*;
    use crate::model::agent_kinds::{NOT_AVAILABLE_ON_HOST, TURNED_OFF};
    use crate::model::teams::BoardOwner;
    use crate::service::ProjectRecord;

    #[test]
    fn a_bot_kind_names_the_same_agent_as_a_session_kind() {
        for kind in [
            "claude",
            "codex",
            "antigravity",
            "gentle-shell",
            "gemini",
            "opencode",
            "cursor",
            "my-own-kind",
        ] {
            let session = crate::model::agent_sessions::agent_of_kind(kind);
            let expected = (session != Agent::Unknown).then_some(session);
            assert_eq!(agent_of_kind(kind), expected, "{kind}");
        }
    }
    use crate::service::fixtures::agent_kind;
    use massorca_protocol::methods::agents::PackageState;

    fn card(id: &str, title: &str, bot: Option<(&str, &str)>, error: Option<&str>) -> BoardCard {
        BoardCard {
            id: id.into(),
            title: title.into(),
            bot_id: bot.map(|(id, _)| id.into()),
            bot: bot.map(|(_, name)| name.into()),
            state: CardState::Working,
            last_known: false,
            error: error.map(Into::into),
            error_code: None,
            workspace_id: None,
            branch: None,
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
            bots: Vec::new(),
            projects: Vec::new(),
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
    fn a_team_carries_the_access_its_wire_record_names() {
        let mut records = [record("t1", Vec::new())];
        records[0].team.access = TeamAccess::Default;
        let view = live_teams(&numbered(&records), "This PC", &records, &[]);
        assert_eq!(view.teams()[0].access, TeamAccess::Default);
    }

    #[test]
    fn the_cards_of_each_column_become_its_tasks_and_name_their_bots() {
        let mut records = [record(
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
        records[0].bots = vec![wire_bot("b1", "Mara", "claude")];
        let view = live_teams(&numbered(&records), "This PC", &records, &[]);
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
        assert_eq!(team.lead_name(), None);
    }

    #[test]
    fn a_team_takes_its_host_and_settings_from_the_read() {
        let records = [record("t1", Vec::new())];
        let view = live_teams(&numbered(&records), "This PC", &records, &[]);
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

    #[test]
    fn a_bot_of_the_roster_without_a_card_is_listed_with_its_job_agent_and_lead() {
        let mut records = [record("t1", Vec::new())];
        records[0].bots = vec![WireBot {
            lead: true,
            ..bot_of_kind("b9", "codex")
        }];
        let view = live_teams(&numbered(&records), "This PC", &records, &[]);
        let team = &view.teams()[0];
        assert!(team.tasks.is_empty());
        assert_eq!(team.lead_name(), Some("Nora"));
        let bot = &team.bots[0];
        assert_eq!(
            (bot.name.as_str(), bot.role.as_str(), bot.agent),
            ("Nora", "Reviews", Some(Agent::Codex))
        );
    }

    #[test]
    fn a_bot_on_a_kind_the_host_does_not_detect_is_marked_unavailable() {
        let mut records = [record("t1", Vec::new())];
        records[0].bots = vec![bot_of_kind("b9", "codex")];
        let mut codex = agent_kind("codex", "Codex", PackageState::Enabled);
        codex.detected = false;
        let view = live_teams(&numbered(&records), "This PC", &records, &[codex]);
        assert_eq!(
            view.teams()[0].bots[0].kind_unavailable,
            Some(NOT_AVAILABLE_ON_HOST)
        );
    }

    #[test]
    fn a_bot_on_a_detected_kind_is_available_and_so_is_one_the_store_does_not_know() {
        let mut records = [record("t1", Vec::new())];
        records[0].bots = vec![bot_of_kind("b9", "claude"), bot_of_kind("b8", "gemini")];
        let claude = agent_kind("claude", "Claude", PackageState::Enabled);
        let view = live_teams(&numbered(&records), "This PC", &records, &[claude]);
        let bots = &view.teams()[0].bots;
        assert!(bots.iter().all(|bot| bot.kind_unavailable.is_none()));
    }

    #[test]
    fn a_bot_and_card_on_a_turned_off_kind_carry_turned_off() {
        let mut records = [record(
            "t1",
            vec![(
                WireColumn::Running,
                vec![card("k1", "Review", Some(("b9", "Nora")), None)],
            )],
        )];
        records[0].bots = vec![bot_of_kind("b9", "codex")];
        let codex = agent_kind("codex", "Codex", PackageState::Disabled);
        let view = live_teams(&numbered(&records), "This PC", &records, &[codex]);
        assert_eq!(view.teams()[0].bots[0].kind_unavailable, Some(TURNED_OFF));
        let board = view
            .board_view(BoardOwner::Team(view.teams()[0].id))
            .expect("the team has a board");
        let running = board
            .columns
            .iter()
            .find(|column| column.column == Column::Running)
            .expect("the running column");
        assert_eq!(running.cards[0].kind_unavailable, Some(TURNED_OFF));
    }

    #[test]
    fn a_card_on_a_bot_whose_kind_is_unavailable_carries_the_mark() {
        let mut records = [record(
            "t1",
            vec![(
                WireColumn::Running,
                vec![card("k1", "Review", Some(("b9", "Nora")), None)],
            )],
        )];
        records[0].bots = vec![bot_of_kind("b9", "codex")];
        let mut codex = agent_kind("codex", "Codex", PackageState::Enabled);
        codex.detected = false;
        let view = live_teams(&numbered(&records), "This PC", &records, &[codex]);
        let board = view
            .board_view(BoardOwner::Team(view.teams()[0].id))
            .expect("the team has a board");
        let running = board
            .columns
            .iter()
            .find(|column| column.column == Column::Running)
            .expect("the running column");
        assert_eq!(
            running.cards[0].kind_unavailable,
            Some(NOT_AVAILABLE_ON_HOST)
        );
    }

    #[test]
    fn a_project_of_the_team_lists_its_brief_and_its_own_board() {
        let mut records = [record("t1", Vec::new())];
        records[0].projects = vec![ProjectRecord {
            project: WireProject {
                id: "p1".into(),
                team_id: "t1".into(),
                name: "Checkout".into(),
                goal: "Ship checkout".into(),
                brief: "Checkout copy".into(),
                created_at_ms: 1,
                source_project: None,
                source_project_name: None,
                source_kind: None,
                code_source: None,
            },
            board: Board {
                team_id: "t1".into(),
                project_id: Some("p1".into()),
                columns: vec![BoardColumn {
                    column: WireColumn::Open,
                    cards: vec![card("k7", "Draft the page", None, None)],
                }],
            },
        }];
        let view = live_teams(&numbered(&records), "This PC", &records, &[]);
        let projects = &view.teams()[0].projects;
        assert_eq!(projects.len(), 1);
        let project = &projects[0];
        assert_eq!(
            (project.name.as_str(), project.brief.as_str()),
            ("Checkout", "Checkout copy")
        );
        let titles: Vec<&str> = project
            .tasks
            .iter()
            .map(|task| task.title.as_str())
            .collect();
        assert_eq!(titles, ["Draft the page"]);
    }

    #[test]
    fn a_project_read_from_the_service_keeps_no_link() {
        let mut records = [record("t1", Vec::new())];
        records[0].projects = vec![ProjectRecord {
            project: WireProject {
                id: "p1".into(),
                team_id: "t1".into(),
                name: "Checkout".into(),
                goal: String::new(),
                brief: String::new(),
                created_at_ms: 1,
                source_project: None,
                source_project_name: None,
                source_kind: None,
                code_source: None,
            },
            board: Board {
                team_id: "t1".into(),
                project_id: Some("p1".into()),
                columns: Vec::new(),
            },
        }];
        let view = live_teams(&numbered(&records), "This PC", &records, &[]);
        let project = &view.teams()[0].projects[0];
        assert!(!project.link_saved);
        assert_eq!(project.link, None);
    }

    fn wire_bot(id: &str, name: &str, kind: &str) -> WireBot {
        WireBot {
            id: id.into(),
            team_id: "t1".into(),
            name: name.into(),
            job: "Reviews".into(),
            kind: kind.into(),
            model: None,
            effort: None,
            lead: false,
            created_at_ms: 1,
        }
    }

    fn bot_of_kind(id: &str, kind: &str) -> WireBot {
        wire_bot(id, "Nora", kind)
    }
}
