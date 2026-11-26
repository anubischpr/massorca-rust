//! A kind's standing launch settings (decision 0174): a program that replaces the declared ones, arguments added after
//! the kind's own and before a start's, and variables set in every run, below a start's own. They apply to every
//! terminal and chat run of the kind, resumes included, and to the kind's sign-in, account and one-shot runs, which use
//! the same program.
//!
//! The settings are kept sealed in the profile's vault, one row per kind, because an environment value may be a secret.
//! No value is ever logged, printed by `Debug`, or returned over the wire: a reader gets the variable names only.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::Duration;

use massorca_protocol::methods::agents::{KindSettings, KindSettingsSetParams};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::kind::KindDecl;
use crate::provided_env::{ProviderError, has_reserved_prefix, same_name};

/// Most standing arguments of a kind.
pub const MAX_ARGS: usize = 256;
/// Most standing variables of a kind.
pub const MAX_ENV: usize = 256;
/// Longest program, argument, name or value, in bytes.
pub const MAX_TEXT_BYTES: usize = 8 * 1024;

/// The stored settings of one kind. `Debug` lists variable names, never values; values are zeroed on drop.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandingSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

impl Drop for StandingSettings {
    fn drop(&mut self) {
        for value in self.env.values_mut() {
            value.zeroize();
        }
    }
}

impl fmt::Debug for StandingSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StandingSettings")
            .field("program", &self.program)
            .field("args", &self.args)
            .field("env", &self.env.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Why a change to a kind's settings was refused. Messages name fields and variable names, never a value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    #[error(
        "the program must be an absolute path or a bare program name, without control characters, at most 8 KiB"
    )]
    InvalidProgram,
    #[error("the arguments must be at most {MAX_ARGS}, each at most 8 KiB and without NUL")]
    InvalidArgs,
    #[error(
        "invalid variable name {0}; use a non-empty name of at most 8 KiB without `=` or control characters"
    )]
    InvalidName(String),
    #[error("{0} is set by MassOrca for every run; use another name")]
    ReservedName(String),
    #[error("the value of {0} must be at most 8 KiB and without NUL")]
    InvalidValue(String),
    #[error("a kind takes at most {MAX_ENV} variables")]
    TooManyVariables,
}

impl StandingSettings {
    /// Whether nothing is set: such settings are removed instead of stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.program.is_none() && self.args.is_empty() && self.env.is_empty()
    }

    /// The standing arguments, then the caller's: an option given at the start comes later, so it wins.
    #[must_use]
    pub fn args_with(&self, caller: &[String]) -> Vec<String> {
        self.args.iter().chain(caller).cloned().collect()
    }

    /// The standing variables with the caller's on top: a name the caller sets (compared without case on Windows)
    /// replaces the standing one.
    #[must_use]
    pub fn env_with(&self, caller: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        let mut env: BTreeMap<String, String> = self
            .env
            .iter()
            .filter(|(name, _)| !caller.keys().any(|set| same_name(set, name)))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        env.extend(
            caller
                .iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        );
        env
    }

    /// Applies `change` to these settings, checking every value it sets against `decl`.
    ///
    /// # Errors
    /// The first value refused; the settings are then unchanged.
    pub fn apply(
        &mut self,
        change: &KindSettingsSetParams,
        decl: &KindDecl,
    ) -> Result<(), SettingsError> {
        let mut next = self.clone();
        if let Some(program) = &change.program {
            next.program = if program.is_empty() {
                None
            } else {
                check_program(program)?;
                Some(program.clone())
            };
        }
        if let Some(args) = &change.args {
            if args.len() > MAX_ARGS
                || args
                    .iter()
                    .any(|arg| arg.len() > MAX_TEXT_BYTES || arg.contains('\0'))
            {
                return Err(SettingsError::InvalidArgs);
            }
            next.args.clone_from(args);
        }
        for name in &change.remove_env {
            next.env.retain(|stored, _| !same_name(stored, name));
        }
        for (name, value) in &change.env {
            check_name(name, decl)?;
            if value.len() > MAX_TEXT_BYTES || value.contains('\0') {
                return Err(SettingsError::InvalidValue(name.clone()));
            }
            next.env.retain(|stored, _| !same_name(stored, name));
            next.env.insert(name.clone(), value.clone());
        }
        if next.env.len() > MAX_ENV {
            return Err(SettingsError::TooManyVariables);
        }
        *self = next;
        Ok(())
    }

    /// What a reader sees: the program, the arguments and the variable names.
    #[must_use]
    pub fn view(&self, kind: &str) -> KindSettings {
        KindSettings {
            kind: kind.to_owned(),
            program: self.program.clone(),
            args: self.args.clone(),
            env_names: self.env.keys().cloned().collect(),
        }
    }

    /// The stored text. It holds the values, so it is zeroed on drop.
    pub(crate) fn to_text(&self) -> Zeroizing<String> {
        Zeroizing::new(serde_json::to_string(self).unwrap_or_default())
    }

    /// Settings from their stored text; `None` when the text is not settings.
    pub(crate) fn from_text(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }
}

