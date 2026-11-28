//! Opening the vault's key. A key is created only for an empty vault with nothing recorded; secrets without a
//! readable protection were sealed by a key that is gone, so the vault is locked rather than silently re-keyed. An
//! opened key must also open the key check in the database, so a key planted in the vault directory is refused.

use std::io;
use std::path::Path;
use std::sync::MutexGuard;

use massorca_core_store::{SecretKeyCheck, now_ms};

use super::agent_kinds::{agent_kind_row_aad, is_agent_kind_scope, kind_of};
use super::services::{is_service_scope, service_row_aad};
use super::{ScopedName, Vault, VaultError};
use crate::cipher::{self, Aad};
use crate::key::{KeyError, KeyStore, LockReason, MasterKey, Protection};
use crate::names::Scope;

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

    /// The opened key, created when the vault is empty and records neither a protection nor a key check.
    pub(super) fn key_or_create<'g>(
        &self,
        guard: KeyGuard<'g, '_>,
    ) -> Result<&'g MasterKey, VaultError> {
        self.key(guard, true)
    }

    /// Forgets the key of a vault that holds no secret: its files, keyring item and key check. The next `set`
    /// creates a fresh key. The way out of an empty vault that is locked.
    ///
    /// # Errors
    /// `VaultNotEmpty` while any secret exists; a key or store failure.
    pub fn reset_key(&self) -> Result<(), VaultError> {
        let mut guard = self.lock();
        let mut rows = self.store.list_secrets()?;
        let kinds: Vec<String> = rows
            .iter()
            .filter_map(|row| kind_of(&row.scope).map(str::to_owned))
            .collect();
        rows.retain(|row| !is_agent_kind_scope(&row.scope));
        if !rows.is_empty() {
            let services = rows
                .iter()
                .filter(|row| is_service_scope(&row.scope))
                .count();
            return Err(VaultError::VaultNotEmpty {
                count: u64::try_from(rows.len()).unwrap_or(u64::MAX),
                services: u64::try_from(services).unwrap_or(u64::MAX),
            });
        }
        if !kinds.is_empty() {
            return Err(VaultError::AgentSettingsStored { kinds });
        }
        *guard = None;
        KeyStore::reset(&self.dir).map_err(VaultError::Key)?;
        self.store.delete_secret_key_check()?;
        Ok(())
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
                if !may_create
                    || !self.store.list_secrets()?.is_empty()
                    || self.store.secret_key_check()?.is_some()
                {
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
            self.confirm(&key)?;
            **guard = Some(key);
        }
        guard.as_ref().ok_or(VaultError::Cipher)
    }

    /// Refuses a key that does not open the vault's key check. A vault from before the check accepts its key once,
    /// only when every secret opens with it, and then records the check.
    fn confirm(&self, key: &MasterKey) -> Result<(), VaultError> {
        if let Some(check) = self.store.secret_key_check()? {
            return if cipher::opens_key_check(key, &check.nonce, &check.sealed) {
                Ok(())
            } else {
                Err(VaultError::Locked(LockReason::KeyMismatch))
            };
        }
        let mut unreadable = Vec::new();
        for meta in self.store.list_secrets()? {
            let opens = self.store.secret_by_id(&meta.id)?.is_some_and(|row| {
                let aad = if is_service_scope(&row.scope) {
                    service_row_aad(&row)
                } else if is_agent_kind_scope(&row.scope) {
                    agent_kind_row_aad(&row)
                } else {
                    row.scope
                        .parse::<Scope>()
                        .ok()
                        .and_then(|scope| Aad::new(&row.id, &scope, &row.name).ok())
                };
                aad.is_some_and(|aad| cipher::open(key, &aad, &row.nonce, &row.ciphertext).is_ok())
            });
            if !opens {
                unreadable.push(ScopedName {
                    scope: meta.scope,
                    name: meta.name,
                });
            }
        }
        if !unreadable.is_empty() {
            return Err(VaultError::KeyMismatch { unreadable });
        }
        let sealed = cipher::seal_key_check(key).map_err(|_| VaultError::Cipher)?;
        self.store.set_secret_key_check(
            &SecretKeyCheck {
                nonce: sealed.nonce.to_vec(),
                sealed: sealed.ciphertext,
            },
            now_ms(),
        )?;
        Ok(())
    }
}
