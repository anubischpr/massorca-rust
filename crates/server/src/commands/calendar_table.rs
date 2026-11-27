//! The calendar command table: `calendar`, the runs of routines and automations in a window of days (decision 0124).

use serde_json::json;

use super::CommandDescriptor;
use super::core_table::command;

pub(super) fn calendar_commands() -> Vec<CommandDescriptor> {
    vec![command(
        "calendar",
        "List the runs of routines and automations in a window of up to 62 days",
        "calendar.occurrences",
        json!({
            "type": "object",
            "required": ["from_ms", "to_ms"],
            "properties": { "from_ms": { "type": "integer" }, "to_ms": { "type": "integer" }, "kind": { "type": "string", "enum": ["routine", "automation"] }, "include_done": { "type": "boolean" } }
        }),
    )]
}
