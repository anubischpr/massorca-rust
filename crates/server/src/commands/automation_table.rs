//! The automation command table: `automations create|edit|show|list|remove|enable|disable` (decision 0123) and
//! `automations run|runs` (decision 0125).

use serde_json::json;

use super::CommandDescriptor;
use super::core_table::{command, no_args};

pub(super) fn automation_commands() -> Vec<CommandDescriptor> {
    vec![
        command(
            "automations.create",
            "Create a scheduled automation in a project",
            "automation.create",
            json!({
                "type": "object",
                "required": ["project", "name", "prompt", "agent", "cadence", "time"],
                "properties": { "project": { "type": "string" }, "name": { "type": "string" }, "prompt": { "type": "string" }, "agent": { "type": "string" }, "cadence": { "type": "string" }, "time": { "type": "string" }, "weekday": { "type": "string" }, "cron": { "type": "string" }, "zone": { "type": "string" }, "grace_minutes": { "type": "integer" }, "creation_key": { "type": "string" } }
            }),
        ),
        command(
            "automations.edit",
            "Change an automation",
            "automation.update",
            json!({
                "type": "object",
                "required": ["automation"],
                "properties": { "automation": { "type": "string" }, "project": { "type": "string" }, "name": { "type": "string" }, "prompt": { "type": "string" }, "agent": { "type": "string" }, "cadence": { "type": "string" }, "time": { "type": "string" }, "weekday": { "type": "string" }, "cron": { "type": "string" }, "zone": { "type": "string" }, "grace_minutes": { "type": "integer" } }
            }),
        ),
        command(
            "automations.show",
            "Show one automation, by id or name",
            "automation.show",
            automation_selector(),
        ),
        command(
            "automations.list",
            "List the scheduled automations",
            "automation.list",
            no_args(),
        ),
        command(
            "automations.remove",
            "Remove an automation",
            "automation.rm",
            automation_selector(),
        ),
        command(
            "automations.enable",
            "Enable an automation and schedule its next run",
            "automation.enable",
            automation_selector(),
        ),
        command(
            "automations.disable",
            "Disable an automation",
            "automation.disable",
            automation_selector(),
        ),
        command(
            "automations.run",
            "Run an automation now",
            "automation.run_now",
            automation_selector(),
        ),
        command(
            "automations.runs",
            "List an automation's runs, newest first",
            "automation.runs",
            json!({
                "type": "object",
                "required": ["automation"],
                "properties": { "automation": { "type": "string" }, "limit": { "type": "integer" }, "before": { "type": "integer" } }
            }),
        ),
    ]
}

fn automation_selector() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["automation"],
        "properties": { "automation": { "type": "string" } }
    })
}
