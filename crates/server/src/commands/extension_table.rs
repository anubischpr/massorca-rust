//! The extension commands (decision 0062): one per `extensions/1` method, ids are the CLI paths.

use serde_json::{Value, json};

use super::CommandDescriptor;

pub(super) fn extension_commands() -> Vec<CommandDescriptor> {
    vec![
        command(
            "extension.list",
            "List the installed extensions",
            "extensions.list",
            no_args(),
        ),
        command(
            "extension.show",
            "Show one extension, installed or from the catalogue",
            "extensions.get",
            args(&["id"], json!({ "id": { "type": "string" } })),
        ),
        command(
            "extension.doctor",
            "Show what detection sees for a package's CLIs, and the PATH it searched",
            "extensions.get",
            args(&["id"], json!({ "id": { "type": "string" } })),
        ),
        command(
            "extension.catalogue",
            "List the built-in catalogue",
            "extensions.catalogue",
            no_args(),
        ),
        command(
            "extension.inspect",
            "Review a package before installing it",
            "extensions.inspect",
            args(&["source"], json!({ "source": { "type": "string" } })),
        ),
        command(
            "extension.install",
            "Install a package after its review is accepted",
            "extensions.install",
            args(
                &["source"],
                json!({
                    "source": { "type": "string" },
                    "consent": { "type": "string" },
                    "run_installer": { "type": "boolean" },
                    "sha256": { "type": "string" }
                }),
            ),
        ),
        command(
            "extension.update",
            "Update an installed extension to its source's current package",
            "extensions.update",
            args(
                &["id"],
                json!({
                    "id": { "type": "string" },
                    "consent": { "type": "string" },
                    "sha256": { "type": "string" }
                }),
            ),
        ),
        command(
            "extension.rollback",
            "Roll an extension back to the version its last update replaced",
            "extensions.rollback",
            args(
                &["id"],
                json!({
                    "id": { "type": "string" },
                    "consent": { "type": "string" }
                }),
            ),
        ),
        command(
            "extension.program-update-review",
            "Show what updating a host program a tool package installed would run",
            "extensions.program_update_review",
            args(
                &["id", "program"],
                json!({ "id": { "type": "string" }, "program": { "type": "string" } }),
            ),
        ),
        command(
            "extension.program-update",
            "Update a host program a tool package installed, with the reviewed vendor command",
            "extensions.program_update",
            args(
                &["id", "program", "consent"],
                json!({
                    "id": { "type": "string" },
                    "program": { "type": "string" },
                    "consent": { "type": "string" }
                }),
            ),
        ),
        command(
            "extension.log",
            "Show the recent install, update, rollback and remove runs of an extension",
            "extensions.log",
            args(&["id"], json!({ "id": { "type": "string" } })),
        ),
        command(
            "extension.enable",
            "Enable an installed extension",
            "extensions.set_enabled",
            args(
                &["id"],
                json!({ "id": { "type": "string" }, "enabled": { "const": true } }),
            ),
        ),
        command(
            "extension.disable",
            "Disable an installed extension, keeping its files and data",
            "extensions.set_enabled",
            args(
                &["id"],
                json!({ "id": { "type": "string" }, "enabled": { "const": false } }),
            ),
        ),
        command(
            "extension.remove",
            "Remove an installed extension and its data",
            "extensions.remove",
            args(
                &["id"],
                json!({
                    "id": { "type": "string" },
                    "delete_agent_homes": { "type": "boolean" },
                    "uninstall_cli": { "type": "boolean" }
                }),
            ),
        ),
        // No CLI command: the desktop's item page shows listed images.
        command(
            "extension.asset",
            "Read a listed image of an extension",
            "extensions.asset",
            args(
                &["id", "path"],
                json!({ "id": { "type": "string" }, "path": { "type": "string" } }),
            ),
        ),
    ]
}

fn command(id: &str, title: &str, method: &str, args_schema: Value) -> CommandDescriptor {
    CommandDescriptor {
        id: id.to_owned(),
        title: title.to_owned(),
        method: method.to_owned(),
        args_schema,
    }
}

fn no_args() -> Value {
    json!({ "type": "object", "properties": {} })
}

fn args(required: &[&str], properties: Value) -> Value {
    json!({ "type": "object", "required": required, "properties": properties })
}
