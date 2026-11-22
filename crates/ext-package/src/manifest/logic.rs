//! Logic contribution kinds. They run sandboxed code, so each needs a `main` in the package.

use serde::Deserialize;

use super::Manifest;
use super::check_path;
use super::contributes::{handler_check, list, required};
use crate::error::{Problems, at};
use crate::ids;

/// A foreign-file importer, run by its handler.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Importer {
    pub id: String,
    pub title: String,
    pub handler: String,
}

/// A handler for a lifecycle event, such as `workspace.created`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Event {
    pub on: String,
    pub handler: String,
}

/// A tool agents may call, served by the core over MCP. `input` is the schema file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Tool {
    pub id: String,
    pub description: String,
    pub input: String,
}

/// A tracker source of items.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ItemSource {
    pub id: String,
    pub title: String,
}

/// A scheduled handler.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub schedule: String,
    pub handler: String,
}

/// Checks the logic kinds: each needs a `main`, handlers are identifiers, and event names are dotted.
pub(crate) fn validate(manifest: &Manifest, problems: &mut Problems) {
    let contributes = &manifest.contributes;
    let main = manifest.main.is_some();

    logic_needs_main(
        "importers",
        !contributes.importers.is_empty(),
        main,
        problems,
    );
    list(
        &contributes.importers,
        "importers",
        "id",
        ids::is_contribution_id,
        |item| item.id.as_str(),
        problems,
        |item, here, problems| {
            required(&item.title, &at(here, "title"), problems);
            handler_check(&item.handler, &at(here, "handler"), main, problems);
        },
    );

    logic_needs_main("events", !contributes.events.is_empty(), main, problems);
    for (index, event) in contributes.events.iter().enumerate() {
        let here = at("/contributes/events", index);
        if !is_event_name(&event.on) {
            problems.add(
                at(&here, "on"),
                format!("`{}` is not a dotted event name", event.on),
            );
        }
        handler_check(&event.handler, &at(&here, "handler"), main, problems);
    }

    logic_needs_main("tools", !contributes.tools.is_empty(), main, problems);
    list(
        &contributes.tools,
        "tools",
        "id",
        ids::is_contribution_id,
        |item| item.id.as_str(),
        problems,
        |item, here, problems| {
            required(&item.description, &at(here, "description"), problems);
            check_path(&item.input, &at(here, "input"), problems);
        },
    );

    logic_needs_main(
        "itemSources",
        !contributes.item_sources.is_empty(),
        main,
        problems,
    );
    list(
        &contributes.item_sources,
        "itemSources",
        "id",
        ids::is_contribution_id,
        |item| item.id.as_str(),
        problems,
        |item, here, problems| {
            required(&item.title, &at(here, "title"), problems);
        },
    );

    logic_needs_main("jobs", !contributes.jobs.is_empty(), main, problems);
    list(
        &contributes.jobs,
        "jobs",
        "id",
        ids::is_contribution_id,
        |item| item.id.as_str(),
        problems,
        |item, here, problems| {
            required(&item.schedule, &at(here, "schedule"), problems);
            handler_check(&item.handler, &at(here, "handler"), main, problems);
        },
    );
}

fn logic_needs_main(kind: &str, present: bool, main: bool, problems: &mut Problems) {
    if present && !main {
        problems.add(
            format!("/contributes/{kind}"),
            "logic contributions need a main in the package",
        );
    }
}

/// Dotted lowercase names such as `workspace.created`.
fn is_event_name(text: &str) -> bool {
    !text.is_empty()
        && text.split('.').all(|part| {
            let mut bytes = part.bytes();
            bytes.next().is_some_and(|first| first.is_ascii_lowercase())
                && bytes
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}
