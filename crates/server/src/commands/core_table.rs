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
            "settings.get",
            "Read a setting",
            "config.get",
            json!({
                "type": "object",
                "required": ["key"],
                "properties": { "owner": { "type": "string" }, "key": { "type": "string" } }
            }),
        ),
        command(
            "settings.set",
            "Change a setting",
            "config.set",
            json!({
                "type": "object",
                "required": ["key", "value"],
                "properties": { "owner": { "type": "string" }, "key": { "type": "string" }, "value": {} }
            }),
        ),
        command("settings.list", "List settings", "config.list", no_args()),
        command(
            "settings.reset",
            "Restore a setting's default",
            "config.reset",
            json!({
                "type": "object",
                "required": ["key"],
                "properties": { "owner": { "type": "string" }, "key": { "type": "string" } }
            }),
        ),
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
                    "path": { "type": "string" },
                    "request_id": { "type": "string" }
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
            "device.list",
            "List the devices paired with the service",
            "device.list",
            no_args(),
        ),
        command(
            "device.revoke",
            "Revoke a paired device",
            "device.revoke",
            selector("device"),
        ),
        command(
            "device.whoami",
            "Show who this connection acts as",
            "device.whoami",
            no_args(),
        ),
        command(
            "pair.offer",
            "Create a one-time pairing offer for a new device",
            "pairing.create",
            json!({
                "type": "object",
                "properties": {
                    "ttl_secs": { "type": "integer", "minimum": 1, "maximum": 3600 },
                    "admin": { "type": "boolean" }
                }
            }),
        ),
        command(
            "server.status",
            "Show the service's network listeners and devices",
            "network.status",
            no_args(),
        ),
        command(
            "workspace.show",
            "Show a workspace",
            "workspace.show",
            selector("workspace"),
        ),
        command(
            "workspace.rm",
            "Remove a workspace",
            "workspace.remove",
            json!({
                "type": "object",
                "required": ["workspace"],
                "properties": { "workspace": { "type": "string" }, "force": { "type": "boolean" } }
            }),
        ),
        command(
            "terminal.create",
            "Start a terminal in a workspace",
            "terminal.create",
            json!({
                "type": "object",
                "required": ["workspace"],
                "properties": {
                    "workspace": { "type": "string" },
                    "shell": {
                        "type": "object",
                        "required": ["program"],
                        "properties": { "program": { "type": "string" }, "args": { "type": "array", "items": { "type": "string" } } }
                    },
                    "cwd": { "type": "string" },
                    "cols": { "type": "integer" },
                    "rows": { "type": "integer" },
                    "title": { "type": "string" },
                    "request_id": { "type": "string" }
                }
            }),
        ),
        command(
            "terminal.list",
            "List terminals",
            "terminal.list",
            json!({ "type": "object", "properties": { "workspace": { "type": "string" } } }),
        ),
        command(
            "terminal.show",
            "Show a terminal",
            "terminal.show",
            selector("terminal"),
        ),
        command(
            "terminal.screen",
            "Print a terminal's current screen as text",
            "terminal.attach",
            json!({
                "type": "object",
                "required": ["terminal"],
                "properties": {
                    "terminal": { "type": "string" },
                    "scrollback": { "type": "boolean" }
                }
            }),
        ),
        command(
            "terminal.attach",
            "Attach to a terminal's output",
            "terminal.attach",
            json!({
                "type": "object",
                "required": ["terminal"],
                "properties": {
                    "terminal": { "type": "string" },
                    "since": { "type": "integer" },
                    "cols": { "type": "integer" },
                    "rows": { "type": "integer" }
                }
            }),
        ),
        command(
            "terminal.send",
            "Type into a terminal",
            "terminal.input",
            json!({
                "type": "object",
                "required": ["terminal", "data"],
                "properties": { "terminal": { "type": "string" }, "data": { "type": "string" } }
            }),
        ),
        command(
            "terminal.resize",
            "Resize a terminal",
            "terminal.resize",
            json!({
                "type": "object",
                "required": ["terminal", "cols", "rows"],
                "properties": { "terminal": { "type": "string" }, "cols": { "type": "integer" }, "rows": { "type": "integer" } }
            }),
        ),
        command(
            "terminal.kill",
            "Stop a terminal's process",
            "terminal.kill",
            json!({
                "type": "object",
                "required": ["terminal"],
                "properties": { "terminal": { "type": "string" }, "force": { "type": "boolean" } }
            }),
        ),
        command(
            "terminal.close",
            "Stop a terminal and forget it",
            "terminal.close",
            selector("terminal"),
        ),
        command(
            "agent.kinds",
            "List agent kinds and whether they are installed",
            "agent.kinds",
            no_args(),
        ),
        command(
            "agent.start",
            "Start an agent in a workspace",
            "agent.start",
            json!({
                "type": "object",
                "required": ["workspace", "kind"],
                "properties": {
                    "workspace": { "type": "string" },
                    "kind": { "type": "string" },
                    "prompt": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" } },
                    "read_only": { "type": "boolean" },
                    "title": { "type": "string" },
                    "request_id": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.list",
            "List agent sessions",
            "agent.list",
            json!({ "type": "object", "properties": { "workspace": { "type": "string" } } }),
        ),
        command(
            "agent.status",
            "Show an agent session's execution and activity",
            "agent.show",
            json!({ "type": "object", "properties": { "session": { "type": "string" } } }),
        ),
        command(
            "agent.stop",
            "Stop an agent session's run",
            "agent.stop",
            json!({
                "type": "object",
                "required": ["session"],
                "properties": { "session": { "type": "string" }, "force": { "type": "boolean" } }
            }),
        ),
        command(
            "agent.prompt",
            "Type a prompt into an agent session's terminal",
            "agent.prompt",
            json!({
                "type": "object",
                "required": ["session", "text"],
                "properties": {
                    "session": { "type": "string" },
                    "text": { "type": "string" },
                    "force": { "type": "boolean" }
                }
            }),
        ),
        command(
            "agent.resume",
            "Start a new run of an exited agent session",
            "agent.resume",
            selector("session"),
        ),
        command(
            "agent.forget",
            "Forget an agent session whose run has ended",
            "agent.forget",
            selector("session"),
        ),
        command(
            "agent.hook",
            "Report an agent run's status from its hook",
            "agent.hook",
            json!({
                "type": "object",
                "required": ["session", "token", "event"],
                "properties": {
                    "session": { "type": "string" },
                    "token": { "type": "string" },
                    "event": { "type": "string" },
                    "payload": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.watch",
            "Stream agent sessions' execution and activity",
            "agent.watch",
            json!({
                "type": "object",
                "properties": { "workspace": { "type": "string" }, "session": { "type": "string" } }
            }),
        ),
        command(
            "agent.delegate",
            "Start a child session for a parent session",
            "agent.delegate",
            json!({
                "type": "object",
                "required": ["parent", "prompt"],
                "properties": {
                    "parent": { "type": "string" },
                    "prompt": { "type": "string" },
                    "kind": { "type": "string" },
                    "name": { "type": "string" },
                    "read_only": { "type": "boolean" },
                    "request_id": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.tree",
            "Show an agent session's delegation tree",
            "agent.tree",
            selector("session"),
        ),
        command(
            "agent.bring-back",
            "Merge a finished child session back into its parent",
            "agent.bring_back",
            json!({
                "type": "object",
                "required": ["child"],
                "properties": {
                    "child": { "type": "string" },
                    "merge": { "type": "boolean" },
                    "prompt": { "type": "boolean" }
                }
            }),
        ),
        command(
            "agent.attach",
            "Attach to an agent session's terminal",
            "agent.show",
            selector("session"),
        ),
    ]
}

pub(super) fn command(
    id: &str,
    title: &str,
    method: &str,
    args_schema: Value,
) -> CommandDescriptor {
    CommandDescriptor {
        id: id.to_owned(),
        title: title.to_owned(),
        method: method.to_owned(),
        args_schema,
    }
}

pub(super) fn no_args() -> Value {
    json!({ "type": "object", "properties": {} })
}

/// Arguments that name one project or workspace by selector.
pub(super) fn selector(name: &str) -> Value {
    let mut properties = Map::new();
    properties.insert(name.to_owned(), json!({ "type": "string" }));
    json!({ "type": "object", "required": [name], "properties": properties })
}
