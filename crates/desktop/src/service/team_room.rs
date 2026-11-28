//! A team's Conference room on the service (capability `team-room/1`, decision 0102): its lines, read when the room
//! opens and again on `team.room_changed`, a person's line posted to it, and a choice card answered (decision 0121).
//! A host with `team-room/3` also answers replies and says which bots are typing (decision 0122).

use std::collections::BTreeMap;
use std::sync::Arc;

use massorca_client::{Client, ClientError};
use massorca_protocol::methods::team_room::{
    RoomMentions, TeamRoomAnswerParams, TeamRoomAnswered, TeamRoomListParams,
    TeamRoomMentionsParams, TeamRoomMessage, TeamRoomPostParams, TeamRoomPosted,
};
use massorca_protocol::methods::teams::{DelegationLink, TeamRef};
use massorca_protocol::{capabilities, codes};
use tokio::sync::{mpsc, oneshot};

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::teams::Sequence;
use super::update::StoreUpdate;

/// What the room of a service that has none says.
pub const NO_ROOM: &str =
    "This host's service has no Conference room yet. Update MassOrca on the host.";
/// What a choice card says on a host that cannot answer it.
pub const NO_CARDS: &str = "This host cannot answer choice cards yet. Update MassOrca on the host.";

/// What one read of a team's room answered: its lines, oldest first, or why they could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamRoomRead {
    /// The service's id of the team.
    pub team: String,
    /// Orders the reads of one team: the store keeps the answer with the highest number.
    pub number: u64,
    /// The host answers choice cards (`team-room/2`); without it a card shows no options.
    pub answers_cards: bool,
    /// The host answers a reply or a question in the room (`team-room/3`); without it the room offers neither.
    pub answers_replies: bool,
    /// The names of the bots working now, for the typing line.
    pub typing: Vec<String>,
    pub room: Result<Vec<TeamRoomMessage>, Refusal>,
    /// The delegated children of the team's tasks, for the task cards (decision 0128); empty on a host without them.
    pub delegations: Vec<DelegationLink>,
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
    /// Asks what an `@` typed so far can name (AUTO-13); the room is not read again.
    Mentions {
        params: TeamRoomMentionsParams,
        reply: oneshot::Sender<Result<RoomMentions, Refusal>>,
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

    /// `team.room_mentions`: what an `@` typed so far in a team's room can name. A host without `team-room/4` names
    /// nothing.
    pub async fn team_room_mentions(
        &self,
        params: TeamRoomMentionsParams,
    ) -> Result<RoomMentions, Refusal> {
        self.ask(|reply| Command::Room(RoomCommand::Mentions { params, reply }))
            .await
    }
}

/// The line as this host takes it: a host that routes gets `route` with no receiver, since the shell names the lead
/// only as its fallback; any other host gets the receiver and no `route` (AUTO-13).
fn for_host(mut params: TeamRoomPostParams, routes: bool) -> TeamRoomPostParams {
    if params.route == Some(true) && routes {
        params.to = None;
    } else {
        params.route = None;
    }
    params
}

/// The delegated children of a team's tasks. A host without the capability, or a refusal, has none to show.
async fn delegations_of(client: &Client, team: &str) -> Result<Vec<DelegationLink>, ClientError> {
    if !client.has_capability(capabilities::TEAM_DELEGATE) {
        return Ok(Vec::new());
    }
    let params = TeamRef {
        team: team.to_owned(),
    };
    match client.team_delegation_list(&params).await {
        Ok(list) => Ok(list.links),
        Err(ClientError::Remote(_) | ClientError::Protocol(_)) => Ok(Vec::new()),
        Err(error) => Err(error),
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
    let answers_cards = client.has_capability(capabilities::TEAM_ROOM_CARDS);
    let answers_replies = client.has_capability(capabilities::TEAM_ROOM_REPLIES);
    if !client.has_capability(capabilities::TEAM_ROOM) {
        return Ok(TeamRoomRead {
            team,
            number,
            answers_cards,
            answers_replies,
            typing: Vec::new(),
            room: Err(no_room()),
            delegations: Vec::new(),
        });
    }
    let params = TeamRoomListParams {
        team: team.clone(),
        limit: None,
    };
    match client.team_room_list(&params).await {
        Ok(list) => Ok(TeamRoomRead {
            delegations: delegations_of(client, &team).await?,
            team,
            number,
            answers_cards,
            answers_replies,
            typing: list.typing.into_iter().map(|bot| bot.bot_name).collect(),
            room: Ok(list.messages),
        }),
        Err(error @ (ClientError::Remote(_) | ClientError::Protocol(_))) => Ok(TeamRoomRead {
            team,
            number,
            answers_cards,
            answers_replies,
            typing: Vec::new(),
            room: Err(Refusal::from(&error)),
            delegations: Vec::new(),
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
                let params = for_host(
                    params,
                    client.has_capability(capabilities::TEAM_ROOM_ROUTING),
                );
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
            RoomCommand::Mentions { params, reply } => {
                let answer = if client.has_capability(capabilities::TEAM_ROOM_ROUTING) {
                    client
                        .team_room_mentions(&params)
                        .await
                        .map_err(|error| Refusal::from(&error))
                } else {
                    Ok(RoomMentions {
                        mentions: Vec::new(),
                    })
                };
                let _ = reply.send(answer);
                return;
            }
            RoomCommand::Answer { params, reply } => {
                let team = params.team.clone();
                let answer = if client.has_capability(capabilities::TEAM_ROOM_CARDS) {
                    client
                        .team_room_answer(&params)
                        .await
                        .map_err(|error| Refusal::from(&error))
                } else {
                    Err(Refusal::Failed {
                        code: codes::SERVICE_CAPABILITY_MISSING.to_owned(),
                        message: NO_CARDS.to_owned(),
                    })
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
        RoomCommand::Mentions { reply, .. } => {
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
            answers_cards: false,
            answers_replies: false,
            typing: Vec::new(),
            delegations: Vec::new(),
            room: Err(Refusal::Failed {
                code: "test".to_owned(),
                message: text.to_owned(),
            }),
        }
    }

    #[test]
    fn a_routed_line_drops_its_fallback_receiver_on_a_host_that_routes_and_its_route_elsewhere() {
        let params = TeamRoomPostParams {
            team: "t1".to_owned(),
            text: "Fix it".to_owned(),
            bot: None,
            to: Some("b2".to_owned()),
            session: None,
            agent_token: None,
            start: None,
            reply_to: None,
            ask: None,
            route: Some(true),
        };
        let routed = for_host(params.clone(), true);
        assert_eq!((routed.to, routed.route), (None, Some(true)));
        let older = for_host(params.clone(), false);
        assert_eq!((older.to.as_deref(), older.route), (Some("b2"), None));
        let picked = for_host(
            TeamRoomPostParams {
                route: None,
                ..params
            },
            true,
        );
        assert_eq!((picked.to.as_deref(), picked.route), (Some("b2"), None));
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
