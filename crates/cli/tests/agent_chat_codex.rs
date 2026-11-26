//! Codex chat sessions end to end (decision 0173), the stream: the handshake and the thread, a reply streamed in parts,
//! a command run after an allow, the turn's end with its usage, and a message MassOrca does not know. See
//! `chat_codex_support` for the fake app-server.

mod chat_claude_support;
mod chat_codex_support;
mod harness;

use chat_claude_support::{
    Transcript, answer, approval_of, connect, remote_code, runtime, tool_status, turns,
};
use chat_codex_support::*;
use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{
    ApprovalDecision, ApprovalState, ChatItemBody, MessageState, NoticeLevel, ToolStatus, TurnState,
};
use serde_json::json;

#[test]
fn a_codex_chat_streams_its_reply_runs_a_command_after_an_allow_and_ends_its_turn() {
    let (sandbox, logs) = sandbox(
        &[
            await_method("turn/start"),
            turn_started(),
            item(
                false,
                json!({ "type": "agentMessage", "id": "m1", "text": "" }),
            ),
            text_delta("m1", "Hello"),
            text_delta("m1", ", world"),
            item(
                true,
                json!({ "type": "agentMessage", "id": "m1", "text": "Hello, world" }),
            ),
            item(false, command("c1", "touch probe.txt", "inProgress")),
            approval_request(90, "c1", "touch probe.txt"),
            await_method("response"),
            item(
                true,
                json!({ "type": "commandExecution", "id": "c1", "command": "touch probe.txt",
                               "status": "completed", "exitCode": 0, "aggregatedOutput": "" }),
            ),
            notify(
                "thread/tokenUsage/updated",
                json!({ "threadId": "$THREAD", "turnId": "$TURN",
                "tokenUsage": { "total": { "inputTokens": 100, "cachedInputTokens": 20, "outputTokens": 7,
                                           "reasoningOutputTokens": 2, "totalTokens": 107 } } }),
            ),
            turn_completed("completed"),
        ],
        &[],
    );
    let session = start_chat(&sandbox, "say hello");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        let pending = transcript
            .until("the permission request", |items| {
                approval_of(items, "90")
                    .filter(|approval| approval.state == ApprovalState::Pending)
                    .cloned()
            })
            .await;
        assert_eq!(pending.tool, "shell");
        assert_eq!(pending.input_summary, "touch probe.txt");
        assert_eq!(pending.tool_use_id.as_deref(), Some("c1"));
        let shown = sandbox.json_ok("default", &["agent", "status", &session]);
        assert_eq!(shown["activity"]["state"], "waiting", "{shown}");

        client
            .agent_chat_answer(&answer(&session, "90", ApprovalDecision::Allow))
            .await
            .expect("the answer");
        let again = client
            .agent_chat_answer(&answer(&session, "90", ApprovalDecision::Deny))
            .await
            .expect_err("a second answer");
        assert_eq!(remote_code(&again), "agent.approval_settled");

        let items = transcript
            .until("the turn's end", |items| {
                (turns(items) == [(TurnState::Completed, Some(Outcome::Success))])
                    .then(|| items.to_vec())
            })
            .await;
        assert!(
            items.iter().any(|item| item.body
                == ChatItemBody::AssistantMessage {
                    text: "Hello, world".to_owned(),
                    state: MessageState::Completed
                }),
            "{items:#?}"
        );
        assert_eq!(tool_status(&items, "c1"), Some(ToolStatus::Completed));
        let usage = items.iter().find_map(|item| match &item.body {
            ChatItemBody::Turn(turn) => turn.usage,
            _ => None,
        });
        assert_eq!(
            usage.map(|usage| usage.total_tokens),
            Some(107),
            "{items:#?}"
        );
    });
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["activity"]["state"], "done", "{shown}");
    assert_eq!(shown["activity"]["outcome"], "success", "{shown}");
    // The thread's id is the session's ref, learned from the app-server.
    assert_eq!(shown["agent_ref"], THREAD, "{shown}");

    // The launch: app-server with the effort as a config override, and no terminal hook.
    let runs = logs.runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0], ["app-server", "-c", "model_reasoning_effort=high"]);
    // The handshake, then the thread with the model and the mode's policy, then the prompt as a turn.
    let input = logs.stdin_lines();
    let methods: Vec<&str> = input
        .iter()
        .filter_map(|line| line["method"].as_str())
        .collect();
    assert_eq!(
        methods,
        ["initialize", "initialized", "thread/start", "turn/start"]
    );
    let thread = &sent_with(&logs, "thread/start")[0];
    assert_eq!(
        thread["params"],
        json!({ "model": "gpt-test", "sandbox": "workspace-write", "approvalPolicy": "on-request" })
    );
    let turn = &sent_with(&logs, "turn/start")[0];
    assert_eq!(turn["params"]["threadId"], THREAD);
    assert_eq!(turn["params"]["input"][0]["text"], "say hello");
    let allow = input
        .iter()
        .find(|line| line["id"] == 90)
        .expect("the answer reached the app-server");
    assert_eq!(allow["result"], json!({ "decision": "accept" }));
}

#[test]
fn a_message_of_a_kind_massorca_does_not_know_is_skipped_with_a_notice() {
    let (sandbox, _logs) = sandbox(
        &[
            await_method("turn/start"),
            turn_started(),
            notify(
                "thread/hologram/updated",
                json!({ "threadId": "$THREAD", "payload": "private" }),
            ),
            item(
                true,
                json!({ "type": "agentMessage", "id": "m1", "text": "Still here." }),
            ),
            turn_completed("completed"),
        ],
        &[],
    );
    let session = start_chat(&sandbox, "hi");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        let items = transcript
            .until("the turn's end", |items| {
                (turns(items) == [(TurnState::Completed, Some(Outcome::Success))])
                    .then(|| items.to_vec())
            })
            .await;
        let notices: Vec<&str> = items
            .iter()
            .filter_map(|item| match &item.body {
                ChatItemBody::Notice {
                    level: NoticeLevel::Warning,
                    text,
                } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(notices.len(), 1, "{items:#?}");
        assert!(notices[0].contains("thread/hologram/updated"));
        assert!(!notices[0].contains("private"));
        assert!(items.iter().any(|item| matches!(&item.body,
            ChatItemBody::AssistantMessage { text, .. } if text == "Still here.")));
    });
}

#[test]
fn the_codex_kind_lists_its_chat_support_from_its_version() {
    let (sandbox, _logs) = sandbox(&[], &[]);
    let kinds = sandbox.json_ok("default", &["agent", "kinds"]);
    let listed = kinds["kinds"]
        .as_array()
        .and_then(|kinds| kinds.iter().find(|kind| kind["id"] == "fakecodex"))
        .map(|kind| kind["chat"].clone())
        .expect("listed");
    assert_eq!(
        listed,
        json!({ "supported": true, "version": "0.172.0", "range": ">=0.172.0, <1.0.0" })
    );
}
