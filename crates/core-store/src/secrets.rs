//! Vault rows (decision 0071): owned secrets hold a nonce and ciphertext, never plaintext; links point at an owned row;
//! requests back the in-chat request card. The caller serialises vault writes, so these stay single statements.

use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// One owned secret. `nonce` and `ciphertext` are opaque to the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRow {
    pub id: String,
    pub scope: String,
    pub name: String,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub created_at: i64,
    pub rotated_at: i64,
    pub used_at: Option<i64>,
}

/// An owned secret without its sealed value, for listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretMeta {
    pub id: String,
    pub scope: String,
    pub name: String,
    pub created_at: i64,
    pub rotated_at: i64,
    pub used_at: Option<i64>,
}

/// A scope's link to an owned secret, under the same name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretLinkRow {
    pub scope: String,
    pub name: String,
    pub secret_id: String,
    pub linked_at: i64,
}

/// One request for a secret by name. `state` is pending, stored, linked, declined or failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRequestRow {
    pub id: String,
    pub scope: String,
    pub name: String,
    pub reason: String,
    pub session: Option<String>,
    /// The connection's principal that asked (`owner` or `device:<id>`), for the request limits.
    pub requester: Option<String>,
    pub state: String,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
}

const SECRET_COLUMNS: &str = "id, scope, name, nonce, ciphertext, created_at, rotated_at, used_at";
const REQUEST_COLUMNS: &str =
    "id, scope, name, reason, session, state, created_at, resolved_at, requester";

fn secret_row(row: &Row<'_>) -> rusqlite::Result<SecretRow> {
    Ok(SecretRow {
        id: row.get(0)?,
        scope: row.get(1)?,
        name: row.get(2)?,
        nonce: row.get(3)?,
        ciphertext: row.get(4)?,
        created_at: row.get(5)?,
        rotated_at: row.get(6)?,
        used_at: row.get(7)?,
    })
}

fn link_row(row: &Row<'_>) -> rusqlite::Result<SecretLinkRow> {
    Ok(SecretLinkRow {
        scope: row.get(0)?,
        name: row.get(1)?,
        secret_id: row.get(2)?,
        linked_at: row.get(3)?,
    })
}

fn request_row(row: &Row<'_>) -> rusqlite::Result<SecretRequestRow> {
    Ok(SecretRequestRow {
        id: row.get(0)?,
        scope: row.get(1)?,
        name: row.get(2)?,
        reason: row.get(3)?,
        session: row.get(4)?,
        state: row.get(5)?,
        created_at: row.get(6)?,
        resolved_at: row.get(7)?,
        requester: row.get(8)?,
    })
}

impl Store {
    /// Adds an owned secret. A second row with the same scope and name is a `Conflict`.
    pub fn insert_secret(&self, row: &SecretRow) -> Result<(), StoreError> {
        self.lock().execute(
            &format!(
                "INSERT INTO secrets ({SECRET_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            ),
            params![
                row.id,
                row.scope,
                row.name,
                row.nonce,
                row.ciphertext,
                row.created_at,
                row.rotated_at,
                row.used_at
            ],
        )?;
        Ok(())
    }

    /// The owned secret `name` in `scope`.
    pub fn find_secret(&self, scope: &str, name: &str) -> Result<Option<SecretRow>, StoreError> {
        self.query_optional(
            &format!("SELECT {SECRET_COLUMNS} FROM secrets WHERE scope = ?1 AND name = ?2"),
            [scope, name],
            secret_row,
        )
    }

    /// The owned secret with `id`.
    pub fn secret_by_id(&self, id: &str) -> Result<Option<SecretRow>, StoreError> {
        self.query_optional(
            &format!("SELECT {SECRET_COLUMNS} FROM secrets WHERE id = ?1"),
            [id],
            secret_row,
        )
    }

    /// Replaces the sealed value of `id`. Returns whether the row exists.
    pub fn update_secret_value(
        &self,
        id: &str,
        nonce: &[u8],
        ciphertext: &[u8],
        rotated_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE secrets SET nonce = ?2, ciphertext = ?3, rotated_at = ?4 WHERE id = ?1",
            params![id, nonce, ciphertext, rotated_at],
        )?;
        Ok(changed > 0)
    }

    /// Records that `id` was handed to a process at `used_at`.
    pub fn touch_secret(&self, id: &str, used_at: i64) -> Result<(), StoreError> {
        self.lock().execute(
            "UPDATE secrets SET used_at = ?2 WHERE id = ?1",
            params![id, used_at],
        )?;
        Ok(())
    }

    /// Removes an owned secret and, through the foreign key, every link to it. Returns whether it existed.
    pub fn delete_secret(&self, id: &str) -> Result<bool, StoreError> {
        Ok(self
            .lock()
            .execute("DELETE FROM secrets WHERE id = ?1", [id])?
            > 0)
    }

    /// Every owned secret without its sealed value, ordered by scope and name.
    pub fn list_secrets(&self) -> Result<Vec<SecretMeta>, StoreError> {
        self.query_all(
            "SELECT id, scope, name, created_at, rotated_at, used_at FROM secrets ORDER BY scope, name",
            [],
            |row| {
                Ok(SecretMeta {
                    id: row.get(0)?,
                    scope: row.get(1)?,
                    name: row.get(2)?,
                    created_at: row.get(3)?,
                    rotated_at: row.get(4)?,
                    used_at: row.get(5)?,
                })
            },
        )
    }

