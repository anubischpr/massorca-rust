use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{ChatItemBody, NoticeLevel, TurnState};
use serde_json::json;

use super::{feed, lines, notify, opened, sent};
use crate::chat::adapter::{ChatAdapter, JournalOp};

#[test]
fn a_failed_turn_shows_its_reason_once_and_retries_are_warnings() {
    let (mut adapter, _) = opened(&[]);
    let retry = feed(
        &mut adapter,
        notify(
            "error",
            json!({ "threadId": "thr-1", "turnId": "t1", "willRetry": true,
                                "error": { "message": "Reconnecting... 1/2" } }),
        ),
    );
    assert!(matches!(
        &retry[0],
        JournalOp::Add {
            body: ChatItemBody::Notice {
                level: NoticeLevel::Warning,
                ..
            },
            ..
        }
    ));
    feed(
        &mut adapter,
        notify(
            "error",
            json!({ "threadId": "thr-1", "turnId": "t1", "willRetry": false,
                                "error": { "message": "at capacity" } }),
        ),
    );
    let end = feed(
        &mut adapter,
        notify(
            "turn/completed",
            json!({ "threadId": "thr-1",
            "turn": { "id": "t1", "status": "failed", "error": { "message": "at capacity" } } }),
        ),
    );
    assert_eq!(
        end,
        [JournalOp::EndTurn {
            state: TurnState::Completed,
            outcome: Some(Outcome::Failure)
        }]
    );
}

#[test]
fn an_interrupt_before_the_turn_is_named_goes_out_when_it_is() {
    let (mut adapter, _) = opened(&[]);
    let start = lines(&adapter.encode_prompt("go"));
    assert!(adapter.encode_interrupt().unwrap().is_empty());
    let ops = feed(
        &mut adapter,
        json!({ "id": start[0]["id"], "result": { "turn": { "id": "t5", "status": "inProgress" } } }),
    );
    assert_eq!(sent(&ops)[0]["params"]["turnId"], "t5");
}
