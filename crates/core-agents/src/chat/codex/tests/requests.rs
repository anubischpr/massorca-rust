use massorca_protocol::methods::chat::{
    ApprovalDecision, ApprovalScope, ApprovalState, ChatItemBody,
};
use serde_json::json;

use super::{feed, lines, notify, opened, sent};
use crate::chat::adapter::{ApprovalAnswer, ChatAdapter, JournalOp};

#[test]
fn an_approval_is_answered_once_by_its_id_and_a_withdrawn_one_is_cancelled() {
    let (mut adapter, _) = opened(&[]);
    let ask = |id: u64, item: &str| {
        json!({ "id": id, "method": "item/commandExecution/requestApproval",
                "params": { "threadId": "thr-1", "turnId": "t1", "itemId": item, "command": "touch x" } })
    };
    let ops = feed(&mut adapter, ask(40, "c1"));
    let JournalOp::Add {
        key,
        body: ChatItemBody::Approval(approval),
        ..
    } = &ops[0]
    else {
        panic!("{ops:?}");
    };
    assert_eq!(key.as_deref(), Some("approval:40"));
    assert_eq!(approval.state, ApprovalState::Pending);
    assert_eq!(approval.input_summary, "touch x");
    assert!(approval.can_always);
    let answer = |decision, scope| ApprovalAnswer {
        decision,
        scope,
        message: None,
    };
    for (decision, scope, wire) in [
        (ApprovalDecision::Allow, ApprovalScope::Once, "accept"),
        (
            ApprovalDecision::Allow,
            ApprovalScope::Always,
            "acceptForSession",
        ),
        (ApprovalDecision::Deny, ApprovalScope::Once, "decline"),
    ] {
        let line = adapter
            .encode_approval("40", &answer(decision, scope))
            .unwrap();
        assert_eq!(
            lines(&line)[0],
            json!({ "id": 40, "result": { "decision": wire } })
        );
    }
    adapter.forget_request("40");
    assert!(
        adapter
            .encode_approval("40", &answer(ApprovalDecision::Allow, ApprovalScope::Once))
            .is_none()
    );

    feed(&mut adapter, ask(41, "c2"));
    let withdrawn = feed(
        &mut adapter,
        notify(
            "serverRequest/resolved",
            json!({ "threadId": "thr-1", "requestId": 41 }),
        ),
    );
    assert!(
        matches!(&withdrawn[0], JournalOp::Replace { body: ChatItemBody::Approval(approval), .. }
        if approval.state == ApprovalState::Cancelled)
    );
    assert!(
        adapter
            .encode_approval("41", &answer(ApprovalDecision::Allow, ApprovalScope::Once))
            .is_none()
    );
}

#[test]
fn the_interrupt_cancels_requests_stops_the_turn_and_skips_its_end() {
    let (mut adapter, _) = opened(&[]);
    adapter.encode_prompt("go");
    feed(
        &mut adapter,
        notify(
            "turn/started",
            json!({ "threadId": "thr-1", "turn": { "id": "t1" } }),
        ),
    );
    feed(
        &mut adapter,
        json!({ "id": 7, "method": "item/fileChange/requestApproval",
                "params": { "threadId": "thr-1", "turnId": "t1", "itemId": "f1" } }),
    );
    let out = lines(&adapter.encode_interrupt().unwrap());
    assert_eq!(
        out[0],
        json!({ "id": 7, "result": { "decision": "cancel" } })
    );
    assert_eq!(out[1]["method"], "turn/interrupt");
    assert_eq!(
        out[1]["params"],
        json!({ "threadId": "thr-1", "turnId": "t1" })
    );
    // A prompt sent before the interrupted turn ends waits for it.
    assert!(adapter.encode_prompt("next").is_empty());
    let late = feed(
        &mut adapter,
        notify(
            "item/agentMessage/delta",
            json!({ "threadId": "thr-1", "turnId": "t1", "itemId": "m", "delta": "x" }),
        ),
    );
    assert!(late.is_empty(), "the interrupted turn's items are skipped");
    let end = feed(
        &mut adapter,
        notify(
            "turn/completed",
            json!({ "threadId": "thr-1", "turn": { "id": "t1", "status": "interrupted" } }),
        ),
    );
    assert!(
        !end.iter().any(|op| matches!(op, JournalOp::EndTurn { .. })),
        "{end:?}"
    );
    assert_eq!(sent(&end)[0]["method"], "turn/start");
}

#[test]
fn a_request_massorca_does_not_serve_is_refused_and_another_threads_items_are_skipped() {
    let (mut adapter, _) = opened(&[]);
    let ops = feed(
        &mut adapter,
        json!({ "id": "q-1", "method": "item/tool/requestUserInput", "params": {} }),
    );
    assert_eq!(sent(&ops)[0]["id"], "q-1");
    assert!(sent(&ops)[0]["error"].is_object());
    let other = feed(
        &mut adapter,
        notify(
            "item/agentMessage/delta",
            json!({ "threadId": "thr-sub", "itemId": "m", "delta": "x" }),
        ),
    );
    assert!(other.is_empty());
}
