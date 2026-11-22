//! The vault of one profile (decision 0071): owned secrets, links and requests over the store, with values sealed by
//! the profile's master key. No method returns a value except [`Vault::environment_for`], for processes the service
//! starts.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use massorca_core_store::{
    SecretLinkRow, SecretMeta, SecretRow, Store, StoreError, new_id, now_ms,
};

use crate::cipher::{self, Aad};
use crate::key::{KeyError, LockReason, MasterKey, Protection};
use crate::names::{Scope, is_valid_name};
use crate::value::SecretValue;

mod case_clash;
mod environment;
mod keys;
mod links;
mod requests;

pub use requests::{MAX_PENDING_REQUESTS, MAX_REQUESTS_PER_MINUTE};

use keys::{Recorded, recorded};

/// Why a vault call was refused. Messages name scopes and names only, never a value.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("the vault is locked: its key will not open ({0:?})")]
    Locked(LockReason),
    #[error("there is no secret {name} in {scope}")]
    NotFound { scope: String, name: String },
    #[error("{name} already exists in {scope}")]
    NameInUse { scope: String, name: String },
    #[error("{name} in {scope} is linked from {owner}; change it there")]
    Linked {
        scope: String,
        name: String,
        owner: String,
    },
    #[error("{name} in {scope} is linked by {}; remove the links too, or unlink them first", .linked.join(", "))]
    HasLinks {
        scope: String,
        name: String,
        linked: Vec<String>,
    },
    #[error(
        "invalid secret name; use 1 to 64 letters, digits and underscores, not starting with a digit"
    )]
    InvalidName,
    #[error("invalid scope; use global, project:<id> or team:<id>")]
    InvalidScope,
    #[error("the value is longer than 64 KiB")]
    TooLarge,
    #[error("the reason must be plain text of at most 200 characters")]
    InvalidReason,
    #[error("the session must be plain text of at most 128 characters")]
    InvalidSession,
    #[error("{name} in {scope} differs only by case from {other} in {other_scope}")]
    NameCaseClash {
        scope: String,
        name: String,
        other_scope: String,
        other: String,
    },
    #[error("the vault holds {count} secrets; remove them before resetting its key")]
    VaultNotEmpty { count: u64 },
    #[error("too many secret requests: {0}")]
    RequestLimit(&'static str),
    #[error("{name} in {scope} could not be opened; set it again with replace")]
    Unreadable { scope: String, name: String },
    #[error("there is no secret request {0}")]
    RequestNotFound(String),
    #[error("secret request {id} is already {state}")]
    RequestResolved { id: String, state: String },
    #[error("the vault key could not be created: {0}")]
    Key(KeyError),
    #[error("the secret could not be sealed or opened")]
    Cipher,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The vault's key protection and whether it opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultStatus {
    /// `None` until the first secret creates the key.
    pub protection: Option<Protection>,
    pub locked: bool,
    pub count: u64,
}

/// One name in one scope, owned or linked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultEntry {
    pub scope: String,
    pub name: String,
    pub owner_scope: String,
    pub linked: bool,
    pub linked_scopes: Vec<String>,
    pub created_at: i64,
    pub rotated_at: i64,
    pub used_at: Option<i64>,
}

/// What a write changed: the entry, and the pending requests it resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub entry: VaultEntry,
    pub resolved_requests: Vec<String>,
}

/// The vault of one profile. Writes are serialised; the key opens on first use and is created by the first secret.
#[derive(Debug)]
pub struct Vault {
    pub(super) store: Store,
    dir: PathBuf,
    force_file: bool,
    key: Mutex<Option<MasterKey>>,
}

impl Vault {
    /// A vault whose key lives in `dir`. `force_file` keeps a new key in an owner-only file.
    #[must_use]
    pub fn new(store: Store, dir: PathBuf, force_file: bool) -> Self {
        Self {
            store,
            dir,
            force_file,
            key: Mutex::new(None),
        }
    }

