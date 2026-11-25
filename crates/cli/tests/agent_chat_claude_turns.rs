//! Claude chat sessions end to end (decision 0101), approvals and turns: a deny, a withdrawn request, an answer to a
//! request sent again, an interrupt, and prompts handed to the CLI while a turn runs.

mod chat_claude_support;
mod harness;

use chat_claude_support::*;
use harness::{single_document, with_json};
use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{
    ApprovalDecision, ApprovalState, ChatItemBody, ToolStatus, TurnState,
};
use serde_json::json;

#[test]
fn a_deny_tells_the_agent_why_and_a_withdrawn_request_is_cancelled() {
    let (sandbox, logs) = sandbox(
        &[
            await_input("user"),
            assistant("m1", bash_call("toolu_1", "rm -r build")),
            permission("r-1", "toolu_1", "rm -r build"),
            await_input("control_response"),
            frame(
                json!({ "type": "user", "parent_tool_use_id": null, "message": { "role": "user", "content": [
                    { "type": "tool_result", "tool_use_id": "toolu_1", "content": "denied", "is_error": true }
                ]}}),
            ),
            assistant("m2", bash_call("toolu_2", "ls")),
            permission("r-2", "toolu_2", "ls"),
            json!({ "type": "control_cancel_request", "request_id": "r-2" }),
            result(true),
        ],
        &[],
        &[],
    );
    let session = start_chat(&sandbox, "clean up");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        transcript
            .until("the first request", |items| approval_of(items, "r-1").map(drop))
            .await;
        let mut deny = answer(&session, "r-1", ApprovalDecision::Deny);
        deny.message = Some("Keep the build folder.".to_owned());
        let denied = client.agent_chat_answer(&deny).await.expect("the deny");
        assert!(matches!(&denied.body, ChatItemBody::Approval(approval) if approval.state == ApprovalState::Denied && approval.scope.is_none()));
        let items = transcript
            .until("the turn's end", |items| {
                (turns(items).first().map(|turn| turn.0) == Some(TurnState::Completed)).then(|| items.to_vec())
            })
            .await;
        assert_eq!(
            approval_of(&items, "r-2").map(|approval| approval.state),
            Some(ApprovalState::Cancelled)
        );
        assert_eq!(tool_status(&items, "toolu_1"), Some(ToolStatus::Failed));
        // A withdrawn request never authorizes.
        let late = client
            .agent_chat_answer(&answer(&session, "r-2", ApprovalDecision::Allow))
            .await
            .expect_err("an answer to a cancelled request");
        assert_eq!(remote_code(&late), "agent.approval_settled");
    });
    let input = logs.stdin_lines();
    let denied = &input[1]["response"];
    assert_eq!(denied["request_id"], "r-1");
    assert_eq!(
        denied["response"],
        json!({ "behavior": "deny", "message": "Keep the build folder.", "toolUseID": "toolu_1" })
    );
    assert_eq!(
        input.len(),
        2,
        "nothing answers the withdrawn request: {input:?}"
    );
}

#[test]
fn an_answer_names_the_request_as_the_person_saw_it() {
    let (sandbox, logs) = sandbox(
        &[
            await_input("user"),
            assistant("m1", bash_call("toolu_1", "ls")),
            permission("r-1", "toolu_1", "ls"),
            permission("r-1", "toolu_1", "ls -la"),
            await_input("control_response"),
            tool_done("toolu_1"),
            result(true),
        ],
        &[],
        &[],
    );
    let session = start_chat(&sandbox, "list");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        let item = transcript
            .until("the request sent again", |items| {
                items
                    .iter()
                    .find(|item| matches!(&item.body, ChatItemBody::Approval(approval) if approval.input_summary == "ls -la"))
                    .cloned()
            })
            .await;
        let mut stale = answer(&session, "r-1", ApprovalDecision::Allow);
        stale.seq = Some(item.seq - 1);
        let refused = client.agent_chat_answer(&stale).await.expect_err("a stale answer");
        assert_eq!(remote_code(&refused), "agent.approval_changed");
        let mut current = answer(&session, "r-1", ApprovalDecision::Allow);
        current.seq = Some(item.seq);
        client.agent_chat_answer(&current).await.expect("the answer");
        transcript
            .until("the turn's end", |items| (turns(items) == [(TurnState::Completed, Some(Outcome::Success))]).then_some(()))
            .await;
    });
    let input = logs.stdin_lines();
    assert_eq!(
        input[1]["response"]["response"]["updatedInput"],
        json!({ "command": "ls -la" })
    );
}

#[test]
fn prompts_forced_into_a_running_turn_show_at_once_and_end_with_the_result_that_folds_them() {
    let (sandbox, logs) = sandbox(
        &[
            await_input("user"),
            assistant("m1", json!([{ "type": "text", "text": "Working." }])),
            await_input("user"),
            await_input("user"),
            assistant("m2", json!([{ "type": "text", "text": "All three done." }])),
            result(true),
        ],
        &[],
        &[],
    );
    let session = start_chat(&sandbox, "first");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        until_text(&mut transcript, "Working.").await;
        for text in ["second", "third"] {
            let sent = force_prompt(&sandbox, &session, text);
            assert_eq!(sent["receipt"], "sent_to_chat", "{sent}");
        }
        // Each forced prompt shows at once, in a running turn of its own.
        transcript
            .until("the held prompts", |items| {
                let shown = items
                    .iter()
                    .filter(|item| matches!(&item.body, ChatItemBody::UserMessage { text } if text == "second" || text == "third"))
                    .count();
                (shown == 2 && turns(items).iter().all(|turn| turn.0 == TurnState::Running) && turns(items).len() == 3)
                    .then_some(())
            })
            .await;
        // The CLI folded them into its turn: its one result closes every turn.
        transcript
            .until("every turn closed", |items| {
                (turns(items) == vec![(TurnState::Completed, Some(Outcome::Success)); 3]).then_some(())
            })
            .await;
    });
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["activity"]["state"], "done", "{shown}");
    assert_eq!(logs.stdin_lines().len(), 3);
}

