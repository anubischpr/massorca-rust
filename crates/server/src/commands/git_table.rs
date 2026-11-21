//! The git commands (decision 0010): one per `git/1` method, ids are the CLI paths.

use serde_json::{Value, json};

use super::CommandDescriptor;

pub(super) fn git_commands() -> Vec<CommandDescriptor> {
    vec![
        command(
            "git.status",
            "Show the changed files of a workspace",
            "git.status",
            workspace_args(json!({ "limit": { "type": "integer", "minimum": 1 } })),
        ),
        command(
            "git.diff",
            "List the files a comparison changes",
            "git.diff",
            workspace_args(json!({
                "mode": { "enum": ["worktree", "staged", "head", "range"] },
                "from": { "type": "string" },
                "to": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1 }
            })),
        ),
        command(
            "git.log",
            "Show the history of a workspace, a page at a time",
            "git.log",
            workspace_args(json!({
                "revision": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1 },
                "cursor": { "type": "string" }
            })),
        ),
        command(
            "git.patch",
            "Show the patch of one file",
            "git.diff_file",
            workspace_args(json!({
                "path": { "type": "string" },
                "mode": { "enum": ["worktree", "staged", "head", "range"] },
                "from": { "type": "string" },
                "to": { "type": "string" },
                "original_path": { "type": "string" },
                "max_bytes": { "type": "integer", "minimum": 1 }
            })),
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

/// A schema that requires `workspace` and allows `extra` properties.
fn workspace_args(extra: Value) -> Value {
    let mut properties = json!({ "workspace": { "type": "string" } });
    if let (Some(all), Some(extra)) = (properties.as_object_mut(), extra.as_object()) {
        all.extend(extra.clone());
    }
    json!({ "type": "object", "required": ["workspace"], "properties": properties })
}
