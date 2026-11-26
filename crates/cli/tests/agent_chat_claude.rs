//! Claude chat sessions end to end (decision 0101), the stream: a reply streamed in parts, a tool run after an allow, the
//! turn's end, and the version range a chat start needs. See `chat_claude_support` for the fake CLI.

mod chat_claude_support;
mod harness;

use chat_claude_support::*;
use harness::{Sandbox, single_document, with_json};
use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{
    ApprovalDecision, ApprovalScope, ApprovalState, ChatItemBody, ToolStatus, TurnState,
};
use serde_json::json;

#[test]
fn a_claude_chat_streams_its_reply_runs_a_tool_after_an_allow_and_ends_its_turn() {
    let (sandbox, logs) = sandbox(
        &[
            await_input("user"),
            frame(json!({ "type": "system", "subtype": "init", "claude_code_version": "2.1.330" })),
            message_start("m1"),
            text_start(0),
            text_delta(0, "Hello"),
            text_delta(0, ", world"),
            assistant("m1", json!([{ "type": "text", "text": "Hello, world" }])),
            assistant("m1", bash_call("toolu_1", "touch probe.txt")),
            permission("r-1", "toolu_1", "touch probe.txt"),
            await_input("control_response"),
            tool_done("toolu_1"),
            result(true),
        ],
        &[],
        &[],
    );
    let session = start_chat(&sandbox, "say hello");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        let pending = transcript
            .until("the permission request", |items| {
                approval_of(items, "r-1")
                    .filter(|approval| approval.state == ApprovalState::Pending)
                    .cloned()
            })
            .await;
        assert!(pending.can_always);
        assert_eq!(pending.tool, "Bash");
        assert_eq!(pending.input_summary, "touch probe.txt");
        assert_eq!(pending.tool_use_id.as_deref(), Some("toolu_1"));
        let shown = sandbox.json_ok("default", &["agent", "status", &session]);
        assert_eq!(shown["activity"]["state"], "waiting", "{shown}");

        let mut always = answer(&session, "r-1", ApprovalDecision::Allow);
        always.scope = Some(ApprovalScope::Always);
        let answered = client.agent_chat_answer(&always).await.expect("the answer");
        let ChatItemBody::Approval(settled) = &answered.body else {
            panic!("{answered:?}");
        };
        assert_eq!(settled.state, ApprovalState::Allowed);
        assert_eq!(settled.scope, Some(ApprovalScope::Always));
        // The first answer wins.
        let again = client
            .agent_chat_answer(&answer(&session, "r-1", ApprovalDecision::Deny))
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
                    state: massorca_protocol::methods::chat::MessageState::Completed
                }),
            "{items:#?}"
        );
        assert_eq!(tool_status(&items, "toolu_1"), Some(ToolStatus::Completed));
    });
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["activity"]["state"], "done", "{shown}");
    assert_eq!(shown["activity"]["outcome"], "success", "{shown}");

    // What reached the CLI: the prompt as a stream-json user message, then the allow with the offered rules.
    let input = logs.stdin_lines();
    assert_eq!(input[0]["type"], "user", "{input:?}");
    assert_eq!(input[0]["message"]["content"][0]["text"], "say hello");
    let allow = &input[1]["response"];
    assert_eq!(allow["request_id"], "r-1");
    assert_eq!(allow["response"]["behavior"], "allow");
    assert_eq!(
        allow["response"]["updatedInput"],
        json!({ "command": "touch probe.txt" })
    );
    assert!(
        allow["response"]["updatedPermissions"].is_array(),
        "{allow}"
    );
    // The launch: the stream-json flags first, then the kind's session id.
    let runs = logs.runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(
        runs[0][..9],
        [
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--permission-prompt-tool",
            "stdio"
        ]
    );
    assert!(runs[0].iter().any(|arg| arg == "--session-id"), "{runs:?}");
}

#[test]
fn a_cli_version_outside_the_range_is_refused_with_the_kinds_chat_support() {
    let (sandbox, logs) = sandbox(
        &[],
        &[],
        &[("FAKE_CLAUDE_VERSION", "1.0.128 (Claude Code)")],
    );
    let kinds = sandbox.json_ok("default", &["agent", "kinds"]);
    let listed = kinds["kinds"]
        .as_array()
        .and_then(|kinds| kinds.iter().find(|kind| kind["id"] == "fakeclaude"))
        .expect("the kind is listed")
        .clone();
    assert_eq!(
        listed["chat"],
        json!({ "supported": false, "reason": "version_outside_range", "version": "1.0.128",
                "range": ">=2.1.258, <3.0.0" }),
        "{listed}"
    );
    let outcome = sandbox
        .run(
            "default",
            &with_json(&[
                "agent",
                "start",
                "fakeclaude",
                "--workspace",
                "main",
                "--chat",
            ]),
        )
        .failure();
    let refused = single_document(&outcome);
    assert_eq!(
        refused["error"]["code"], "agent.chat_unsupported",
        "{refused}"
    );
    assert_eq!(refused["error"]["data"], listed["chat"], "{refused}");
    // No chat run started.
    assert!(logs.runs().is_empty());
}

#[test]
fn a_cli_version_that_cannot_be_read_is_unknown_and_a_supported_one_is_listed() {
    let (sandbox, _logs) = sandbox(&[], &[], &[("FAKE_CLAUDE_VERSION", "Claude Code")]);
    let kinds = sandbox.json_ok("default", &["agent", "kinds"]);
    let chat_of = |id: &str| {
        kinds["kinds"]
            .as_array()
            .and_then(|kinds| kinds.iter().find(|kind| kind["id"] == id))
            .map(|kind| kind["chat"].clone())
    };
    assert_eq!(
        chat_of("fakeclaude").expect("listed"),
        json!({ "supported": false, "reason": "version_unknown", "range": ">=2.1.258, <3.0.0" })
    );
    let refused = sandbox
        .run(
            "default",
            &with_json(&[
                "agent",
                "start",
                "fakeclaude",
                "--workspace",
                "main",
                "--chat",
            ]),
        )
        .failure();
    assert_eq!(
        single_document(&refused)["error"]["code"],
        "agent.chat_unsupported"
    );

    let (supported, _logs) = sandbox_supported();
    let kinds = supported.json_ok("default", &["agent", "kinds"]);
    let listed = kinds["kinds"]
        .as_array()
        .and_then(|kinds| kinds.iter().find(|kind| kind["id"] == "fakeclaude"))
        .map(|kind| kind["chat"].clone())
        .expect("listed");
    assert_eq!(
        listed,
        json!({ "supported": true, "version": "2.1.330", "range": ">=2.1.258, <3.0.0" })
    );
}

fn sandbox_supported() -> (Sandbox, Logs) {
    sandbox(&[], &[], &[])
}
