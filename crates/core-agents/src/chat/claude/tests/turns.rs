//! The result that ends a turn, the interrupt, and what the adapter forgets with them.

use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{ApprovalDecision, ApprovalScope, NoticeLevel, TurnState};
use serde_json::json;

use super::*;
use crate::chat::adapter::{ApprovalAnswer, ChatAdapter, JournalOp};

#[test]
fn a_result_ends_the_turn_with_its_outcome() {
    let mut adapter = adapter();
    assert_eq!(
        adapter.translate(&frame(json!({
            "type": "result", "subtype": "success", "is_error": false, "result": "Done.", "session_id": "s-1"
        }))),
        [JournalOp::EndTurn {
            state: TurnState::Completed,
            outcome: Some(Outcome::Success)
        }]
    );
    let failed = adapter.translate(&frame(json!({
        "type": "result", "subtype": "error_max_turns", "is_error": true, "session_id": "s-1"
    })));
    assert_eq!(notice_level(&failed), [NoticeLevel::Error]);
    assert_eq!(
        failed.last(),
        Some(&JournalOp::EndTurn {
            state: TurnState::Completed,
            outcome: Some(Outcome::Failure)
        })
    );
}

#[test]
fn an_interrupt_is_a_control_request_that_forgets_pending_requests_and_skips_its_result() {
    let mut adapter = adapter();
    adapter.translate(&frame(json!({
        "type": "control_request", "request_id": "r-5",
        "request": { "subtype": "can_use_tool", "tool_name": "Bash", "input": { "command": "ls" } }
    })));
    let interrupt = line(&adapter.encode_interrupt().expect("an interrupt"));
    assert_eq!(interrupt["type"], "control_request");
    assert_eq!(
        interrupt["request"],
        json!({ "subtype": "interrupt", "cancel_queued": true })
    );
    assert!(
        interrupt["request_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    let allow = ApprovalAnswer {
        decision: ApprovalDecision::Allow,
        scope: ApprovalScope::Once,
        message: None,
    };
    assert_eq!(adapter.encode_approval("r-5", &allow), None);
    // The CLI's answer to the interrupt and the result of the interrupted turn carry nothing.
    assert_eq!(
        adapter.translate(&frame(json!({
            "type": "control_response",
            "response": { "subtype": "success", "request_id": interrupt["request_id"] }
        }))),
        []
    );
    assert_eq!(
        adapter.translate(&frame(json!({
            "type": "result", "subtype": "error_during_execution", "is_error": true, "session_id": "s-1"
        }))),
        []
    );
    // The next turn's result ends it.
    assert_eq!(
        adapter
            .translate(&frame(
                json!({ "type": "result", "subtype": "success", "is_error": false })
            ))
            .len(),
        1
    );
}

#[test]
fn a_turn_end_forgets_its_partial_messages_and_unfinished_calls() {
    let mut adapter = adapter();
    adapter.translate(&stream(
        json!({ "type": "message_start", "message": { "id": "m1" } }),
    ));
    adapter.translate(&assistant(
        "m1",
        None,
        json!([{ "type": "tool_use", "id": "toolu_1", "name": "Bash", "input": { "command": "ls" } }]),
    ));
    adapter.translate(&frame(
        json!({ "type": "result", "subtype": "success", "is_error": false }),
    ));
    assert!(
        adapter.calls.is_empty() && adapter.streaming.is_empty() && adapter.blocks_seen.is_empty()
    );
    // A result for a call of the ended turn changes nothing.
    assert_eq!(
        adapter.translate(&tool_result("toolu_1", json!("x"), false, None)),
        []
    );

    adapter.translate(&stream(
        json!({ "type": "message_start", "message": { "id": "m2" } }),
    ));
    adapter.translate(&stream(json!({
        "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" }
    })));
    adapter.translate(&assistant(
        "m2",
        None,
        json!([{ "type": "tool_use", "id": "toolu_2", "name": "Bash", "input": { "command": "ls" } }]),
    ));
    adapter.encode_interrupt();
    assert!(
        adapter.calls.is_empty() && adapter.streamed.is_empty() && adapter.streaming.is_empty()
    );
}
