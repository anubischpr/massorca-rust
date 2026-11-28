//! A kind's standing launch settings (decision 0174): one sealed row per agent kind, under the scope `agent-kind:<kind>`
//! with the name `SETTINGS`, which no `secret.*` scope can name. The value is the agents domain's own text; its
//! environment values may be secrets, so the whole value is sealed like one. No secret method lists, counts, links or
//! hands these rows to a run's environment.

use massorca_core_store::{SecretRow, new_id, now_ms};

use super::{Vault, VaultError};
use crate::cipher::{self, Aad};
use crate::value::SecretValue;

const SCOPE_PREFIX: &str = "agent-kind:";
const ROW_NAME: &str = "SETTINGS";
/// Longest agent kind id, as the agents domain declares it.
const MAX_KIND_CHARS: usize = 32;

/// Whether a stored row's scope is an agent kind's settings.
pub(super) fn is_agent_kind_scope(scope: &str) -> bool {
    scope.starts_with(SCOPE_PREFIX)
}

/// The kind a settings row belongs to, or `None` for a row of another scope.
pub(super) fn kind_of(scope: &str) -> Option<&str> {
    scope.strip_prefix(SCOPE_PREFIX)
}

/// The associated data of a settings row, or `None` for a malformed one.
pub(super) fn agent_kind_row_aad(row: &SecretRow) -> Option<Aad> {
    let kind = kind_of(&row.scope)?;
    (is_valid_kind(kind) && row.name == ROW_NAME && !row.id.is_empty() && !row.id.contains('\0'))
        .then(|| Aad::for_internal_row(&row.id, &row.scope, &row.name))
}

/// `[a-z0-9-]{1,32}`, as an agent kind id.
fn is_valid_kind(kind: &str) -> bool {
    (1..=MAX_KIND_CHARS).contains(&kind.len())
        && kind
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn scope_of(kind: &str) -> Result<String, VaultError> {
    if is_valid_kind(kind) {
        Ok(format!("{SCOPE_PREFIX}{kind}"))
    } else {
        Err(VaultError::InvalidAgentKind)
    }
}

impl Vault {
    /// Stores `kind`'s settings, replacing any stored before. Creates the key on first use.
    ///
    /// # Errors
    /// `InvalidAgentKind`, `Locked`, or a store or key failure.
    pub fn set_agent_kind_settings(
        &self,
        kind: &str,
        value: &SecretValue,
    ) -> Result<(), VaultError> {
        let scope = scope_of(kind)?;
        let mut guard = self.lock();
        let key = self.key_or_create(&mut guard)?;
        let now = now_ms();
        match self.store.find_secret(&scope, ROW_NAME)? {
            Some(row) => {
                let aad = Aad::for_internal_row(&row.id, &scope, ROW_NAME);
                let sealed = cipher::seal(key, &aad, value).map_err(|_| VaultError::Cipher)?;
                self.store
                    .update_secret_value(&row.id, &sealed.nonce, &sealed.ciphertext, now)?;
            }
            None => {
                let id = new_id();
                let aad = Aad::for_internal_row(&id, &scope, ROW_NAME);
                let sealed = cipher::seal(key, &aad, value).map_err(|_| VaultError::Cipher)?;
                self.store.insert_secret(&SecretRow {
                    id,
                    scope,
                    name: ROW_NAME.to_owned(),
                    nonce: sealed.nonce.to_vec(),
                    ciphertext: sealed.ciphertext,
                    created_at: now,
                    rotated_at: now,
                    used_at: None,
                })?;
            }
        }
        Ok(())
    }

    /// `kind`'s stored settings, or `None` when it has none; only a stored row needs the key.
    ///
    /// # Errors
    /// `InvalidAgentKind`, `Locked` when settings are stored and the key will not open, `Unreadable`, or a store
    /// failure.
    pub fn agent_kind_settings(&self, kind: &str) -> Result<Option<SecretValue>, VaultError> {
        let scope = scope_of(kind)?;
        let mut guard = self.lock();
        let Some(row) = self.store.find_secret(&scope, ROW_NAME)? else {
            return Ok(None);
        };
        let key = self.key_in(&mut guard)?;
        let unreadable = || VaultError::Unreadable {
            scope: scope.clone(),
            name: ROW_NAME.to_owned(),
        };
        let aad = agent_kind_row_aad(&row).ok_or_else(unreadable)?;
        cipher::open(key, &aad, &row.nonce, &row.ciphertext)
            .map(Some)
            .map_err(|_| unreadable())
    }

    /// Deletes `kind`'s settings; whether any were stored. Needs no key, so a locked vault can clear them.
    ///
    /// # Errors
    /// `InvalidAgentKind` or a store failure.
    pub fn remove_agent_kind_settings(&self, kind: &str) -> Result<bool, VaultError> {
        let scope = scope_of(kind)?;
        let _guard = self.lock();
        match self.store.find_secret(&scope, ROW_NAME)? {
            Some(row) => Ok(self.store.delete_secret(&row.id)?),
            None => Ok(false),
        }
    }
}
