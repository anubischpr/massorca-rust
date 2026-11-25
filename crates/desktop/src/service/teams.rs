//! The Bot Teams reads the bridge runs (capability `teams/1`): every team the service lists, each with its
//! Board and bots. A read takes a number when it starts, and the store applies only the newest answer, so a
//! slow read never replaces a newer one.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use massorca_client::{Client, ClientError};
use massorca_protocol::codes;
use massorca_protocol::methods::tasks::{Task, TaskRef};
use massorca_protocol::methods::team_room::{TeamRoomPostParams, TeamRoomPosted};
use massorca_protocol::methods::teams::{
    Board, BoardParams, Bot, BotAddParams, BotRemoveParams, BotUpdateParams, Team, TeamAccess,
    TeamCreateParams, TeamProject, TeamProjectAddParams, TeamProjectRemoveParams,
    TeamProjectUpdateParams, TeamRef, TeamRemoveParams, TeamUpdateParams,
};
use tokio::sync::{mpsc, oneshot};

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::team_room::RoomCommand;
use super::update::StoreUpdate;

/// A team as the service reports it: its Board, every task of the team in four columns, its bots in creation
/// order, and its projects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamRecord {
    pub team: Team,
    pub board: Board,
    pub bots: Vec<Bot>,
    pub projects: Vec<ProjectRecord>,
}

/// A project of a team as the service reports it, with its own Board: the cards of that project only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRecord {
    pub project: TeamProject,
    pub board: Board,
}

/// What one read answered: every team with its Board, or why the service could not list them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamsRead {
    /// Orders the reads: the store keeps the answer with the highest number.
    pub number: u64,
    pub teams: Result<Vec<TeamRecord>, Refusal>,
}

/// Numbers the reads of one bridge. The bridge keeps one for its whole life, so the numbers stay ordered
/// across reconnects.
#[derive(Debug, Default)]
pub(crate) struct Sequence {
    last: AtomicU64,
}

impl Sequence {
    pub(super) fn next(&self) -> u64 {
        self.last.fetch_add(1, Ordering::SeqCst) + 1
    }
}

/// Reads every team with its Board. A refusal from the service is an answer; a lost link is an error, so the
/// bridge reconnects instead of reporting the teams as absent.
pub(super) async fn read(client: &Client, sequence: &Sequence) -> Result<TeamsRead, ClientError> {
    let number = sequence.next();
    match fetch(client).await {
        Ok(records) => Ok(TeamsRead {
            number,
            teams: Ok(records),
        }),
        Err(error @ (ClientError::Remote(_) | ClientError::Protocol(_))) => Ok(TeamsRead {
            number,
            teams: Err(Refusal::from(&error)),
        }),
        Err(error) => Err(error),
    }
}

async fn fetch(client: &Client) -> Result<Vec<TeamRecord>, ClientError> {
    let mut records = Vec::new();
    for team in client.team_list().await?.teams {
        match team_record(client, team).await {
            Ok(record) => records.push(record),
            // Removed while the list was read: it is simply not in this read.
            Err(ClientError::Remote(body)) if body.code == codes::TEAM_NOT_FOUND => {}
            Err(error) => return Err(error),
        }
    }
    Ok(records)
}

async fn team_record(client: &Client, team: Team) -> Result<TeamRecord, ClientError> {
    let team_ref = TeamRef {
        team: team.id.clone(),
    };
    let board = client
        .team_board(&BoardParams {
            team: team.id.clone(),
            project: None,
        })
        .await?;
    let bots = client.team_bot_list(&team_ref).await?.bots;
    let mut projects = Vec::new();
    for project in client.team_project_list(&team_ref).await?.projects {
        let params = BoardParams {
            team: team.id.clone(),
            project: Some(project.id.clone()),
        };
        match client.team_board(&params).await {
            Ok(board) => projects.push(ProjectRecord { project, board }),
            // Removed while the team was read: it is simply not in this read.
            Err(ClientError::Remote(body)) if body.code == codes::TEAM_PROJECT_NOT_FOUND => {}
            Err(error) => return Err(error),
        }
    }
    Ok(TeamRecord {
        team,
        board,
        bots,
        projects,
    })
}

