//! Frames that carry the conversation: partial and whole messages, tool calls and their results, and `system` frames.
//!
//! - `stream_event` `content_block_start` (text or thinking) adds a running `assistant_message` or `thinking`;
//!   `content_block_delta` appends to it. The whole `assistant` frame that follows replaces the text when it differs, or
//!   adds the block when no partial message announced it.
//! - An `assistant` `tool_use` block adds a `tool_use` (`started`); `Task`/`Agent` add a `subagent` instead, `TodoWrite`
//!   a `plan`. `tool_progress` marks the call `updated`. A `user` frame's `tool_result` completes or fails it, and an
//!   edit tool's success adds a `diff`. Items a sub-agent produces (`parent_tool_use_id`) name its item as `parent`.
//! - `system` `api_retry`, `compact_boundary`, the model refusals and `permission_denied` become notices; the rest of
//!   the vocabulary is bookkeeping and is skipped.

use massorca_protocol::methods::chat::{
    ChatItemBody, DiffStatus, MessageState, NoticeLevel, SubagentStatus, ToolStatus, ToolUse,
};
use serde_json::Value;

use super::items::{edit_patch, input_text, plan_entries, result_text};
use super::{
    Call, CallKind, ClaudeAdapter, bounded, bounded_name, notice, parent_of, text_of, tool_key,
};
use crate::chat::adapter::JournalOp;

/// Tool names that start a sub-agent.
const SUBAGENT_TOOLS: &[&str] = &["Task", "Agent"];
/// The tool whose input is the agent's checklist.
const PLAN_TOOL: &str = "TodoWrite";
/// The tool that presents a plan for approval; its input may name the file the plan was written to.
const EXIT_PLAN_TOOL: &str = "ExitPlanMode";
/// Tools whose success changed one file.
const EDIT_TOOLS: &[&str] = &["Edit", "MultiEdit", "Write"];

/// `system` subtypes of the vocabulary that carry nothing for the journal.
const SKIPPED_SYSTEM: &[&str] = &[
    "init",
    "status",
    "control_request_progress",
    "local_command_output",
    "hook_started",
    "hook_progress",
    "hook_response",
    "plugin_install",
    "task_started",
    "task_updated",
    "task_progress",
    "background_tasks_changed",
    "thinking_tokens",
    "session_state_changed",
    "worker_shutting_down",
    "commands_changed",
    "notification",
    "files_persisted",
    "memory_recall",
    "elicitation_complete",
    "mirror_error",
    "informational",
];

