//! Permission requests, their cancellation and their answers.

use massorca_protocol::methods::chat::{
    Approval, ApprovalDecision, ApprovalScope, ApprovalState, ApprovalSuggestion, ChatItemBody,
    NoticeLevel,
};
use serde_json::{Value, json};

use super::*;
use crate::chat::adapter::{ApprovalAnswer, ChatAdapter, JournalOp};
use crate::chat::claude::UNREADABLE_REQUEST_MESSAGE;

fn mixed_request(request_id: &str, input: Value) -> String {
    frame(json!({
        "type": "control_request", "request_id": request_id,
        "request": {
            "subtype": "can_use_tool", "tool_name": "Bash", "input": input,
            "permission_suggestions": [
                { "type": "addRules", "behavior": "allow", "destination": "localSettings",
                  "rules": [{ "toolName": "Bash", "ruleContent": "touch probe.txt" }] },
                { "type": "addRules", "behavior": "allow", "destination": "userSettings",
                  "rules": [{ "toolName": "Bash" }] },
                { "type": "setMode", "mode": "bypassPermissions", "destination": "session" },
                { "type": "addRules", "behavior": "deny", "destination": "session",
                  "rules": [{ "toolName": "Read" }] },
                { "type": "addDirectories", "directories": ["/elsewhere"], "destination": "session" },
                { "type": "addRules", "behavior": "allow", "destination": "session",
                  "rules": [{ "toolName": "Grep" }] }
            ],
            "tool_use_id": "toolu_12"
        }
    }))
}

fn allow(scope: ApprovalScope) -> ApprovalAnswer {
    ApprovalAnswer {
        decision: ApprovalDecision::Allow,
        scope,
        message: None,
    }
}

#[test]
fn a_permission_request_is_a_pending_approval_that_lists_only_the_rules_always_would_write() {
    let mut adapter = adapter();
    let ops = adapter.translate(&mixed_request(
        "r-1",
        json!({ "command": "touch probe.txt" }),
    ));
    assert_eq!(
        ops,
        [add(
            "approval:r-1",
            None,
            ChatItemBody::Approval(Approval {
                request_id: "r-1".to_owned(),
                tool: "Bash".to_owned(),
                input_summary: "touch probe.txt".to_owned(),
                truncated: false,
                tool_use_id: Some("toolu_12".to_owned()),
                can_always: true,
                suggestions: vec![
                    ApprovalSuggestion {
                        kind: "addRules".to_owned(),
                        rules: vec!["Bash(touch probe.txt)".to_owned()],
                        destination: "localSettings".to_owned(),
                    },
                    ApprovalSuggestion {
                        kind: "addRules".to_owned(),
                        rules: vec!["Grep".to_owned()],
                        destination: "session".to_owned(),
                    },
                ],
                state: ApprovalState::Pending,
                scope: None,
                settled_at_ms: None,
            })
        )]
    );
    // `always` writes back only the listed rules: no mode switch, no user settings, no deny rule, no directory.
    let answered = line(
        &adapter
            .encode_approval("r-1", &allow(ApprovalScope::Always))
            .expect("pending"),
    );
    assert_eq!(
        answered,
        json!({
            "type": "control_response",
            "response": { "subtype": "success", "request_id": "r-1", "response": {
                "behavior": "allow",
                "updatedInput": { "command": "touch probe.txt" },
                "updatedPermissions": [
                    { "type": "addRules", "behavior": "allow", "destination": "localSettings",
                      "rules": [{ "toolName": "Bash", "ruleContent": "touch probe.txt" }] },
                    { "type": "addRules", "behavior": "allow", "destination": "session",
                      "rules": [{ "toolName": "Grep" }] }
                ],
                "toolUseID": "toolu_12"
            }}
        })
    );
    // A once answer writes no rules.
    let once = line(
        &adapter
            .encode_approval("r-1", &allow(ApprovalScope::Once))
            .expect("pending"),
    );
    assert!(
        once["response"]["response"]
            .get("updatedPermissions")
            .is_none(),
        "{once}"
    );
}

#[test]
fn a_request_with_only_unacceptable_suggestions_offers_no_always() {
    let mut adapter = adapter();
    let ops = adapter.translate(&frame(json!({
        "type": "control_request", "request_id": "r-1",
        "request": { "subtype": "can_use_tool", "tool_name": "Edit", "input": { "file_path": "a.rs" },
                     "permission_suggestions": [{ "type": "setMode", "mode": "acceptEdits", "destination": "session" }] }
    })));
    assert!(
        matches!(&ops[..], [JournalOp::Add { body: ChatItemBody::Approval(approval), .. }]
            if !approval.can_always && approval.suggestions.is_empty()),
        "{ops:?}"
    );
    let answered = line(
        &adapter
            .encode_approval("r-1", &allow(ApprovalScope::Always))
            .expect("pending"),
    );
    assert!(
        answered["response"]["response"]
            .get("updatedPermissions")
            .is_none()
    );
}

