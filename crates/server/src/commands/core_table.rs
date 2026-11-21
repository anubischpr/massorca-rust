//! The core command table: every Phase 1 command, with the request it issues and its argument schema.

use serde_json::{Map, Value, json};

use super::CommandDescriptor;

/// The core commands. Ids are the CLI paths. `service.status` and `service.restart` are CLI compositions, so their `method` is the first request they issue; a palette must not run them as a single call.
pub fn core_commands() -> Vec<CommandDescriptor> {
    vec![
        command(
            "service.info",
            "Show the service identity and capabilities",
            "service.info",
            no_args(),
        ),
        command(
            "service.status",
            "Show whether the service is running",
            "service.info",
            no_args(),
        ),
        command(
            "service.stop",
            "Stop the service",
            "service.shutdown",
            no_args(),
        ),
        command(
            "service.restart",
            "Restart the service",
            "service.shutdown",
            no_args(),
        ),
        command(
            "events.subscribe",
            "Stream change events",
            "events.subscribe",
            no_args(),
        ),
        command(
            "config.get",
            "Read a setting",
            "config.get",
            json!({
                "type": "object",
                "required": ["key"],
                "properties": { "owner": { "type": "string" }, "key": { "type": "string" } }
            }),
        ),
        command(
            "config.set",
            "Change a setting",
            "config.set",
            json!({
                "type": "object",
                "required": ["key", "value"],
                "properties": { "owner": { "type": "string" }, "key": { "type": "string" }, "value": {} }
            }),
        ),
        command("config.list", "List settings", "config.list", no_args()),
        command(
            "project.add",
            "Register a project",
            "project.add",
            json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string" },
                    "name": { "type": "string" },
                    "kind": { "enum": ["git", "folder"] },
                    "workspaces_root": { "type": "string" }
                }
            }),
        ),
        command("project.list", "List projects", "project.list", no_args()),
        command(
            "project.show",
            "Show a project",
            "project.show",
            selector("project"),
        ),
        command(
            "project.rm",
            "Remove a project",
            "project.remove",
            selector("project"),
        ),
        command(
            "workspace.create",
            "Create a workspace",
            "workspace.create",
            json!({
                "type": "object",
                "required": ["project", "name"],
                "properties": {
                    "project": { "type": "string" },
                    "name": { "type": "string" },
                    "branch": { "type": "string" },
                    "base": { "type": "string" },
                    "existing_branch": { "type": "boolean" },
                    "path": { "type": "string" }
                }
            }),
        ),
        command(
            "workspace.list",
            "List workspaces",
            "workspace.list",
            json!({ "type": "object", "properties": { "project": { "type": "string" } } }),
        ),
        command(
            "workspace.show",
            "Show a workspace",
            "workspace.show",
            selector("workspace"),
        ),
        command(
            "workspace.remove",
            "Remove a workspace",
            "workspace.remove",
            json!({
                "type": "object",
                "required": ["workspace"],
                "properties": { "workspace": { "type": "string" }, "force": { "type": "boolean" } }
            }),
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

/// Arguments that name one project or workspace by selector.
fn selector(name: &str) -> Value {
    let mut properties = Map::new();
    properties.insert(name.to_owned(), json!({ "type": "string" }));
    json!({ "type": "object", "required": [name], "properties": properties })
}
