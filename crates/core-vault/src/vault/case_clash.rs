//! Names that differ only by case: Windows environment variables ignore case, so one process must never receive two
//! of them. A process gets the global scope plus one project, so those are checked together.

use super::{Vault, VaultError};
use crate::names::Scope;

impl Vault {
    /// Refuses `name` in `scope` when an owned or linked name that one process could receive alongside it differs
    /// from it only by case.
    pub(super) fn check_case_clash(&self, scope: &Scope, name: &str) -> Result<(), VaultError> {
        let scope_text = scope.to_string();
        let shares_a_process = |other: &str| match scope {
            Scope::Global => other == "global" || other.starts_with("project:"),
            Scope::Project(_) => other == "global" || other == scope_text,
            Scope::Team(_) => other == scope_text,
        };
        let secrets = self.store.list_secrets()?;
        let links = self.store.list_secret_links()?;
        let names = secrets
            .iter()
            .map(|secret| (&secret.scope, &secret.name))
            .chain(links.iter().map(|link| (&link.scope, &link.name)));
        for (other_scope, other) in names {
            if other != name && other.eq_ignore_ascii_case(name) && shares_a_process(other_scope) {
                return Err(VaultError::NameCaseClash {
                    scope: scope_text,
                    name: name.to_owned(),
                    other_scope: other_scope.clone(),
                    other: other.clone(),
                });
            }
        }
        Ok(())
    }
}