/// A Bot Teams write the pages ask the bridge to send. Each one answers through its oneshot.
pub(crate) enum TeamCommand {
    CreateTeam {
        params: TeamCreateParams,
        reply: oneshot::Sender<Result<Team, Refusal>>,
    },
    AddBot {
        params: BotAddParams,
        reply: oneshot::Sender<Result<Bot, Refusal>>,
    },
    TaskDone {
        task: String,
        reply: oneshot::Sender<Result<Task, Refusal>>,
    },
    TaskReopen {
        task: String,
        reply: oneshot::Sender<Result<Task, Refusal>>,
    },
    SaveBrief {
        team: String,
        project: String,
        brief: String,
        reply: oneshot::Sender<Result<TeamProject, Refusal>>,
    },
    DeleteProject {
        team: String,
        project: String,
        reply: oneshot::Sender<Result<TeamProject, Refusal>>,
    },
    SetAccess {
        team: String,
        access: TeamAccess,
        reply: oneshot::Sender<Result<Team, Refusal>>,
    },
    UpdateTeam {
        params: TeamUpdateParams,
        reply: oneshot::Sender<Result<Team, Refusal>>,
    },
    RemoveTeam {
        team: String,
        reply: oneshot::Sender<Result<Team, Refusal>>,
    },
    AddProject {
        params: TeamProjectAddParams,
        reply: oneshot::Sender<Result<TeamProject, Refusal>>,
    },
    UpdateBot {
        params: BotUpdateParams,
        reply: oneshot::Sender<Result<Bot, Refusal>>,
    },
    RemoveBot {
        params: BotRemoveParams,
        reply: oneshot::Sender<Result<Bot, Refusal>>,
    },
}

/// Runs one write on its own task. The answer goes first, then the teams are read again, so the pages follow
/// the service and not the call.
pub(super) fn spawn_call(
    client: &Client,
    command: TeamCommand,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    sequence: &Arc<Sequence>,
) {
    match command {
        TeamCommand::CreateTeam { params, reply } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client.team_create(&params).await
            });
        }
        TeamCommand::TaskDone { task, reply } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client.task_done(&TaskRef { task }).await
            });
        }
        TeamCommand::TaskReopen { task, reply } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client.task_reopen(&TaskRef { task }).await
            });
        }
        TeamCommand::SaveBrief {
            team,
            project,
            brief,
            reply,
        } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client
                    .team_project_update(&TeamProjectUpdateParams {
                        team,
                        project,
                        name: None,
                        goal: None,
                        brief: Some(brief),
                        source_project: None,
                        code_source: None,
                    })
                    .await
            });
        }
        TeamCommand::DeleteProject {
            team,
            project,
            reply,
        } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client
                    .team_project_remove(&TeamProjectRemoveParams { team, project })
                    .await
            });
        }
        TeamCommand::SetAccess {
            team,
            access,
            reply,
        } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client
                    .team_update(&TeamUpdateParams {
                        team,
                        name: None,
                        purpose: None,
                        character: None,
                        access: Some(access),
                        workspaces: None,
                    })
                    .await
            });
        }
        TeamCommand::UpdateTeam { params, reply } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client.team_update(&params).await
            });
        }
        TeamCommand::RemoveTeam { team, reply } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client
                    .team_remove(&TeamRemoveParams { team, force: false })
                    .await
            });
        }
        TeamCommand::AddBot { params, reply } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client.team_bot_add(&params).await
            });
        }
        TeamCommand::AddProject { params, reply } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client.team_project_add(&params).await
            });
        }
        TeamCommand::UpdateBot { params, reply } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client.team_bot_update(&params).await
            });
        }
        TeamCommand::RemoveBot { params, reply } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client.team_bot_remove(&params).await
            });
        }
    }
}

