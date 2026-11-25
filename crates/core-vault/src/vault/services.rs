//! Hosted-service connections (decision 0094): a token, and optionally a base URL and an email, per service. They are
//! sealed rows of the `secrets` table under the scope `service:<id>`, which no `secret.*` scope can name. The token
//! is write-only, and no agent receives any of them.

use std::fmt;

use massorca_core_store::{SecretRow, new_id, now_ms};

use super::{Vault, VaultError};
use crate::cipher::{self, Aad};
use crate::key::MasterKey;
use crate::names::is_plain;
use crate::value::SecretValue;

const SCOPE_PREFIX: &str = "service:";
const MAX_SERVICE_CHARS: usize = 64;
const MAX_BASE_URL_CHARS: usize = 2048;
const MAX_EMAIL_CHARS: usize = 320;

/// One entry kind of a service connection, stored as its own sealed row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceField {
    Token,
    BaseUrl,
    Email,
}

impl ServiceField {
    const ALL: [Self; 3] = [Self::Token, Self::BaseUrl, Self::Email];

    /// The row's name in the `secrets` table.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Token => "TOKEN",
            Self::BaseUrl => "BASE_URL",
            Self::Email => "EMAIL",
        }
    }
}

impl fmt::Display for ServiceField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Token => "token",
            Self::BaseUrl => "base URL",
            Self::Email => "email",
        })
    }
}

/// What a connect stores. A field left out clears the one stored before.
#[derive(Debug)]
pub struct ServiceConnection<'a> {
    pub token: &'a SecretValue,
    pub base_url: Option<&'a str>,
    pub email: Option<&'a str>,
}

/// A stored connection as a client may see it: never the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredService {
    pub base_url: Option<String>,
    pub email: Option<String>,
    /// When the token was last written, in epoch milliseconds.
    pub updated_at: i64,
}

/// Whether a stored row's scope is a service connection's.
pub(super) fn is_service_scope(scope: &str) -> bool {
    scope.starts_with(SCOPE_PREFIX)
}

/// The associated data of a service row, or `None` for a row of another scope or a malformed one.
pub(super) fn service_row_aad(row: &SecretRow) -> Option<Aad> {
    let service = row.scope.strip_prefix(SCOPE_PREFIX)?;
    let known = ServiceField::ALL
        .iter()
        .any(|field| field.name() == row.name);
    (is_valid_service(service) && known && !row.id.is_empty() && !row.id.contains('\0'))
        .then(|| Aad::for_service_row(&row.id, &row.scope, &row.name))
}

