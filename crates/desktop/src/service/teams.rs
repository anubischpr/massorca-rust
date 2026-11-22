//! The Bot Teams reads the bridge runs (capability `teams/1`): every team the service lists, each with its
//! Board. A read takes a number when it starts, and the store applies only the newest answer, so a slow read
//! never replaces a newer one.

use std::sync::atomic::{AtomicU64, Ordering};

use massorca_client::{Client, ClientError};
use massorca_protocol::codes;
use massorca_protocol::methods::teams::{Board, BoardParams, Team};

use super::refusal::Refusal;

/// A team as the service reports it, with its Board: every task of the team, in four columns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamRecord {
    pub team: Team,
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
pub(super) struct Sequence {
    last: AtomicU64,
}

impl Sequence {
    fn next(&self) -> u64 {
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
        let params = BoardParams {
            team: team.id.clone(),
            project: None,
        };
        match client.team_board(&params).await {
            Ok(board) => records.push(TeamRecord { team, board }),
            // Removed while the list was read: it is simply not in this read.
            Err(ClientError::Remote(body)) if body.code == codes::TEAM_NOT_FOUND => {}
            Err(error) => return Err(error),
        }
    }
    Ok(records)
}
