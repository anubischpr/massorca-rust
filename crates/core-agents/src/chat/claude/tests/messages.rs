//! Partial and whole messages, system frames, and frames the adapter does not know.

use massorca_protocol::methods::chat::{ChatItemBody, MessageState, NoticeLevel};
use serde_json::json;

use super::*;
use crate::chat::adapter::{ChatAdapter, JournalOp};

#[test]
fn partial_text_adds_a_running_message_and_appends_its_deltas() {
    let mut adapter = adapter();
    assert_eq!(
        adapter.translate(&stream(
            json!({ "type": "message_start", "message": { "id": "m1" } })
        )),
        []
    );
    assert_eq!(
        adapter.translate(&stream(json!({
            "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" }
        }))),
        [add("msg:m1:0", None, running_text(""))]
    );
    assert_eq!(
        adapter.translate(&stream(json!({
            "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "Hel" }
        }))),
        [JournalOp::Append {
            key: "msg:m1:0".to_owned(),
            text: "Hel".to_owned()
        }]
    );
    adapter.translate(&stream(json!({
        "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "lo" }
    })));
    // The whole message repeats what the deltas built: nothing changes.
    assert_eq!(
        adapter.translate(&assistant(
            "m1",
            None,
            json!([{ "type": "text", "text": "Hello" }])
        )),
        []
    );
}

#[test]
fn a_whole_message_that_differs_from_its_deltas_replaces_the_text() {
    let mut adapter = adapter();
    adapter.translate(&stream(
        json!({ "type": "message_start", "message": { "id": "m1" } }),
    ));
    adapter.translate(&stream(json!({
        "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" }
    })));
    adapter.translate(&stream(json!({
        "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "Hel" }
    })));
    assert_eq!(
        adapter.translate(&assistant(
            "m1",
            None,
            json!([{ "type": "text", "text": "Hello" }])
        )),
        [JournalOp::Replace {
            key: "msg:m1:0".to_owned(),
            body: running_text("Hello")
        }]
    );
}

#[test]
fn thinking_streams_like_text() {
    let mut adapter = adapter();
    adapter.translate(&stream(
        json!({ "type": "message_start", "message": { "id": "m2" } }),
    ));
    assert_eq!(
        adapter.translate(&stream(json!({
            "type": "content_block_start", "index": 0,
            "content_block": { "type": "thinking", "thinking": "" }
        }))),
        [add(
            "msg:m2:0",
            None,
            ChatItemBody::Thinking {
                text: String::new(),
                state: MessageState::Running
            }
        )]
    );
    assert_eq!(
        adapter.translate(&stream(json!({
            "type": "content_block_delta", "index": 0,
            "delta": { "type": "thinking_delta", "thinking": "Let me look." }
        }))),
        [JournalOp::Append {
            key: "msg:m2:0".to_owned(),
            text: "Let me look.".to_owned()
        }]
    );
}

#[test]
fn without_partial_messages_each_block_of_a_whole_message_is_added() {
    let mut adapter = adapter();
    assert_eq!(
        adapter.translate(&assistant(
            "m3",
            None,
            json!([{ "type": "thinking", "thinking": "plan" }, { "type": "text", "text": "Done." }])
        )),
        [
            add(
                "msg:m3:0",
                None,
                ChatItemBody::Thinking {
                    text: "plan".to_owned(),
                    state: MessageState::Running
                }
            ),
            add("msg:m3:1", None, running_text("Done."))
        ]
    );
    // A later frame of the same message carries its next block.
    assert_eq!(
        adapter.translate(&assistant(
            "m3",
            None,
            json!([{ "type": "text", "text": "More." }])
        )),
        [add("msg:m3:2", None, running_text("More."))]
    );
}

#[test]
fn system_frames_become_notices_or_are_skipped() {
    let mut adapter = adapter();
    for (value, levels) in [
        (
            json!({ "type": "system", "subtype": "init", "session_id": "s-1", "claude_code_version": "2.1.330" }),
            vec![],
        ),
        (
            json!({ "type": "system", "subtype": "api_retry", "attempt": 1, "max_retries": 10 }),
            vec![NoticeLevel::Warning],
        ),
        (
            json!({ "type": "system", "subtype": "compact_boundary" }),
            vec![NoticeLevel::Info],
        ),
        (
            json!({ "type": "system", "subtype": "model_refusal_no_fallback" }),
            vec![NoticeLevel::Error],
        ),
        (
            json!({ "type": "system", "subtype": "permission_denied", "tool_name": "Bash" }),
            vec![NoticeLevel::Warning],
        ),
        (
            json!({ "type": "system", "subtype": "session_state_changed", "state": "idle" }),
            vec![],
        ),
        (
            json!({ "type": "system", "subtype": "something_new" }),
            vec![NoticeLevel::Warning],
        ),
        (
            json!({ "type": "rate_limit_event", "rate_limit_info": {} }),
            vec![],
        ),
        (json!({ "type": "keep_alive" }), vec![]),
        (
            json!({ "type": "auth_status", "isAuthenticating": false, "output": [], "error": "expired" }),
            vec![NoticeLevel::Error],
        ),
    ] {
        assert_eq!(
            notice_level(&adapter.translate(&frame(value.clone()))),
            levels,
            "{value}"
        );
    }
}

#[test]
fn unknown_or_unreadable_frames_become_a_warning_without_their_content() {
    let mut adapter = adapter();
    for text in [
        "not json",
        r#"{"no":"type"}"#,
        r#"{"type":"brand_new_frame","secret":"do not show"}"#,
    ] {
        let ops = adapter.translate(text);
        assert_eq!(notice_level(&ops), [NoticeLevel::Warning], "{text}");
        let JournalOp::Add {
            body: ChatItemBody::Notice { text: shown, .. },
            ..
        } = &ops[0]
        else {
            panic!("{ops:?}");
        };
        assert!(!shown.contains("do not show"), "{shown}");
    }
    assert_eq!(adapter.translate(""), []);
    // A tool result for a call the adapter never saw changes nothing.
    assert_eq!(
        adapter.translate(&tool_result("toolu_unknown", json!("x"), false, None)),
        []
    );
}
