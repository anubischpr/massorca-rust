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
            "service.tools",
            "Show the git, forge CLI and shell tools on the service's host",
            "service.tools",
            json!({
                "type": "object",
                "properties": { "refresh": { "type": "boolean" } }
            }),
        ),
        command(
            "agent-context",
            "List every command with its method, permission and parameters",
            "commands.describe",
            no_args(),
        ),
        command(
            "host.memory",
            "Show the memory of the service and its terminals",
            "host.memory",
            no_args(),
        ),
        command(
            "host.ports",
            "Show the listening ports of the terminals' processes",
            "host.ports",
            no_args(),
        ),
        // A CLI composition like `service.status`: its method is the first request, `service.info`, on the host.
        command(
            "host.self-test",
            "Check a paired host step by step, or this computer's own service: address, port, key, protocol, pairing, service and round trip",
            "service.info",
            json!({ "type": "object", "properties": { "name": { "type": "string" } } }),
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
                    "workspaces_root": { "type": "string" },
                    "icon": { "type": "object", "description": "A Lucide icon, an emoji or an image; unset takes the icon the repository's files hold" }
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
            "project.inspect",
            "Show what adding a path would register",
            "project.inspect",
            json!({
                "type": "object",
                "required": ["path"],
                "properties": { "path": { "type": "string" } }
            }),
        ),
        command(
            "project.rm",
            "Remove a project",
            "project.remove",
            json!({
                "type": "object",
                "required": ["project"],
                "properties": { "project": { "type": "string" }, "forget_workspaces": { "type": "boolean" } }
            }),
        ),
        command(
            "project.set-base",
            "Set or clear a project's base branch",
            "project.set_base",
            json!({
                "type": "object",
                "required": ["project"],
                "properties": {
                    "project": { "type": "string" },
                    "base": { "type": ["string", "null"] }
                }
            }),
        ),
        command(
            "project.update",
            "Change a project's icon",
            "project.update",
            json!({
                "type": "object",
                "required": ["project"],
                "properties": {
                    "project": { "type": "string" },
                    "icon": {
                        "type": ["object", "null"],
                        "description": "A Lucide icon, an emoji or an image; null clears the icon and an absent field keeps it"
                    }
                }
            }),
        ),
        command(
            "project.scripts",
            "Show a project's orca.yaml scripts with their hashes and trust",
            "project.scripts",
            json!({
                "type": "object",
                "required": ["project"],
                "properties": { "project": { "type": "string" }, "workspace": { "type": "string" } }
            }),
        ),
        command(
            "project.trust-scripts",
            "Show a project's orca.yaml scripts and trust one by its hash",
            "project.trust_script",
            json!({
                "type": "object",
                "required": ["project"],
                "properties": {
                    "project": { "type": "string" },
                    "workspace": { "type": "string" },
                    "hash": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Hashes of shown scripts to trust; each is sent to project.trust_script"
                    }
                }
            }),
        ),
        command(
            "workspace.create",
            "Create a workspace",
            "workspace.create",
            json!({
                "type": "object",
                "required": ["project"],
                "properties": {
                    "project": { "type": "string" },
                    "name": { "type": "string" },
                    "branch": { "type": "string" },
                    "base": { "type": "string" },
                    "existing_branch": { "type": "boolean" },
                    "refresh_base": { "type": "boolean" },
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
            "device.forget",
            "Remove a revoked device from the list",
            "device.forget",
            selector("device"),
        ),
        command(
            "device.grant",
            "Give a paired device full access",
            "device.grant",
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
                    "admin": { "type": "boolean" },
                    "addresses": { "type": "array", "items": { "type": "string" }, "maxItems": 8 }
                }
            }),
        ),
        command(
            "pair.cancel",
            "Retire a pending pairing offer before it expires",
            "pairing.cancel",
            json!({
                "type": "object",
                "properties": {
                    "offer_id": { "type": "string" },
                    "all": { "type": "boolean" }
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
            "workspace.current",
            "Show the workspace that contains a directory",
            "workspace.current",
            json!({
                "type": "object",
                "required": ["path"],
                "properties": { "path": { "type": "string" } }
            }),
        ),
        command(
            "workspace.rm",
            "Remove a workspace",
            "workspace.remove",
            json!({
                "type": "object",
                "required": ["workspace"],
                "properties": {
                    "workspace": { "type": "string" },
                    "force": { "type": "boolean" },
                    "allow_failed_archive": { "type": "boolean" }
                }
            }),
        ),
        command(
            "workspace.update",
            "Rename a workspace or set its comment",
            "workspace.update",
            json!({
                "type": "object",
                "required": ["workspace"],
                "properties": {
                    "workspace": { "type": "string" },
                    "name": { "type": "string" },
                    "comment": { "type": "string" }
                }
            }),
        ),
        command(
            "workspace.rename",
            "Rename a workspace whose name the service chose, and its branch",
            "workspace.rename",
            json!({
                "type": "object",
                "required": ["workspace", "name", "from_generated"],
                "properties": {
                    "workspace": { "type": "string" },
                    "name": { "type": "string" },
                    "from_generated": { "type": "boolean" },
                    "expected_branch": { "type": "string" }
                }
            }),
        ),
        command(
            "workspace.check",
            "Check a workspace's folder and its git registration",
            "workspace.check",
            selector("workspace"),
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
                    "request_id": { "type": "string" },
                    "permission_mode": { "type": "string" }
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
            "terminal.wait",
            "Wait until a terminal's process exits, up to a timeout",
            "terminal.wait",
            json!({
                "type": "object",
                "required": ["terminal", "condition"],
                "properties": {
                    "terminal": { "type": "string" },
                    "condition": { "type": "string", "enum": ["exit"] },
                    "timeout_ms": { "type": "integer" }
                }
            }),
        ),
        command(
            "agent.kinds",
            "List agent kinds: package state (installed), whether a program is detected, and problems",
            "agent.kinds",
            no_args(),
        ),
        command(
            "agent.models",
            "List the models a kind's picker offers for an account: declared choices, then the vendor CLI's listing",
            "agent.models",
            json!({
                "type": "object",
                "required": ["kind"],
                "properties": {
                    "kind": { "type": "string" },
                    "account": { "type": "string" },
                    "refresh": { "type": "boolean" }
                }
            }),
        ),
        command(
            "agent.start",
            "Start an agent in a workspace, or with no project in the conversations folder",
            "agent.start",
            json!({
                "type": "object",
                "required": ["kind"],
                "properties": {
                    "workspace": { "type": "string" },
                    "kind": { "type": "string" },
                    "prompt": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" } },
                    "read_only": { "type": "boolean" },
                    "title": { "type": "string" },
                    "request_id": { "type": "string" },
                    "model": { "type": "string" },
                    "effort": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.list",
            "List agent sessions",
            "agent.list",
            json!({
                "type": "object",
                "properties": { "workspace": { "type": "string" }, "archived": { "type": "boolean" } }
            }),
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
                    "force": { "type": "boolean" },
                    "attachments": attachments_schema()
                }
            }),
        ),
        command(
            "agent.queue.add",
            "Queue a prompt behind a busy agent",
            "agent.queue",
            json!({
                "type": "object",
                "required": ["session", "text"],
                "properties": {
                    "session": { "type": "string" },
                    "text": { "type": "string" },
                    "request_id": { "type": "string" },
                    "attachments": attachments_schema()
                }
            }),
        ),
        command(
            "agent.queue.list",
            "List the queued prompts of a session",
            "agent.queue.list",
            selector("session"),
        ),
        command(
            "agent.queue.edit",
            "Replace the text of a waiting or failed queued prompt",
            "agent.queue.edit",
            json!({
                "type": "object",
                "required": ["id", "text"],
                "properties": {
                    "id": { "type": "string" },
                    "text": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.queue.remove",
            "Remove a waiting or failed queued prompt",
            "agent.queue.remove",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }),
        ),
        command(
            "agent.queue.send-now",
            "Send a queued prompt at once, ahead of the queue",
            "agent.queue.send_now",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }),
        ),
        command(
            "agent.queue.resume",
            "Lift a session's queue pauses and deliver its head once the agent is ready",
            "agent.queue.resume",
            selector("session"),
        ),
        command(
            "agent.resume",
            "Start a new run of an exited agent session",
            "agent.resume",
            json!({
                "type": "object",
                "required": ["session"],
                "properties": {
                    "session": { "type": "string" },
                    "permission_mode": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.fork",
            "Start a new agent session from a copy of an exited session's conversation",
            "agent.fork",
            json!({
                "type": "object",
                "required": ["session"],
                "properties": {
                    "session": { "type": "string" },
                    "prompt": { "type": "string" },
                    "permission_mode": { "type": "string" },
                    "title": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.mode",
            "Switch an agent session's permission mode, live or at its next resume",
            "agent.set_mode",
            json!({
                "type": "object",
                "required": ["session", "permission_mode"],
                "properties": {
                    "session": { "type": "string" },
                    "permission_mode": { "type": "string" },
                    "force": { "type": "boolean" }
                }
            }),
        ),
        command(
            "agent.set-options",
            "Change an agent session's model, effort or fast mode, live where its kind allows or at its next resume",
            "agent.set_options",
            json!({
                "type": "object",
                "required": ["session"],
                "properties": {
                    "session": { "type": "string" },
                    "model": { "type": "string" },
                    "effort": { "type": "string" },
                    "fast": { "type": "boolean" }
                }
            }),
        ),
        command(
            "agent.seen",
            "Mark an agent session's waiting or done state as seen",
            "agent.mark_seen",
            selector("session"),
        ),
        command(
            "agent.forget",
            "Forget an agent session whose run has ended; with remove_workspace, remove its workspace too",
            "agent.forget",
            json!({
                "type": "object",
                "required": ["session"],
                "properties": {
                    "session": { "type": "string" },
                    "remove_workspace": { "type": "boolean" },
                    "force": { "type": "boolean" },
                    "discard_ignored": { "type": "boolean" },
                    "expect_dirty": { "type": "boolean" }
                }
            }),
        ),
        command(
            "agent.forget_preview",
            "Say what forgetting an agent session would do to it and to its workspace, changing nothing",
            "agent.forget_preview",
            selector("session"),
        ),
        command(
            "agent.rename",
            "Set an agent session's title",
            "agent.rename",
            json!({
                "type": "object",
                "required": ["session", "title"],
                "properties": { "session": { "type": "string" }, "title": { "type": "string" } }
            }),
        ),
        command(
            "agent.hide",
            "Archive (hide) an exited agent session from agent lists, or show it again with archived false",
            "agent.archive_session",
            json!({
                "type": "object",
                "required": ["session", "archived"],
                "properties": { "session": { "type": "string" }, "archived": { "type": "boolean" } }
            }),
        ),
        command(
            "agent.release",
            "Archive an exited agent session's output and free its terminal; the record stays",
            "agent.release",
            selector("session"),
        ),
        command(
            "agent.retain",
            "Keep an agent session's exited terminal from eviction, or clear it with retained false",
            "agent.retain",
            json!({
                "type": "object",
                "required": ["session", "retained"],
                "properties": { "session": { "type": "string" }, "retained": { "type": "boolean" } }
            }),
        ),
        command(
            "agent.archive",
            "Show the archived output of a released agent session",
            "agent.archive",
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
            "agent.chat.subscribe",
            "Stream a chat session's transcript from a cursor",
            "agent.chat.subscribe",
            json!({
                "type": "object",
                "required": ["session"],
                "properties": {
                    "session": { "type": "string" },
                    "after": {
                        "type": "object",
                        "properties": { "epoch": { "type": "string" }, "seq": { "type": "integer" } }
                    },
                    "limit": { "type": "integer" }
                }
            }),
        ),
        command(
            "agent.chat.start",
            "Start an agent session whose run is a structured chat",
            "agent.chat.start",
            json!({
                "type": "object",
                "required": ["kind"],
                "properties": {
                    "kind": { "type": "string" },
                    "workspace": { "type": "string" },
                    "prompt": { "type": "string" },
                    "title": { "type": "string" },
                    "read_only": { "type": "boolean" },
                    "permission_mode": { "type": "string" },
                    "model": { "type": "string" },
                    "effort": { "type": "string" },
                    "request_id": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.chat.approval.answer",
            "Answer a chat session's permission request",
            "agent.chat.approval.answer",
            json!({
                "type": "object",
                "required": ["session", "request_id", "decision"],
                "properties": {
                    "session": { "type": "string" },
                    "request_id": { "type": "string" },
                    "decision": { "type": "string", "enum": ["allow", "deny"] },
                    "scope": { "type": "string", "enum": ["once", "always"] },
                    "message": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.chat.interrupt",
            "Interrupt a chat session's running turn and keep the session",
            "agent.chat.interrupt",
            selector("session"),
        ),
        command(
            "agent.chat.history",
            "Page back through a chat session's transcript",
            "agent.chat.history",
            json!({
                "type": "object",
                "required": ["session"],
                "properties": {
                    "session": { "type": "string" },
                    "before": {
                        "type": "object",
                        "properties": { "epoch": { "type": "string" }, "seq": { "type": "integer" } }
                    },
                    "limit": { "type": "integer" }
                }
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
                    "request_id": { "type": "string" },
                    "permission_mode": { "type": "string" }
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
        command(
            "agent.credentials.import",
            "Copy an agent kind's login files into this profile",
            "agent.credentials.import",
            json!({
                "type": "object",
                "required": ["kind", "from"],
                "properties": {
                    "kind": { "type": "string" },
                    "from": { "type": "string" },
                    "replace": { "type": "boolean" }
                }
            }),
        ),
        command(
            "agent.credentials.show",
            "Show an agent kind's credential home in this profile",
            "agent.credentials.show",
            selector("kind"),
        ),
        command(
            "agent.credentials.remove",
            "Delete an agent kind's credential home in this profile",
            "agent.credentials.remove",
            selector("kind"),
        ),
        command(
            "agent.accounts.list",
            "List an agent kind's accounts and their states",
            "agent.accounts.list",
            selector("kind"),
        ),
        command(
            "agent.accounts.add",
            "Sign a new account of an agent kind in",
            "agent.accounts.add",
            json!({
                "type": "object",
                "required": ["kind"],
                "properties": {
                    "kind": { "type": "string" },
                    "label": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.accounts.sign-in",
            "Sign an existing account of an agent kind in again",
            "agent.accounts.sign_in",
            json!({
                "type": "object",
                "required": ["kind", "account"],
                "properties": {
                    "kind": { "type": "string" },
                    "account": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.accounts.import",
            "Import an agent kind's login files into a new account",
            "agent.accounts.import",
            json!({
                "type": "object",
                "required": ["kind"],
                "properties": {
                    "kind": { "type": "string" },
                    "from": { "type": "string" },
                    "label": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.accounts.candidates",
            "Show where an import without a folder would copy each agent kind's login files from",
            "agent.accounts.import_candidates",
            json!({
                "type": "object",
                "properties": {
                    "kind": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.accounts.activate",
            "Make an agent account the active one, or a project's choice",
            "agent.accounts.activate",
            json!({
                "type": "object",
                "required": ["kind", "account"],
                "properties": {
                    "kind": { "type": "string" },
                    "account": { "type": "string" },
                    "project": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.accounts.remove",
            "Delete an agent account",
            "agent.accounts.remove",
            json!({
                "type": "object",
                "required": ["kind", "account"],
                "properties": {
                    "kind": { "type": "string" },
                    "account": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.accounts.check",
            "Check an agent account's sign-in now",
            "agent.accounts.check",
            json!({
                "type": "object",
                "required": ["kind"],
                "properties": {
                    "kind": { "type": "string" },
                    "account": { "type": "string" }
                }
            }),
        ),
        command(
            "agent.title",
            "Name a chat from its first prompt and reply",
            "agent.generate_title",
            json!({
                "type": "object",
                "required": ["kind", "prompt"],
                "properties": {
                    "kind": { "type": "string" },
                    "prompt": { "type": "string" },
                    "reply": { "type": "string" },
                    "model": { "type": "string" },
                    "account": { "type": "string" }
                }
            }),
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

/// The `attachments` of `agent.prompt` and `agent.queue.add`: files of the session's workspace (decision 0172).
pub(super) fn attachments_schema() -> Value {
    json!({
        "type": "array",
        "maxItems": 10,
        "items": {
            "type": "object",
            "required": ["path"],
            "properties": { "path": { "type": "string" } }
        }
    })
}
