use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// One row of `devices`: a paired network device (decision 0041).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    /// `sha256:<base64url>` of the device's public key; unique.
    pub key_pin: String,
    /// Permission classes, as their wire names.
    pub scopes: Vec<String>,
    pub paired_at: i64,
    pub last_seen_at: Option<i64>,
    pub revoked_at: Option<i64>,
}

const SELECT_DEVICES: &str =
    "SELECT id, name, key_pin, scopes, paired_at, last_seen_at, revoked_at FROM devices";

fn device_from_row(row: &Row<'_>) -> rusqlite::Result<DeviceRow> {
    let scopes: String = row.get("scopes")?;
    Ok(DeviceRow {
        id: row.get("id")?,
        name: row.get("name")?,
        key_pin: row.get("key_pin")?,
        scopes: scopes
            .split(',')
            .filter(|scope| !scope.is_empty())
            .map(str::to_owned)
            .collect(),
        paired_at: row.get("paired_at")?,
        last_seen_at: row.get("last_seen_at")?,
        revoked_at: row.get("revoked_at")?,
    })
}

impl Store {
    /// Inserts a device. A duplicate id or key is `StoreError::Conflict`.
    pub fn insert_device(&self, row: &DeviceRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO devices (id, name, key_pin, scopes, paired_at, last_seen_at, revoked_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                row.id,
                row.name,
                row.key_pin,
                row.scopes.join(","),
                row.paired_at,
                row.last_seen_at,
                row.revoked_at
            ],
        )?;
        Ok(())
    }

    pub fn get_device(&self, id: &str) -> Result<Option<DeviceRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_DEVICES} WHERE id = ?1"),
            [id],
            device_from_row,
        )
    }

    pub fn find_device_by_key_pin(&self, key_pin: &str) -> Result<Option<DeviceRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_DEVICES} WHERE key_pin = ?1"),
            [key_pin],
            device_from_row,
        )
    }

    /// Every device, revoked ones included, oldest pairing first.
    pub fn list_devices(&self) -> Result<Vec<DeviceRow>, StoreError> {
        self.query_all(
            &format!("{SELECT_DEVICES} ORDER BY paired_at, id"),
            [],
            device_from_row,
        )
    }

    /// Marks a device revoked at `at`. Returns whether a device that was not yet revoked changed.
    pub fn revoke_device(&self, id: &str, at: i64) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE devices SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
            params![id, at],
        )?;
        Ok(changed > 0)
    }

    /// Deletes a revoked device. Returns whether one was deleted; an active device is never deleted.
    pub fn forget_device(&self, id: &str) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "DELETE FROM devices WHERE id = ?1 AND revoked_at IS NOT NULL",
            params![id],
        )?;
        Ok(changed > 0)
    }

    /// Records that a device connected at `at`.
    pub fn touch_device(&self, id: &str, at: i64) -> Result<(), StoreError> {
        self.lock().execute(
            "UPDATE devices SET last_seen_at = ?2 WHERE id = ?1",
            params![id, at],
        )?;
        Ok(())
    }

    /// Replaces a device's permission classes, as their wire names. The pairing flow sets them once; this lets a test hold
    /// a paired device to a narrower set. Returns whether the device exists. Test-only: built under `cfg(test)` or the
    /// `test-support` feature, so production code cannot call it.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_device_scopes(&self, id: &str, scopes: &[String]) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE devices SET scopes = ?2 WHERE id = ?1",
            params![id, scopes.join(",")],
        )?;
        Ok(changed > 0)
    }
}
