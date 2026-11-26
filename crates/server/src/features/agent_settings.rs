//! A kind's standing launch settings (decision 0174, capability `agent-settings/1`): `agent.settings.get`, `.set` and
//! `.clear`. They choose the program, arguments and environment of every run of a kind, so each method needs host-admin
//! scope, as a secret does. The rules live in `core-agents`; the values are sealed in the vault, and a reader gets the
//! variable names only. Params that carry values are decoded with redacted errors, and no value is ever logged.

use std::sync::Arc;

use massorca_core_agents::{ProviderError, SettingsVault};
use massorca_core_vault::{SecretValue, Vault, VaultError};
use massorca_protocol::methods::agents::{
    self, KindSettings, KindSettingsCleared, KindSettingsRef, KindSettingsSetParams,
};
use massorca_protocol::{ErrorBody, capabilities};
use zeroize::Zeroizing;

use super::agents::on_agents;
use super::secret_params::Redacted;
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    let cap = capabilities::AGENT_SETTINGS;
    Ok(vec![
        methods.register(
            Owner::Core,
            agents::SETTINGS_GET,
            cap,
            Permission::HostAdmin,
            get,
        )?,
        methods.register(
            Owner::Core,
            agents::SETTINGS_SET,
            cap,
            Permission::HostAdmin,
            |ctx, params: Redacted<KindSettingsSetParams>| set(ctx, params.0),
        )?,
        methods.register(
            Owner::Core,
            agents::SETTINGS_CLEAR,
            cap,
            Permission::HostAdmin,
            clear,
        )?,
        registries.capabilities.register(cap, Owner::Core)?,
    ])
}

async fn get(ctx: RequestCtx, params: KindSettingsRef) -> Result<KindSettings, ErrorBody> {
    on_agents(&ctx, move |agents| agents.kind_settings(&params.kind)).await
}

async fn set(ctx: RequestCtx, params: KindSettingsSetParams) -> Result<KindSettings, ErrorBody> {
    on_agents(&ctx, move |agents| agents.set_kind_settings(&params)).await
}

async fn clear(ctx: RequestCtx, params: KindSettingsRef) -> Result<KindSettingsCleared, ErrorBody> {
    on_agents(&ctx, move |agents| agents.clear_kind_settings(&params.kind)).await
}

/// The vault as the agents domain's settings store: one sealed row per kind.
pub(crate) struct VaultSettings(pub(crate) Arc<Vault>);

/// A vault refusal as the agents domain sees it: the vault's message, which names no value.
fn refused(error: &VaultError) -> ProviderError {
    ProviderError(error.to_string())
}

impl SettingsVault for VaultSettings {
    fn load(&self, kind: &str) -> Result<Option<Zeroizing<String>>, ProviderError> {
        let stored = self
            .0
            .agent_kind_settings(kind)
            .map_err(|error| refused(&error))?;
        Ok(stored.map(|value| Zeroizing::new(value.expose().to_owned())))
    }

    fn save(&self, kind: &str, text: &str) -> Result<(), ProviderError> {
        let value = SecretValue::new(text.to_owned())
            .map_err(|_| ProviderError("the settings are longer than 64 KiB".to_owned()))?;
        self.0
            .set_agent_kind_settings(kind, &value)
            .map_err(|error| refused(&error))
    }

    fn remove(&self, kind: &str) -> Result<bool, ProviderError> {
        self.0
            .remove_agent_kind_settings(kind)
            .map_err(|error| refused(&error))
    }
}
