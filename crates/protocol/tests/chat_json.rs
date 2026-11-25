//! The structured chat wire (decision 0101): method names, event and item shapes, omitted optionals, unknown values from
//! a newer peer, and that payloads from an older peer still decode.

use std::fmt::Debug;

use massorca_protocol::capabilities;
use massorca_protocol::methods::agents::{
    Activity, ActivitySource, ActivityState, AgentKind, AgentStartParams, Outcome, PromptReceipt,
    ReceiptKind,
};
use massorca_protocol::methods::chat::{
    self, AgentChatStartParams, Approval, ApprovalDecision, ApprovalScope, ApprovalState,
    ChatApprovalAnswerParams, ChatBatch, ChatCursor, ChatEvent, ChatHistoryPage, ChatHistoryParams,
    ChatInterrupted, ChatItem, ChatItemBody, ChatSnapshot, ChatSubscribeParams,
    ChatUnsupportedReason, ChatUpdate, EndReason, KindChat, MessageState, NoticeLevel, PlanEntry,
    PlanStatus, ResetReason, RunType, SubagentStatus, ToolStatus, ToolUse, Turn, TurnState,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

/// Checks that `value` serializes to `expected` and that `expected` decodes back to `value`.
fn assert_wire<T>(value: &T, expected: &Value)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    assert_eq!(&serde_json::to_value(value).expect("serializes"), expected);
    let decoded: T = serde_json::from_value(expected.clone()).expect("deserializes");
    assert_eq!(&decoded, value);
}

fn cursor(seq: u64) -> ChatCursor {
    ChatCursor {
        epoch: "e-1".to_owned(),
        seq,
    }
}

fn item(id: &str, position: u64, seq: u64, body: ChatItemBody) -> ChatItem {
    ChatItem {
        id: id.to_owned(),
        position,
        seq,
        turn: Some("turn-1".to_owned()),
        parent: None,
        at_ms: 1_767_225_600_000,
        body,
    }
}

fn pending_approval() -> Approval {
    Approval {
        request_id: "req-1".to_owned(),
        tool: "Bash".to_owned(),
        input_summary: "rm -rf build".to_owned(),
        truncated: false,
        tool_use_id: Some("toolu_1".to_owned()),
        can_always: true,
        state: ApprovalState::Pending,
        scope: None,
        settled_at_ms: None,
    }
}

#[test]
fn method_names_and_the_capability_match_the_wire_table() {
    assert_eq!(chat::START, "agent.chat.start");
    assert_eq!(chat::SUBSCRIBE, "agent.chat.subscribe");
    assert_eq!(chat::HISTORY, "agent.chat.history");
    assert_eq!(chat::APPROVAL_ANSWER, "agent.chat.approval.answer");
    assert_eq!(chat::INTERRUPT, "agent.chat.interrupt");
    assert_eq!(capabilities::AGENT_CHAT, "agent-chat/1");
}

#[test]
fn chat_error_codes_match_the_wire_contract() {
    assert_eq!(chat::CHAT_UNSUPPORTED, "agent.chat_unsupported");
    assert_eq!(chat::NOT_CHAT, "agent.not_chat");
    assert_eq!(chat::APPROVAL_SETTLED, "agent.approval_settled");
    assert_eq!(chat::CURSOR_STALE, "agent.cursor_stale");
}

#[test]
fn the_chat_start_carries_the_agent_start_fields_at_the_top_level() {
    let params = AgentChatStartParams {
        start: AgentStartParams {
            workspace: Some("w-1".to_owned()),
            kind: "claude".to_owned(),
            prompt: Some("fix the build".to_owned()),
            args: Vec::new(),
            env: Default::default(),
            read_only: false,
            title: None,
            cols: None,
            rows: None,
            request_id: Some("r-1".to_owned()),
            permission_mode: Some("default".to_owned()),
            model: None,
            effort: None,
        },
    };
    assert_wire(
        &params,
        &json!({
            "workspace": "w-1",
            "kind": "claude",
            "prompt": "fix the build",
            "read_only": false,
            "request_id": "r-1",
            "permission_mode": "default"
        }),
    );
}

