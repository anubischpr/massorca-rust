//! The Bot Teams command table. Ids are the CLI paths; the palette-only entries run methods that have no CLI path yet.

use serde_json::json;

use super::CommandDescriptor;
use super::core_table::{command, no_args, selector};

/// The team commands, then the palette entries for the team, bot and project methods without a CLI path.
pub fn team_commands() -> Vec<CommandDescriptor> {
    vec![
        command(
            "team.create",
            "Create a team",
            "team.create",
            json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "purpose": { "type": "string" },
                    "character": { "enum": ["triangle", "square", "circle", "diamond", "hexagon", "star"] },
                    "access": { "enum": ["full"] },
                    "workspaces": { "type": "boolean" }
                }
            }),
        ),
        command("team.list", "List teams", "team.list", no_args()),
        command("team.show", "Show a team", "team.show", selector("team")),
        command(
            "team.update",
            "Change a team",
            "team.update",
            json!({
                "type": "object",
                "required": ["team"],
                "properties": {
                    "team": { "type": "string" },
                    "name": { "type": "string" },
                    "purpose": { "type": "string" },
                    "character": { "enum": ["triangle", "square", "circle", "diamond", "hexagon", "star"] },
                    "access": { "enum": ["full"] },
                    "workspaces": { "type": "boolean" }
                }
            }),
        ),
        command(
            "team.rm",
            "Remove a team with its bots, projects and tasks",
            "team.rm",
            json!({
                "type": "object",
                "required": ["team"],
                "properties": { "team": { "type": "string" }, "force": { "type": "boolean" } }
            }),
        ),
        command(
            "team.bot.list",
            "List a team's bots",
            "team.bot_list",
            selector("team"),
        ),
        command(
            "team.bot.add",
            "Add a bot to a team",
            "team.bot_add",
            json!({
                "type": "object",
                "required": ["team", "name", "job", "kind"],
                "properties": {
                    "team": { "type": "string" },
                    "name": { "type": "string" },
                    "job": { "type": "string" },
                    "kind": { "type": "string" },
                    "model": { "type": "string" },
                    "effort": { "type": "string" },
                    "lead": { "type": "boolean" }
                }
            }),
        ),
        command(
            "team.bot.update",
            "Change a team's bot",
            "team.bot_update",
            json!({
                "type": "object",
                "required": ["team", "bot"],
                "properties": {
                    "team": { "type": "string" },
                    "bot": { "type": "string" },
                    "name": { "type": "string" },
                    "job": { "type": "string" },
                    "kind": { "type": "string" },
                    "model": { "type": "string" },
                    "effort": { "type": "string" },
                    "lead": { "type": "boolean" }
                }
            }),
        ),
        command(
            "team.bot.rm",
            "Remove a bot from a team",
            "team.bot_rm",
            json!({
                "type": "object",
                "required": ["team", "bot"],
                "properties": {
                    "team": { "type": "string" },
                    "bot": { "type": "string" },
                    "unassign": { "type": "boolean" }
                }
            }),
        ),
        command(
            "team.project.list",
            "List a team's projects",
            "team.project_list",
            selector("team"),
        ),
        command(
            "team.project.add",
            "Add a project to a team",
            "team.project_add",
            json!({
                "type": "object",
                "required": ["team", "name"],
                "properties": {
                    "team": { "type": "string" },
                    "name": { "type": "string" },
                    "goal": { "type": "string" },
                    "brief": { "type": "string" },
                    "source_project": { "type": "string" }
                }
            }),
        ),
        command(
            "team.project.show",
            "Show a team's project",
            "team.project_show",
            json!({
                "type": "object",
                "required": ["team", "project"],
                "properties": {
                    "team": { "type": "string" },
                    "project": { "type": "string" }
                }
            }),
        ),
        command(
            "team.project.update",
            "Change a team's project",
            "team.project_update",
            json!({
                "type": "object",
                "required": ["team", "project"],
                "properties": {
                    "team": { "type": "string" },
                    "project": { "type": "string" },
                    "name": { "type": "string" },
                    "goal": { "type": "string" },
                    "brief": { "type": "string" },
                    "source_project": { "type": "string" }
                }
            }),
        ),
        command(
            "team.project.rm",
            "Remove a team's project",
            "team.project_rm",
            json!({
                "type": "object",
                "required": ["team", "project"],
                "properties": {
                    "team": { "type": "string" },
                    "project": { "type": "string" }
                }
            }),
        ),
        command(
            "team.room.list",
            "List a team's Conference room, oldest first",
            "team.room_list",
            json!({
                "type": "object",
                "required": ["team"],
                "properties": {
                    "team": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 0 }
                }
            }),
        ),
        command(
            "team.room.post",
            "Post a line to a team's Conference room; a person's line becomes a task, a routine or a choice card",
            "team.room_post",
            json!({
                "type": "object",
                "required": ["team", "text"],
                "properties": {
                    "team": { "type": "string" },
                    "text": { "type": "string" },
                    "bot": { "type": "string" },
                    "to": { "type": "string" }
                }
            }),
        ),
        command(
            "team.room.answer",
            "Answer a Conference room choice card once: a bot, another team, or this team's Todo",
            "team.room_answer",
            json!({
                "type": "object",
                "required": ["team", "message", "choice"],
                "properties": {
                    "team": { "type": "string" },
                    "message": { "type": "string" },
                    "choice": { "type": "string" }
                }
            }),
        ),
        command(
            "team.memory.list",
            "List a team's memory lines, or one bot's",
            "team.memory_list",
            json!({
                "type": "object",
                "required": ["team"],
                "properties": {
                    "team": { "type": "string" },
                    "bot": { "type": "string" }
                }
            }),
        ),
        command(
            "team.memory.add",
            "Add a line to a team's memory",
            "team.memory_add",
            json!({
                "type": "object",
                "required": ["team", "text"],
                "properties": {
                    "team": { "type": "string" },
                    "bot": { "type": "string" },
                    "text": { "type": "string" },
                    "task": { "type": "string" }
                }
            }),
        ),
        command(
            "team.memory.rm",
            "Remove a line from a team's memory",
            "team.memory_rm",
            json!({
                "type": "object",
                "required": ["team", "line"],
                "properties": {
                    "team": { "type": "string" },
                    "line": { "type": "string" }
                }
            }),
        ),
        command(
            "team.routine.list",
            "List a team's routines",
            "team.routine_list",
            json!({
                "type": "object",
                "required": ["team"],
                "properties": { "team": { "type": "string" } }
            }),
        ),
        command(
            "team.routine.add",
            "Add a routine to a team",
            "team.routine_add",
            json!({
                "type": "object",
                "required": ["team", "bot", "name", "prompt", "cadence", "time"],
                "properties": { "team": { "type": "string" }, "bot": { "type": "string" }, "name": { "type": "string" }, "prompt": { "type": "string" }, "cadence": { "type": "string" }, "time": { "type": "string" }, "weekday": { "type": "string" }, "month_day": { "type": "integer" }, "run_date": { "type": "string" }, "zone": { "type": "string" } }
            }),
        ),
        command(
            "team.routine.update",
            "Change a team's routine",
            "team.routine_update",
            json!({
                "type": "object",
                "required": ["team", "routine"],
                "properties": { "team": { "type": "string" }, "routine": { "type": "string" }, "bot": { "type": "string" }, "name": { "type": "string" }, "prompt": { "type": "string" }, "cadence": { "type": "string" }, "time": { "type": "string" }, "weekday": { "type": "string" }, "month_day": { "type": "integer" }, "run_date": { "type": "string" }, "zone": { "type": "string" } }
            }),
        ),
        command(
            "team.routine.rm",
            "Remove a team's routine and its runs",
            "team.routine_rm",
            json!({
                "type": "object",
                "required": ["team", "routine"],
                "properties": { "team": { "type": "string" }, "routine": { "type": "string" } }
            }),
        ),
        command(
            "team.routine.pause",
            "Pause a team's routine",
            "team.routine_pause",
            json!({
                "type": "object",
                "required": ["team", "routine"],
                "properties": { "team": { "type": "string" }, "routine": { "type": "string" } }
            }),
        ),
        command(
            "team.routine.resume",
            "Resume a team's routine from now",
            "team.routine_resume",
            json!({
                "type": "object",
                "required": ["team", "routine"],
                "properties": { "team": { "type": "string" }, "routine": { "type": "string" } }
            }),
        ),
        command(
            "team.routine.runs",
            "List a routine's runs, newest first",
            "team.routine_runs",
            json!({
                "type": "object",
                "required": ["team", "routine"],
                "properties": { "team": { "type": "string" }, "routine": { "type": "string" }, "limit": { "type": "integer" } }
            }),
        ),
        command(
            "team.routine.run",
            "Run a routine once now",
            "team.routine_run",
            json!({
                "type": "object",
                "required": ["team", "routine"],
                "properties": { "team": { "type": "string" }, "routine": { "type": "string" } }
            }),
        ),
        command(
            "team.board",
            "Show a team's Board",
            "team.board",
            json!({
                "type": "object",
                "required": ["team"],
                "properties": { "team": { "type": "string" }, "project": { "type": "string" } }
            }),
        ),
        command(
            "task.create",
            "Create a task on a team's Board",
            "task.create",
            json!({
                "type": "object",
                "required": ["team", "title"],
                "properties": {
                    "team": { "type": "string" },
                    "title": { "type": "string" },
                    "body": { "type": "string" },
                    "bot": { "type": "string" },
                    "project": { "type": "string" },
                    "origin": { "enum": ["room", "cli"] }
                }
            }),
        ),
        command(
            "task.list",
            "List tasks, filtered by team, project, bot or column",
            "task.list",
            json!({
                "type": "object",
                "properties": {
                    "team": { "type": "string" },
                    "project": { "type": "string" },
                    "bot": { "type": "string" },
                    "column": { "enum": ["open", "running", "waiting", "done"] }
                }
            }),
        ),
        command("task.show", "Show a task", "task.show", selector("task")),
        command(
            "task.update",
            "Change a task",
            "task.update",
            json!({
                "type": "object",
                "required": ["task"],
                "properties": {
                    "task": { "type": "string" },
                    "title": { "type": "string" },
                    "body": { "type": "string" },
                    "bot": { "type": "string" },
                    "project": { "type": "string" }
                }
            }),
        ),
        command(
            "task.start",
            "Start a task's bot on it",
            "task.start",
            selector("task"),
        ),
        command(
            "task.done",
            "Mark a task done by hand",
            "task.done",
            selector("task"),
        ),
        command(
            "task.reopen",
            "Reopen a task: clear its outcome and error",
            "task.reopen",
            selector("task"),
        ),
        command(
            "task.stop",
            "Stop the agent a task runs in",
            "task.stop",
            json!({
                "type": "object",
                "required": ["task"],
                "properties": { "task": { "type": "string" }, "force": { "type": "boolean" } }
            }),
        ),
        command("task.rm", "Remove a task", "task.rm", selector("task")),
        command(
            "team.delegate",
            "Hand a task to a teammate from inside a bot's run",
            "team.delegate",
            json!({
                "type": "object",
                "required": ["text"],
                "properties": {
                    "text": { "type": "string" },
                    "bot": { "type": "string" },
                    "team": { "type": "string" },
                    "read_only": { "type": "boolean" }
                }
            }),
        ),
        command(
            "team.message.send",
            "Message the parent's or a child's bot from inside a bot's run",
            "team.message_send",
            json!({
                "type": "object",
                "required": ["to", "type", "subject", "body"],
                "properties": {
                    "to": { "type": "string" },
                    "type": { "enum": ["status", "question", "worker_done", "handoff", "escalation"] },
                    "priority": { "enum": ["normal", "high", "urgent"] },
                    "subject": { "type": "string" },
                    "body": { "type": "string" },
                    "outcome": { "enum": ["succeeded", "failed"] },
                    "reply_to": { "type": "string" }
                }
            }),
        ),
        command(
            "team.message.reply",
            "Answer a message in this bot run's inbox",
            "team.message_reply",
            json!({
                "type": "object",
                "required": ["message", "body"],
                "properties": {
                    "message": { "type": "string" },
                    "body": { "type": "string" },
                    "type": { "enum": ["status", "question", "handoff", "escalation"] },
                    "priority": { "enum": ["normal", "high", "urgent"] }
                }
            }),
        ),
        command(
            "team.inbox",
            "List this bot run's unread messages",
            "team.inbox",
            json!({ "type": "object", "properties": { "mark_read": { "type": "boolean" } } }),
        ),
    ]
}
