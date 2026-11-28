//! The adapter of Claude Code's stream-json dialect (decision 0101; inventory AG-17). The CLI runs with
//! [`STREAM_JSON_ARGS`]: it reads one JSON message per line on standard input and writes one frame per line on standard
//! output, and asks for permissions through control requests on the same streams (`--permission-prompt-tool stdio`).
//!
//! The frame vocabulary is the one of Claude Code 2.1.258 (pv96's `claude-stream-json-frame-schema.ts`):
//!
//! - [`launch`]: the launch arguments and the user message a prompt becomes.
//! - [`frames`]: partial and whole messages, tool calls and their results, system frames.
//! - [`approvals`]: permission requests, their cancellation and their answers.
//! - [`turns`]: the `result` that ends a turn, and the interrupt.
//!
//! A frame outside the vocabulary becomes a warning notice that names its type, never its content.

mod approvals;
mod frames;
mod items;
mod launch;
mod turns;

use std::collections::HashMap;

use massorca_protocol::methods::chat::{
    Approval, ChatItemBody, MAX_SUMMARY_BYTES, NoticeLevel, ToolUse,
};
use serde_json::Value;

pub use self::approvals::{DENIED_MESSAGE, UNREADABLE_REQUEST_MESSAGE};
pub use self::launch::STREAM_JSON_ARGS;
use super::adapter::{ApprovalAnswer, ChatAdapter, JournalOp, PromptImage};

/// The name a kind's `chat.adapter` gives this dialect.
pub const ADAPTER_NAME: &str = "claude-stream-json";

/// The longest name (a tool, a frame type or subtype) a notice or an item carries.
const MAX_NAME_CHARS: usize = 128;

/// A tool call the adapter is following until its result.
#[derive(Debug, Clone)]
struct Call {
    /// The parent key of the call's items.
    parent: Option<String>,
    what: CallKind,
}

#[derive(Debug, Clone)]
enum CallKind {
    Tool(ToolUse),
    Subagent {
        description: String,
        agent_type: Option<String>,
    },
    /// A plan written by its input; its result changes nothing.
    Plan,
    /// An edit tool: its item, and the file change its success makes.
    Edit {
        tool: ToolUse,
        path: String,
        input: Value,
    },
}

/// A permission request waiting for the person.
#[derive(Debug, Clone)]
struct Request {
    /// The item the request is shown as.
    approval: Approval,
    input: Value,
    /// The offered changes an `always` answer writes back: only those the item lists.
    accepted: Vec<Value>,
}

/// Claude Code's stream-json adapter. One per run: it follows the run's tool calls, partial messages and requests.
#[derive(Debug, Default)]
pub struct ClaudeAdapter {
    calls: HashMap<String, Call>,
    requests: HashMap<String, Request>,
    /// The text each streamed block holds so far, by item key.
    streamed: HashMap<String, String>,
    /// The message a stream of partial frames is writing, by its parent key (`""` for the session's own agent).
    streaming: HashMap<String, String>,
    /// How many content blocks the whole `assistant` frames of a message have carried.
    blocks_seen: HashMap<String, usize>,
    /// Interrupts sent whose `result` has not come yet.
    interrupts_pending: u32,
    next_request: u64,
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets what the adapter follows within a turn: its partial messages and its unfinished calls.
    fn end_of_turn(&mut self) {
        self.calls.clear();
        self.streamed.clear();
        self.streaming.clear();
        self.blocks_seen.clear();
    }
}

fn line_of(value: &Value) -> Vec<u8> {
    let mut bytes = value.to_string().into_bytes();
    bytes.push(b'\n');
    bytes
}

/// `text` cut to [`MAX_SUMMARY_BYTES`] on a character boundary, and whether it was cut.
pub(crate) fn bounded(text: &str) -> (String, bool) {
    if text.len() <= MAX_SUMMARY_BYTES {
        return (text.to_owned(), false);
    }
    let mut end = MAX_SUMMARY_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

/// A name from the CLI (a tool, a frame type or subtype) as an item or a notice carries it: at most [`MAX_NAME_CHARS`]
/// characters, without control characters.
pub(crate) fn bounded_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect()
}

fn notice(level: NoticeLevel, text: impl Into<String>) -> JournalOp {
    JournalOp::Add {
        key: None,
        parent: None,
        body: ChatItemBody::Notice {
            level,
            text: text.into(),
        },
    }
}

fn text_of(value: &Value) -> Option<&str> {
    value.as_str().filter(|text| !text.is_empty())
}

fn tool_key(id: &str) -> String {
    format!("tool:{id}")
}

/// The parent key of a frame's items: the item of the sub-agent call named by `parent_tool_use_id`.
fn parent_of(frame: &Value) -> Option<String> {
    text_of(&frame["parent_tool_use_id"]).map(tool_key)
}

/// Frame types of the vocabulary that carry nothing for the journal.
const SKIPPED_TYPES: &[&str] = &[
    "control_response",
    "keep_alive",
    "rate_limit_event",
    "tool_use_summary",
    "prompt_suggestion",
    "command_lifecycle",
    "conversation_reset",
    "streamlined_text",
    "streamlined_tool_use_summary",
];

impl ChatAdapter for ClaudeAdapter {
    fn launch_args(&mut self, planned: Vec<String>) -> Vec<String> {
        launch::launch_args(planned)
    }

    fn encode_prompt(&mut self, text: &str) -> Vec<u8> {
        launch::user_message(text)
    }

    fn encode_prompt_with_images(&mut self, text: &str, images: &[PromptImage]) -> Option<Vec<u8>> {
        Some(launch::user_message_with_images(text, images))
    }

    fn carries_images(&self) -> bool {
        true
    }

    fn translate(&mut self, line: &str) -> Vec<JournalOp> {
        if line.trim().is_empty() {
            return Vec::new();
        }
        let Ok(frame) = serde_json::from_str::<Value>(line) else {
            return vec![notice(
                NoticeLevel::Warning,
                "A line from the agent could not be read.",
            )];
        };
        match frame["type"].as_str() {
            Some("stream_event") => self.stream_event(&frame),
            Some("assistant") => self.assistant(&frame),
            Some("user") => self.user(&frame),
            Some("result") => self.result(&frame),
            Some("system") => self.system(&frame),
            Some("tool_progress") => self.tool_progress(&frame),
            Some("control_request") => self.control_request(&frame),
            Some("control_cancel_request") => self.control_cancel(&frame),
            Some("auth_status") => match text_of(&frame["error"]) {
                Some(_) => vec![notice(
                    NoticeLevel::Error,
                    "The agent could not authenticate.",
                )],
                None => Vec::new(),
            },
            Some(known) if SKIPPED_TYPES.contains(&known) => Vec::new(),
            Some(other) => vec![notice(
                NoticeLevel::Warning,
                format!(
                    "A frame of a kind MassOrca does not know was skipped ({}).",
                    bounded_name(other)
                ),
            )],
            None => vec![notice(
                NoticeLevel::Warning,
                "A line from the agent could not be read.",
            )],
        }
    }

    fn encode_approval(&mut self, request_id: &str, answer: &ApprovalAnswer) -> Option<Vec<u8>> {
        self.answer_line(request_id, answer)
    }

    fn forget_request(&mut self, request_id: &str) {
        self.requests.remove(request_id);
    }

    fn encode_interrupt(&mut self) -> Option<Vec<u8>> {
        Some(self.interrupt_line())
    }
}

#[cfg(test)]
mod tests;
