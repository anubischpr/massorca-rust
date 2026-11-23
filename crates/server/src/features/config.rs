//! `config.get`, `config.set` and `config.list` (capability `config/1`) over the settings registry.

use std::collections::HashMap;

use massorca_core_store::{Store, StoreError};
use massorca_protocol::methods::config::{self, ConfigKey, ConfigList, ConfigSet, ConfigValue};
use massorca_protocol::methods::events::{self, ChangeEvent};
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
/// Where the branch prefix of a new workspace comes from: `none`, `git-username` or `custom`.
pub(crate) const WORKSPACES_BRANCH_PREFIX: &str = "workspaces.branch_prefix";
/// The text of the branch prefix when `workspaces.branch_prefix` is `custom`.
pub(crate) const WORKSPACES_BRANCH_PREFIX_CUSTOM: &str = "workspaces.branch_prefix_custom";

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
            aliases: Vec::new(),
        })?,
        registries.settings.register(SettingDecl {
            owner: Owner::Core,
            key: WORKSPACES_BRANCH_PREFIX.to_owned(),
            kind: SettingKind::Choice(&["none", "git-username", "custom"]),
            default: Value::String("none".to_owned()),
            description: "Branch prefix of new workspaces: none, git-username or custom."
                .to_owned(),
            aliases: Vec::new(),
        })?,
        registries.settings.register(SettingDecl {
            owner: Owner::Core,
            key: WORKSPACES_BRANCH_PREFIX_CUSTOM.to_owned(),
            kind: SettingKind::String,
            default: Value::String(String::new()),
            description: "Branch prefix text, used when workspaces.branch_prefix is custom."
                .to_owned(),
            aliases: Vec::new(),
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
        registries.methods.register(
            Owner::Core,
            config::RESET,
            capabilities::CONFIG,
            Permission::HostAdmin,
            reset,
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
    let (owner, key, aliases) = (
        decl.owner.to_string(),
        decl.key.clone(),
        decl.aliases.clone(),
    );
    let before = store_blocking::run(&ctx.state, move |store| {
        let before = stored_text(store, &owner, &key, &aliases)?;
        let retired: Vec<&str> = aliases.iter().map(String::as_str).collect();
        store.replace_setting(&owner, &key, &json, &retired)?;
        Ok(before)
    })
    .await?;
    let before = before.as_deref().map(stored_json).transpose()?;
    let after = view(&decl, Some(params.value));
    publish_if_changed(&ctx, &decl, before, &after);
    Ok(after)
}

async fn reset(ctx: RequestCtx, params: ConfigKey) -> Result<ConfigValue, ErrorBody> {
    let decl = find(
        &ctx.state.registries.settings,
        params.owner.as_deref(),
        &params.key,
    )?;
    let (owner, key, aliases) = (
        decl.owner.to_string(),
        decl.key.clone(),
        decl.aliases.clone(),
    );
    let before = store_blocking::run(&ctx.state, move |store| {
        let before = stored_text(store, &owner, &key, &aliases)?;
        let retired: Vec<&str> = aliases.iter().map(String::as_str).collect();
        store.remove_setting(&owner, &key, &retired)?;
        Ok(before)
    })
    .await?;
    let before = before.as_deref().map(stored_json).transpose()?;
    let after = view(&decl, None);
    publish_if_changed(&ctx, &decl, before, &after);
    Ok(after)
}

/// Publishes `config.changed` when the value in effect before differs from the one now.
fn publish_if_changed(
    ctx: &RequestCtx,
    decl: &SettingDecl,
    before: Option<Value>,
    after: &ConfigValue,
) {
    if before.unwrap_or_else(|| decl.default.clone()) != after.value {
        ctx.state.events.publish(ChangeEvent {
            topic: events::TOPIC_CONFIG_CHANGED.to_owned(),
            id: format!("{}/{}", decl.owner, decl.key),
        });
    }
}

async fn list(ctx: RequestCtx, _params: NoParams) -> Result<ConfigList, ErrorBody> {
    let rows = store_blocking::run(&ctx.state, |store| store.list_settings(None)).await?;
    let mut stored: HashMap<(String, String), String> = rows
        .into_iter()
        .map(|(owner, key, json)| ((owner, key), json))
        .collect();

    let mut values = Vec::new();
    for decl in ctx.state.registries.settings.declarations() {
        let owner = decl.owner.to_string();
        let json = stored
            .remove(&(owner.clone(), decl.key.clone()))
            .or_else(|| {
                decl.aliases
                    .iter()
                    .find_map(|alias| stored.get(&(owner.clone(), alias.clone())).cloned())
            });
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
    settings
        .find(&owner, key)
        .or_else(|| settings.find_by_alias(&owner, key))
        .ok_or_else(unknown)
}

async fn stored_value(ctx: &RequestCtx, decl: &SettingDecl) -> Result<Option<Value>, ErrorBody> {
    let (owner, key, aliases) = (
        decl.owner.to_string(),
        decl.key.clone(),
        decl.aliases.clone(),
    );
    let json = store_blocking::run(&ctx.state, move |store| {
        stored_text(store, &owner, &key, &aliases)
    })
    .await?;
    json.as_deref().map(stored_json).transpose()
}

/// The stored text of `key`, or of the first old name that still holds a value. The canonical row always wins.
fn stored_text(
    store: &Store,
    owner: &str,
    key: &str,
    aliases: &[String],
) -> Result<Option<String>, StoreError> {
    if let Some(json) = store.get_setting(owner, key)? {
        return Ok(Some(json));
    }
    for alias in aliases {
        if let Some(json) = store.get_setting(owner, alias)? {
            return Ok(Some(json));
        }
    }
    Ok(None)
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
