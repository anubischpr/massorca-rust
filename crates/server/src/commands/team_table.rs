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
    ]
}
