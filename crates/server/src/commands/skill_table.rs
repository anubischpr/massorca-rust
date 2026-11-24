//! The skill commands (SKL-01): `skills installed` lists the skills found, `skills roots` the roots they were read from.

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