/// Answers a write with `refusal`, while there is no connection to ask.
pub(super) fn refuse(command: TeamCommand, refusal: Refusal) {
    match command {
        TeamCommand::CreateTeam { reply, .. }
        | TeamCommand::SetAccess { reply, .. }
        | TeamCommand::UpdateTeam { reply, .. }
        | TeamCommand::RemoveTeam { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        TeamCommand::AddBot { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        TeamCommand::TaskDone { reply, .. } | TeamCommand::TaskReopen { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        TeamCommand::SaveBrief { reply, .. } | TeamCommand::DeleteProject { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        TeamCommand::AddProject { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        TeamCommand::UpdateBot { reply, .. } | TeamCommand::RemoveBot { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
    }
}

fn spawn_write<T, Fut>(
    client: &Client,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    sequence: &Arc<Sequence>,
    reply: oneshot::Sender<Result<T, Refusal>>,
    call: impl FnOnce(Client) -> Fut + Send + 'static,
) where
    T: Send + 'static,
    Fut: Future<Output = Result<T, ClientError>> + Send + 'static,
{
    let client = client.clone();
    let updates = updates.clone();
    let sequence = sequence.clone();
    tokio::spawn(async move {
        let answer = call(client.clone()).await;
        let _ = reply.send(answer.map_err(|error| teams_refusal(&error)));
        refresh(&client, &updates, &sequence).await;
    });
}

/// Reads the teams again and sends the answer. A lost link is left to the session, which reconnects.
async fn refresh(
    client: &Client,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    sequence: &Sequence,
) {
    if let Ok(read) = read(client, sequence).await {
        let _ = updates.send(StoreUpdate::Teams(read));
    }
}

/// A refused write: a name the service already uses keeps its message for the name field, a team or bot that is
/// gone reads as the pages say it, and anything else is the service's own sentence.
fn teams_refusal(error: &ClientError) -> Refusal {
    match error {
        ClientError::Remote(body)
            if body.code == codes::TEAM_NAME_IN_USE || body.code == codes::BOT_NAME_IN_USE =>
        {
            Refusal::NameProblem {
                message: body.message.clone(),
            }
        }
        ClientError::Remote(body)
            if body.code == codes::TEAM_NOT_FOUND
                || body.code == codes::BOT_NOT_FOUND
                || body.code == codes::TEAM_PROJECT_NOT_FOUND =>
        {
            Refusal::Failed {
                code: body.code.clone(),
                message: "That item is gone. Nothing was changed.".to_owned(),
            }
        }
        _ => Refusal::from(error),
    }
}

impl ServiceHandle {
    /// `team.update` with `access`. The team shows the new access when the read that follows the answer lands.
    pub async fn set_team_access(&self, team: String, access: TeamAccess) -> Result<Team, Refusal> {
        self.ask(|reply| {
            Command::Team(TeamCommand::SetAccess {
                team,
                access,
                reply,
            })
        })
        .await
    }

    /// Reads a team's Conference room; the answer reaches the store.
    pub fn read_team_room(&self, team: String) {
        self.send(Command::Room(RoomCommand::Read { team }));
    }

    /// `team.room_post`: a person's line to a team's room. The room is read again once the service answers.
    pub async fn post_to_team_room(
        &self,
        params: TeamRoomPostParams,
    ) -> Result<TeamRoomPosted, Refusal> {
        self.ask(|reply| Command::Room(RoomCommand::Post { params, reply }))
            .await
    }

    /// `team.update`. The team shows the change when the read that follows the answer lands.
    pub async fn update_team(&self, params: TeamUpdateParams) -> Result<Team, Refusal> {
        self.ask(|reply| Command::Team(TeamCommand::UpdateTeam { params, reply }))
            .await
    }

    /// `team.rm` without `force`: a team with a running task is refused, and the refusal says so. The team leaves
    /// the teams when the read that follows the answer lands.
    pub async fn remove_team(&self, team: String) -> Result<Team, Refusal> {
        self.ask(|reply| Command::Team(TeamCommand::RemoveTeam { team, reply }))
            .await
    }

    /// `team.project_add`. The project shows when the read that follows the answer lands.
    pub async fn add_team_project(
        &self,
        params: TeamProjectAddParams,
    ) -> Result<TeamProject, Refusal> {
        self.ask(|reply| Command::Team(TeamCommand::AddProject { params, reply }))
            .await
    }

    /// `team.bot_update`. The bot shows its change when the read that follows the answer lands.
    pub async fn update_bot(&self, params: BotUpdateParams) -> Result<Bot, Refusal> {
        self.ask(|reply| Command::Team(TeamCommand::UpdateBot { params, reply }))
            .await
    }

    /// `team.bot_rm`. A bot that owns open tasks is refused unless `unassign` is set. The bot leaves the teams when
    /// the read that follows the answer lands.
    pub async fn remove_bot(&self, params: BotRemoveParams) -> Result<Bot, Refusal> {
        self.ask(|reply| Command::Team(TeamCommand::RemoveBot { params, reply }))
            .await
    }
}

/// Numbers the protocol's ids for the pages, which key on numbers. An id keeps the number the first read that
/// names it gave it, for the life of the store, so a page keeps pointing at the same team, card or bot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamIds {
    numbers: BTreeMap<String, u32>,
}

impl TeamIds {
    /// Numbers every id a team names: the team, its bots, its cards and the bots on them, and each project with
    /// the cards of its Board.
    pub fn register(&mut self, record: &TeamRecord) {
        self.number(&record.team.id);
        for bot in &record.bots {
            self.number(&bot.id);
        }
        self.register_board(&record.board);
        for project in &record.projects {
            self.number(&project.project.id);
            self.register_board(&project.board);
        }
    }

    fn register_board(&mut self, board: &Board) {
        for column in &board.columns {
            for card in &column.cards {
                self.number(&card.id);
                if let Some(bot) = &card.bot_id {
                    self.number(bot);
                }
            }
        }
    }

    /// The protocol id numbered `number`, if one is.
    pub fn key(&self, number: u32) -> Option<&str> {
        self.numbers
            .iter()
            .find(|(_, known)| **known == number)
            .map(|(id, _)| id.as_str())
    }

    /// The number of `id`, if a read has named it.
    pub fn get(&self, id: &str) -> Option<u32> {
        self.numbers.get(id).copied()
    }

    fn number(&mut self, id: &str) {
        if !self.numbers.contains_key(id) {
            let next = u32::try_from(self.numbers.len() + 1).unwrap_or(u32::MAX);
            self.numbers.insert(id.to_owned(), next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::update::fixtures::team;

    #[test]
    fn a_later_read_keeps_the_numbers_of_the_ids_it_does_not_change() {
        let mut ids = TeamIds::default();
        ids.register(&team("t1", "Desarrollo"));
        let first = ids.get("t1");
        ids.register(&team("t2", "Ops"));
        assert!(first.is_some());
        assert_eq!(ids.get("t1"), first);
        assert_ne!(ids.get("t2"), first);
        assert_eq!(ids.get("unknown"), None);
    }
}
