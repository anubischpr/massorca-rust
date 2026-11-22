//! The vault key check (decision 0071): a constant sealed under the master key when the key is created. A key that
//! does not open it is not the key that sealed the vault's secrets.

use rusqlite::params;

use crate::error::StoreError;
use crate::store::Store;

/// The sealed constant; opaque to the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretKeyCheck {
    pub nonce: Vec<u8>,
    pub sealed: Vec<u8>,
}

impl Store {
    /// The recorded key check, if any.
    pub fn secret_key_check(&self) -> Result<Option<SecretKeyCheck>, StoreError> {
        self.query_optional(
            "SELECT nonce, sealed FROM secret_key_check WHERE id = 1",
            [],
            |row| {
                Ok(SecretKeyCheck {
                    nonce: row.get(0)?,
                    sealed: row.get(1)?,
                })
            },
        )
    }

    /// Records `check`, replacing any earlier one.
    pub fn set_secret_key_check(&self, check: &SecretKeyCheck, now: i64) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT OR REPLACE INTO secret_key_check (id, nonce, sealed, created_at) VALUES (1, ?1, ?2, ?3)",
            params![check.nonce, check.sealed, now],
        )?;
        Ok(())
    }

    /// Forgets the key check, for a key reset of an empty vault.
    pub fn delete_secret_key_check(&self) -> Result<(), StoreError> {
        self.lock().execute("DELETE FROM secret_key_check", [])?;
        Ok(())
    }
}