#[test]
fn an_interrupt_ends_the_turns_of_held_prompts_and_cancels_what_the_cli_queued() {
    let (sandbox, logs) = sandbox(
        &[
            await_input("user"),
            assistant("m1", json!([{ "type": "text", "text": "Working." }])),
            await_input("user"),
            await_input("interrupt"),
            result(false),
        ],
        &[],
        &[],
    );
    let session = start_chat(&sandbox, "first");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        until_text(&mut transcript, "Working.").await;
        force_prompt(&sandbox, &session, "second");
        transcript
            .until("the held turn", |items| {
                (turns(items).len() == 2).then_some(())
            })
            .await;
        let interrupted = client
            .agent_chat_interrupt(&session)
            .await
            .expect("the interrupt");
        assert!(interrupted.interrupted);
        transcript
            .until("both turns interrupted", |items| {
                (turns(items)
                    == [
                        (TurnState::Interrupted, None),
                        (TurnState::Interrupted, None),
                    ])
                .then_some(())
            })
            .await;
    });
    let input = logs.stdin_lines();
    assert_eq!(
        input[2]["request"],
        json!({ "subtype": "interrupt", "cancel_queued": true }),
        "{input:?}"
    );
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["activity"]["state"], "idle", "{shown}");
}

#[test]
fn a_run_that_ends_closes_the_turns_of_its_held_prompts() {
    let (sandbox, _logs) = sandbox(
        &[
            await_input("user"),
            assistant("m1", json!([{ "type": "text", "text": "Working." }])),
            await_input("user"),
        ],
        &[],
        &[],
    );
    let session = start_chat(&sandbox, "first");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        until_text(&mut transcript, "Working.").await;
        force_prompt(&sandbox, &session, "second");
        transcript
            .until("the held turn", |items| {
                (turns(items).len() == 2).then_some(())
            })
            .await;
        sandbox.json_ok("default", &["agent", "stop", &session]);
        transcript
            .until("both turns ended", |items| {
                (turns(items)
                    == [
                        (TurnState::Interrupted, None),
                        (TurnState::Interrupted, None),
                    ])
                .then_some(())
            })
            .await;
    });
}

#[test]
fn an_interrupt_ends_the_turn_and_cancels_its_request_and_the_session_takes_the_next_prompt() {
    let (sandbox, logs) = sandbox(
        &[
            await_input("user"),
            assistant("m1", bash_call("toolu_1", "cargo build")),
            permission("r-1", "toolu_1", "cargo build"),
            await_input("interrupt"),
            result(false),
            await_input("user"),
            assistant("m2", json!([{ "type": "text", "text": "Back again." }])),
            result(true),
        ],
        &[],
        &[],
    );
    let session = start_chat(&sandbox, "build it");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        transcript
            .until("the request", |items| approval_of(items, "r-1").map(drop))
            .await;
        // A prompt now would land inside the turn.
        let busy = sandbox
            .run(
                "default",
                &with_json(&["agent", "prompt", &session, "next"]),
            )
            .failure();
        assert_eq!(single_document(&busy)["error"]["code"], "agent.busy");

        let interrupted = client
            .agent_chat_interrupt(&session)
            .await
            .expect("the interrupt");
        assert!(interrupted.interrupted);
        let turn = interrupted.turn.expect("the interrupted turn");
        let items = transcript
            .until("the interrupted turn", |items| {
                (turns(items) == [(TurnState::Interrupted, None)]).then(|| items.to_vec())
            })
            .await;
        assert!(items.iter().any(|item| item.id == turn));
        assert_eq!(
            approval_of(&items, "r-1").map(|approval| approval.state),
            Some(ApprovalState::Cancelled)
        );
        assert_eq!(
            tool_status(&items, "toolu_1"),
            Some(ToolStatus::Interrupted)
        );
        let late = client
            .agent_chat_answer(&answer(&session, "r-1", ApprovalDecision::Allow))
            .await
            .expect_err("an answer after the interrupt");
        assert_eq!(remote_code(&late), "agent.approval_settled");
        // With no running turn, an interrupt does nothing.
        let idle = client
            .agent_chat_interrupt(&session)
            .await
            .expect("a second interrupt");
        assert!(!idle.interrupted);

        // The session and its run stay: the next prompt is a new turn, which the CLI's result ends.
        let sent = sandbox.json_ok("default", &["agent", "prompt", &session, "again"]);
        assert_eq!(sent["receipt"], "sent_to_chat", "{sent}");
        transcript
            .until("the second turn", |items| {
                (turns(items)
                    == [
                        (TurnState::Interrupted, None),
                        (TurnState::Completed, Some(Outcome::Success)),
                    ])
                .then_some(())
            })
            .await;
    });
    let input = logs.stdin_lines();
    assert_eq!(input[1]["type"], "control_request", "{input:?}");
    assert_eq!(input[1]["request"]["subtype"], "interrupt");
    assert_eq!(input[2]["message"]["content"][0]["text"], "again");
    assert_eq!(
        input.len(),
        3,
        "nothing answers the cancelled request: {input:?}"
    );
}
