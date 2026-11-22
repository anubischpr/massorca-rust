//! The one path by which values leave the vault: the environment of a process the service starts.

use std::collections::BTreeMap;

use massorca_core_store::{SecretRow, now_ms};

use super::case_clash::first_clash;
use super::{ScopedName, Vault, VaultError};
use crate::cipher::{self, Aad};
use crate::names::Scope;
use crate::value::SecretValue;

impl Vault {
    /// The variables an agent started in `project` receives: global secrets, overridden by the project's own or
    /// linked ones of the same name. Every row opens before any is marked used.
    ///
    /// # Errors
    /// `Locked` when there are secrets and the key will not open; `Unreadable` naming a row that does not open;
    /// `NameClash` when two names differ only by case, which an older build could store.
    pub fn environment_for(&self, project: &str) -> Result<Vec<(String, SecretValue)>, VaultError> {
        let mut guard = self.lock();
        let chosen = self.chosen_rows(project)?;
        if chosen.is_empty() {
            return Ok(Vec::new());
        }
        let names: Vec<ScopedName> = chosen
            .iter()
            .map(|(name, (scope, _))| ScopedName {
                scope: scope.clone(),
                name: name.clone(),
            })
            .collect();
        if let Some(clash) = first_clash(&names) {
            return Err(VaultError::NameClash { clash });
        }
        let key = self.key_in(&mut guard)?;
        let mut variables = Vec::with_capacity(chosen.len());
        let mut opened = Vec::with_capacity(chosen.len());
        for (name, (_, row)) in chosen {
            let unreadable = || VaultError::Unreadable {
                scope: row.scope.clone(),
                name: row.name.clone(),
            };
            let owner: Scope = row.scope.parse().map_err(|_| unreadable())?;
            let aad = Aad::new(&row.id, &owner, &row.name).map_err(|_| unreadable())?;
            let value =
                cipher::open(key, &aad, &row.nonce, &row.ciphertext).map_err(|_| unreadable())?;
            opened.push(row.id);
            variables.push((name, value));
        }
        let now = now_ms();
        for id in opened {
            self.store.touch_secret(&id, now)?;
        }
        Ok(variables)
    }

    /// The row each variable name resolves to, with the scope it was found in: global first, then the project's own
    /// and linked names.
    fn chosen_rows(
        &self,
        project: &str,
    ) -> Result<BTreeMap<String, (String, SecretRow)>, VaultError> {
        let project_scope = Scope::Project(project.to_owned()).to_string();
        let mut chosen = BTreeMap::new();
        for scope in ["global", project_scope.as_str()] {
            for meta in self.store.list_secrets()? {
                if meta.scope == scope
                    && let Some(row) = self.store.secret_by_id(&meta.id)?
                {
                    chosen.insert(row.name.clone(), (scope.to_owned(), row));
                }
            }
            for link in self.store.list_secret_links()? {
                if link.scope == scope
                    && let Some(row) = self.store.secret_by_id(&link.secret_id)?
                {
                    chosen.insert(link.name.clone(), (scope.to_owned(), row));
                }
            }
        }
        Ok(chosen)
    }
}
