//! The contribution kinds a package declares. Each kind lives in its own module: data kinds in `data`, logic kinds in
//! `logic`, and the process tier in `agent`.

use std::collections::HashSet;

use serde::Deserialize;

use super::Manifest;
use super::agent::{AgentAdapter, McpServer};
use super::data::{Command, DocumentFile, Keybinding, Skill, ThemeFile};
use super::logic::{Event, Importer, ItemSource, Job, Tool};
use super::settings::Setting;
use super::views::View;
use crate::error::{Problems, at};
use crate::ids;

/// Every contribution a package declares. Missing kinds are empty; unknown kinds are rejected.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", default)]
pub struct Contributes {
    pub commands: Vec<Command>,
    pub keybindings: Vec<Keybinding>,
    pub themes: Vec<ThemeFile>,
    pub icon_themes: Vec<ThemeFile>,
    pub locales: Vec<DocumentFile>,
    pub skills: Vec<Skill>,
    pub recipes: Vec<DocumentFile>,
    pub agent_rules: Vec<DocumentFile>,
    pub settings: Vec<Setting>,
    pub views: Vec<View>,
    pub importers: Vec<Importer>,
    pub events: Vec<Event>,
    pub tools: Vec<Tool>,
    pub item_sources: Vec<ItemSource>,
    pub jobs: Vec<Job>,
    pub agent_adapters: Vec<AgentAdapter>,
    pub mcp_servers: Vec<McpServer>,
}

/// Checks every contribution kind against the manifest.
pub(crate) fn validate(manifest: &Manifest, problems: &mut Problems) {
    super::data::validate(manifest, problems);
    super::settings::validate(manifest, problems);
    super::views::validate(manifest, problems);
    super::logic::validate(manifest, problems);
    super::agent::validate(manifest, problems);
}

/// Checks one list of contributions: each id follows `rule` and is unique within the kind, then `check` runs on it.
/// `base` of the pointer is `/contributes/<kind>`.
pub(crate) fn list<T>(
    items: &[T],
    kind: &str,
    field: &str,
    rule: fn(&str) -> bool,
    name: impl Fn(&T) -> &str,
    problems: &mut Problems,
    check: impl Fn(&T, &str, &mut Problems),
) {
    let base = format!("/contributes/{kind}");
    let mut seen = HashSet::new();
    for (index, item) in items.iter().enumerate() {
        let here = at(&base, index);
        let value = name(item);
        if !rule(value) {
            problems.add(
                at(&here, field),
                format!("`{value}` is not a valid {field}"),
            );
        } else if !seen.insert(value) {
            problems.add(
                at(&here, field),
                format!("`{value}` is declared twice in {kind}"),
            );
        }
        check(item, &here, problems);
    }
}

/// Reports an empty required text at `pointer`.
pub(crate) fn required(text: &str, pointer: &str, problems: &mut Problems) {
    if text.trim().is_empty() {
        problems.add(pointer, "is required");
    }
}

/// A handler must be an identifier, and it needs a `main` in the package to run.
pub(crate) fn handler_check(name: &str, pointer: &str, main: bool, problems: &mut Problems) {
    if !ids::is_handler(name) {
        problems.add(pointer, format!("`{name}` is not a handler identifier"));
    } else if !main {
        problems.add(pointer, "needs a main in the package");
    }
}

/// Whether the package declares any contribution beyond the theme kinds.
pub(crate) fn has_non_theme_contributions(contributes: &Contributes) -> bool {
    !(contributes.commands.is_empty()
        && contributes.keybindings.is_empty()
        && contributes.importers.is_empty()
        && contributes.locales.is_empty()
        && contributes.skills.is_empty()
        && contributes.recipes.is_empty()
        && contributes.agent_rules.is_empty()
        && contributes.settings.is_empty()
        && contributes.views.is_empty()
        && contributes.events.is_empty()
        && contributes.tools.is_empty()
        && contributes.item_sources.is_empty()
        && contributes.jobs.is_empty()
        && contributes.agent_adapters.is_empty()
        && contributes.mcp_servers.is_empty())
}
