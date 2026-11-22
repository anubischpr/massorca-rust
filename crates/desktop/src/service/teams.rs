//! The Bot Teams reads the bridge runs (capability `teams/1`): every team the service lists, each with its
//! Board. A read takes a number when it starts, and the store applies only the newest answer, so a slow read
//! never replaces a newer one.

use std::collections::BTreeMap;
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

/// Numbers the protocol's ids for the pages, which key on numbers. An id keeps the number the first read that
/// names it gave it, for the life of the store, so a page keeps pointing at the same team, card or bot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamIds {
    numbers: BTreeMap<String, u32>,
}

impl TeamIds {
    /// Numbers every id a team names: the team, its cards and the bots on them.
    pub fn register(&mut self, record: &TeamRecord) {
        self.number(&record.team.id);
        for column in &record.board.columns {
            for card in &column.cards {
                self.number(&card.id);
                if let Some(bot) = &card.bot_id {
                    self.number(bot);
                }
            }
        }
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
