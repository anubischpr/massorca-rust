//! `agent.settings.*` (decision 0174): a kind's standing launch settings, and the program a run of the kind uses.

use std::path::PathBuf;

use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    KindSettings, KindSettingsCleared, KindSettingsSetParams,
};

use super::{AgentError, Agents};
use crate::kind::{KindDecl, valid_id};
use crate::kind_settings::{KindSettingsStore, SettingsError, StandingSettings};
use crate::provided_env::{PROVIDER_TIMEOUT, ProviderError};
use crate::resolve::{resolve, resolve_first};

impl From<ProviderError> for AgentError {
    fn from(error: ProviderError) -> Self {
        unavailable(&error)
    }
}

impl From<SettingsError> for AgentError {
    fn from(error: SettingsError) -> Self {
        AgentError::new(codes::REQUEST_INVALID_PARAMS, error.to_string())
    }
}

fn unavailable(error: &ProviderError) -> AgentError {
    AgentError::new(
        codes::AGENT_SECRETS_UNAVAILABLE,
        format!("the kind's launch settings are unavailable: {error}"),
    )
}

/// A kind id the settings can be kept under; one that is not refused as an unknown kind.
fn checked_id(kind: &str) -> Result<(), AgentError> {
    if valid_id(kind) {
        Ok(())
    } else {
        Err(AgentError::new(
            codes::AGENT_KIND_NOT_FOUND,
            format!("no agent kind {kind}"),
        ))
    }
}

impl Agents {
    fn settings_store(&self) -> Result<&KindSettingsStore, AgentError> {
        self.inner.config.kind_settings.as_ref().ok_or_else(|| {
            AgentError::new(
                codes::AGENT_SECRETS_UNAVAILABLE,
                "this service keeps no launch settings for agent kinds",
            )
        })
    }

    /// `agent.settings.get`: `kind`'s standing settings, with variable names only. Answers for a kind that is no longer
    /// declared too, so leftover settings can be seen and cleared.
    ///
    /// # Errors
    /// `agent.kind_not_found` for an id no kind can have; `agent.secrets_unavailable` when the vault cannot read them.
    pub fn kind_settings(&self, kind: &str) -> Result<KindSettings, AgentError> {
        checked_id(kind)?;
        let Some(store) = &self.inner.config.kind_settings else {
            return Ok(StandingSettings::default().view(kind));
        };
        Ok(store.load(kind)?.view(kind))
    }

    /// `agent.settings.set`: changes a declared kind's standing settings, and returns them as a reader sees them.
    ///
    /// # Errors
    /// `agent.kind_not_found`; `request.invalid_params` naming the refused field, never a value; `agent.not_installed`
    /// when the program does not resolve on the program search now; `agent.secrets_unavailable` when the vault cannot
    /// read or store them.
    pub fn set_kind_settings(
        &self,
        change: &KindSettingsSetParams,
    ) -> Result<KindSettings, AgentError> {
        checked_id(&change.kind)?;
        let decl = self.declared_kind(&change.kind)?;
        let store = self.settings_store()?;
        let settings = store.update::<AgentError>(&change.kind, |settings| {
            settings.apply(change, &decl)?;
            // A new program is checked where the runs start, as a start checks it; every start checks it again.
            if change
                .program
                .as_deref()
                .is_some_and(|program| !program.is_empty())
            {
                self.program_for(&decl, settings)?;
            }
            Ok(())
        })?;
        Ok(settings.view(&change.kind))
    }

    /// `agent.settings.clear`: removes `kind`'s standing settings, declared or not.
    ///
    /// # Errors
    /// `agent.kind_not_found` for an id no kind can have; `agent.secrets_unavailable` when the vault cannot remove them.
    pub fn clear_kind_settings(&self, kind: &str) -> Result<KindSettingsCleared, AgentError> {
        checked_id(kind)?;
        let removed = match &self.inner.config.kind_settings {
            Some(store) => store.remove(kind)?,
            None => false,
        };
        Ok(KindSettingsCleared {
            kind: kind.to_owned(),
            removed,
        })
    }

    /// `kind`'s standing settings for a run, waiting at most [`PROVIDER_TIMEOUT`] for the vault.
    pub(crate) fn standing_settings(&self, kind: &str) -> Result<StandingSettings, AgentError> {
        match &self.inner.config.kind_settings {
            Some(store) => Ok(store.load_bounded(kind, PROVIDER_TIMEOUT)?),
            None => Ok(StandingSettings::default()),
        }
    }

    /// The program a run of `decl` uses: the standing one when the settings name one, else the first declared one that
    /// resolves.
    pub(crate) fn program_for(
        &self,
        decl: &KindDecl,
        settings: &StandingSettings,
    ) -> Result<PathBuf, AgentError> {
        let search = self.program_search();
        match &settings.program {
            Some(program) => resolve(program, &search).ok_or_else(|| program_not_found(&decl.id)),
            None => resolve_first(&decl.program, &search).ok_or_else(|| {
                AgentError::new(
                    codes::AGENT_NOT_INSTALLED,
                    format!(
                        "agent kind {} is not installed: none of its programs was found",
                        decl.id
                    ),
                )
            }),
        }
    }
}

