//! `config.get`, `config.set` and `config.list` (capability `config/1`) over the settings registry.

use std::collections::HashMap;

use massorca_protocol::methods::config::{self, ConfigKey, ConfigList, ConfigSet, ConfigValue};
use massorca_protocol::{ErrorBody, capabilities, codes};
use serde_json::Value;

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::{NoParams, config_keys};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;
use crate::settings::{SettingDecl, SettingKind, SettingsRegistry};
use crate::store_blocking;

/// Where new workspaces go when a project does not set a root of its own.
pub(crate) const WORKSPACES_DEFAULT_ROOT: &str = "workspaces.default_root";

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let mut handles = Vec::new();
    for decl in config_keys::core_settings(home_path(&["MassOrca", "Conversations"])?) {
        handles.push(registries.settings.register(decl)?);
    }
    handles.extend([
        registries
            .capabilities
            .register(capabilities::CONFIG, Owner::Core)?,
        registries.settings.register(SettingDecl {
            owner: Owner::Core,
            key: WORKSPACES_DEFAULT_ROOT.to_owned(),
            kind: SettingKind::Path,
            default: Value::String(default_workspace_root()?),
            description: "Folder for new workspaces when a project does not set its own root."
                .to_owned(),
        })?,
        registries.methods.register(
            Owner::Core,
            config::GET,
            capabilities::CONFIG,
            Permission::Read,
            get,
        )?,
        registries.methods.register(
            Owner::Core,
            config::SET,
            capabilities::CONFIG,
            Permission::HostAdmin,
            set,
        )?,
        registries.methods.register(
            Owner::Core,
            config::LIST,
            capabilities::CONFIG,
            Permission::Read,
            list,
        )?,
    ]);
    Ok(handles)
}

/// `<user home>/massorca/workspaces`, as a string the protocol can carry.
pub(crate) fn default_workspace_root() -> Result<String, ServiceError> {
    home_path(&["massorca", "workspaces"])
}

/// `parts` joined under the user's home directory, as a string the protocol can carry.
fn home_path(parts: &[&str]) -> Result<String, ServiceError> {
    let base = directories::BaseDirs::new().ok_or_else(|| {
        ServiceError::DefaultWorkspaceRoot("there is no user home directory".to_owned())
    })?;
    let mut path = base.home_dir().to_path_buf();
    path.extend(parts);
    path.into_os_string().into_string().map_err(|_| {
        ServiceError::DefaultWorkspaceRoot("the user home directory is not UTF-8".to_owned())
    })
}

async fn get(ctx: RequestCtx, params: ConfigKey) -> Result<ConfigValue, ErrorBody> {
    let decl = find(
        &ctx.state.registries.settings,
        params.owner.as_deref(),
        &params.key,
    )?;
    let stored = stored_value(&ctx, &decl).await?;
    Ok(view(&decl, stored))
}

async fn set(ctx: RequestCtx, params: ConfigSet) -> Result<ConfigValue, ErrorBody> {
    let decl = find(
        &ctx.state.registries.settings,
        params.owner.as_deref(),
        &params.key,
    )?;
    if let Some(expectation) = decl.check(&params.value) {
        return Err(ErrorBody::new(
            codes::CONFIG_INVALID_VALUE,
            format!("{} {expectation}", decl.key),
        ));
    }
    let json = serde_json::to_string(&params.value)
        .map_err(|error| ErrorBody::new(codes::INTERNAL, error.to_string()))?;
    let (owner, key) = (decl.owner.to_string(), decl.key.clone());
    store_blocking::run(&ctx.state, move |store| {
        store.set_setting(&owner, &key, &json)
    })
    .await?;
    Ok(view(&decl, Some(params.value)))
}

async fn list(ctx: RequestCtx, _params: NoParams) -> Result<ConfigList, ErrorBody> {
    let rows = store_blocking::run(&ctx.state, |store| store.list_settings(None)).await?;
    let mut stored: HashMap<(String, String), String> = rows
        .into_iter()
        .map(|(owner, key, json)| ((owner, key), json))
        .collect();

    let mut values = Vec::new();
    for decl in ctx.state.registries.settings.declarations() {
        let json = stored.remove(&(decl.owner.to_string(), decl.key.clone()));
        let value = json.as_deref().map(stored_json).transpose()?;
        values.push(view(&decl, value));
    }
    Ok(ConfigList { values })
}

/// The declaration `owner` made for `key`. A missing owner means core, as the protocol says.
fn find(
    settings: &SettingsRegistry,
    owner: Option<&str>,
    key: &str,
) -> Result<SettingDecl, ErrorBody> {
    let owner_text = owner.unwrap_or(config::DEFAULT_OWNER);
    let unknown = || {
        ErrorBody::new(
            codes::CONFIG_UNKNOWN_KEY,
            format!("there is no setting {key} for owner {owner_text}"),
        )
    };
    let owner = Owner::parse(owner_text).ok_or_else(unknown)?;
    settings.find(&owner, key).ok_or_else(unknown)
}

async fn stored_value(ctx: &RequestCtx, decl: &SettingDecl) -> Result<Option<Value>, ErrorBody> {
    let (owner, key) = (decl.owner.to_string(), decl.key.clone());
    let json =
        store_blocking::run(&ctx.state, move |store| store.get_setting(&owner, &key)).await?;
    json.as_deref().map(stored_json).transpose()
}

fn stored_json(text: &str) -> Result<Value, ErrorBody> {
    serde_json::from_str(text).map_err(|error| {
        ErrorBody::new(
            codes::INTERNAL,
            format!("a stored setting is not valid JSON: {error}"),
        )
    })
}

/// The wire view: the stored value or the default, and whether that value is still the default.
fn view(decl: &SettingDecl, stored: Option<Value>) -> ConfigValue {
    let value = stored.unwrap_or_else(|| decl.default.clone());
    let is_default = value == decl.default;
    ConfigValue {
        owner: decl.owner.to_string(),
        key: decl.key.clone(),
        value,
        default: decl.default.clone(),
        is_default,
        kind: Some(decl.kind.wire_name().to_owned()),
        choices: decl
            .kind
            .choices()
            .map(|list| list.iter().map(|&choice| choice.to_owned()).collect()),
        min: decl.kind.range().map(|(min, _)| min),
        max: decl.kind.range().map(|(_, max)| max),
    }
}
