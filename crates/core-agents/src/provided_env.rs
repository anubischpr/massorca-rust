//! Variables a run receives from outside the agents domain, the profile's vault secrets (decision 0071). Values may be
//! secrets: no `Debug`, error or log line carries one, only names. Only [`ProvidedEnv`] is zeroed on drop: the values
//! are then copied into ordinary strings in `LaunchPlan::env` and the terminal's spawn spec.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use massorca_core_vault::is_dangerous_env_name;
use zeroize::Zeroizing;

use crate::kind::KindDecl;

/// How long a launch waits for the provider before it fails with `agent.secrets_unavailable`: a slow keyring must not
/// hold the start gate, and so every other start, for longer.
pub const PROVIDER_TIMEOUT: Duration = Duration::from_secs(10);

/// Whether a run may receive the vault's variables: only when its caller holds host-admin scope (decision 0071).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretGrant {
    Provide,
    /// The run starts without vault variables; the reason is shown to the caller.
    Withhold(&'static str),
}

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

/// Gives the variables for a run in a project, by project id. Called before a launch is planned, under the start gate
/// only, before the homes gate and the status lock, and waited for at most [`PROVIDER_TIMEOUT`]; a slow provider
/// therefore delays other starts by at most that long.
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

/// Calls `provider` on its own thread and waits at most `timeout`. A provider that answers later is ignored.
pub(crate) fn provide_bounded(
    provider: &EnvProvider,
    project_id: &str,
    timeout: Duration,
) -> Result<ProvidedEnv, ProviderError> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let (provider, project_id) = (provider.clone(), project_id.to_owned());
    std::thread::Builder::new()
        .name("vault-env".to_owned())
        .spawn(move || {
            let _ = sender.send(provider.provide(&project_id));
        })
        .map_err(|_| ProviderError("the vault reader could not start".to_owned()))?;
    receiver.recv_timeout(timeout).map_err(|_| {
        ProviderError(format!(
            "the vault did not answer within {} seconds",
            timeout.as_secs()
        ))
    })?
}

impl fmt::Debug for EnvProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EnvProvider")
    }
}

/// The first provided name the run must not receive: a `MASSORCA_*` variable, the kind's credential home variable, or
/// a name that loads code, redirects lookups or swaps an endpoint ([`is_dangerous_env_name`]). A name in the kind's
/// `strip_env` is allowed: providing it is an explicit grant.
#[must_use]
pub fn reserved_name<'a>(provided: &'a ProvidedEnv, decl: &KindDecl) -> Option<&'a str> {
    provided.names().find(|name| {
        has_reserved_prefix(name)
            || is_dangerous_env_name(name)
            || decl
                .credentials
                .as_ref()
                .is_some_and(|credentials| same_name(name, &credentials.home_env))
    })
}

/// The first name the caller's env sets that a run holding secrets may not receive ([`is_dangerous_env_name`]).
#[must_use]
pub fn dangerous_caller_name(caller_env: &BTreeMap<String, String>) -> Option<&str> {
    caller_env
        .keys()
        .map(String::as_str)
        .find(|name| is_dangerous_env_name(name))
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

    use std::time::Duration;

    use super::{EnvProvider, ProvidedEnv, has_reserved_prefix, provide_bounded};

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
    fn a_provider_slower_than_the_timeout_fails_naming_the_wait() {
        let slow = EnvProvider::new(|_| {
            std::thread::sleep(Duration::from_secs(2));
            Ok(ProvidedEnv::default())
        });
        let error = provide_bounded(&slow, "p", Duration::from_millis(50)).unwrap_err();
        assert!(error.0.contains("did not answer"), "{error}");
        let quick = EnvProvider::new(|_| Ok(ProvidedEnv::default()));
        assert!(provide_bounded(&quick, "p", Duration::from_secs(5)).is_ok());
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
