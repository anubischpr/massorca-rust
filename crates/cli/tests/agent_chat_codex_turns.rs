//! Codex chat sessions end to end (decision 0173), turns: a deny, an interrupt that cancels a pending request and never
//! authorizes it, and a stopped chat that resumes the thread it learned. See `chat_codex_support` for the fake app-server.

mod chat_claude_support;
mod chat_codex_support;
mod harness;

use chat_claude_support::{
    Transcript, answer, approval_of, connect, remote_code, runtime, tool_status, turns,
};
use chat_codex_support::*;
use harness::{single_document, with_json};
use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{
    ApprovalDecision, ApprovalState, ChatItemBody, ToolStatus, TurnState,
};
use serde_json::json;

#[test]
fn a_deny_is_sent_as_a_decline_and_the_command_fails() {
    let (sandbox, logs) = sandbox(
        &[
            await_method("turn/start"),
            turn_started(),
            item(false, command("c1", "rm -rf build", "inProgress")),
            approval_request(5, "c1", "rm -rf build"),
            await_method("response"),
            item(true, command("c1", "rm -rf build", "declined")),
            turn_completed("completed"),
        ],
        &[],
    );
    let session = start_chat(&sandbox, "clean");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        transcript
            .until("the request", |items| approval_of(items, "5").map(drop))
            .await;
        let denied = client
            .agent_chat_answer(&answer(&session, "5", ApprovalDecision::Deny))
            .await
            .expect("the deny");
        assert!(matches!(&denied.body, ChatItemBody::Approval(approval)
            if approval.state == ApprovalState::Denied));
        let items = transcript
            .until("the turn's end", |items| {
                (turns(items).len() == 1 && turns(items)[0].0 == TurnState::Completed)
                    .then(|| items.to_vec())
            })
            .await;
        assert_eq!(tool_status(&items, "c1"), Some(ToolStatus::Failed));
    });
    let decline = logs
        .stdin_lines()
        .into_iter()
        .find(|line| line["id"] == 5)
        .expect("the answer reached the app-server");
    assert_eq!(decline["result"], json!({ "decision": "decline" }));
}

#[test]
fn an_interrupt_ends_the_turn_cancels_its_request_and_the_session_takes_the_next_prompt() {
    let (sandbox, logs) = sandbox(
        &[
            await_method("turn/start"),
            turn_started(),
            item(false, command("c1", "cargo build", "inProgress")),
            approval_request(11, "c1", "cargo build"),
            await_method("turn/interrupt"),
            turn_completed("interrupted"),
            await_method("turn/start"),
            turn_started(),
            item(
                true,
                json!({ "type": "agentMessage", "id": "m2", "text": "Back again." }),
            ),
            turn_completed("completed"),
        ],
        &[],
    );
    let session = start_chat(&sandbox, "build it");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        transcript
            .until("the request", |items| approval_of(items, "11").map(drop))
            .await;
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
        let items = transcript
            .until("the interrupted turn", |items| {
                (turns(items) == [(TurnState::Interrupted, None)]).then(|| items.to_vec())
            })
            .await;
        assert_eq!(
            approval_of(&items, "11").map(|approval| approval.state),
            Some(ApprovalState::Cancelled)
        );
        assert_eq!(tool_status(&items, "c1"), Some(ToolStatus::Interrupted));
        let late = client
            .agent_chat_answer(&answer(&session, "11", ApprovalDecision::Allow))
            .await
            .expect_err("an answer after the interrupt");
        assert_eq!(remote_code(&late), "agent.approval_settled");

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
    // The request is cancelled, never accepted, before the turn's interrupt.
    let answers: Vec<_> = input.iter().filter(|line| line["id"] == 11).collect();
    assert_eq!(answers.len(), 1, "{input:?}");
    assert_eq!(answers[0]["result"], json!({ "decision": "cancel" }));
    let interrupt = &sent_with(&logs, "turn/interrupt")[0];
    assert_eq!(
        interrupt["params"],
        json!({ "threadId": THREAD, "turnId": "turn-1" })
    );
    let turns_sent = sent_with(&logs, "turn/start");
    assert_eq!(turns_sent.len(), 2, "{input:?}");
    assert_eq!(turns_sent[1]["params"]["input"][0]["text"], "again");
}

#[test]
fn a_stopped_codex_chat_resumes_the_thread_it_learned() {
    let (sandbox, logs) = sandbox(
        &[
            await_method("turn/start"),
            turn_started(),
            turn_completed("completed"),
        ],
        &[
            await_method("turn/start"),
            turn_started(),
            item(
                true,
                json!({ "type": "agentMessage", "id": "m9", "text": "Resumed." }),
            ),
            turn_completed("completed"),
        ],
    );
    let session = start_chat(&sandbox, "first");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        transcript
            .until("the first turn", |items| {
                (turns(items) == [(TurnState::Completed, Some(Outcome::Success))]).then_some(())
            })
            .await;
    });
    let stopped = sandbox.json_ok("default", &["agent", "stop", &session]);
    assert_eq!(stopped["execution"]["state"], "exited", "{stopped}");
    assert_eq!(stopped["agent_ref"], THREAD, "{stopped}");

    let resumed = sandbox.json_ok("default", &["agent", "resume", &session]);
    assert_eq!(resumed["run"], "chat", "{resumed}");
    let sent = sandbox.json_ok("default", &["agent", "prompt", &session, "second"]);
    assert_eq!(sent["receipt"], "sent_to_chat", "{sent}");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        transcript
            .until("the resumed turn", |items| {
                items
                    .iter()
                    .any(|item| {
                        matches!(&item.body,
                        ChatItemBody::AssistantMessage { text, .. } if text == "Resumed.")
                    })
                    .then_some(())
            })
            .await;
    });
    let resumes = sent_with(&logs, "thread/resume");
    assert_eq!(resumes.len(), 1, "{:?}", logs.stdin_lines());
    assert_eq!(resumes[0]["params"]["threadId"], THREAD);
    assert_eq!(resumes[0]["params"]["model"], "gpt-test");
    let runs = logs.runs();
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert_eq!(runs[1][0], "app-server", "the resume is not a CLI argument");
    assert!(!runs[1].iter().any(|arg| arg == THREAD), "{runs:?}");
}
