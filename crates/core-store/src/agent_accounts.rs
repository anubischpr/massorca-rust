//! Agent account rows (decision 0059): the accounts of each kind, the active account per kind, and per-project
//! choices. The credential homes themselves live on disk; these rows say which exist and which one is used.

use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// One row of `agent_accounts`. `state` is the wire vocabulary, checked by the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentAccountRow {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub created_at: i64,
    /// The identity the kind's check reported, for example an email. Never read from credential contents.
    pub identity: Option<String>,
    pub state: String,
}

const SELECT_ACCOUNTS: &str =
    "SELECT id, kind, label, created_at, identity, state FROM agent_accounts";

fn account_from_row(row: &Row<'_>) -> rusqlite::Result<AgentAccountRow> {
    Ok(AgentAccountRow {
        id: row.get(0)?,
        kind: row.get(1)?,
        label: row.get(2)?,
        created_at: row.get(3)?,
        identity: row.get(4)?,
        state: row.get(5)?,
    })
}

impl Store {
    pub fn insert_agent_account(&self, row: &AgentAccountRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO agent_accounts (id, kind, label, created_at, identity, state) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                row.id,
                row.kind,
                row.label,
                row.created_at,
                row.identity,
                row.state
            ],
        )?;
        Ok(())
    }

    pub fn get_agent_account(&self, id: &str) -> Result<Option<AgentAccountRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_ACCOUNTS} WHERE id = ?1"),
            [id],
            account_from_row,
        )
    }

    /// The accounts of one kind, oldest first.
    pub fn list_agent_accounts(&self, kind: &str) -> Result<Vec<AgentAccountRow>, StoreError> {
        self.query_all(
            &format!("{SELECT_ACCOUNTS} WHERE kind = ?1 ORDER BY created_at, id"),
            [kind],
            account_from_row,
        )
    }

    /// Records the outcome of a check: the state, and the identity only when the check reported one. Returns whether an
    /// account of that id exists.
    pub fn set_agent_account_check(
        &self,
        id: &str,
        state: &str,
        identity: Option<&str>,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE agent_accounts SET state = ?2, identity = COALESCE(?3, identity) WHERE id = ?1",
            params![id, state, identity],
        )?;
        Ok(changed > 0)
    }

    /// Makes `account` the kind's active account, replacing the previous one.
    pub fn set_active_agent_account(&self, kind: &str, account: &str) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO agent_account_active (kind, account) VALUES (?1, ?2) ON CONFLICT(kind) DO UPDATE SET account = excluded.account",
            [kind, account],
        )?;
        Ok(())
    }

    /// The kind's active account id, when one is set.
    pub fn get_active_agent_account(&self, kind: &str) -> Result<Option<String>, StoreError> {
        self.query_optional(
            "SELECT account FROM agent_account_active WHERE kind = ?1",
            [kind],
            |row| row.get(0),
        )
    }

    /// Makes `account` the account a project uses for one kind, replacing an earlier choice.
    pub fn set_agent_account_choice(
        &self,
        kind: &str,
        project_id: &str,
        account: &str,
    ) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO agent_account_choices (kind, project_id, account) VALUES (?1, ?2, ?3) ON CONFLICT(kind, project_id) DO UPDATE SET account = excluded.account",
            [kind, project_id, account],
        )?;
        Ok(())
    }

    /// The account a project chose for one kind, when it chose one.
    pub fn get_agent_account_choice(
        &self,
        kind: &str,
        project_id: &str,
    ) -> Result<Option<String>, StoreError> {
        self.query_optional(
            "SELECT account FROM agent_account_choices WHERE kind = ?1 AND project_id = ?2",
            [kind, project_id],
            |row| row.get(0),
        )
    }

    /// The projects that chose `account`, by id.
    pub fn projects_using_agent_account(&self, account: &str) -> Result<Vec<String>, StoreError> {
        self.query_all(
            "SELECT project_id FROM agent_account_choices WHERE account = ?1 ORDER BY project_id",
            [account],
            |row| row.get(0),
        )
    }

    /// Deletes an account, its active mark and its project choices. Returns whether it existed.
    pub fn delete_agent_account(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM agent_accounts WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }
}
