//! The one path by which values leave the vault: the environment of a process the service starts.

use std::collections::BTreeMap;

use massorca_core_store::{SecretRow, now_ms};

use super::{Vault, VaultError};
use crate::cipher::{self, Aad};
use crate::names::Scope;
use crate::value::SecretValue;

impl Vault {
    /// The variables an agent started in `project` receives: global secrets, overridden by the project's own or
    /// linked ones of the same name. Every row opens before any is marked used.
    ///
    /// # Errors
    /// `Locked` when there are secrets and the key will not open; `Unreadable` naming a row that does not open.
    pub fn environment_for(&self, project: &str) -> Result<Vec<(String, SecretValue)>, VaultError> {
        let mut guard = self.lock();
        let chosen = self.chosen_rows(project)?;
        if chosen.is_empty() {
            return Ok(Vec::new());
        }
        let key = self.key_in(&mut guard)?;
        let mut variables = Vec::with_capacity(chosen.len());
        let mut opened = Vec::with_capacity(chosen.len());
        for (name, row) in chosen {
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

    /// The row each variable name resolves to: global first, then the project's own and linked names.
    fn chosen_rows(&self, project: &str) -> Result<BTreeMap<String, SecretRow>, VaultError> {
        let project_scope = Scope::Project(project.to_owned()).to_string();
        let mut chosen = BTreeMap::new();
        for scope in ["global", project_scope.as_str()] {
            for meta in self.store.list_secrets()? {
                if meta.scope == scope
                    && let Some(row) = self.store.secret_by_id(&meta.id)?
                {
                    chosen.insert(row.name.clone(), row);
                }
            }
            for link in self.store.list_secret_links()? {
                if link.scope == scope
                    && let Some(row) = self.store.secret_by_id(&link.secret_id)?
                {
                    chosen.insert(link.name.clone(), row);
                }
            }
        }
        Ok(chosen)
    }
}
