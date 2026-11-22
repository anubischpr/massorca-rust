//! The secret commands (decision 0071): one per CLI leaf of `massorca secret`; no schema has a value property.

use serde_json::{Value, json};

use super::CommandDescriptor;

pub(super) fn secret_commands() -> Vec<CommandDescriptor> {
    vec![
        command(
            "secret.status",
            "Show where the vault key is kept, whether it opens, and how many secrets it holds",
            "secret.status",
            json!({ "type": "object", "properties": {} }),
        ),
        command(
            "secret.list",
            "List secret names by scope, never values",
            "secret.list",
            json!({ "type": "object", "properties": { "scope": { "type": "string" } } }),
        ),
        command(
            "secret.set",
            "Store or rotate a secret; the value is read from stdin or a hidden prompt",
            "secret.set",
            named_args(json!({ "replace": { "type": "boolean" } })),
        ),
        command(
            "secret.link",
            "Use another scope's secret under the same name",
            "secret.link",
            json!({
                "type": "object",
                "required": ["scope", "name", "from_scope"],
                "properties": {
                    "scope": { "type": "string" },
                    "name": { "type": "string" },
                    "from_scope": { "type": "string" }
                }
            }),
        ),
        command(
            "secret.unlink",
            "Remove a link; the owner's secret stays",
            "secret.unlink",
            named_args(json!({})),
        ),
        command(
            "secret.remove",
            "Remove an owned secret; refuses one with links unless told to remove them too",
            "secret.remove",
            named_args(json!({
                "include_links": { "type": "boolean" },
                "force": { "type": "boolean" }
            })),
        ),
        command(
            "secret.request",
            "Ask for a secret by name, with a reason the owner sees",
            "secret.request",
            named_args(json!({
                "reason": { "type": "string" },
                "session": { "type": "string" }
            })),
        ),
        command(
            "secret.requests",
            "List open requests for secrets, or every request",
            "secret.requests",
            json!({ "type": "object", "properties": { "all": { "type": "boolean" } } }),
        ),
        command(
            "secret.reset-key",
            "Forget the key of a vault that holds no secret, so the next secret creates a fresh one",
            "secret.reset_key",
            json!({ "type": "object", "properties": {} }),
        ),
        command(
            "secret.decline",
            "Decline a pending request for a secret",
            "secret.decline",
            json!({
                "type": "object",
                "required": ["request"],
                "properties": { "request": { "type": "string" } }
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

/// A schema that requires `scope` and `name` and allows `extra` properties.
fn named_args(extra: Value) -> Value {
    let mut properties = json!({ "scope": { "type": "string" }, "name": { "type": "string" } });
    if let (Some(all), Some(extra)) = (properties.as_object_mut(), extra.as_object()) {
        all.extend(extra.clone());
    }
    json!({ "type": "object", "required": ["scope", "name"], "properties": properties })
}
