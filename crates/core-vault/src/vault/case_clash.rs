//! Names that differ only by case: Windows environment variables ignore case, so one process must never receive two
//! of them. A process gets the global scope plus one project, so those are checked together.

use super::{ScopedName, Vault, VaultError};
use crate::names::Scope;

/// Whether one process can receive names from both scopes: global with global or any project, a project with itself
/// or global, a team with itself.
fn share_a_process(a: &str, b: &str) -> bool {
    a == b
        || (a == "global" && b.starts_with("project:"))
        || (b == "global" && a.starts_with("project:"))
}

/// The first pair of names that differ only by case and that one process can receive together.
pub(super) fn first_clash(names: &[ScopedName]) -> Option<Vec<ScopedName>> {
    names.iter().enumerate().find_map(|(index, a)| {
        names[index + 1..]
            .iter()
            .find(|b| {
                a.name != b.name
                    && a.name.eq_ignore_ascii_case(&b.name)
                    && share_a_process(&a.scope, &b.scope)
            })
            .map(|b| vec![a.clone(), b.clone()])
    })
}

impl Vault {
    /// Every pair of stored names, owned or linked, that differ only by case and that one process would receive
    /// together; an older build could store them.
    ///
    /// # Errors
    /// A store failure.
    pub fn name_clashes(&self) -> Result<Vec<Vec<ScopedName>>, VaultError> {
        let mut names: Vec<ScopedName> = self
            .store
            .list_secrets()?
            .into_iter()
            .map(|secret| ScopedName {
                scope: secret.scope,
                name: secret.name,
            })
            .collect();
        names.extend(
            self.store
                .list_secret_links()?
                .into_iter()
                .map(|link| ScopedName {
                    scope: link.scope,
                    name: link.name,
                }),
        );
        let mut clashes = Vec::new();
        for (index, a) in names.iter().enumerate() {
            for b in &names[index + 1..] {
                if let Some(pair) = first_clash(&[a.clone(), b.clone()]) {
                    clashes.push(pair);
                }
            }
        }
        Ok(clashes)
    }

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
