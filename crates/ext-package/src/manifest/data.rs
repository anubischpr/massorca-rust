//! Data contribution kinds. They run no code. A command may name a handler, which needs a `main` in the package.

use serde::Deserialize;

use super::contributes::{handler_check, list, required};
use super::{Manifest, check_path};
use crate::error::{Problems, at};
use crate::ids;

/// A palette command. Without a `handler` it is data only.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Command {
    pub id: String,
    pub title: String,
    pub handler: Option<String>,
    pub keys: Option<String>,
}

/// A key bound to a command id.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Keybinding {
    pub keys: String,
    pub command: String,
}

/// A theme or icon theme file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ThemeFile {
    pub id: String,
    pub title: String,
    pub document: String,
}

/// A file the package ships for a kind: a locale, a recipe or an agent rule.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DocumentFile {
    pub id: String,
    pub document: String,
}

/// A folder placed for agents; it must hold a `SKILL.md`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub title: String,
    pub path: String,
}

/// Checks the data kinds against the manifest's `main`.
pub(crate) fn validate(manifest: &Manifest, problems: &mut Problems) {
    let contributes = &manifest.contributes;
    let main = manifest.main.is_some();

    list(
        &contributes.commands,
        "commands",
        "id",
        ids::is_contribution_id,
        |item| item.id.as_str(),
        problems,
        |item, here, problems| {
            required(&item.title, &at(here, "title"), problems);
            if let Some(keys) = &item.keys {
                required(keys, &at(here, "keys"), problems);
            }
            if let Some(handler) = &item.handler {
                handler_check(handler, &at(here, "handler"), main, problems);
            }
        },
    );
    for (index, binding) in contributes.keybindings.iter().enumerate() {
        let here = at("/contributes/keybindings", index);
        required(&binding.keys, &at(&here, "keys"), problems);
        required(&binding.command, &at(&here, "command"), problems);
    }
    for (kind, items) in [
        ("themes", &contributes.themes),
        ("iconThemes", &contributes.icon_themes),
    ] {
        list(
            items,
            kind,
            "id",
            ids::is_contribution_id,
            |item| item.id.as_str(),
            problems,
            |item, here, problems| {
                required(&item.title, &at(here, "title"), problems);
                check_path(&item.document, &at(here, "document"), problems);
            },
        );
    }
    for (kind, items) in [
        ("locales", &contributes.locales),
        ("recipes", &contributes.recipes),
        ("agentRules", &contributes.agent_rules),
    ] {
        list(
            items,
            kind,
            "id",
            ids::is_contribution_id,
            |item| item.id.as_str(),
            problems,
            |item, here, problems| {
                check_path(&item.document, &at(here, "document"), problems);
            },
        );
    }
    list(
        &contributes.skills,
        "skills",
        "id",
        ids::is_contribution_id,
        |item| item.id.as_str(),
        problems,
        |item, here, problems| {
            required(&item.title, &at(here, "title"), problems);
            check_path(&item.path, &at(here, "path"), problems);
        },
    );
}