/// An absolute path or a bare name, as the program search resolves them; no control character.
fn check_program(program: &str) -> Result<(), SettingsError> {
    let path = std::path::Path::new(program);
    let bare = !program.contains(['/', '\\']);
    if program.len() > MAX_TEXT_BYTES
        || program.chars().any(char::is_control)
        || !(bare || path.is_absolute())
    {
        return Err(SettingsError::InvalidProgram);
    }
    Ok(())
}

/// A name a process can take, and that no launch sets for itself: not `MASSORCA_*` and not the kind's credential home.
fn check_name(name: &str, decl: &KindDecl) -> Result<(), SettingsError> {
    if name.is_empty()
        || name.len() > MAX_TEXT_BYTES
        || name.contains('=')
        || name.chars().any(char::is_control)
    {
        return Err(SettingsError::InvalidName(name.to_owned()));
    }
    let credential_home = decl
        .credentials
        .as_ref()
        .is_some_and(|credentials| same_name(name, &credentials.home_env));
    if has_reserved_prefix(name) || credential_home {
        return Err(SettingsError::ReservedName(name.to_owned()));
    }
    Ok(())
}

/// Where the settings are kept: the profile's vault in the service.
pub trait SettingsVault: Send + Sync {
    /// The stored text of `kind`'s settings, or `None` when it has none.
    ///
    /// # Errors
    /// Why the vault could not read it; the message names no value.
    fn load(&self, kind: &str) -> Result<Option<Zeroizing<String>>, ProviderError>;

    /// Stores `text` as `kind`'s settings, replacing any stored before.
    ///
    /// # Errors
    /// Why the vault could not store it; the message names no value.
    fn save(&self, kind: &str, text: &str) -> Result<(), ProviderError>;

    /// Removes `kind`'s settings; whether any were stored.
    ///
    /// # Errors
    /// Why the vault could not remove them.
    fn remove(&self, kind: &str) -> Result<bool, ProviderError>;
}

/// The settings store the agents service uses. Cheap to clone; changes are serialised, so two changes of one kind never
/// lose each other's part.
#[derive(Clone)]
pub struct KindSettingsStore {
    vault: Arc<dyn SettingsVault>,
    changes: Arc<Mutex<()>>,
}

impl fmt::Debug for KindSettingsStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KindSettingsStore")
    }
}

impl KindSettingsStore {
    pub fn new(vault: impl SettingsVault + 'static) -> Self {
        Self {
            vault: Arc::new(vault),
            changes: Arc::new(Mutex::new(())),
        }
    }

    /// A store held in memory, for tests and for a service without a vault.
    #[must_use]
    pub fn in_memory() -> Self {
        Self::new(MemoryVault::default())
    }

