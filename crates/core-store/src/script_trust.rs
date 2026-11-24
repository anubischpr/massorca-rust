//! Trusted repository scripts (decision 0092): per project and script kind, the hash of the content the owner trusted.

use rusqlite::params;

use crate::{Store, StoreError, now_ms};

impl Store {
    /// The content hash of the `kind` script the owner trusted for `project_id`, or `None` when none is trusted.
    pub fn trusted_script_hash(
        &self,
        project_id: &str,
        kind: &str,
    ) -> Result<Option<String>, StoreError> {
        self.query_optional(
            "SELECT content_hash FROM script_trust WHERE project_id = ?1 AND kind = ?2",
            [project_id, kind],
            |row| row.get(0),
        )
    }

    /// Records `hash` as the trusted `kind` script of `project_id`, replacing the one trusted before.
    pub fn trust_script(&self, project_id: &str, kind: &str, hash: &str) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO script_trust (project_id, kind, content_hash, trusted_at) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(project_id, kind) DO UPDATE SET content_hash = excluded.content_hash, trusted_at = excluded.trusted_at",
            params![project_id, kind, hash, now_ms()],
        )?;
        Ok(())
    }
}
