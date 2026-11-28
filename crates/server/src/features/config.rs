//! `config.get`, `config.set` and `config.list` (capability `config/1`) over the settings registry.

use std::collections::HashMap;

use massorca_core_store::{Store, StoreError};
use massorca_core_terminal::TerminalTheme;
use massorca_core_workspace::WorkspaceLayout;
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
/// Where a new project's workspaces go under the default root: `nested` or `flat` (ADR 0008).
pub(crate) const WORKSPACES_LAYOUT: &str = "workspaces.layout";
const LAYOUT_NESTED: &str = "nested";
const LAYOUT_FLAT: &str = "flat";
/// The app theme; terminals answer colour queries from its palette.
const APPEARANCE_THEME: &str = "appearance.theme";

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let mut handles = Vec::new();
    for decl in config_keys::core_settings(default_conversations_root()?) {
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
        registries.settings.register(SettingDecl {
            owner: Owner::Core,
            key: WORKSPACES_LAYOUT.to_owned(),
            kind: SettingKind::Choice(&[LAYOUT_NESTED, LAYOUT_FLAT]),
            default: Value::String(LAYOUT_NESTED.to_owned()),
            description: "Where a new project's workspaces go: in a folder named after the project, or directly in the default root."
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

/// `<user home>/MassOrca/Conversations`: where conversations without a project are saved unless `conversations.folder`
/// says otherwise (decision 0094).
pub(crate) fn default_conversations_root() -> Result<String, ServiceError> {
    home_path(&["MassOrca", "Conversations"])
}

/// `<user home>/massorca/workspaces`, as a string the protocol can carry.
pub(crate) fn default_workspace_root() -> Result<String, ServiceError> {
    home_path(&["massorca", "workspaces"])
}

/// The layout a project added now gets under the default root: the stored `workspaces.layout`, else nested.
pub(crate) async fn workspace_layout(ctx: &RequestCtx) -> Result<WorkspaceLayout, ErrorBody> {
    let stored = store_blocking::run(&ctx.state, |store| {
        store.get_setting(config::DEFAULT_OWNER, WORKSPACES_LAYOUT)
    })
    .await?;
    let value = stored
        .as_deref()
        .map(stored_json)
        .transpose()?
        .unwrap_or_else(|| Value::String(LAYOUT_NESTED.to_owned()));
    match value.as_str() {
        Some(LAYOUT_NESTED) => Ok(WorkspaceLayout::Nested),
        Some(LAYOUT_FLAT) => Ok(WorkspaceLayout::Flat),
        _ => Err(ErrorBody::new(
            codes::CONFIG_INVALID_VALUE,
            "workspaces.layout must be nested or flat",
        )),
    }
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
    follow_theme(&ctx, &decl, &after);
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
    follow_theme(&ctx, &decl, &after);
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

/// The palette terminals answer colour queries with: light for `light`, else dark, since the service cannot see the
/// operating system's theme behind `system`.
pub(crate) fn terminal_theme(value: Option<&Value>) -> TerminalTheme {
    match value.and_then(Value::as_str) {
        Some("light") => TerminalTheme::Light,
        _ => TerminalTheme::Dark,
    }
}

/// The terminal palette of the stored `appearance.theme`; an unreadable value counts as unset.
pub(crate) fn stored_terminal_theme(store: &Store) -> TerminalTheme {
    let stored = store
        .get_setting(config::DEFAULT_OWNER, APPEARANCE_THEME)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<Value>(&json).ok());
    terminal_theme(stored.as_ref())
}

/// Whether the global `usage.enabled` switch is on (decision 0176): only a stored JSON `true` under the `core` owner is
/// on. Unset, unreadable or any other value is off, so a broken store never lends an account credential. Read at every
/// call through the agents service's [`massorca_core_agents::UsageGate`], never cached. The key is read by name: its
/// declaration on the Settings page belongs to the usage feature.
pub(crate) fn stored_usage_enabled(store: &Store) -> bool {
    store
        .get_setting(
            config::DEFAULT_OWNER,
            massorca_core_agents::USAGE_ENABLED_SETTING,
        )
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<Value>(&json).ok())
        .is_some_and(|value| value == Value::Bool(true))
}

/// Keeps the terminals' palette in step when `appearance.theme` changes.
fn follow_theme(ctx: &RequestCtx, decl: &SettingDecl, after: &ConfigValue) {
    if decl.owner.to_string() == config::DEFAULT_OWNER && decl.key == APPEARANCE_THEME {
        ctx.state
            .terminals
            .set_theme(terminal_theme(Some(&after.value)));
    }
}

#[cfg(test)]
mod tests {
    use super::terminal_theme;
    use massorca_core_terminal::TerminalTheme;
    use serde_json::json;

    #[test]
    fn only_light_picks_the_light_terminal_palette() {
        assert_eq!(terminal_theme(Some(&json!("light"))), TerminalTheme::Light);
        assert_eq!(terminal_theme(Some(&json!("dark"))), TerminalTheme::Dark);
        assert_eq!(terminal_theme(Some(&json!("system"))), TerminalTheme::Dark);
        assert_eq!(terminal_theme(None), TerminalTheme::Dark);
    }

    #[test]
    fn only_a_stored_true_turns_the_usage_switch_on_and_it_is_read_live() {
        use massorca_core_agents::{USAGE_ENABLED_SETTING, UsageGate};
        use massorca_core_store::Store;

        use super::stored_usage_enabled;

        let store = Store::open_in_memory().unwrap();
        let live = store.clone();
        let gate = UsageGate::new(move || stored_usage_enabled(&live));
        assert!(!gate.is_on(), "unset is off");
        for (stored, on) in [
            ("true", true),
            ("false", false),
            ("\"true\"", false),
            ("1", false),
            ("null", false),
            ("not json", false),
        ] {
            store
                .replace_setting("core", USAGE_ENABLED_SETTING, stored, &[])
                .unwrap();
            assert_eq!(gate.is_on(), on, "{stored}");
        }
        store
            .replace_setting("core", USAGE_ENABLED_SETTING, "false", &[])
            .unwrap();
        store
            .replace_setting("ext.other", USAGE_ENABLED_SETTING, "true", &[])
            .unwrap();
        assert!(!gate.is_on(), "another owner's key is not the switch");
    }
}