/// The program the settings name does not resolve. The message never repeats it: a path may hold a name to keep.
fn program_not_found(kind: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_NOT_INSTALLED,
        format!(
            "agent kind {kind} is not installed: the program its launch settings name is not found or not runnable"
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use massorca_protocol::codes;
    use massorca_protocol::methods::agents::KindSettingsSetParams;

    use std::sync::Arc;

    use serde_json::json;

    use crate::packages::{KindPackages, PackageKind, PackageState};
    use crate::sessions::tests::{service, service_with};

    const SECRET: &str = "placeholder-settings-6f13";

    /// This test binary, as a program that resolves.
    fn this_binary() -> String {
        std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn settings_are_set_read_back_with_names_only_and_cleared() {
        let (agents, _, _profile) = service();
        assert_eq!(
            agents.kind_settings("claude").unwrap(),
            massorca_protocol::methods::agents::KindSettings {
                kind: "claude".to_owned(),
                ..Default::default()
            }
        );
        let set = agents
            .set_kind_settings(&KindSettingsSetParams {
                kind: "claude".to_owned(),
                program: Some(this_binary()),
                args: Some(vec!["--verbose".to_owned()]),
                env: BTreeMap::from([("API_TOKEN".to_owned(), SECRET.to_owned())]),
                remove_env: Vec::new(),
            })
            .unwrap();
        assert_eq!(set.program.as_deref(), Some(this_binary().as_str()));
        assert_eq!(set.args, ["--verbose"]);
        assert_eq!(set.env_names, ["API_TOKEN"]);
        assert_eq!(agents.kind_settings("claude").unwrap(), set);
        assert!(!format!("{set:?}").contains(SECRET));

        assert!(agents.clear_kind_settings("claude").unwrap().removed);
        assert!(!agents.clear_kind_settings("claude").unwrap().removed);
        assert!(agents.kind_settings("claude").unwrap().env_names.is_empty());
    }

    #[test]
    fn an_unknown_kind_or_a_missing_bare_program_is_refused() {
        let (agents, _, _profile) = service();
        let unknown = agents
            .set_kind_settings(&KindSettingsSetParams {
                kind: "no-such-kind".to_owned(),
                args: Some(vec!["--x".to_owned()]),
                ..Default::default()
            })
            .unwrap_err();
        assert_eq!(unknown.code, codes::AGENT_KIND_NOT_FOUND);
        assert_eq!(
            agents.kind_settings("Not/AnId").unwrap_err().code,
            codes::AGENT_KIND_NOT_FOUND
        );

        let missing = agents
            .set_kind_settings(&KindSettingsSetParams {
                kind: "claude".to_owned(),
                program: Some("massorca-test-no-such-wrapper".to_owned()),
                ..Default::default()
            })
            .unwrap_err();
        assert_eq!(missing.code, codes::AGENT_NOT_INSTALLED);
        assert!(!missing.message.contains("massorca-test-no-such-wrapper"));
        assert!(agents.kind_settings("claude").unwrap().program.is_none());
    }

    /// One kind, `fake`, whose declared program no `PATH` resolves.
    struct MissingFake;

    impl KindPackages for MissingFake {
        fn kinds(&self) -> Vec<PackageKind> {
            vec![PackageKind {
                kind_id: "fake".to_owned(),
                package_id: "ext.fake".to_owned(),
                version: "1.0.0".to_owned(),
                state: PackageState::Enabled,
                decl: Ok(json!({
                    "id": "fake",
                    "name": "Fake",
                    "program": ["massorca-test-no-such-agent"],
                    "resume": ["--resume", "{ref}"]
                })
                .to_string()),
            }]
        }
    }

    #[test]
    fn a_kind_with_a_standing_program_starts_with_it_even_when_its_own_is_missing() {
        let (agents, _, _profile) = service_with(Arc::new(MissingFake));
        assert_eq!(
            agents.startable_kind("fake").unwrap_err().code,
            codes::AGENT_NOT_INSTALLED
        );
        agents
            .set_kind_settings(&KindSettingsSetParams {
                kind: "fake".to_owned(),
                program: Some(this_binary()),
                ..Default::default()
            })
            .unwrap();
        let (_, program) = agents.startable_kind("fake").unwrap();
        assert_eq!(program.to_string_lossy(), this_binary());

        // A standing program that no longer resolves refuses the start and never names the program.
        let gone = std::env::temp_dir()
            .join("massorca-test-gone-wrapper")
            .to_string_lossy()
            .into_owned();
        let store = agents.inner.config.kind_settings.as_ref().unwrap();
        store
            .update::<crate::ProviderError>("fake", |settings| {
                settings.program = Some(gone.clone());
                Ok(())
            })
            .unwrap();
        let refused = agents.startable_kind("fake").unwrap_err();
        assert_eq!(refused.code, codes::AGENT_NOT_INSTALLED);
        assert!(!refused.message.contains(&gone), "{}", refused.message);
    }
}