impl ClaudeAdapter {
    /// One `stream_event` frame: partial text and thinking.
    pub(super) fn stream_event(&mut self, frame: &Value) -> Vec<JournalOp> {
        let event = &frame["event"];
        let parent = parent_of(frame);
        let stream = parent.clone().unwrap_or_default();
        match event["type"].as_str() {
            Some("message_start") => {
                if let Some(id) = text_of(&event["message"]["id"]) {
                    self.streaming.insert(stream, id.to_owned());
                }
                Vec::new()
            }
            Some("content_block_start") => {
                let (Some(message), Some(index)) =
                    (self.streaming.get(&stream), event["index"].as_u64())
                else {
                    return Vec::new();
                };
                let block = &event["content_block"];
                let key = format!("msg:{message}:{index}");
                let (field, thinking) = match block["type"].as_str() {
                    Some("text") => ("text", false),
                    Some("thinking") => ("thinking", true),
                    _ => return Vec::new(),
                };
                let text = block[field].as_str().unwrap_or_default().to_owned();
                self.streamed.insert(key.clone(), text.clone());
                let state = MessageState::Running;
                vec![JournalOp::Add {
                    key: Some(key),
                    parent,
                    body: if thinking {
                        ChatItemBody::Thinking { text, state }
                    } else {
                        ChatItemBody::AssistantMessage { text, state }
                    },
                }]
            }
            Some("content_block_delta") => {
                let (Some(message), Some(index)) =
                    (self.streaming.get(&stream), event["index"].as_u64())
                else {
                    return Vec::new();
                };
                let delta = &event["delta"];
                let text = match delta["type"].as_str() {
                    Some("text_delta") => delta["text"].as_str(),
                    Some("thinking_delta") => delta["thinking"].as_str(),
                    _ => None,
                };
                let key = format!("msg:{message}:{index}");
                match (
                    text.filter(|text| !text.is_empty()),
                    self.streamed.get_mut(&key),
                ) {
                    (Some(text), Some(held)) => {
                        held.push_str(text);
                        vec![JournalOp::Append {
                            key,
                            text: text.to_owned(),
                        }]
                    }
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    /// One whole `assistant` frame: its text, thinking and tool calls.
    pub(super) fn assistant(&mut self, frame: &Value) -> Vec<JournalOp> {
        let message = &frame["message"];
        let id = text_of(&message["id"])
            .or_else(|| text_of(&frame["uuid"]))
            .unwrap_or("message")
            .to_owned();
        let parent = parent_of(frame);
        let mut ops = Vec::new();
        for block in message["content"].as_array().into_iter().flatten() {
            let seen = self.blocks_seen.entry(id.clone()).or_default();
            let index = *seen;
            *seen += 1;
            let key = format!("msg:{id}:{index}");
            match block["type"].as_str() {
                Some(kind @ ("text" | "thinking")) => {
                    let Some(text) = text_of(&block[kind]) else {
                        continue;
                    };
                    let thinking = kind == "thinking";
                    let state = MessageState::Running;
                    let body = if thinking {
                        ChatItemBody::Thinking {
                            text: text.to_owned(),
                            state,
                        }
                    } else {
                        ChatItemBody::AssistantMessage {
                            text: text.to_owned(),
                            state,
                        }
                    };
                    match self.streamed.insert(key.clone(), text.to_owned()) {
                        Some(held) if held == text => {}
                        Some(_) => ops.push(JournalOp::Replace { key, body }),
                        None => ops.push(JournalOp::Add {
                            key: Some(key),
                            parent: parent.clone(),
                            body,
                        }),
                    }
                }
                Some("tool_use") => ops.extend(self.tool_use(block, parent.clone())),
                _ => {}
            }
        }
        ops
    }

    /// One `tool_use` block.
    pub(super) fn tool_use(&mut self, block: &Value, parent: Option<String>) -> Vec<JournalOp> {
        let (Some(id), Some(name)) = (text_of(&block["id"]), text_of(&block["name"])) else {
            return Vec::new();
        };
        let input = block["input"].clone();
        let key = tool_key(id);
        let (input_summary, truncated) = bounded(&input_text(name, &input));
        let tool = ToolUse {
            tool_use_id: id.to_owned(),
            name: bounded_name(name),
            input_summary,
            status: ToolStatus::Started,
            output_summary: None,
            truncated,
        };
        let (what, body) = if SUBAGENT_TOOLS.contains(&name) {
            let description = input["description"]
                .as_str()
                .or_else(|| input["prompt"].as_str())
                .unwrap_or_default();
            let (description, _) = bounded(description);
            let agent_type = text_of(&input["subagent_type"]).map(bounded_name);
            (
                CallKind::Subagent {
                    description: description.clone(),
                    agent_type: agent_type.clone(),
                },
                ChatItemBody::Subagent {
                    tool_use_id: id.to_owned(),
                    description,
                    agent_type,
                    status: SubagentStatus::Running,
                },
            )
        } else if name == PLAN_TOOL {
            (
                CallKind::Plan,
                ChatItemBody::Plan {
                    entries: plan_entries(&input),
                    document: None,
                },
            )
        } else if EDIT_TOOLS.contains(&name)
            && let Some(path) = text_of(&input["file_path"])
        {
            (
                CallKind::Edit {
                    tool: tool.clone(),
                    path: path.to_owned(),
                    input: input.clone(),
                },
                ChatItemBody::ToolUse(tool),
            )
        } else {
            (CallKind::Tool(tool.clone()), ChatItemBody::ToolUse(tool))
        };
        self.calls.insert(
            id.to_owned(),
            Call {
                parent: parent.clone(),
                what,
            },
        );
        let mut ops = vec![JournalOp::Add {
            key: Some(key),
            parent: parent.clone(),
            body,
        }];
        if name == EXIT_PLAN_TOOL
            && let Some(document) =
                text_of(&input["planFilePath"]).or_else(|| text_of(&input["plan_file_path"]))
        {
            ops.push(JournalOp::Add {
                key: Some(format!("plan:{id}")),
                parent,
                body: ChatItemBody::Plan {
                    entries: Vec::new(),
                    document: Some(bounded(document).0),
                },
            });
        }
        ops
    }

    /// One `user` frame: the tool results it carries. Its text is the person's own (or a sub-agent's prompt), which the
    /// journal already holds or does not show.
    pub(super) fn user(&mut self, frame: &Value) -> Vec<JournalOp> {
        let content = &frame["message"]["content"];
        let structured = frame.get("tool_use_result");
        let mut ops = Vec::new();
        for part in content.as_array().into_iter().flatten() {
            if part["type"] != "tool_result" {
                continue;
            }
            let Some(id) = text_of(&part["tool_use_id"]) else {
                continue;
            };
            let failed = part["is_error"].as_bool().unwrap_or(false);
            ops.extend(self.finish_call(id, &result_text(&part["content"]), failed, structured));
        }
        ops
    }

    /// The end of the call `id`: its item completes or fails, and an edit's success adds its diff.
    pub(super) fn finish_call(
        &mut self,
        id: &str,
        output: &str,
        failed: bool,
        structured: Option<&Value>,
    ) -> Vec<JournalOp> {
        let Some(call) = self.calls.remove(id) else {
            return Vec::new();
        };
        let key = tool_key(id);
        let status = if failed {
            ToolStatus::Failed
        } else {
            ToolStatus::Completed
        };
        let finished = |mut tool: ToolUse| {
            let (output, cut) = bounded(output);
            tool.status = status;
            tool.output_summary = Some(output);
            tool.truncated |= cut;
            tool
        };
        match call.what {
            CallKind::Tool(tool) => vec![JournalOp::Replace {
                key,
                body: ChatItemBody::ToolUse(finished(tool)),
            }],
            CallKind::Subagent {
                description,
                agent_type,
            } => vec![JournalOp::Replace {
                key,
                body: ChatItemBody::Subagent {
                    tool_use_id: id.to_owned(),
                    description,
                    agent_type,
                    status: if failed {
                        SubagentStatus::Failed
                    } else {
                        SubagentStatus::Completed
                    },
                },
            }],
            CallKind::Plan => Vec::new(),
            CallKind::Edit { tool, path, input } => {
                let name = tool.name.clone();
                let mut ops = vec![JournalOp::Replace {
                    key,
                    body: ChatItemBody::ToolUse(finished(tool)),
                }];
                if !failed {
                    let created = structured.and_then(|result| result["type"].as_str())
                        == Some("create")
                        || (name == "Write" && output.starts_with("File created"));
                    let (patch, truncated) =
                        bounded(&edit_patch(&name, &input, structured.filter(|_| !created)));
                    ops.push(JournalOp::Add {
                        key: Some(format!("diff:{id}")),
                        parent: call.parent,
                        body: ChatItemBody::Diff {
                            path,
                            status: if created {
                                DiffStatus::Added
                            } else {
                                DiffStatus::Modified
                            },
                            old_path: None,
                            patch,
                            truncated,
                        },
                    });
                }
                ops
            }
        }
    }

    /// `tool_progress`: the call is still running.
    pub(super) fn tool_progress(&mut self, frame: &Value) -> Vec<JournalOp> {
        let Some(id) = text_of(&frame["tool_use_id"]) else {
            return Vec::new();
        };
        let key = tool_key(id);
        match self.calls.get_mut(id).map(|call| &mut call.what) {
            Some(CallKind::Tool(tool) | CallKind::Edit { tool, .. })
                if tool.status != ToolStatus::Updated =>
            {
                tool.status = ToolStatus::Updated;
                vec![JournalOp::Replace {
                    key,
                    body: ChatItemBody::ToolUse(tool.clone()),
                }]
            }
            _ => Vec::new(),
        }
    }

    /// `system` frames: notices for what the person should see, nothing for bookkeeping.
    pub(super) fn system(&mut self, frame: &Value) -> Vec<JournalOp> {
        let subtype = frame["subtype"].as_str().unwrap_or_default();
        match subtype {
            "api_retry" => {
                let attempt = frame["attempt"].as_u64();
                let most = frame["max_retries"].as_u64();
                let text = match (attempt, most) {
                    (Some(attempt), Some(most)) => {
                        format!("The model request failed; retrying (attempt {attempt} of {most}).")
                    }
                    _ => "The model request failed; retrying.".to_owned(),
                };
                vec![notice(NoticeLevel::Warning, text)]
            }
            "compact_boundary" => {
                vec![notice(NoticeLevel::Info, "The conversation was compacted.")]
            }
            "model_refusal_fallback" => vec![notice(
                NoticeLevel::Warning,
                "The model declined the request; a fallback model answers instead.",
            )],
            "model_refusal_no_fallback" => vec![notice(
                NoticeLevel::Error,
                "The model declined the request.",
            )],
            "permission_denied" => {
                let tool = frame["tool_name"]
                    .as_str()
                    .map_or_else(|| "a tool".to_owned(), bounded_name);
                vec![notice(
                    NoticeLevel::Warning,
                    format!("The permission mode denied {tool}."),
                )]
            }
            "task_notification" => {
                let Some(id) = text_of(&frame["tool_use_id"]) else {
                    return Vec::new();
                };
                let failed = match frame["status"].as_str() {
                    Some("completed") => false,
                    Some("failed" | "killed" | "stopped") => true,
                    _ => return Vec::new(),
                };
                match self.calls.get(id).map(|call| &call.what) {
                    Some(CallKind::Subagent { .. }) => self.finish_call(id, "", failed, None),
                    _ => Vec::new(),
                }
            }
            known if SKIPPED_SYSTEM.contains(&known) => Vec::new(),
            other => vec![notice(
                NoticeLevel::Warning,
                format!(
                    "A frame of a kind MassOrca does not know was skipped (system {}).",
                    bounded_name(other)
                ),
            )],
        }
    }
}
