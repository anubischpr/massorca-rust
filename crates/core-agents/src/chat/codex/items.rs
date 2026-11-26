//! The thread's items. `item/started` adds an item and `item/completed` replaces it with its final form; deltas append to
//! messages and grow a command's output.
//!
//! - `agentMessage`: an assistant message, `running` until it completes; `item/agentMessage/delta` appends.
//! - `reasoning`: thinking, from `item/reasoning/summaryTextDelta` and the completed item's `summary`. Raw reasoning
//!   (`item/reasoning/textDelta`) is not shown.
//! - `commandExecution`, `fileChange`, `mcpToolCall`, `webSearch`: tool calls. A completed `fileChange` also adds one diff
//!   item per changed file.
//! - `userMessage`: skipped; the service already shows the person's prompt.
//! - `contextCompaction`: an info notice once it completes. `plan`: skipped, `turn/plan/updated` shows the plan.
//! - Any other type: one warning notice per type, never its content.

use massorca_protocol::methods::chat::{
    ChatItemBody, DiffStatus, MessageState, NoticeLevel, PlanEntry, PlanStatus, ToolStatus, ToolUse,
};
use serde_json::Value;

use super::{CodexAdapter, Tool, notice, text_of};
use crate::chat::adapter::JournalOp;
use crate::chat::claude::{bounded, bounded_name};

/// Item types that carry nothing for the journal.
const SKIPPED_ITEMS: &[&str] = &["userMessage", "plan", "hookPrompt"];

fn item_key(id: &str) -> String {
    format!("item:{id}")
}

/// Text from a string, or from a list of strings or `{text}` parts joined by line breaks.
fn joined_text(value: &Value) -> Option<String> {
    if let Some(text) = text_of(value) {
        return Some(text.to_owned());
    }
    let parts: Vec<&str> = value
        .as_array()?
        .iter()
        .filter_map(|part| text_of(part).or_else(|| text_of(&part["text"])))
        .collect();
    (!parts.is_empty()).then(|| parts.join("\n"))
}

/// A command as text: a string, or its words joined by spaces.
pub(super) fn command_text(value: &Value) -> Option<String> {
    text_of(value).map(str::to_owned).or_else(|| {
        let words: Vec<&str> = value.as_array()?.iter().filter_map(Value::as_str).collect();
        (!words.is_empty()).then(|| words.join(" "))
    })
}

/// The status a tool item's `status` names.
fn tool_status(item: &Value, completed: bool) -> ToolStatus {
    let exit_failed = item["exitCode"].as_i64().is_some_and(|code| code != 0);
    match item["status"].as_str() {
        Some("failed" | "declined") => ToolStatus::Failed,
        Some("completed") if exit_failed => ToolStatus::Failed,
        Some("completed") => ToolStatus::Completed,
        _ if completed && exit_failed => ToolStatus::Failed,
        _ if completed => ToolStatus::Completed,
        _ => ToolStatus::Started,
    }
}

