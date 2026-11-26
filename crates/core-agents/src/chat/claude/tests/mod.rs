//! Frame translation of the Claude adapter, from hand-written frames in the documented shapes (no recorded conversation).

mod approvals;
mod messages;
mod tools;
mod turns;

use massorca_protocol::methods::chat::{ChatItemBody, MessageState, NoticeLevel};
use serde_json::{Value, json};

use super::ClaudeAdapter;
use crate::chat::adapter::{ChatAdapter, JournalOp};

pub(super) fn adapter() -> ClaudeAdapter {
    ClaudeAdapter::new()
}

pub(super) fn frame(value: Value) -> String {
    value.to_string()
}

pub(super) fn line(bytes: &[u8]) -> Value {
    let text = std::str::from_utf8(bytes).expect("UTF-8");
    let body = text.strip_suffix('\n').expect("one line");
    assert!(!body.contains('\n'), "{text:?}");
    serde_json::from_str(body).expect("JSON")
}

pub(super) fn add(key: &str, parent: Option<&str>, body: ChatItemBody) -> JournalOp {
    JournalOp::Add {
        key: Some(key.to_owned()),
        parent: parent.map(str::to_owned),
        body,
    }
}

pub(super) fn running_text(text: &str) -> ChatItemBody {
    ChatItemBody::AssistantMessage {
        text: text.to_owned(),
        state: MessageState::Running,
    }
}

pub(super) fn stream(event: Value) -> String {
    frame(
        json!({ "type": "stream_event", "event": event, "parent_tool_use_id": null, "session_id": "s-1" }),
    )
}

pub(super) fn assistant(id: &str, parent: Option<&str>, content: Value) -> String {
    frame(json!({
        "type": "assistant",
        "message": { "id": id, "role": "assistant", "content": content },
        "parent_tool_use_id": parent,
        "session_id": "s-1"
    }))
}

pub(super) fn tool_result(
    id: &str,
    content: Value,
    is_error: bool,
    extra: Option<Value>,
) -> String {
    let mut value = json!({
        "type": "user",
        "message": { "role": "user", "content": [
            { "type": "tool_result", "tool_use_id": id, "content": content, "is_error": is_error }
        ]},
        "parent_tool_use_id": null,
        "session_id": "s-1"
    });
    if let Some(extra) = extra {
        value["tool_use_result"] = extra;
    }
    frame(value)
}

pub(super) fn notice_level(ops: &[JournalOp]) -> Vec<NoticeLevel> {
    ops.iter()
        .filter_map(|op| match op {
            JournalOp::Add {
                body: ChatItemBody::Notice { level, .. },
                ..
            } => Some(*level),
            _ => None,
        })
        .collect()
}

#[test]
fn the_launch_adds_the_stream_json_flags_before_the_kinds_arguments() {
    let args = adapter().launch_args(vec!["--session-id".to_owned(), "r-1".to_owned()]);
    assert_eq!(
        args,
        [
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--permission-prompt-tool",
            "stdio",
            "--session-id",
            "r-1"
        ]
    );
}

#[test]
fn a_prompt_is_one_stream_json_user_message() {
    let bytes = adapter().encode_prompt("fix \"it\"\nplease");
    assert_eq!(
        line(&bytes),
        json!({
            "type": "user",
            "message": { "role": "user", "content": [{ "type": "text", "text": "fix \"it\"\nplease" }] },
            "parent_tool_use_id": null,
            "session_id": ""
        })
    );
}
