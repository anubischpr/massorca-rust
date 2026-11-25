//! A team's Conference room on the service (capability `team-room/1`, decision 0102): its lines, read when the room
//! opens and again on `team.room_changed`, a person's line posted to it, and a choice card answered (decision 0121).

use std::collections::BTreeMap;
use std::sync::Arc;

use massorca_client::{Client, ClientError};
use massorca_protocol::methods::team_room::{
    TeamRoomAnswerParams, TeamRoomAnswered, TeamRoomListParams, TeamRoomMessage,
    TeamRoomPostParams, TeamRoomPosted,
};
use massorca_protocol::{capabilities, codes};
use tokio::sync::{mpsc, oneshot};

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::teams::Sequence;
use super::update::StoreUpdate;

/// What the room of a service that has none says.
pub const NO_ROOM: &str =
    "This host's service has no Conference room yet. Update MassOrca on the host.";

/// What one read of a team's room answered: its lines, oldest first, or why they could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamRoomRead {
    /// The service's id of the team.
    pub team: String,
    /// Orders the reads of one team: the store keeps the answer with the highest number.
    pub number: u64,
    pub room: Result<Vec<TeamRoomMessage>, Refusal>,
}

/// The rooms the store holds, by the service's team id.
pub type TeamRooms = BTreeMap<String, TeamRoomRead>;

/// The refusal of a service without `team-room/1`.
fn no_room() -> Refusal {
    Refusal::Failed {
        code: codes::SERVICE_CAPABILITY_MISSING.to_owned(),
        message: NO_ROOM.to_owned(),
    }
}

/// Keeps `read` unless an answer read later for the same team is held.
pub fn keep_newest(rooms: &mut TeamRooms, read: TeamRoomRead) {
    if rooms
        .get(&read.team)
        .is_some_and(|held| held.number >= read.number)
    {
        return;
    }
    rooms.insert(read.team.clone(), read);
}

/// Room calls the bridge runs for the shell.
#[derive(Debug)]
pub(crate) enum RoomCommand {
    /// Reads the team's room and sends it to the store.
    Read { team: String },
    /// Posts a person's line, then reads the room again.
    Post {
        params: TeamRoomPostParams,
        reply: oneshot::Sender<Result<TeamRoomPosted, Refusal>>,
    },
    /// Answers a choice card, then reads the room again.
    Answer {
        params: TeamRoomAnswerParams,
        reply: oneshot::Sender<Result<TeamRoomAnswered, Refusal>>,
    },
}

impl ServiceHandle {
    /// `team.room_answer`: answers a choice card of a team's room once. The room is read again once the service
    /// answers.
    pub async fn answer_team_room_card(
        &self,
        params: TeamRoomAnswerParams,
    ) -> Result<TeamRoomAnswered, Refusal> {
        self.ask(|reply| Command::Room(RoomCommand::Answer { params, reply }))
            .await
    }
}

/// Reads one team's room. A service without the capability answers with `NO_ROOM`; a refusal is an answer; a lost
/// link is an error, so the bridge reconnects.
pub(super) async fn read(
    client: &Client,
    sequence: &Sequence,
    team: String,
) -> Result<TeamRoomRead, ClientError> {
    let number = sequence.next();
    if !client.has_capability(capabilities::TEAM_ROOM) {
        return Ok(TeamRoomRead {
            team,
            number,
            room: Err(no_room()),
        });
    }
    let params = TeamRoomListParams {
        team: team.clone(),
        limit: None,
    };
    match client.team_room_list(&params).await {
        Ok(list) => Ok(TeamRoomRead {
            team,
            number,
            room: Ok(list.messages),
        }),
        Err(error @ (ClientError::Remote(_) | ClientError::Protocol(_))) => Ok(TeamRoomRead {
            team,
            number,
            room: Err(Refusal::from(&error)),
        }),
        Err(error) => Err(error),
    }
}

/// Runs one room call in the background and sends what it learned to the store.
pub(super) fn spawn_call(
    client: &Client,
    command: RoomCommand,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    sequence: &Arc<Sequence>,
) {
    let (client, updates, sequence) = (client.clone(), updates.clone(), sequence.clone());
    tokio::spawn(async move {
        let team = match command {
            RoomCommand::Read { team } => team,
            RoomCommand::Post { params, reply } => {
                let team = params.team.clone();
                let answer = if client.has_capability(capabilities::TEAM_ROOM) {
                    client
                        .team_room_post(&params)
                        .await
                        .map_err(|error| Refusal::from(&error))
                } else {
                    Err(no_room())
                };
                let _ = reply.send(answer);
                team
            }
            RoomCommand::Answer { params, reply } => {
                let team = params.team.clone();
                let answer = if client.has_capability(capabilities::TEAM_ROOM) {
                    client
                        .team_room_answer(&params)
                        .await
                        .map_err(|error| Refusal::from(&error))
                } else {
                    Err(no_room())
                };
                let _ = reply.send(answer);
                team
            }
        };
        if let Ok(read) = read(&client, &sequence, team).await {
            let _ = updates.send(StoreUpdate::TeamRoom(read));
        }
    });
}

/// Refuses a room call while no service is connected.
pub(super) fn refuse(command: RoomCommand, refusal: Refusal) {
    match command {
        RoomCommand::Read { .. } => {}
        RoomCommand::Post { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        RoomCommand::Answer { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_of(team: &str, number: u64, text: &str) -> TeamRoomRead {
        TeamRoomRead {
            team: team.to_owned(),
            number,
            room: Err(Refusal::Failed {
                code: "test".to_owned(),
                message: text.to_owned(),
            }),
        }
    }

    #[test]
    fn a_stale_room_read_never_replaces_a_newer_one_of_the_same_team() {
        let mut rooms = TeamRooms::new();
        keep_newest(&mut rooms, read_of("t1", 2, "newer"));
        keep_newest(&mut rooms, read_of("t1", 1, "older"));
        keep_newest(&mut rooms, read_of("t2", 1, "other team"));
        assert_eq!(rooms["t1"], read_of("t1", 2, "newer"));
        assert_eq!(rooms["t2"].number, 1);
    }
}