#[test]
fn a_run_type_reads_by_its_wire_name_and_defaults_to_terminal() {
    assert_wire(&RunType::Terminal, &json!("terminal"));
    assert_wire(&RunType::Chat, &json!("chat"));
    assert_eq!(RunType::default(), RunType::Terminal);
    let newer: RunType = serde_json::from_value(json!("voice")).expect("decodes");
    assert_eq!(newer, RunType::Unknown);
}

#[test]
fn a_kind_says_whether_it_runs_as_a_chat_and_why_not() {
    assert_wire(
        &KindChat {
            supported: true,
            reason: None,
            version: Some("2.1.258".to_owned()),
            range: Some(">=2.1.0, <2.3.0".to_owned()),
        },
        &json!({ "supported": true, "version": "2.1.258", "range": ">=2.1.0, <2.3.0" }),
    );
    assert_wire(
        &KindChat {
            supported: false,
            reason: Some(ChatUnsupportedReason::VersionOutsideRange),
            version: Some("1.0.0".to_owned()),
            range: None,
        },
        &json!({ "supported": false, "reason": "version_outside_range", "version": "1.0.0" }),
    );
    for (wire, reason) in [
        ("not_detected", ChatUnsupportedReason::NotDetected),
        ("version_unknown", ChatUnsupportedReason::VersionUnknown),
        ("from_a_newer_peer", ChatUnsupportedReason::Unknown),
    ] {
        let decoded: ChatUnsupportedReason = serde_json::from_value(json!(wire)).expect("decodes");
        assert_eq!(decoded, reason, "{wire}");
    }
}

#[test]
fn a_kind_from_an_older_peer_has_no_chat_and_a_chat_kind_round_trips() {
    let older = json!({
        "id": "claude",
        "name": "Claude Code",
        "icon": "claude",
        "owner": "core",
        "installed": true,
        "detected": true,
        "package": { "state": "enabled" },
        "resumable": true,
        "status_mechanism": "claude-settings",
        "credentials": { "supported": true, "imported": false }
    });
    let mut kind: AgentKind = serde_json::from_value(older.clone()).expect("an older kind decodes");
    assert_eq!(kind.chat, None);
    assert_eq!(
        serde_json::to_value(&kind).expect("serializes"),
        older,
        "no chat key is sent when there is none"
    );

    kind.chat = Some(KindChat {
        supported: true,
        reason: None,
        version: None,
        range: None,
    });
    let mut expected = older;
    expected["chat"] = json!({ "supported": true });
    assert_wire(&kind, &expected);
}

#[test]
fn an_agent_start_payload_from_before_chat_still_decodes() {
    let params: AgentStartParams =
        serde_json::from_value(json!({ "workspace": "w-1", "kind": "claude" }))
            .expect("an older start decodes");
    assert_eq!(params.kind, "claude");
    let chat: AgentChatStartParams =
        serde_json::from_value(json!({ "kind": "claude" })).expect("a bare chat start decodes");
    assert_eq!(chat.start.workspace, None);
}

#[test]
fn a_prompt_to_a_chat_is_sent_to_chat() {
    assert_wire(
        &PromptReceipt {
            receipt: ReceiptKind::SentToChat,
            bytes: 5,
        },
        &json!({ "receipt": "sent_to_chat", "bytes": 5 }),
    );
}

