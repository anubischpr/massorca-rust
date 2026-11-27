use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// A person's room line waiting for its routing query (AUTO-13): the query's run, whether the line's task starts at
/// once, and whether its author held host-admin scope. `state` is `open` until the query's answer is acted on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamRoomRouteRow {
    pub message_id: String,
    pub team_id: String,
    pub session: Option<String>,
    pub state: String,
    pub start: bool,
    pub admin: bool,
    pub created_at: i64,
}

impl TeamRoomRouteRow {
    /// An open route of line `message_id`, with no run yet.
    pub fn open(message_id: &str, team_id: &str, start: bool, admin: bool, at: i64) -> Self {
        Self {
            message_id: message_id.to_owned(),
            team_id: team_id.to_owned(),
            session: None,
            state: "open".to_owned(),
            start,
            admin,
            created_at: at,
        }
    }
}

const SELECT_ROUTE: &str =
    "SELECT message_id, team_id, session, state, start, admin, created_at FROM team_room_routes";

fn route_from_row(row: &Row<'_>) -> rusqlite::Result<TeamRoomRouteRow> {
    Ok(TeamRoomRouteRow {
        message_id: row.get("message_id")?,
        team_id: row.get("team_id")?,
        session: row.get("session")?,
        state: row.get("state")?,
        start: row.get("start")?,
        admin: row.get("admin")?,
        created_at: row.get("created_at")?,
    })
}

impl Store {
    pub fn insert_team_room_route(&self, row: &TeamRoomRouteRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO team_room_routes (message_id, team_id, session, state, start, admin, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                row.message_id,
                row.team_id,
                row.session,
                row.state,
                row.start,
                row.admin,
                row.created_at
            ],
        )?;
        Ok(())
    }

    /// Names the run of the routing query of line `message_id`.
    pub fn set_team_room_route_session(
        &self,
        message_id: &str,
        session: &str,
    ) -> Result<(), StoreError> {
        self.lock().execute(
            "UPDATE team_room_routes SET session = ?2 WHERE message_id = ?1",
            params![message_id, session],
        )?;
        Ok(())
    }

    /// The route whose query run is `session`.
    pub fn find_team_room_route(
        &self,
        session: &str,
    ) -> Result<Option<TeamRoomRouteRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_ROUTE} WHERE session = ?1"),
            [session],
            route_from_row,
        )
    }

    pub fn get_team_room_route(
        &self,
        message_id: &str,
    ) -> Result<Option<TeamRoomRouteRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_ROUTE} WHERE message_id = ?1"),
            [message_id],
            route_from_row,
        )
    }

    /// Settles an open route: `false` when it was settled already. One statement, so two settles racing act once.
    pub fn settle_team_room_route(&self, message_id: &str) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE team_room_routes SET state = 'settled' WHERE message_id = ?1 AND state = 'open'",
            [message_id],
        )?;
        Ok(changed == 1)
    }

    /// Makes the posted line `id` a conversation of `bot_id` whose reply is open; `false` when it already is one.
    pub fn open_team_room_reply(&self, id: &str, bot_id: Option<&str>) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE team_room_messages SET reply_bot_id = ?2, reply_state = 'open' WHERE id = ?1 AND reply_state IS NULL",
            params![id, bot_id],
        )?;
        Ok(changed == 1)
    }
}
