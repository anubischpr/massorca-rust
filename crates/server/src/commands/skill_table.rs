//! The skill commands: `skills installed` and `skills roots` (SKL-01), placing skills for agent tools (SKL-02), and the skills
//! MassOrca ships (SKL-04).

use serde_json::{Value, json};

use super::CommandDescriptor;

pub(super) fn skill_commands() -> Vec<CommandDescriptor> {
    vec![
        command(
            "skills.installed",
            "List the installed skills this host finds",
            "skills.discover",
            discover_args(),
        ),
        command(
            "skills.roots",
            "List the skill roots this host reads, and why one was skipped",
            "skills.discover",
            discover_args(),
        ),
        command(
            "skills.preview",
            "Show what placing a skill for agent tools would do",
            "skills.place-preview",
            place_args(),
        ),
        command(
            "skills.place",
            "Place a skill for agent tools",
            "skills.place",
            place_args(),
        ),
        command(
            "skills.unplace",
            "Remove the copies MassOrca placed for a skill",
            "skills.unplace",
            json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "agents": { "type": "array", "items": { "enum": PLACEMENT_AGENTS } }
                }
            }),
        ),
        command(
            "skills.managed",
            "List the skills MassOrca placed on this host",
            "skills.managed",
            json!({ "type": "object", "properties": {} }),
        ),
        command(
            "skills.bundled.list",
            "List the skills MassOrca ships and whether each is placed on this host",
            "skills.bundled.list",
            json!({ "type": "object", "properties": {} }),
        ),
        command(
            "skills.bundled.install",
            "Install a skill MassOrca ships for the chosen agents, or for every agent folder on this host",
            "skills.bundled.install",
            bundled_install_args(),
        ),
        command(
            "skills.bundled.refresh",
            "Bring an installed skill MassOrca ships up to this build's text",
            "skills.bundled.refresh",
            bundled_name_args(),
        ),
        command(
            "skills.bundled.remove",
            "Remove a skill MassOrca ships from the agent folders it was placed in",
            "skills.unplace",
            bundled_name_args(),
        ),
    ]
}

/// The arguments of `skills.bundled.install`: a skill, and the agents to place it for (none means every agent folder here).
fn bundled_install_args() -> Value {
    json!({
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": { "type": "string" },
            "agents": { "type": "array", "items": { "enum": PLACEMENT_AGENTS } }
        }
    })
}

/// The arguments of a call that names a bundled skill only: `skills.bundled.refresh` and the removal.
fn bundled_name_args() -> Value {
    json!({
        "type": "object",
        "required": ["name"],
        "properties": { "name": { "type": "string" } }
    })
}

/// The agent ids a placement accepts; the same ids pv96 uses.
const PLACEMENT_AGENTS: [&str; 11] = [
    "codex", "claude", "cursor", "gemini", "droid", "continue", "trae", "grok", "aug", "muse",
    "zcode",
];

fn place_args() -> Value {
    json!({
        "type": "object",
        "required": ["skill_id", "agents"],
        "properties": {
            "skill_id": { "type": "string" },
            "agents": { "type": "array", "minItems": 1, "items": { "enum": PLACEMENT_AGENTS } }
        }
    })
}

fn command(id: &str, title: &str, method: &str, args_schema: Value) -> CommandDescriptor {
    CommandDescriptor {
        id: id.to_owned(),
        title: title.to_owned(),
        method: method.to_owned(),
        args_schema,
    }
}

fn discover_args() -> Value {
    json!({
        "type": "object",
        "properties": {
            "refresh": { "type": "boolean" },
            "source_kinds": {
                "type": "array",
                "items": { "enum": ["home", "repo", "bundled", "plugin"] }
            }
        }
    })
}
