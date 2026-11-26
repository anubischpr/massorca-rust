//! Wire shapes of the messages between delegation bots (decision 0129).

use massorca_protocol::methods::team_room::{
    MessageDelivery, MessagePriority, MessageType, TeamInboxParams, TeamMessage,
    TeamMessageSendParams, WorkerOutcome,
};
use massorca_protocol::methods::teams::ChildReport;
use serde_json::json;

fn send() -> TeamMessageSendParams {
    TeamMessageSendParams {
        to: "parent".to_owned(),
        kind: MessageType::WorkerDone,
        priority: MessagePriority::Normal,
        subject: "Parser done".to_owned(),
        body: "secret body words".to_owned(),
        outcome: Some(WorkerOutcome::Succeeded),
        task: None,
        thread: None,
        reply_to: None,
        session: Some("session-1".to_owned()),
        agent_token: Some("secret-token".to_owned()),
    }
}

#[test]
fn a_send_names_its_type_on_the_wire_and_omits_what_it_lacks() {
    let value = serde_json::to_value(send()).unwrap();
    assert_eq!(value["type"], json!("worker_done"));
    assert_eq!(value["outcome"], json!("succeeded"));
    assert_eq!(value["priority"], json!("normal"));
    assert!(value.get("task").is_none());
    assert!(value.get("reply_to").is_none());
}

#[test]
fn types_priorities_and_outcomes_from_a_newer_peer_decode_as_unknown() {
    let decoded: TeamMessageSendParams = serde_json::from_value(json!({
        "to": "parent",
        "type": "merge_ready",
        "priority": "eventually",
        "subject": "s",
        "body": "b",
        "outcome": "partial"
    }))
    .unwrap();
    assert_eq!(decoded.kind, MessageType::Unknown);
    assert_eq!(decoded.priority, MessagePriority::Unknown);
    assert_eq!(decoded.outcome, Some(WorkerOutcome::Unknown));
}

#[test]
fn a_send_without_a_priority_is_normal() {
    let decoded: TeamMessageSendParams = serde_json::from_value(json!({
        "to": "parent", "type": "status", "subject": "s", "body": "b"
    }))
    .unwrap();
    assert_eq!(decoded.priority, MessagePriority::Normal);
}

#[test]
fn debug_shows_neither_a_body_nor_a_token() {
    let shown = format!("{:?}", send());
    assert!(!shown.contains("secret body words"));
    assert!(!shown.contains("secret-token"));
    assert!(shown.contains("Parser done"));
    let inbox = TeamInboxParams {
        mark_read: true,
        session: None,
        agent_token: Some("secret-token".to_owned()),
    };
    assert!(!format!("{inbox:?}").contains("secret-token"));
}

#[test]
fn a_message_from_an_older_host_without_delivery_reads_as_inbox() {
    let message: TeamMessage = serde_json::from_value(json!({
        "id": "m1", "type": "status", "priority": "high", "subject": "s", "body": "b",
        "thread": "m1", "from_task": "c", "to_task": "p", "created_at_ms": 1
    }))
    .unwrap();
    assert_eq!(message.delivery, MessageDelivery::Inbox);
    assert!(!message.read);
}

#[test]
fn an_empty_child_report_is_left_off_the_wire() {
    assert!(ChildReport::default().is_empty());
    let report = ChildReport {
        status: Some("halfway".to_owned()),
        outcome: Some(WorkerOutcome::Failed),
    };
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        json!({ "status": "halfway", "outcome": "failed" })
    );
}
