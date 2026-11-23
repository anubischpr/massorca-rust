use rusqlite::params;

use crate::error::StoreError;
use crate::now_ms;
use crate::store::Store;

impl Store {
    /// Stores `json` verbatim for `owner` and `key`, replacing any earlier value.
    pub fn set_setting(&self, owner: &str, key: &str, json: &str) -> Result<(), StoreError> {
        self.lock().execute(
            concat!(
                "INSERT INTO settings (owner, key, value, updated_at) VALUES (?1, ?2, ?3, ?4) ",
                "ON CONFLICT (owner, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at"
            ),
            params![owner, key, json, now_ms()],
        )?;
        Ok(())
    }

    /// Stores `json` for `key` and removes the `retired` rows in one transaction, so a reader never sees both.
    pub fn replace_setting(
        &self,
        owner: &str,
        key: &str,
        json: &str,
        retired: &[&str],
    ) -> Result<(), StoreError> {
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        transaction.execute(
            concat!(
                "INSERT INTO settings (owner, key, value, updated_at) VALUES (?1, ?2, ?3, ?4) ",
                "ON CONFLICT (owner, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at"
            ),
            params![owner, key, json, now_ms()],
        )?;
        for old in retired {
            transaction.execute(
                "DELETE FROM settings WHERE owner = ?1 AND key = ?2",
                params![owner, old],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Removes `key` and the `retired` rows in one transaction. Returns whether `key` itself was stored.
    pub fn remove_setting(
        &self,
        owner: &str,
        key: &str,
        retired: &[&str],
    ) -> Result<bool, StoreError> {
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let removed = transaction.execute(
            "DELETE FROM settings WHERE owner = ?1 AND key = ?2",
            params![owner, key],
        )?;
        for old in retired {
            transaction.execute(
                "DELETE FROM settings WHERE owner = ?1 AND key = ?2",
                params![owner, old],
            )?;
        }
        transaction.commit()?;
        Ok(removed > 0)
    }

    pub fn get_setting(&self, owner: &str, key: &str) -> Result<Option<String>, StoreError> {
        self.query_optional(
            "SELECT value FROM settings WHERE owner = ?1 AND key = ?2",
            [owner, key],
            |row| row.get(0),
        )
    }

    /// Returns whether a setting was removed.
    pub fn delete_setting(&self, owner: &str, key: &str) -> Result<bool, StoreError> {
        let removed = self.lock().execute(
            "DELETE FROM settings WHERE owner = ?1 AND key = ?2",
            [owner, key],
        )?;
        Ok(removed > 0)
    }

    /// `(owner, key, json)` rows ordered by owner then key; `None` lists every owner.
    pub fn list_settings(
        &self,
        owner: Option<&str>,
    ) -> Result<Vec<(String, String, String)>, StoreError> {
        self.query_all(
            "SELECT owner, key, value FROM settings WHERE (?1 IS NULL OR owner = ?1) ORDER BY owner, key",
            [owner],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
    }
}