    /// The key's protection, whether it opens, and the number of owned secrets. Never creates a key.
    ///
    /// # Errors
    /// A store failure.
    pub fn status(&self) -> Result<VaultStatus, VaultError> {
        let mut guard = self.lock();
        let count = u64::try_from(self.store.list_secrets()?.len()).unwrap_or(u64::MAX);
        let recorded = recorded(&self.dir);
        let protection = match recorded {
            Recorded::Protection(protection) => Some(protection),
            Recorded::Nothing | Recorded::Unreadable => None,
        };
        let locked = match recorded {
            // Secrets without a recorded key were sealed by a key that is gone.
            Recorded::Nothing => count > 0,
            Recorded::Protection(_) | Recorded::Unreadable => self.key_in(&mut guard).is_err(),
        };
        Ok(VaultStatus {
            protection,
            locked,
            count,
        })
    }

    /// Every name, owned and linked, in `scope` or in all scopes.
    ///
    /// # Errors
    /// A store failure.
    pub fn list(&self, scope: Option<&Scope>) -> Result<Vec<VaultEntry>, VaultError> {
        let secrets = self.store.list_secrets()?;
        let links = self.store.list_secret_links()?;
        let by_id: BTreeMap<&str, &SecretMeta> =
            secrets.iter().map(|s| (s.id.as_str(), s)).collect();
        let mut entries = Vec::new();
        for secret in &secrets {
            let linked_scopes = links
                .iter()
                .filter(|link| link.secret_id == secret.id)
                .map(|link| link.scope.clone())
                .collect();
            entries.push(owned_entry(secret, linked_scopes));
        }
        for link in &links {
            if let Some(owner) = by_id.get(link.secret_id.as_str()) {
                entries.push(linked_entry(link, owner));
            }
        }
        let wanted = scope.map(ToString::to_string);
        entries.retain(|entry| wanted.as_ref().is_none_or(|scope| &entry.scope == scope));
        entries.sort_by(|a, b| (&a.scope, &a.name).cmp(&(&b.scope, &b.name)));
        Ok(entries)
    }

    /// Creates `name` in `scope`, or rotates it with `replace`. Resolves pending requests for it as stored.
    ///
    /// # Errors
    /// `NameInUse`, `NameCaseClash`, `Linked`, `NotFound` (replace of a missing name), `Locked`, or a store or key
    /// failure.
    pub fn set(
        &self,
        scope: &Scope,
        name: &str,
        value: &SecretValue,
        replace: bool,
    ) -> Result<Written, VaultError> {
        check_name(name)?;
        let mut guard = self.lock();
        let (scope_text, now) = (scope.to_string(), now_ms());
        if let Some(link) = self.store.find_secret_link(&scope_text, name)? {
            let owner = self.owner_scope(&link)?;
            return Err(VaultError::Linked {
                scope: scope_text,
                name: name.to_owned(),
                owner,
            });
        }
        let existing = self.store.find_secret(&scope_text, name)?;
        match (existing, replace) {
            (Some(_), false) => {
                return Err(VaultError::NameInUse {
                    scope: scope_text,
                    name: name.to_owned(),
                });
            }
            (None, true) => {
                return Err(VaultError::NotFound {
                    scope: scope_text,
                    name: name.to_owned(),
                });
            }
            (Some(row), true) => {
                let key = self.key_or_create(&mut guard)?;
                let sealed = seal_row(key, &row.id, scope, name, value)?;
                self.store
                    .update_secret_value(&row.id, &sealed.nonce, &sealed.ciphertext, now)?;
            }
            (None, false) => {
                self.check_case_clash(scope, name)?;
                let key = self.key_or_create(&mut guard)?;
                let id = new_id();
                let sealed = seal_row(key, &id, scope, name, value)?;
                self.store.insert_secret(&SecretRow {
                    id,
                    scope: scope_text.clone(),
                    name: name.to_owned(),
                    nonce: sealed.nonce.to_vec(),
                    ciphertext: sealed.ciphertext,
                    created_at: now,
                    rotated_at: now,
                    used_at: None,
                })?;
            }
        }
        let resolved = self
            .store
            .resolve_secret_requests_for(&scope_text, name, "stored", now)?;
        drop(guard);
        Ok(Written {
            entry: self.entry(&scope_text, name)?,
            resolved_requests: resolved,
        })
    }