#[test]
fn subscribe_and_history_params_omit_absent_cursors() {
    assert_wire(
        &ChatSubscribeParams {
            session: "s-1".to_owned(),
            after: None,
            limit: None,
        },
        &json!({ "session": "s-1" }),
    );
    assert_wire(
        &ChatSubscribeParams {
            session: "s-1".to_owned(),
            after: Some(cursor(42)),
            limit: Some(80),
        },
        &json!({ "session": "s-1", "after": { "epoch": "e-1", "seq": 42 }, "limit": 80 }),
    );
    assert_wire(
        &ChatHistoryParams {
            session: "s-1".to_owned(),
            before: Some(cursor(7)),
            limit: None,
        },
        &json!({ "session": "s-1", "before": { "epoch": "e-1", "seq": 7 } }),
    );
    assert_wire(
        &ChatHistoryPage {
            epoch: "e-1".to_owned(),
            items: vec![item(
                "m-1",
                1,
                1,
                ChatItemBody::UserMessage {
                    text: "hi".to_owned(),
                },
            )],
            has_older: false,
        },
        &json!({
            "epoch": "e-1",
            "items": [{
                "id": "m-1", "position": 1, "seq": 1, "turn": "turn-1", "at_ms": 1_767_225_600_000_i64,
                "type": "user_message", "text": "hi"
            }],
            "has_older": false
        }),
    );
}

#[test]
fn a_snapshot_carries_the_head_the_page_the_latest_turn_and_the_activity() {
    let turn = ChatItem {
        turn: None,
        ..item(
            "turn-1",
            1,
            9,
            ChatItemBody::Turn(Turn {
                state: TurnState::Running,
                outcome: None,
                user_item: Some("m-1".to_owned()),
                completed_at_ms: None,
            }),
        )
    };
    let event = ChatEvent::Snapshot(Box::new(ChatSnapshot {
        cursor: cursor(9),
        items: vec![turn.clone()],
        has_older: true,
        latest_turn: Some(turn),
        activity: Some(Activity {
            state: ActivityState::Working,
            outcome: None,
            source: ActivitySource::Output,
            at_ms: 5,
            seen_at_ms: None,
        }),
    }));
    let turn_json = json!({
        "id": "turn-1", "position": 1, "seq": 9, "at_ms": 1_767_225_600_000_i64,
        "type": "turn", "state": "running", "user_item": "m-1"
    });
    assert_wire(
        &event,
        &json!({
            "event": "snapshot",
            "cursor": { "epoch": "e-1", "seq": 9 },
            "items": [turn_json.clone()],
            "has_older": true,
            "latest_turn": turn_json,
            "activity": { "state": "working", "source": "output", "at_ms": 5 }
        }),
    );
}

#[test]
fn a_batch_carries_whole_items_and_text_appends() {
    let event = ChatEvent::Batch(ChatBatch {
        cursor: cursor(12),
        updates: vec![
            ChatUpdate::Item {
                item: item(
                    "a-1",
                    10,
                    10,
                    ChatItemBody::AssistantMessage {
                        text: "Hel".to_owned(),
                        state: MessageState::Running,
                    },
                ),
            },
            ChatUpdate::Append {
                id: "a-1".to_owned(),
                seq: 11,
                offset: 3,
                text: "lo".to_owned(),
            },
        ],
        activity: None,
    });
    assert_wire(
        &event,
        &json!({
            "event": "batch",
            "cursor": { "epoch": "e-1", "seq": 12 },
            "updates": [
                { "op": "item", "item": {
                    "id": "a-1", "position": 10, "seq": 10, "turn": "turn-1", "at_ms": 1_767_225_600_000_i64,
                    "type": "assistant_message", "text": "Hel", "state": "running"
                }},
                { "op": "append", "id": "a-1", "seq": 11, "offset": 3, "text": "lo" }
            ]
        }),
    );
}

#[test]
fn reset_and_end_events_carry_their_reason() {
    assert_wire(
        &ChatEvent::Reset {
            reason: ResetReason::TooOld,
        },
        &json!({ "event": "reset", "reason": "too_old" }),
    );
    assert_wire(
        &ChatEvent::Reset {
            reason: ResetReason::EpochChanged,
        },
        &json!({ "event": "reset", "reason": "epoch_changed" }),
    );
    assert_wire(
        &ChatEvent::End {
            reason: EndReason::Forgotten,
        },
        &json!({ "event": "end", "reason": "forgotten" }),
    );
    let reason: EndReason = serde_json::from_value(json!("migrated")).expect("decodes");
    assert_eq!(reason, EndReason::Unknown);
    let reason: ResetReason = serde_json::from_value(json!("compacted")).expect("decodes");
    assert_eq!(reason, ResetReason::Unknown);
}