    /// `kind`'s settings, waiting at most `timeout` for the vault; empty when it has none.
    ///
    /// # Errors
    /// The vault's refusal, a slow vault, or stored text that is not settings.
    pub fn load_bounded(
        &self,
        kind: &str,
        timeout: Duration,
    ) -> Result<StandingSettings, ProviderError> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let (vault, kind) = (Arc::clone(&self.vault), kind.to_owned());
        std::thread::Builder::new()
            .name("kind-settings".to_owned())
            .spawn(move || {
                let _ = sender.send(vault.load(&kind));
            })
            .map_err(|_| ProviderError("the vault reader could not start".to_owned()))?;
        let text = receiver.recv_timeout(timeout).map_err(|_| {
            ProviderError(format!(
                "the vault did not answer within {} seconds",
                timeout.as_secs()
            ))
        })??;
        parse(text.as_ref().map(|text| text.as_str()))
    }

    /// Reads `kind`'s settings, lets `change` edit them, and stores the result, all under the change lock. Empty settings
    /// are removed instead of stored.
    ///
    /// # Errors
    /// The vault's refusal, or `change`'s.
    pub fn update<E: From<ProviderError>>(
        &self,
        kind: &str,
        change: impl FnOnce(&mut StandingSettings) -> Result<(), E>,
    ) -> Result<StandingSettings, E> {
        let _changing = self.changes.lock().unwrap_or_else(PoisonError::into_inner);
        let stored = self.vault.load(kind)?;
        let mut settings = parse(stored.as_ref().map(|text| text.as_str()))?;
        change(&mut settings)?;
        if settings.is_empty() {
            self.vault.remove(kind)?;
        } else {
            self.vault.save(kind, &settings.to_text())?;
        }
        Ok(settings)
    }

    /// Removes `kind`'s settings; whether any were stored.
    ///
    /// # Errors
    /// The vault's refusal.
    pub fn remove(&self, kind: &str) -> Result<bool, ProviderError> {
        let _changing = self.changes.lock().unwrap_or_else(PoisonError::into_inner);
        self.vault.remove(kind)
    }

    /// `kind`'s settings, read without a time limit; empty when it has none.
    ///
    /// # Errors
    /// The vault's refusal, or stored text that is not settings.
    pub fn load(&self, kind: &str) -> Result<StandingSettings, ProviderError> {
        parse(self.vault.load(kind)?.as_ref().map(|text| text.as_str()))
    }
}

fn parse(text: Option<&str>) -> Result<StandingSettings, ProviderError> {
    match text {
        None => Ok(StandingSettings::default()),
        Some(text) => StandingSettings::from_text(text)
            .ok_or_else(|| ProviderError("the stored settings could not be read".to_owned())),
    }
}

/// Settings kept in memory, by kind.
#[derive(Default)]
struct MemoryVault(Mutex<HashMap<String, Zeroizing<String>>>);

impl SettingsVault for MemoryVault {
    fn load(&self, kind: &str) -> Result<Option<Zeroizing<String>>, ProviderError> {
        let stored = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(stored.get(kind).cloned())
    }

