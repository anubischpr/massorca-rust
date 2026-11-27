use massorca_protocol::methods::chat::{
    ApprovalDecision, ApprovalScope, ApprovalState, ChatItemBody,
};
use serde_json::json;

use super::{approval_added, feed, lines, notify, opened, sent};
use crate::chat::adapter::{ApprovalAnswer, ChatAdapter, JournalOp, approval_key};

#[test]
fn an_approval_is_answered_once_by_its_id_and_a_withdrawn_one_is_cancelled() {
    let (mut adapter, _) = opened(&[]);
    let ask = |id: u64, item: &str| {
        json!({ "id": id, "method": "item/commandExecution/requestApproval",
                "params": { "threadId": "thr-1", "turnId": "t1", "itemId": item, "command": "touch x" } })
    };
    let ops = feed(&mut adapter, ask(40, "c1"));
    let (key, approval) = approval_added(&ops).expect("a pending approval");
    assert_eq!(key, Some(approval_key(&approval.request_id)));
    let request = approval.request_id.clone();
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
            .encode_approval(&request, &answer(decision, scope))
            .unwrap();
        assert_eq!(
            lines(&line)[0],
            json!({ "id": 40, "result": { "decision": wire } })
        );
    }
    adapter.forget_request(&request);
    assert!(
        adapter
            .encode_approval(
                &request,
                &answer(ApprovalDecision::Allow, ApprovalScope::Once)
            )
            .is_none()
    );

    let (_, second) =
        approval_added(&feed(&mut adapter, ask(41, "c2"))).expect("a pending approval");
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
            .encode_approval(
                &second.request_id,
                &answer(ApprovalDecision::Allow, ApprovalScope::Once)
            )
            .is_none()
    );
}

#[test]
fn two_runs_name_their_approvals_apart_though_the_app_server_numbers_them_alike() {
    let ask = json!({ "id": 40, "method": "item/commandExecution/requestApproval",
                      "params": { "threadId": "thr-1", "turnId": "t1", "itemId": "c1", "command": "touch x" } });
    let (mut first_run, _) = opened(&[]);
    let (mut second_run, _) = opened(&[]);
    let (first_key, first) =
        approval_added(&feed(&mut first_run, ask.clone())).expect("a pending approval");
    let (second_key, second) =
        approval_added(&feed(&mut second_run, ask)).expect("a pending approval");
    // A journal keeps a key's item for the whole session, so a later run's approval must not take an earlier one's key.
    assert_ne!(first_key, second_key);
    assert_ne!(first.request_id, second.request_id);
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

#[test]
fn an_approval_of_another_thread_is_declined_at_once_and_never_pending() {
    let (mut adapter, _) = opened(&[]);
    let ops = feed(
        &mut adapter,
        json!({ "id": 50, "method": "item/commandExecution/requestApproval",
                "params": { "threadId": "thr-sub", "turnId": "t9", "itemId": "c9", "command": "rm x" } }),
    );
    assert_eq!(
        sent(&ops),
        [json!({ "id": 50, "result": { "decision": "decline" } })]
    );
    assert!(approval_added(&ops).is_none(), "{ops:?}");
}

#[test]
fn an_approval_of_an_interrupted_turn_is_cancelled_at_once_and_never_pending() {
    let (mut adapter, _) = opened(&[]);
    adapter.encode_prompt("go");
    feed(
        &mut adapter,
        notify(
            "turn/started",
            json!({ "threadId": "thr-1", "turn": { "id": "t1" } }),
        ),
    );
    assert!(adapter.encode_interrupt().is_some());
    let ops = feed(
        &mut adapter,
        json!({ "id": 51, "method": "item/commandExecution/requestApproval",
                "params": { "threadId": "thr-1", "turnId": "t1", "itemId": "c1", "command": "touch x" } }),
    );
    assert_eq!(
        sent(&ops),
        [json!({ "id": 51, "result": { "decision": "cancel" } })]
    );
    assert!(approval_added(&ops).is_none(), "{ops:?}");
}

#[test]
fn a_withdrawn_request_is_cancelled_whatever_thread_its_notice_names() {
    let (mut adapter, _) = opened(&[]);
    let ops = feed(
        &mut adapter,
        json!({ "id": 52, "method": "item/commandExecution/requestApproval",
                "params": { "threadId": "thr-1", "turnId": "t1", "itemId": "c2", "command": "touch y" } }),
    );
    assert!(approval_added(&ops).is_some(), "{ops:?}");
    let withdrawn = feed(
        &mut adapter,
        notify(
            "serverRequest/resolved",
            json!({ "threadId": "thr-sub", "requestId": 52 }),
        ),
    );
    assert!(
        matches!(&withdrawn[..], [JournalOp::Replace { body: ChatItemBody::Approval(approval), .. }]
        if approval.state == ApprovalState::Cancelled),
        "{withdrawn:?}"
    );
}
