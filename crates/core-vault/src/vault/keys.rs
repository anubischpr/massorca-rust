//! Opening the vault's key. A key is created only for an empty vault with nothing recorded; secrets without a
//! readable protection were sealed by a key that is gone, so the vault is locked rather than silently re-keyed.

use std::io;
use std::path::Path;
use std::sync::MutexGuard;

use super::{Vault, VaultError};
use crate::key::{KeyError, KeyStore, LockReason, MasterKey, Protection};

const PROTECTION_FILE: &str = "protection";

/// What `<dir>/protection` records, read without opening the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Recorded {
    Nothing,
    Protection(Protection),
    Unreadable,
}

pub(super) fn recorded(dir: &Path) -> Recorded {
    match std::fs::read_to_string(dir.join(PROTECTION_FILE)) {
        Ok(word) => word
            .trim()
            .parse()
            .map_or(Recorded::Unreadable, Recorded::Protection),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Recorded::Nothing,
        Err(_) => Recorded::Unreadable,
    }
}

type KeyGuard<'g, 'm> = &'g mut MutexGuard<'m, Option<MasterKey>>;

impl Vault {
    /// The opened key of a vault that has one; nothing is created.
    pub(super) fn key_in<'g>(&self, guard: KeyGuard<'g, '_>) -> Result<&'g MasterKey, VaultError> {
        self.key(guard, false)
    }

    /// The opened key, created when the vault is empty and records no protection.
    pub(super) fn key_or_create<'g>(
        &self,
        guard: KeyGuard<'g, '_>,
    ) -> Result<&'g MasterKey, VaultError> {
        self.key(guard, true)
    }

    fn key<'g>(
        &self,
        guard: KeyGuard<'g, '_>,
        may_create: bool,
    ) -> Result<&'g MasterKey, VaultError> {
        match recorded(&self.dir) {
            Recorded::Protection(_) => {}
            Recorded::Unreadable => {
                **guard = None;
                return Err(VaultError::Locked(LockReason::UnknownProtection));
            }
            Recorded::Nothing => {
                // A cached key whose record is gone would seal values no later start can open.
                **guard = None;
                if !may_create || !self.store.list_secrets()?.is_empty() {
                    return Err(VaultError::Locked(LockReason::MissingKey));
                }
            }
        }
        if guard.is_none() {
            let (_, key) = KeyStore::open_or_create(&self.dir, self.force_file).map_err(
                |error| match error {
                    KeyError::Locked(reason) => VaultError::Locked(reason),
                    other => VaultError::Key(other),
                },
            )?;
            **guard = Some(key);
        }
        guard.as_ref().ok_or(VaultError::Cipher)
    }
}
