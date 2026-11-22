//! Links: another scope uses an owned secret under the same name, never a copy.

use massorca_core_store::{SecretLinkRow, now_ms};

use super::{Vault, VaultError, Written, check_name};
use crate::names::Scope;

impl Vault {
    /// Links `name` in `scope` to the secret of the same name owned by `from`.
    ///
    /// # Errors
    /// `NotFound` when `from` owns no such secret, `NameInUse` when `scope` already has the name, or `Locked`.
    pub fn link(&self, scope: &Scope, name: &str, from: &Scope) -> Result<Written, VaultError> {
        check_name(name)?;
        let mut guard = self.lock();
        // A locked vault takes no writes, links included.
        self.key_in(&mut guard)?;
        let (scope_text, from_text, now) = (scope.to_string(), from.to_string(), now_ms());
        let owner =
            self.store
                .find_secret(&from_text, name)?
                .ok_or_else(|| VaultError::NotFound {
                    scope: from_text.clone(),
                    name: name.to_owned(),
                })?;
        if scope_text == from_text
            || self.store.find_secret(&scope_text, name)?.is_some()
            || self.store.find_secret_link(&scope_text, name)?.is_some()
        {
            return Err(VaultError::NameInUse {
                scope: scope_text,
                name: name.to_owned(),
            });
        }
        self.store.insert_secret_link(&SecretLinkRow {
            scope: scope_text.clone(),
            name: name.to_owned(),
            secret_id: owner.id,
            linked_at: now,
        })?;
        let resolved = self
            .store
            .resolve_secret_requests_for(&scope_text, name, "linked", now)?;
        drop(guard);
        Ok(Written {
            entry: self.entry(&scope_text, name)?,
            resolved_requests: resolved,
        })
    }

    /// Removes the link `name` in `scope`; the owner's secret stays.
    ///
    /// # Errors
    /// `NotFound` when `scope` has no link of that name.
    pub fn unlink(&self, scope: &Scope, name: &str) -> Result<(), VaultError> {
        let _guard = self.lock();
        let scope_text = scope.to_string();
        if self.store.delete_secret_link(&scope_text, name)? {
            Ok(())
        } else {
            Err(VaultError::NotFound {
                scope: scope_text,
                name: name.to_owned(),
            })
        }
    }
}