/// The paths a `fileChange` item changes.
pub(super) fn changed_paths(item: &Value) -> Vec<String> {
    item["changes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|change| text_of(&change["path"]).map(str::to_owned))
        .collect()
}

/// What a change's `kind` (a string, or an object tagged by `type`) did to its file, and its earlier path.
fn diff_status(kind: &Value) -> (DiffStatus, Option<String>) {
    let tag = kind.as_str().or_else(|| kind["type"].as_str());
    let moved = text_of(&kind["move_path"])
        .or_else(|| text_of(&kind["movePath"]))
        .map(str::to_owned);
    match (tag, moved) {
        (Some("add"), _) => (DiffStatus::Added, None),
        (Some("delete"), _) => (DiffStatus::Deleted, None),
        (_, Some(moved)) => (DiffStatus::Renamed, Some(moved)),
        _ => (DiffStatus::Modified, None),
    }
}

impl CodexAdapter {
    /// `item/started` or `item/completed`.
    pub(super) fn item(&mut self, item: &Value, completed: bool) -> Vec<JournalOp> {
        let (Some(kind), Some(id)) = (text_of(&item["type"]), text_of(&item["id"])) else {
            return vec![notice(
                NoticeLevel::Warning,
                "An item from the agent could not be read.",
            )];
        };
        let key = item_key(id);
        match kind {
            "agentMessage" => {
                let text = joined_text(&item["text"]).unwrap_or_default();
                self.message(key, text, completed, false)
            }
            "reasoning" => match joined_text(&item["summary"]) {
                Some(text) => self.message(key, text, completed, true),
                // Nothing to show yet; a streamed item completes with its turn.
                None => Vec::new(),
            },
            "commandExecution" | "fileChange" | "mcpToolCall" | "webSearch" => {
                self.tool_item(kind, id, item, completed)
            }
            "contextCompaction" if completed => {
                vec![notice(NoticeLevel::Info, "The conversation was compacted.")]
            }
            "contextCompaction" => Vec::new(),
            known if SKIPPED_ITEMS.contains(&known) => Vec::new(),
            other => self.unknown("item", other),
        }
    }

    /// An assistant message or thinking item with its whole `text`.
    fn message(
        &mut self,
        key: String,
        text: String,
        completed: bool,
        thinking: bool,
    ) -> Vec<JournalOp> {
        let state = if completed {
            MessageState::Completed
        } else {
            MessageState::Running
        };
        let (text, _) = bounded(&text);
        let empty = text.is_empty();
        let body = if thinking {
            ChatItemBody::Thinking { text, state }
        } else {
            ChatItemBody::AssistantMessage { text, state }
        };
        if self.seen.insert(key.clone()) {
            vec![JournalOp::Add {
                key: Some(key),
                parent: None,
                body,
            }]
        } else if completed && !empty {
            // The completed item's text is whole: it replaces what the deltas wrote.
            vec![JournalOp::Replace { key, body }]
        } else {
            Vec::new()
        }
    }

    /// `item/agentMessage/delta` or `item/reasoning/summaryTextDelta`: text appended to its item.
    pub(super) fn text_delta(&mut self, params: &Value, thinking: bool) -> Vec<JournalOp> {
        let (Some(id), Some(delta)) = (text_of(&params["itemId"]), params["delta"].as_str()) else {
            return Vec::new();
        };
        let key = item_key(id);
        if self.seen.contains(&key) {
            return vec![JournalOp::Append {
                key,
                text: delta.to_owned(),
            }];
        }
        self.message(key, delta.to_owned(), false, thinking)
    }

    /// A tool item: its call, and on a completed `fileChange` its diffs.
    fn tool_item(&mut self, kind: &str, id: &str, item: &Value, completed: bool) -> Vec<JournalOp> {
        let (name, input) = match kind {
            "commandExecution" => ("shell".to_owned(), command_text(&item["command"])),
            "fileChange" => (
                "apply_patch".to_owned(),
                Some(changed_paths(item).join("\n")),
            ),
            "mcpToolCall" => {
                let tool = text_of(&item["tool"]).unwrap_or("mcp");
                let name = match text_of(&item["server"]) {
                    Some(server) => format!("{server}/{tool}"),
                    None => tool.to_owned(),
                };
                let input = (!item["arguments"].is_null()).then(|| item["arguments"].to_string());
                (name, input)
            }
            _ => (
                "web_search".to_owned(),
                text_of(&item["query"]).map(str::to_owned),
            ),
        };
        let mut status = tool_status(item, completed);
        if kind == "webSearch" {
            status = if completed || !item["action"].is_null() {
                ToolStatus::Completed
            } else {
                ToolStatus::Started
            };
        }
        let failure = text_of(&item["error"]["message"]).map(str::to_owned);
        if failure.is_some() {
            status = ToolStatus::Failed;
        }
        let streamed = self
            .tools
            .get(id)
            .map(|tool| tool.output.clone())
            .unwrap_or_default();
        let output = failure
            .or_else(|| text_of(&item["aggregatedOutput"]).map(str::to_owned))
            .or_else(|| joined_text(&item["result"]["content"]))
            .or_else(|| (!streamed.is_empty()).then_some(streamed.clone()));
        let (input_summary, input_cut) = bounded(&input.unwrap_or_default());
        let (output_summary, output_cut) = match output {
            Some(output) => {
                let (text, cut) = bounded(&output);
                (Some(text), cut)
            }
            None => (None, false),
        };
        let tool = ToolUse {
            tool_use_id: id.to_owned(),
            name: bounded_name(&name),
            input_summary,
            status,
            output_summary,
            truncated: input_cut || output_cut,
        };
        self.tools.insert(
            id.to_owned(),
            Tool {
                tool: tool.clone(),
                output: streamed,
            },
        );
        let key = item_key(id);
        self.seen.insert(key.clone());
        let mut ops = vec![JournalOp::Add {
            key: Some(key),
            parent: None,
            body: ChatItemBody::ToolUse(tool),
        }];
        if kind == "fileChange" && completed && status == ToolStatus::Completed {
            ops.extend(self.diffs(id, item));
        }
        ops
    }

    /// One diff item per change of a completed `fileChange`.
    fn diffs(&self, id: &str, item: &Value) -> Vec<JournalOp> {
        item["changes"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(index, change)| {
                let path = text_of(&change["path"])?;
                let (status, old_path) = diff_status(&change["kind"]);
                let (patch, truncated) = bounded(change["diff"].as_str().unwrap_or_default());
                Some(JournalOp::Add {
                    key: Some(format!("diff:{id}:{index}")),
                    parent: None,
                    body: ChatItemBody::Diff {
                        path: path.to_owned(),
                        status,
                        old_path,
                        patch,
                        truncated,
                    },
                })
            })
            .collect()
    }

    /// `item/commandExecution/outputDelta`: the command's output so far, on its item.
    pub(super) fn output_delta(&mut self, params: &Value) -> Vec<JournalOp> {
        let (Some(id), Some(delta)) = (text_of(&params["itemId"]), params["delta"].as_str()) else {
            return Vec::new();
        };
        let Some(tool) = self.tools.get_mut(id) else {
            return Vec::new();
        };
        tool.output.push_str(delta);
        let (output, cut) = bounded(&tool.output);
        tool.tool.output_summary = Some(output);
        tool.tool.truncated |= cut;
        tool.tool.status = ToolStatus::Updated;
        vec![JournalOp::Replace {
            key: item_key(id),
            body: ChatItemBody::ToolUse(tool.tool.clone()),
        }]
    }

    /// `turn/plan/updated`: the turn's plan, as a whole list each time.
    pub(super) fn plan_updated(&mut self, params: &Value) -> Vec<JournalOp> {
        let entries: Vec<PlanEntry> = params["plan"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|step| {
                let text = text_of(&step["step"])?;
                let status = match step["status"].as_str() {
                    Some("completed") => PlanStatus::Completed,
                    Some("inProgress" | "in_progress") => PlanStatus::InProgress,
                    _ => PlanStatus::Pending,
                };
                Some(PlanEntry {
                    text: bounded(text).0,
                    status,
                })
            })
            .collect();
        if entries.is_empty() {
            return Vec::new();
        }
        let turn = text_of(&params["turnId"]).unwrap_or("turn");
        vec![JournalOp::Add {
            key: Some(format!("plan:{turn}")),
            parent: None,
            body: ChatItemBody::Plan {
                entries,
                document: None,
            },
        }]
    }
}
