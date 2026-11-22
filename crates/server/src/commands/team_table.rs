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
                    "brief": { "type": "string" }
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
                    "brief": { "type": "string" }
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
            "team.board",
            "Show a team's Board",
            "team.board",
            json!({
                "type": "object",
                "required": ["team"],
                "properties": { "team": { "type": "string" }, "project": { "type": "string" } }
            }),
        ),
    ]
}