    fn save(&self, kind: &str, text: &str) -> Result<(), ProviderError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(kind.to_owned(), Zeroizing::new(text.to_owned()));
        Ok(())
    }

    fn remove(&self, kind: &str) -> Result<bool, ProviderError> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(kind)
            .is_some())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use massorca_protocol::methods::agents::KindSettingsSetParams;
    use serde_json::json;

    use super::{KindSettingsStore, SettingsError, StandingSettings};
    use crate::kind::KindDecl;

    const SECRET: &str = "placeholder-standing-8b2d";

    fn decl() -> KindDecl {
        KindDecl::from_json(
            &json!({
                "id": "fake",
                "name": "Fake",
                "program": ["fake"],
                "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] }
            })
            .to_string(),
        )
        .expect("parses")
    }

    fn change() -> KindSettingsSetParams {
        KindSettingsSetParams {
            kind: "fake".to_owned(),
            ..Default::default()
        }
    }

    fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn a_change_sets_replaces_and_removes_only_what_it_names() {
        let mut settings = StandingSettings::default();
        settings
            .apply(
                &KindSettingsSetParams {
                    program: Some("fake-wrapper".to_owned()),
                    args: Some(vec!["--model".to_owned(), "standing".to_owned()]),
                    env: env(&[("A", "1"), ("B", "2")]),
                    ..change()
                },
                &decl(),
            )
            .unwrap();
        settings
            .apply(
                &KindSettingsSetParams {
                    env: env(&[("B", "3")]),
                    remove_env: vec!["A".to_owned()],
                    ..change()
                },
                &decl(),
            )
            .unwrap();
        assert_eq!(settings.program.as_deref(), Some("fake-wrapper"));
        assert_eq!(settings.args, ["--model", "standing"]);
        assert_eq!(settings.env, env(&[("B", "3")]));

        settings
            .apply(
                &KindSettingsSetParams {
                    program: Some(String::new()),
                    args: Some(Vec::new()),
                    remove_env: vec!["B".to_owned()],
                    ..change()
                },
                &decl(),
            )
            .unwrap();
        assert!(settings.is_empty(), "{settings:?}");
    }

    #[test]
    fn a_refused_change_leaves_the_settings_as_they_were_and_names_no_value() {
        let mut settings = StandingSettings::default();
        settings
            .apply(
                &KindSettingsSetParams {
                    args: Some(vec!["--kept".to_owned()]),
                    ..change()
                },
                &decl(),
            )
            .unwrap();
        let before = settings.clone();
        for (refused, expected) in [
            (
                KindSettingsSetParams {
                    program: Some("relative/agent".to_owned()),
                    ..change()
                },
                SettingsError::InvalidProgram,
            ),
            (
                KindSettingsSetParams {
                    args: Some(vec!["a\0b".to_owned()]),
                    ..change()
                },
                SettingsError::InvalidArgs,
            ),
            (
                KindSettingsSetParams {
                    env: env(&[("MASSORCA_SESSION_ID", SECRET)]),
                    ..change()
                },
                SettingsError::ReservedName("MASSORCA_SESSION_ID".to_owned()),
            ),
            (
                KindSettingsSetParams {
                    env: env(&[("FAKE_AGENT_HOME", SECRET)]),
                    ..change()
                },
                SettingsError::ReservedName("FAKE_AGENT_HOME".to_owned()),
            ),
            (
                KindSettingsSetParams {
                    env: env(&[("A=B", SECRET)]),
                    ..change()
                },
                SettingsError::InvalidName("A=B".to_owned()),
            ),
            (
                KindSettingsSetParams {
                    args: Some(vec!["--changed".to_owned()]),
                    env: env(&[("TOKEN", "with\0nul")]),
                    ..change()
                },
                SettingsError::InvalidValue("TOKEN".to_owned()),
            ),
        ] {
            let error = settings.apply(&refused, &decl()).unwrap_err();
            assert_eq!(error, expected);
            assert!(!error.to_string().contains(SECRET), "{error}");
            assert_eq!(settings, before, "a refused change changes nothing");
        }
    }

    #[test]
    fn the_caller_comes_after_the_standing_arguments_and_wins_each_variable() {
        let settings = StandingSettings {
            program: None,
            args: vec!["--model".to_owned(), "standing".to_owned()],
            env: env(&[("SHARED", "standing"), ("ONLY_STANDING", "s")]),
        };
        assert_eq!(
            settings.args_with(&["--model".to_owned(), "caller".to_owned()]),
            ["--model", "standing", "--model", "caller"]
        );
        let merged = settings.env_with(&env(&[("SHARED", "caller")]));
        assert_eq!(merged, env(&[("SHARED", "caller"), ("ONLY_STANDING", "s")]));
    }

    #[test]
    fn debug_and_the_view_show_names_and_never_values() {
        let settings = StandingSettings {
            program: Some("fake".to_owned()),
            args: Vec::new(),
            env: env(&[("API_TOKEN", SECRET)]),
        };
        let shown = format!("{settings:?}");
        assert!(
            shown.contains("API_TOKEN") && !shown.contains(SECRET),
            "{shown}"
        );
        let view = serde_json::to_string(&settings.view("fake")).unwrap();
        assert!(
            view.contains("API_TOKEN") && !view.contains(SECRET),
            "{view}"
        );
    }

    #[test]
    fn the_store_keeps_settings_and_removes_empty_ones() {
        let store = KindSettingsStore::in_memory();
        let timeout = Duration::from_secs(5);
        assert!(store.load_bounded("fake", timeout).unwrap().is_empty());
        store
            .update::<super::ProviderError>("fake", |settings| {
                settings.args = vec!["--x".to_owned()];
                Ok(())
            })
            .unwrap();
        assert_eq!(store.load_bounded("fake", timeout).unwrap().args, ["--x"]);
        store
            .update::<super::ProviderError>("fake", |settings| {
                settings.args.clear();
                Ok(())
            })
            .unwrap();
        assert!(
            !store.remove("fake").unwrap(),
            "empty settings were removed, not stored"
        );
    }
}
