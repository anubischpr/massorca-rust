//! Variables a run receives from outside the agents domain, the profile's vault secrets (decision 0071). Values may be
//! secrets: no `Debug`, error or log line carries one, only names.

use std::fmt;
use std::sync::Arc;

use zeroize::Zeroizing;

use crate::kind::KindDecl;

/// The `MASSORCA_*` variables belong to the run itself and can never be provided.
const RESERVED_PREFIX: &str = "MASSORCA_";

/// Named values for one run, zeroed on drop. `Debug` lists the names only.
#[derive(Clone, Default)]
pub struct ProvidedEnv(Vec<(String, Zeroizing<String>)>);

impl ProvidedEnv {
    #[must_use]
    pub fn new(variables: Vec<(String, Zeroizing<String>)>) -> Self {
        Self(variables)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(name, _)| name.as_str())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for ProvidedEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.names()).finish()
    }
}

/// Why the provider could not give a run its variables. The message names causes and variable names, never a value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ProviderError(pub String);

type ProviderFn = dyn Fn(&str) -> Result<ProvidedEnv, ProviderError> + Send + Sync;

/// Gives the variables for a run in a project, by project id. Called before a launch is planned, with no lock held.
#[derive(Clone)]
pub struct EnvProvider(Arc<ProviderFn>);

impl EnvProvider {
    pub fn new(
        provide: impl Fn(&str) -> Result<ProvidedEnv, ProviderError> + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(provide))
    }

    /// The variables for a run in `project_id`.
    ///
    /// # Errors
    /// The provider's refusal, which names no value.
    pub fn provide(&self, project_id: &str) -> Result<ProvidedEnv, ProviderError> {
        (self.0)(project_id)
    }
}

impl fmt::Debug for EnvProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EnvProvider")
    }
}

/// The first provided name the run must not receive: a `MASSORCA_*` variable, or the kind's credential home variable.
/// A name in the kind's `strip_env` is allowed: providing it is an explicit grant.
#[must_use]
pub fn reserved_name<'a>(provided: &'a ProvidedEnv, decl: &KindDecl) -> Option<&'a str> {
    provided.names().find(|name| {
        has_reserved_prefix(name)
            || decl
                .credentials
                .as_ref()
                .is_some_and(|credentials| same_name(name, &credentials.home_env))
    })
}

fn has_reserved_prefix(name: &str) -> bool {
    name.get(..RESERVED_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(RESERVED_PREFIX))
}

/// Environment names compare without case on Windows.
pub(crate) fn same_name(left: &str, right: &str) -> bool {
    if cfg!(windows) {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

#[cfg(test)]
mod tests {
    use zeroize::Zeroizing;

    use super::{ProvidedEnv, has_reserved_prefix};

    #[test]
    fn debug_lists_names_and_never_values() {
        let env = ProvidedEnv::new(vec![(
            "API_KEY".to_owned(),
            Zeroizing::new("provided-value-8812".to_owned()),
        )]);
        let text = format!("{env:?}");
        assert!(text.contains("API_KEY"));
        assert!(!text.contains("provided-value-8812"));
    }

    #[test]
    fn the_massorca_prefix_is_reserved_in_any_case() {
        for reserved in ["MASSORCA_X", "massorca_token", "MASSORCA_"] {
            assert!(has_reserved_prefix(reserved), "{reserved}");
        }
        for allowed in ["MASSORC", "API_KEY", "XMASSORCA_X"] {
            assert!(!has_reserved_prefix(allowed), "{allowed}");
        }
    }
}