#[test]
fn every_item_type_has_its_wire_shape() {
    let cases: Vec<(ChatItemBody, Value)> = vec![
        (
            ChatItemBody::Thinking {
                text: "hmm".to_owned(),
                state: MessageState::Completed,
            },
            json!({ "type": "thinking", "text": "hmm", "state": "completed" }),
        ),
        (
            ChatItemBody::ToolUse(ToolUse {
                tool_use_id: "toolu_1".to_owned(),
                name: "Bash".to_owned(),
                input_summary: "cargo test".to_owned(),
                status: ToolStatus::Completed,
                output_summary: Some("ok".to_owned()),
                truncated: true,
            }),
            json!({
                "type": "tool_use", "tool_use_id": "toolu_1", "name": "Bash", "input_summary": "cargo test",
                "status": "completed", "output_summary": "ok", "truncated": true
            }),
        ),
        (
            ChatItemBody::ToolUse(ToolUse {
                tool_use_id: "toolu_2".to_owned(),
                name: "Read".to_owned(),
                input_summary: "a.rs".to_owned(),
                status: ToolStatus::Started,
                output_summary: None,
                truncated: false,
            }),
            json!({
                "type": "tool_use", "tool_use_id": "toolu_2", "name": "Read", "input_summary": "a.rs",
                "status": "started"
            }),
        ),
        (
            ChatItemBody::Plan {
                entries: vec![
                    PlanEntry {
                        text: "read".to_owned(),
                        status: PlanStatus::Completed,
                    },
                    PlanEntry {
                        text: "write".to_owned(),
                        status: PlanStatus::InProgress,
                    },
                ],
            },
            json!({ "type": "plan", "entries": [
                { "text": "read", "status": "completed" },
                { "text": "write", "status": "in_progress" }
            ]}),
        ),
        (
            ChatItemBody::Diff {
                path: "src/a.rs".to_owned(),
                patch: "@@ -1 +1 @@".to_owned(),
                truncated: false,
            },
            json!({ "type": "diff", "path": "src/a.rs", "patch": "@@ -1 +1 @@" }),
        ),
        (
            ChatItemBody::Subagent {
                tool_use_id: "toolu_3".to_owned(),
                description: "search the repo".to_owned(),
                agent_type: Some("Explore".to_owned()),
                status: SubagentStatus::Running,
            },
            json!({
                "type": "subagent", "tool_use_id": "toolu_3", "description": "search the repo",
                "agent_type": "Explore", "status": "running"
            }),
        ),
        (
            ChatItemBody::Approval(pending_approval()),
            json!({
                "type": "approval", "request_id": "req-1", "tool": "Bash", "input_summary": "rm -rf build",
                "tool_use_id": "toolu_1", "can_always": true, "state": "pending"
            }),
        ),
        (
            ChatItemBody::Notice {
                level: NoticeLevel::Warning,
                text: "retrying".to_owned(),
            },
            json!({ "type": "notice", "level": "warning", "text": "retrying" }),
        ),
        (
            ChatItemBody::Turn(Turn {
                state: TurnState::Completed,
                outcome: Some(Outcome::Success),
                user_item: None,
                completed_at_ms: Some(9),
            }),
            json!({ "type": "turn", "state": "completed", "outcome": "success", "completed_at_ms": 9 }),
        ),
    ];
    for (body, body_json) in cases {
        let value = ChatItem {
            parent: Some("sub-1".to_owned()),
            ..item("i-1", 3, 4, body)
        };
        let mut expected = json!({
            "id": "i-1", "position": 3, "seq": 4, "turn": "turn-1", "parent": "sub-1",
            "at_ms": 1_767_225_600_000_i64
        });
        for (key, field) in body_json.as_object().expect("an object") {
            expected[key] = field.clone();
        }
        assert_wire(&value, &expected);
    }
}

