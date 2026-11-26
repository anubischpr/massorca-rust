//! What a tool call's input and result say: the text an item shows, a plan's entries, and an edit's patch.

use massorca_protocol::methods::chat::{PlanEntry, PlanStatus};
use serde_json::Value;

use super::{bounded, text_of};

/// Most entries a plan item carries.
const MAX_PLAN_ENTRIES: usize = 200;

/// A tool's input as one line of text: a command, a path or a pattern when the tool takes one, else its JSON.
pub(super) fn input_text(name: &str, input: &Value) -> String {
    let field = match name {
        "Bash" | "PowerShell" => "command",
        "Read" | "Edit" | "MultiEdit" | "Write" => "file_path",
        "NotebookEdit" => "notebook_path",
        "Glob" | "Grep" => "pattern",
        "WebFetch" => "url",
        "WebSearch" => "query",
        _ => "",
    };
    match input.get(field).and_then(Value::as_str) {
        Some(text) if !field.is_empty() => text.to_owned(),
        _ => match input {
            Value::Null => String::new(),
            other => other.to_string(),
        },
    }
}

/// A tool result's content as text: a string, or the text parts of a list.
pub(super) fn result_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| match part {
                Value::String(text) => Some(text.as_str()),
                _ => part["text"].as_str().filter(|_| part["type"] == "text"),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub(super) fn plan_entries(input: &Value) -> Vec<PlanEntry> {
    input["todos"]
        .as_array()
        .map(|todos| {
            todos
                .iter()
                .filter_map(|todo| {
                    let text = text_of(&todo["content"])?;
                    let status = match todo["status"].as_str() {
                        Some("pending") => PlanStatus::Pending,
                        Some("in_progress") => PlanStatus::InProgress,
                        Some("completed") => PlanStatus::Completed,
                        _ => PlanStatus::Unknown,
                    };
                    Some(PlanEntry {
                        text: bounded(text).0,
                        status,
                    })
                })
                .take(MAX_PLAN_ENTRIES)
                .collect()
        })
        .unwrap_or_default()
}

/// A unified diff of an edit tool's change: the CLI's structured patch when its result carries one, else one hunk built
/// from the tool's input (whose line numbers the input does not give).
pub(super) fn edit_patch(name: &str, input: &Value, structured: Option<&Value>) -> String {
    if let Some(hunks) = structured.and_then(|result| result["structuredPatch"].as_array())
        && !hunks.is_empty()
    {
        let mut patch = String::new();
        for hunk in hunks {
            let number = |field: &str| hunk[field].as_u64().unwrap_or(0);
            patch.push_str(&format!(
                "@@ -{},{} +{},{} @@\n",
                number("oldStart"),
                number("oldLines"),
                number("newStart"),
                number("newLines")
            ));
            for line in hunk["lines"].as_array().into_iter().flatten() {
                if let Some(line) = line.as_str() {
                    patch.push_str(line);
                    patch.push('\n');
                }
            }
        }
        return patch;
    }
    let lines = |text: &str, sign: char| -> Vec<String> {
        text.lines().map(|line| format!("{sign}{line}")).collect()
    };
    let pairs: Vec<(String, String)> = match name {
        "Write" => vec![(
            String::new(),
            input["content"].as_str().unwrap_or_default().to_owned(),
        )],
        "MultiEdit" => input["edits"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|edit| {
                (
                    edit["old_string"].as_str().unwrap_or_default().to_owned(),
                    edit["new_string"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect(),
        _ => vec![(
            input["old_string"].as_str().unwrap_or_default().to_owned(),
            input["new_string"].as_str().unwrap_or_default().to_owned(),
        )],
    };
    let mut patch = String::new();
    for (old, new) in pairs {
        let (removed, added) = (lines(&old, '-'), lines(&new, '+'));
        let old_start = usize::from(!removed.is_empty());
        let new_start = usize::from(!added.is_empty());
        patch.push_str(&format!(
            "@@ -{old_start},{} +{new_start},{} @@\n",
            removed.len(),
            added.len()
        ));
        for line in removed.iter().chain(&added) {
            patch.push_str(line);
            patch.push('\n');
        }
    }
    patch
}