#[test]
fn an_answer_is_kept_until_the_service_forgets_it_and_a_repeated_request_replaces_its_input() {
    let mut adapter = adapter();
    adapter.translate(&mixed_request("r-1", json!({ "command": "ls" })));
    adapter.translate(&mixed_request("r-1", json!({ "command": "ls -la" })));
    let first = line(
        &adapter
            .encode_approval("r-1", &allow(ApprovalScope::Once))
            .expect("pending"),
    );
    assert_eq!(
        first["response"]["response"]["updatedInput"],
        json!({ "command": "ls -la" })
    );
    assert!(
        adapter
            .encode_approval("r-1", &allow(ApprovalScope::Once))
            .is_some(),
        "not forgotten yet"
    );
    adapter.forget_request("r-1");
    assert_eq!(
        adapter.encode_approval("r-1", &allow(ApprovalScope::Once)),
        None
    );
    // A deny carries the person's message.
    adapter.translate(&mixed_request("r-2", json!({ "command": "rm -r build" })));
    let deny = ApprovalAnswer {
        decision: ApprovalDecision::Deny,
        scope: ApprovalScope::Once,
        message: Some("Use the build folder instead.".to_owned()),
    };
    assert_eq!(
        line(&adapter.encode_approval("r-2", &deny).expect("pending"))["response"]["response"],
        json!({ "behavior": "deny", "message": "Use the build folder instead.", "toolUseID": "toolu_12" })
    );
}

#[test]
fn a_request_whose_input_is_not_an_object_is_denied_without_an_item() {
    let mut adapter = adapter();
    for input in [json!("ls"), json!(null), json!([1, 2])] {
        let ops = adapter.translate(&mixed_request("r-9", input.clone()));
        assert_eq!(notice_level(&ops), [NoticeLevel::Warning], "{input}");
        let Some(JournalOp::Send { bytes }) = ops.last() else {
            panic!("{ops:?}");
        };
        assert_eq!(line(bytes)["response"]["response"]["behavior"], "deny");
        assert_eq!(
            adapter.encode_approval("r-9", &allow(ApprovalScope::Once)),
            None
        );
    }
}

#[test]
fn a_cancelled_request_is_cancelled_and_can_no_longer_be_answered() {
    let mut adapter = adapter();
    adapter.translate(&frame(json!({
        "type": "control_request", "request_id": "r-2",
        "request": { "subtype": "can_use_tool", "tool_name": "Write", "input": { "file_path": "a.txt" } }
    })));
    let ops = adapter.translate(&frame(
        json!({ "type": "control_cancel_request", "request_id": "r-2" }),
    ));
    assert!(
        matches!(ops.as_slice(), [JournalOp::Replace { key, body: ChatItemBody::Approval(approval) }]
            if key == "approval:r-2" && approval.state == ApprovalState::Cancelled && !approval.can_always),
        "{ops:?}"
    );
    let allow = ApprovalAnswer {
        decision: ApprovalDecision::Allow,
        scope: ApprovalScope::Once,
        message: None,
    };
    assert_eq!(
        adapter.encode_approval("r-2", &allow),
        None,
        "a cancel never authorizes"
    );
    // A cancel of a request nobody holds changes nothing.
    assert_eq!(
        adapter.translate(&frame(
            json!({ "type": "control_cancel_request", "request_id": "r-2" })
        )),
        []
    );
}

#[test]
fn an_unreadable_permission_request_is_denied_and_another_request_refused() {
    let mut adapter = adapter();
    let ops = adapter.translate(&frame(json!({
        "type": "control_request", "request_id": "r-3",
        "request": { "subtype": "can_use_tool", "input": {} }
    })));
    assert_eq!(notice_level(&ops), [NoticeLevel::Warning]);
    let Some(JournalOp::Send { bytes }) = ops.last() else {
        panic!("{ops:?}");
    };
    assert_eq!(
        line(bytes)["response"],
        json!({ "subtype": "success", "request_id": "r-3",
                "response": { "behavior": "deny", "message": UNREADABLE_REQUEST_MESSAGE } })
    );
    let ops = adapter.translate(&frame(json!({
        "type": "control_request", "request_id": "r-4", "request": { "subtype": "hook_callback" }
    })));
    let Some(JournalOp::Send { bytes }) = ops.last() else {
        panic!("{ops:?}");
    };
    assert_eq!(line(bytes)["response"]["subtype"], "error");
    assert_eq!(line(bytes)["response"]["request_id"], "r-4");
}