    /// Removes the owned secret `name` in `scope` and returns the scopes whose links went with it. A secret that
    /// others link to needs `include_links`; a locked vault needs `force`, since nothing can be checked.
    ///
    /// # Errors
    /// `NotFound`, `Linked` (the name is a link here), `HasLinks`, `Locked`, or a store failure.
    pub fn remove(
        &self,
        scope: &Scope,
        name: &str,
        include_links: bool,
        force: bool,
    ) -> Result<Vec<String>, VaultError> {
        let mut guard = self.lock();
        let scope_text = scope.to_string();
        let Some(secret) = self.store.find_secret(&scope_text, name)? else {
            if let Some(link) = self.store.find_secret_link(&scope_text, name)? {
                return Err(VaultError::Linked {
                    owner: self.owner_scope(&link)?,
                    scope: scope_text,
                    name: name.to_owned(),
                });
            }
            return Err(VaultError::NotFound {
                scope: scope_text,
                name: name.to_owned(),
            });
        };
        if !force {
            self.key_in(&mut guard)?;
        }
        let linked: Vec<String> = self
            .store
            .list_secret_links()?
            .into_iter()
            .filter(|link| link.secret_id == secret.id)
            .map(|link| link.scope)
            .collect();
        if !linked.is_empty() && !include_links {
            return Err(VaultError::HasLinks {
                scope: scope_text,
                name: name.to_owned(),
                linked,
            });
        }
        self.store.delete_secret(&secret.id)?;
        Ok(linked)
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, Option<MasterKey>> {
        self.key.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn owner_scope(&self, link: &SecretLinkRow) -> Result<String, VaultError> {
        Ok(self
            .store
            .secret_by_id(&link.secret_id)?
            .map_or_else(String::new, |row| row.scope))
    }

    pub(super) fn entry(&self, scope: &str, name: &str) -> Result<VaultEntry, VaultError> {
        self.list(None)?
            .into_iter()
            .find(|entry| entry.scope == scope && entry.name == name)
            .ok_or_else(|| VaultError::NotFound {
                scope: scope.to_owned(),
                name: name.to_owned(),
            })
    }
}

pub(super) fn check_name(name: &str) -> Result<(), VaultError> {
    if is_valid_name(name) {
        Ok(())
    } else {
        Err(VaultError::InvalidName)
    }
}

fn seal_row(
    key: &MasterKey,
    id: &str,
    scope: &Scope,
    name: &str,
    value: &SecretValue,
) -> Result<cipher::Sealed, VaultError> {
    let aad = Aad::new(id, scope, name).map_err(|_| VaultError::InvalidScope)?;
    cipher::seal(key, &aad, value).map_err(|_| VaultError::Cipher)
}

fn owned_entry(secret: &SecretMeta, linked_scopes: Vec<String>) -> VaultEntry {
    VaultEntry {
        scope: secret.scope.clone(),
        name: secret.name.clone(),
        owner_scope: secret.scope.clone(),
        linked: false,
        linked_scopes,
        created_at: secret.created_at,
        rotated_at: secret.rotated_at,
        used_at: secret.used_at,
    }
}

fn linked_entry(link: &SecretLinkRow, owner: &SecretMeta) -> VaultEntry {
    VaultEntry {
        scope: link.scope.clone(),
        name: link.name.clone(),
        owner_scope: owner.scope.clone(),
        linked: true,
        linked_scopes: Vec::new(),
        created_at: link.linked_at,
        rotated_at: owner.rotated_at,
        used_at: owner.used_at,
    }
}