#[test]
fn a_settled_approval_carries_its_scope_and_time() {
    let approval = Approval {
        state: ApprovalState::Allowed,
        scope: Some(ApprovalScope::Always),
        settled_at_ms: Some(77),
        can_always: false,
        tool_use_id: None,
        ..pending_approval()
    };
    assert_wire(
        &approval,
        &json!({
            "request_id": "req-1", "tool": "Bash", "input_summary": "rm -rf build",
            "state": "allowed", "scope": "always", "settled_at_ms": 77
        }),
    );
    for (wire, state) in [
        ("denied", ApprovalState::Denied),
        ("cancelled", ApprovalState::Cancelled),
        ("expired", ApprovalState::Unknown),
    ] {
        let decoded: ApprovalState = serde_json::from_value(json!(wire)).expect("decodes");
        assert_eq!(decoded, state, "{wire}");
    }
}

#[test]
fn an_approval_answer_takes_a_decision_and_optional_scope_and_message() {
    assert_wire(
        &ChatApprovalAnswerParams {
            session: "s-1".to_owned(),
            request_id: "req-1".to_owned(),
            decision: ApprovalDecision::Allow,
            scope: Some(ApprovalScope::Once),
            message: None,
        },
        &json!({ "session": "s-1", "request_id": "req-1", "decision": "allow", "scope": "once" }),
    );
    assert_wire(
        &ChatApprovalAnswerParams {
            session: "s-1".to_owned(),
            request_id: "req-1".to_owned(),
            decision: ApprovalDecision::Deny,
            scope: None,
            message: Some("use cargo clean".to_owned()),
        },
        &json!({ "session": "s-1", "request_id": "req-1", "decision": "deny", "message": "use cargo clean" }),
    );
}

#[test]
fn an_unknown_decision_is_refused_rather_than_read_as_allow() {
    let refused = serde_json::from_value::<ApprovalDecision>(json!("allow_all"));
    assert!(refused.is_err(), "an unknown decision never decodes");
}

#[test]
fn an_interrupt_says_whether_a_turn_was_running() {
    assert_wire(
        &ChatInterrupted {
            interrupted: true,
            turn: Some("turn-1".to_owned()),
        },
        &json!({ "interrupted": true, "turn": "turn-1" }),
    );
    assert_wire(
        &ChatInterrupted {
            interrupted: false,
            turn: None,
        },
        &json!({ "interrupted": false }),
    );
}

#[test]
fn values_from_a_newer_peer_decode_as_unknown() {
    let event: ChatEvent =
        serde_json::from_value(json!({ "event": "presence", "who": "x" })).expect("decodes");
    assert_eq!(event, ChatEvent::Unknown);
    let update: ChatUpdate =
        serde_json::from_value(json!({ "op": "remove", "id": "a-1" })).expect("decodes");
    assert_eq!(update, ChatUpdate::Unknown);
    let item: ChatItem = serde_json::from_value(json!({
        "id": "q-1", "position": 2, "seq": 2, "at_ms": 1, "type": "question", "question": "which?"
    }))
    .expect("an item of a newer type decodes");
    assert_eq!(item.body, ChatItemBody::Unknown);
    assert_eq!(item.id, "q-1");
    for (wire, status) in [
        ("updated", ToolStatus::Updated),
        ("failed", ToolStatus::Failed),
        ("interrupted", ToolStatus::Interrupted),
        ("paused", ToolStatus::Unknown),
    ] {
        let decoded: ToolStatus = serde_json::from_value(json!(wire)).expect("decodes");
        assert_eq!(decoded, status, "{wire}");
    }
    for (wire, state) in [
        ("interrupted", TurnState::Interrupted),
        ("unverifiable", TurnState::Unverifiable),
        ("compacting", TurnState::Unknown),
    ] {
        let decoded: TurnState = serde_json::from_value(json!(wire)).expect("decodes");
        assert_eq!(decoded, state, "{wire}");
    }
    let scope: ApprovalScope = serde_json::from_value(json!("project")).expect("decodes");
    assert_eq!(scope, ApprovalScope::Unknown);
}