/// 1 to 64 lowercase ASCII letters, digits and `-`, starting with a letter.
fn is_valid_service(service: &str) -> bool {
    service.len() <= MAX_SERVICE_CHARS
        && service.starts_with(|c: char| c.is_ascii_lowercase())
        && service
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn scope_of(service: &str) -> Result<String, VaultError> {
    if is_valid_service(service) {
        Ok(format!("{SCOPE_PREFIX}{service}"))
    } else {
        Err(VaultError::InvalidService)
    }
}

fn plain_field(
    field: ServiceField,
    text: Option<&str>,
    max_chars: usize,
) -> Result<Option<SecretValue>, VaultError> {
    let Some(text) = text else {
        return Ok(None);
    };
    if text.is_empty() || !is_plain(text, max_chars) {
        return Err(VaultError::InvalidServiceField(field));
    }
    SecretValue::new(text.to_owned())
        .map(Some)
        .map_err(|_| VaultError::InvalidServiceField(field))
}

impl Vault {
    /// Stores `service`'s connection, replacing every field stored before. Creates the key on first use.
    ///
    /// # Errors
    /// `InvalidService`, `InvalidServiceField`, `Locked`, or a store or key failure.
    pub fn set_service(
        &self,
        service: &str,
        connection: &ServiceConnection<'_>,
    ) -> Result<(), VaultError> {
        let scope = scope_of(service)?;
        let base_url = plain_field(
            ServiceField::BaseUrl,
            connection.base_url,
            MAX_BASE_URL_CHARS,
        )?;
        let email = plain_field(ServiceField::Email, connection.email, MAX_EMAIL_CHARS)?;
        let mut guard = self.lock();
        let key = self.key_or_create(&mut guard)?;
        let now = now_ms();
        self.put(
            key,
            &scope,
            ServiceField::Token,
            Some(connection.token),
            now,
        )?;
        self.put(key, &scope, ServiceField::BaseUrl, base_url.as_ref(), now)?;
        self.put(key, &scope, ServiceField::Email, email.as_ref(), now)?;
        Ok(())
    }

    /// `service`'s stored connection without its token, or `None` when no token is stored.
    ///
    /// # Errors
    /// `InvalidService`, `Locked` when a connection is stored and the key will not open, `Unreadable`, or a store
    /// failure.
    pub fn service(&self, service: &str) -> Result<Option<StoredService>, VaultError> {
        let scope = scope_of(service)?;
        let mut guard = self.lock();
        let Some(token) = self.store.find_secret(&scope, ServiceField::Token.name())? else {
            return Ok(None);
        };
        let key = self.key_in(&mut guard)?;
        Ok(Some(StoredService {
            base_url: self.open_field(key, &scope, ServiceField::BaseUrl)?,
            email: self.open_field(key, &scope, ServiceField::Email)?,
            updated_at: token.rotated_at,
        }))
    }

    /// Deletes every entry of `service`; whether anything was stored. Needs no key, so a locked vault can disconnect.
    ///
    /// # Errors
    /// `InvalidService` or a store failure.
    pub fn remove_service(&self, service: &str) -> Result<bool, VaultError> {
        let scope = scope_of(service)?;
        let _guard = self.lock();
        let mut removed = false;
        for field in ServiceField::ALL {
            if let Some(row) = self.store.find_secret(&scope, field.name())? {
                removed |= self.store.delete_secret(&row.id)?;
            }
        }
        Ok(removed)
    }

    fn put(
        &self,
        key: &MasterKey,
        scope: &str,
        field: ServiceField,
        value: Option<&SecretValue>,
        now: i64,
    ) -> Result<(), VaultError> {
        let name = field.name();
        let existing = self.store.find_secret(scope, name)?;
        match (existing, value) {
            (Some(row), Some(value)) => {
                let aad = Aad::for_service_row(&row.id, scope, name);
                let sealed = cipher::seal(key, &aad, value).map_err(|_| VaultError::Cipher)?;
                self.store
                    .update_secret_value(&row.id, &sealed.nonce, &sealed.ciphertext, now)?;
            }
            (None, Some(value)) => {
                let id = new_id();
                let aad = Aad::for_service_row(&id, scope, name);
                let sealed = cipher::seal(key, &aad, value).map_err(|_| VaultError::Cipher)?;
                self.store.insert_secret(&SecretRow {
                    id,
                    scope: scope.to_owned(),
                    name: name.to_owned(),
                    nonce: sealed.nonce.to_vec(),
                    ciphertext: sealed.ciphertext,
                    created_at: now,
                    rotated_at: now,
                    used_at: None,
                })?;
            }
            (Some(row), None) => {
                self.store.delete_secret(&row.id)?;
            }
            (None, None) => {}
        }
        Ok(())
    }

    fn open_field(
        &self,
        key: &MasterKey,
        scope: &str,
        field: ServiceField,
    ) -> Result<Option<String>, VaultError> {
        debug_assert!(field != ServiceField::Token, "the token is write-only");
        let Some(row) = self.store.find_secret(scope, field.name())? else {
            return Ok(None);
        };
        let unreadable = || VaultError::Unreadable {
            scope: scope.to_owned(),
            name: field.name().to_owned(),
        };
        let aad = service_row_aad(&row).ok_or_else(unreadable)?;
        let value =
            cipher::open(key, &aad, &row.nonce, &row.ciphertext).map_err(|_| unreadable())?;
        Ok(Some(value.expose().to_owned()))
    }
}