    /// Adds a link. A second link with the same scope and name is a `Conflict`; a missing secret is a `Constraint`.
    pub fn insert_secret_link(&self, link: &SecretLinkRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO secret_links (scope, name, secret_id, linked_at) VALUES (?1, ?2, ?3, ?4)",
            params![link.scope, link.name, link.secret_id, link.linked_at],
        )?;
        Ok(())
    }

    /// The link `name` in `scope`.
    pub fn find_secret_link(
        &self,
        scope: &str,
        name: &str,
    ) -> Result<Option<SecretLinkRow>, StoreError> {
        self.query_optional(
            "SELECT scope, name, secret_id, linked_at FROM secret_links WHERE scope = ?1 AND name = ?2",
            [scope, name],
            link_row,
        )
    }

    /// Removes a link. Returns whether it existed.
    pub fn delete_secret_link(&self, scope: &str, name: &str) -> Result<bool, StoreError> {
        let removed = self.lock().execute(
            "DELETE FROM secret_links WHERE scope = ?1 AND name = ?2",
            [scope, name],
        )?;
        Ok(removed > 0)
    }

    /// Every link, ordered by scope and name.
    pub fn list_secret_links(&self) -> Result<Vec<SecretLinkRow>, StoreError> {
        self.query_all(
            "SELECT scope, name, secret_id, linked_at FROM secret_links ORDER BY scope, name",
            [],
            link_row,
        )
    }

    /// Adds a request.
    pub fn insert_secret_request(&self, request: &SecretRequestRow) -> Result<(), StoreError> {
        self.lock().execute(
            &format!(
                "INSERT INTO secret_requests ({REQUEST_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            ),
            params![
                request.id,
                request.scope,
                request.name,
                request.reason,
                request.session,
                request.state,
                request.created_at,
                request.resolved_at,
                request.requester
            ],
        )?;
        Ok(())
    }

    /// The request with `id`.
    pub fn secret_request(&self, id: &str) -> Result<Option<SecretRequestRow>, StoreError> {
        self.query_optional(
            &format!("SELECT {REQUEST_COLUMNS} FROM secret_requests WHERE id = ?1"),
            [id],
            request_row,
        )
    }

    /// Requests in creation order; only pending ones when `pending_only`.
    pub fn list_secret_requests(
        &self,
        pending_only: bool,
    ) -> Result<Vec<SecretRequestRow>, StoreError> {
        self.query_all(
            &format!(
                "SELECT {REQUEST_COLUMNS} FROM secret_requests WHERE (?1 = 0 OR state = 'pending') ORDER BY created_at, id"
            ),
            [pending_only],
            request_row,
        )
    }

    /// Pending requests `requester` made for `scope`, from any session.
    pub fn count_pending_secret_requests(
        &self,
        scope: &str,
        requester: &str,
    ) -> Result<u64, StoreError> {
        let count: i64 = self.lock().query_row(
            "SELECT COUNT(*) FROM secret_requests \
             WHERE scope = ?1 AND requester = ?2 AND state = 'pending'",
            params![scope, requester],
            |row| row.get(0),
        )?;
        Ok(u64::try_from(count).unwrap_or(0))
    }

    /// Deletes answered requests resolved before `answered_before` and pending ones created before
    /// `pending_before` (both ms), and returns how many went.
    pub fn prune_secret_requests(
        &self,
        answered_before: i64,
        pending_before: i64,
    ) -> Result<u64, StoreError> {
        let deleted = self.lock().execute(
            "DELETE FROM secret_requests \
             WHERE (state <> 'pending' AND resolved_at < ?1) OR (state = 'pending' AND created_at < ?2)",
            params![answered_before, pending_before],
        )?;
        Ok(u64::try_from(deleted).unwrap_or(0))
    }

    /// Requests `requester` made at or after `since` (ms), in any scope and state.
    pub fn count_secret_requests_since(
        &self,
        requester: &str,
        since: i64,
    ) -> Result<u64, StoreError> {
        let count: i64 = self.lock().query_row(
            "SELECT COUNT(*) FROM secret_requests WHERE requester = ?1 AND created_at >= ?2",
            params![requester, since],
            |row| row.get(0),
        )?;
        Ok(u64::try_from(count).unwrap_or(0))
    }

    /// Moves a pending request to `state`. Returns whether a pending request with `id` existed.
    pub fn resolve_secret_request(
        &self,
        id: &str,
        state: &str,
        resolved_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE secret_requests SET state = ?2, resolved_at = ?3 WHERE id = ?1 AND state = 'pending'",
            params![id, state, resolved_at],
        )?;
        Ok(changed > 0)
    }

    /// Moves every pending request for `name` in `scope` to `state`, and returns their ids.
    pub fn resolve_secret_requests_for(
        &self,
        scope: &str,
        name: &str,
        state: &str,
        resolved_at: i64,
    ) -> Result<Vec<String>, StoreError> {
        let connection = self.lock();
        let mut statement = connection.prepare(
            "UPDATE secret_requests SET state = ?3, resolved_at = ?4 \
             WHERE scope = ?1 AND name = ?2 AND state = 'pending' RETURNING id",
        )?;
        let ids = statement
            .query_map(params![scope, name, state, resolved_at], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(ids)
    }
}
