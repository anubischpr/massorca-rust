//! The adapter of the fake structured agent (`massorca-fake-agent chat`), for MassOrca's own tests. Its dialect is one
//! JSON object per line:
//!
//! Standard input, from the service:
//! - `{"type":"user","text":"..."}`: the person's message.
//! - `{"type":"interrupt"}`: interrupt the running turn.
//! - `{"type":"approval","id":"...","allow":true|false}`: answer a permission request.
//!
//! Standard output, from the agent:
//! - `{"type":"text","id":"...","text":"..."}`: assistant text. The first line with an id adds a running message; later
//!   lines with the same id append to it.
//! - `{"type":"notice","text":"..."}`: a notice of level `info`.
//! - `{"type":"done","outcome":"success"|"failure"}`: the turn ends; its messages complete.
//!
//! Any other line becomes a `warning` notice that says the line could not be read (without the line).

use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{
    ApprovalDecision, ChatItemBody, MessageState, NoticeLevel, TurnState,
};
use serde_json::{Value, json};

use super::adapter::{ApprovalAnswer, ChatAdapter, JournalOp};

/// The fake structured agent's adapter. It keeps the message ids it has seen, so a repeated id appends.
#[derive(Debug, Default)]
pub struct TestAdapter {
    seen: std::collections::HashSet<String>,
}

/// The first argument that starts the fake agent in its structured mode.
pub const TEST_CHAT_ARG: &str = "chat";

fn line_of(value: &Value) -> Vec<u8> {
    let mut bytes = value.to_string().into_bytes();
    bytes.push(b'\n');
    bytes
}

fn unreadable() -> Vec<JournalOp> {
    vec![JournalOp::Add {
        key: None,
        parent: None,
        body: ChatItemBody::Notice {
            level: NoticeLevel::Warning,
            text: "a line from the agent could not be read".to_owned(),
        },
    }]
}

impl ChatAdapter for TestAdapter {
    fn launch_args(&self, planned: Vec<String>) -> Vec<String> {
        std::iter::once(TEST_CHAT_ARG.to_owned())
            .chain(planned)
            .collect()
    }

    fn encode_prompt(&mut self, text: &str) -> Vec<u8> {
        line_of(&json!({ "type": "user", "text": text }))
    }

    fn translate(&mut self, line: &str) -> Vec<JournalOp> {
        let Ok(frame) = serde_json::from_str::<Value>(line) else {
            return unreadable();
        };
        let text = frame["text"].as_str().map(str::to_owned);
        match (frame["type"].as_str(), text) {
            (Some("text"), Some(text)) => {
                let Some(id) = frame["id"].as_str() else {
                    return unreadable();
                };
                if self.seen.insert(id.to_owned()) {
                    vec![JournalOp::Add {
                        key: Some(id.to_owned()),
                        parent: None,
                        body: ChatItemBody::AssistantMessage {
                            text,
                            state: MessageState::Running,
                        },
                    }]
                } else {
                    vec![JournalOp::Append {
                        key: id.to_owned(),
                        text,
                    }]
                }
            }
            (Some("notice"), Some(text)) => vec![JournalOp::Add {
                key: None,
                parent: None,
                body: ChatItemBody::Notice {
                    level: NoticeLevel::Info,
                    text,
                },
            }],
            (Some("done"), _) => vec![JournalOp::EndTurn {
                state: TurnState::Completed,
                outcome: match frame["outcome"].as_str() {
                    Some("success") => Some(Outcome::Success),
                    Some("failure") => Some(Outcome::Failure),
                    _ => None,
                },
            }],
            _ => unreadable(),
        }
    }

    fn encode_approval(&mut self, request_id: &str, answer: &ApprovalAnswer) -> Option<Vec<u8>> {
        Some(line_of(&json!({
            "type": "approval",
            "id": request_id,
            "allow": answer.decision == ApprovalDecision::Allow,
        })))
    }

    fn forget_request(&mut self, _request_id: &str) {}

    fn encode_interrupt(&mut self) -> Option<Vec<u8>> {
        Some(line_of(&json!({ "type": "interrupt" })))
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::chat::ApprovalScope;

    use super::*;

    #[test]
    fn the_first_line_of_a_message_adds_it_and_later_ones_append() {
        let mut adapter = TestAdapter::default();
        assert_eq!(
            adapter.translate(r#"{"type":"text","id":"a1","text":"Hel"}"#),
            vec![JournalOp::Add {
                key: Some("a1".to_owned()),
                parent: None,
                body: ChatItemBody::AssistantMessage {
                    text: "Hel".to_owned(),
                    state: MessageState::Running,
                },
            }]
        );
        assert_eq!(
            adapter.translate(r#"{"type":"text","id":"a1","text":"lo"}"#),
            vec![JournalOp::Append {
                key: "a1".to_owned(),
                text: "lo".to_owned(),
            }]
        );
        assert_eq!(
            adapter.translate(r#"{"type":"done","outcome":"success"}"#),
            vec![JournalOp::EndTurn {
                state: TurnState::Completed,
                outcome: Some(Outcome::Success),
            }]
        );
    }

    #[test]
    fn an_unreadable_line_becomes_a_warning_without_its_text() {
        let mut adapter = TestAdapter::default();
        for line in ["not json", r#"{"type":"mystery"}"#, r#"{"type":"text"}"#] {
            let ops = adapter.translate(line);
            assert!(
                matches!(
                    ops.as_slice(),
                    [JournalOp::Add { body: ChatItemBody::Notice { level: NoticeLevel::Warning, text }, .. }]
                        if !text.contains(line)
                ),
                "{line}: {ops:?}"
            );
        }
    }

    #[test]
    fn prompts_answers_and_interrupts_are_one_json_line_each() {
        let line = |bytes: Vec<u8>| {
            let text = String::from_utf8(bytes).expect("UTF-8");
            let body = text.strip_suffix('\n').expect("one line");
            assert!(!body.contains('\n'), "{text:?}");
            serde_json::from_str::<Value>(body).expect("JSON")
        };
        let mut adapter = TestAdapter::default();
        assert_eq!(
            line(adapter.encode_prompt("hi \"there\"\nnext")),
            json!({ "type": "user", "text": "hi \"there\"\nnext" })
        );
        let answer = ApprovalAnswer {
            decision: ApprovalDecision::Deny,
            scope: ApprovalScope::Once,
            message: None,
        };
        assert_eq!(
            line(adapter.encode_approval("r1", &answer).expect("an answer")),
            json!({ "type": "approval", "id": "r1", "allow": false })
        );
        assert_eq!(
            line(adapter.encode_interrupt().expect("an interrupt")),
            json!({ "type": "interrupt" })
        );
        assert_eq!(
            adapter.launch_args(vec!["--mode".to_owned(), "plan".to_owned()]),
            ["chat", "--mode", "plan"]
        );
    }
}
